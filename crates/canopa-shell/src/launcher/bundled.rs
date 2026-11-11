//! Bundled providers use the same executable JSONL boundary as configured extensions.
use super::{Action, Batch, Config, Extension, NamedAction, ResultItem};
use canopa_launcher::{Catalog, ExtensionInfo, Preferences};
use chrono::{DateTime, Days, Local, TimeZone, Utc};
use std::{
    collections::{BTreeMap, HashSet},
    io::{self, Read, Write},
};
pub fn definitions(config: &Config) -> Vec<Extension> {
    let exe = canopa_file::executable::installed()
        .unwrap_or_else(|_| "canopa-shell".into())
        .to_string_lossy()
        .into_owned();
    let mut extensions = config.extensions.clone();
    for (id, name, keyword, mode) in [
        ("builtin:web", "Web", "web", "web"),
        ("builtin:date-time", "Date and time", "date", "date-time"),
    ] {
        extensions.push(Extension {
            id: id.into(),
            name: name.into(),
            keyword: keyword.into(),
            global: true,
            argv: vec![exe.clone(), "--launcher-extension".into(), mode.into()],
        });
    }
    extensions
}
pub fn effective(config: &Config, prefs: &Preferences) -> (Config, Option<String>) {
    let mut config = config.clone();
    let mut keywords = HashSet::from(["apps", "files", "=", "clip", "snip"].map(str::to_owned));
    let mut error = None;
    config.extensions = definitions(&config)
        .into_iter()
        .filter_map(|mut ext| {
            if let Some(value) = prefs.extensions.get(&ext.id) {
                if !value.enabled {
                    return None;
                }
                ext.keyword = value.keyword.clone();
            }
            if !keywords.insert(ext.keyword.clone()) {
                error = Some(format!("Duplicate extension keyword: {}", ext.keyword));
                return None;
            }
            Some(ext)
        })
        .collect();
    if let Some(web) = config
        .extensions
        .iter()
        .find(|e| e.id == "builtin:web")
        .cloned()
    {
        for search in &prefs.searches {
            if !keywords.insert(search.keyword.clone()) {
                error = Some(format!("Duplicate search keyword: {}", search.keyword));
                continue;
            }
            let mut ext = web.clone();
            ext.id = format!("builtin:web:{}", search.keyword);
            ext.name = format!("Web · {}", search.keyword);
            ext.keyword = search.keyword.clone();
            ext.global = false;
            ext.argv[2] = "web-search".into();
            ext.argv.push(search.template.clone());
            config.extensions.push(ext);
        }
    }
    (config, error)
}
pub fn catalog(
    configs: &[Config],
    prefs: &Preferences,
    diagnostics: &BTreeMap<String, (String, Vec<String>)>,
) -> Catalog {
    let mut entries = BTreeMap::new();
    for config in configs {
        for ext in definitions(config) {
            let (description, actions) = match ext.id.as_str() {
                "builtin:web" => (
                    "Paste an HTTP(S) URL, or use the keyword to search. Browser actions run only when selected.",
                    vec!["Open in browser".into(), "Copy URL".into()],
                ),
                "builtin:date-time" => (
                    "Local dates, time zones and Unix timestamps. Try time in Tokyo, 90 days from today, or timestamp now.",
                    vec!["Copy".into()],
                ),
                _ => (
                    "Configured executable extension. Its actions appear here after it returns results.",
                    vec![],
                ),
            };
            let pref = prefs.extensions.get(&ext.id);
            let (status, observed) = diagnostics.get(&ext.id).cloned().unwrap_or_default();
            entries.entry(ext.id.clone()).or_insert(ExtensionInfo {
                id: ext.id,
                name: ext.name,
                keyword: pref.map(|p| p.keyword.clone()).unwrap_or(ext.keyword),
                enabled: pref.is_none_or(|p| p.enabled),
                description: description.into(),
                actions: if observed.is_empty() {
                    actions
                } else {
                    observed
                },
                status,
            });
        }
    }
    Catalog {
        extensions: entries.into_values().collect(),
    }
}
fn copy(label: String, description: &str) -> Batch {
    Batch {
        results: vec![ResultItem {
            id: "value".into(),
            label: label.clone(),
            description: description.into(),
            icon: Some("accessories-calculator".into()),
            actions: vec![NamedAction {
                label: "Copy".into(),
                action: Action::Copy { text: label },
            }],
        }],
        status: String::new(),
    }
}
fn web(query: &str, explicit: bool, template: &str) -> Batch {
    let query = query.trim();
    let url = if canopa_launcher::valid_url(query) {
        query.to_owned()
    } else if explicit && !query.is_empty() {
        match canopa_launcher::search_url(template, query) {
            Ok(url) => url,
            Err(_) => return Batch::default(),
        }
    } else {
        return Batch::default();
    };
    let label = if canopa_launcher::valid_url(query) {
        query.chars().take(480).collect()
    } else {
        format!(
            "Search web: {}",
            query.chars().take(450).collect::<String>()
        )
    };
    Batch {
        results: vec![ResultItem {
            id: "web".into(),
            label,
            description: "Open in your default browser · requests start when selected".into(),
            icon: Some("web-browser".into()),
            actions: vec![
                NamedAction {
                    label: "Open in browser".into(),
                    action: Action::OpenUrl { url: url.clone() },
                },
                NamedAction {
                    label: "Copy URL".into(),
                    action: Action::Copy { text: url },
                },
            ],
        }],
        status: String::new(),
    }
}
fn zone(query: &str) -> Option<String> {
    let wanted = query.trim().replace(' ', "_").to_ascii_lowercase();
    if wanted == "utc" {
        return Some("UTC".into());
    }
    let table = std::fs::read_to_string("/usr/share/zoneinfo/zone1970.tab")
        .or_else(|_| std::fs::read_to_string("/usr/share/zoneinfo/zone.tab"))
        .ok()?;
    let mut matches: Vec<_> = table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split('\t').nth(2))
        .filter(|name| {
            name.to_ascii_lowercase() == wanted
                || name
                    .rsplit('/')
                    .next()
                    .is_some_and(|city| city.eq_ignore_ascii_case(&wanted))
        })
        .map(str::to_owned)
        .collect();
    matches.sort();
    matches.dedup();
    (matches.len() == 1).then(|| matches.remove(0))
}
fn datetime(query: &str, explicit: bool, now: DateTime<Local>) -> Batch {
    let query = query.trim();
    let lower = query.to_ascii_lowercase();
    let wanted = lower
        .strip_prefix("time in ")
        .or_else(|| lower.strip_prefix("time "))
        .or(if explicit && !lower.contains(' ') {
            Some(lower.as_str())
        } else {
            None
        });
    if let Some(zone) = wanted.and_then(zone) {
        // TZ affects this child only. The outer provider bounds its process group.
        if let Ok(output) = std::process::Command::new("date")
            .env("TZ", &zone)
            .env("LC_ALL", "C")
            .args([
                "--date",
                &format!("@{}", now.timestamp()),
                "+%Y-%m-%d %H:%M:%S %Z (%:z)",
            ])
            .output()
            && output.status.success()
            && output.stdout.len() < 512
            && let Ok(value) = String::from_utf8(output.stdout)
        {
            return copy(value.trim().into(), &format!("Time in {zone}"));
        }
    }
    if let Some(value) = lower
        .strip_prefix("timestamp ")
        .or_else(|| lower.strip_prefix("unix "))
    {
        if value == "now" {
            return copy(now.timestamp().to_string(), "Unix timestamp · seconds");
        }
        if let Ok(seconds) = value.parse::<i64>()
            && let Some(date) = Utc.timestamp_opt(seconds, 0).single()
        {
            return copy(
                date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                "Unix timestamp to UTC",
            );
        }
        if let Ok(date) = DateTime::parse_from_rfc3339(query.split_once(' ').unwrap().1) {
            return copy(
                date.timestamp().to_string(),
                "Date to Unix timestamp · seconds",
            );
        }
        return Batch::default();
    }
    let offset = match lower.as_str() {
        "today" => Some(0),
        "tomorrow" => Some(1),
        "yesterday" => Some(-1),
        _ => {
            let words: Vec<_> = lower.split_whitespace().collect();
            match words.as_slice() {
                [amount, unit, "from", "today"] | [amount, unit, "ago"] => amount
                    .parse::<i64>()
                    .ok()
                    .filter(|n| n.unsigned_abs() <= 365_000)
                    .and_then(|n| {
                        let days = match *unit {
                            "day" | "days" => Some(n),
                            "week" | "weeks" => n.checked_mul(7),
                            _ => None,
                        }?;
                        Some(if words.last() == Some(&"ago") {
                            -days
                        } else {
                            days
                        })
                    }),
                _ => None,
            }
        }
    };
    if let Some(days) = offset {
        let date = if days >= 0 {
            now.date_naive()
                .checked_add_days(Days::new(days.unsigned_abs()))
        } else {
            now.date_naive()
                .checked_sub_days(Days::new(days.unsigned_abs()))
        };
        if let Some(date) = date {
            return copy(date.to_string(), "Calendar date · local time zone");
        }
    }
    Batch::default()
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol_version: u8,
    query: String,
    limit: usize,
}
/// A bundled extension's answer, computed in the Shell instead of a process
/// per query. `mode` is the extension's argv mode, such as `web-search`.
pub fn answer(mode: &str, template: &str, query: &str) -> Option<Batch> {
    let mut batch = match mode {
        "web" | "web-search" => web(query, mode == "web-search", template),
        "date-time" | "date-time-search" => {
            datetime(query, mode == "date-time-search", Local::now())
        }
        _ => return None,
    };
    batch.results.truncate(super::RESULT_LIMIT);
    Some(batch)
}

pub fn run(mode: &str, template: Option<&str>) -> Result<(), String> {
    let mut input = String::new();
    io::stdin()
        .take(8193)
        .read_to_string(&mut input)
        .map_err(|_| "Could not read request")?;
    if input.len() > 8192 || !input.ends_with('\n') || input[..input.len() - 1].contains('\n') {
        return Err("Expected one bounded JSON request".into());
    }
    let request: Request = serde_json::from_str(&input).map_err(|_| "Invalid request")?;
    if request.protocol_version != 1 || request.limit > 20 || request.query.len() > 1024 {
        return Err("Unsupported request".into());
    }
    let prefs = Preferences::read(&canopa_launcher::preferences_path().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut batch = answer(
        mode,
        template.unwrap_or(&prefs.web_template),
        &request.query,
    )
    .ok_or("Unknown bundled extension")?;
    batch.results.truncate(request.limit);
    serde_json::to_writer(
        io::stdout(),
        &serde_json::json!({"protocol_version":1,"results":batch.results}),
    )
    .map_err(|e| e.to_string())?;
    io::stdout().write_all(b"\n").map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_includes_disabled_extensions_and_safe_failure_status() {
        let mut prefs = Preferences::default();
        prefs.extensions.insert(
            "builtin:web".into(),
            canopa_launcher::ExtensionPreference {
                enabled: false,
                keyword: "search".into(),
            },
        );
        let diagnostics =
            BTreeMap::from([("builtin:web".into(), ("Provider timed out".into(), vec![]))]);
        let catalog = catalog(&[Config::default()], &prefs, &diagnostics);
        let web = catalog
            .extensions
            .iter()
            .find(|e| e.id == "builtin:web")
            .unwrap();
        assert!(!web.enabled);
        assert_eq!(web.keyword, "search");
        assert_eq!(web.status, "Provider timed out");
        assert!(!web.actions.is_empty());
    }
    #[test]
    fn web_is_quiet_for_local_queries_and_only_builds_actions() {
        assert!(
            web("files tax return", false, "https://example.test/?q={query}")
                .results
                .is_empty()
        );
        let b = web("a&b", true, "https://example.test/?q={query}");
        assert!(
            matches!(&b.results[0].actions[0].action,Action::OpenUrl{url} if url=="https://example.test/?q=a%26b")
        );
        assert!(web("javascript:alert(1)", false, "bad").results.is_empty());
    }
    #[test]
    fn calendar_and_timestamp_results_use_an_explicit_instant() {
        let now = Local
            .with_ymd_and_hms(2026, 11, 3, 12, 0, 0)
            .single()
            .unwrap();
        for (q, value) in [
            ("90 days from today", "2027-02-01"),
            ("2 weeks ago", "2026-10-20"),
            ("timestamp 0", "1970-01-01T00:00:00Z"),
            ("timestamp 2026-11-03T12:00:00+01:00", "1793703600"),
        ] {
            assert_eq!(datetime(q, false, now).results[0].label, value, "{q}");
        }
        assert!(datetime("documents", false, now).results.is_empty());
        assert_eq!(zone("Tokyo").as_deref(), Some("Asia/Tokyo"));
    }
}
