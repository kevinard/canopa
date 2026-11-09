use super::calculator::Expression;
use super::{Action, Batch, Extension, MAX_QUERY_BYTES, NamedAction, RESULT_LIMIT, ResultItem};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, process::Stdio, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_OUTPUT: usize = 256 * 1024;

/// All provider descendants belong to this request. Cancellation drops the group
/// guard even if a child holds stdout open after the provider itself exits.
async fn capture(
    command: &mut tokio::process::Command,
    input: &[u8],
    allow_nodefs_status: bool,
) -> Result<Vec<u8>, String> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| "Provider executable is unavailable".to_string())?;
    let _group = crate::command::ProcessGroup::new(child.id());
    let mut stdin = child.stdin.take().ok_or("Provider input is unavailable")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Provider output is unavailable")?;
    tokio::time::timeout(Duration::from_secs(2), async {
        stdin
            .write_all(input)
            .await
            .map_err(|_| "Provider input failed")?;
        drop(stdin);
        let mut bytes = Vec::new();
        stdout
            .take((MAX_OUTPUT + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| "Provider output failed")?;
        if bytes.len() > MAX_OUTPUT {
            return Err("Provider output exceeded 256 KiB".into());
        }
        let status = child.wait().await.map_err(|_| "Provider process failed")?;
        // qalc -n can return 1 with a numerical result; its caller validates
        // the complete result. Extension processes must exit successfully.
        if !(status.success() || allow_nodefs_status && status.code() == Some(1)) {
            return Err("Provider exited unsuccessfully".into());
        }
        Ok(bytes)
    })
    .await
    .map_err(|_| "Provider timed out".to_string())?
}

pub(crate) async fn calculate(query: &str, explicit: bool) -> Batch {
    let expression = match Expression::parse(query) {
        Ok(expression) => expression,
        Err(error) => {
            return Batch {
                status: if explicit && !query.is_empty() {
                    error.into()
                } else {
                    String::new()
                },
                ..Batch::default()
            };
        }
    };
    if !explicit && !expression.automatic {
        return Batch::default();
    }
    let mut command = tokio::process::Command::new("qalc");
    command
        .env("LC_ALL", "C")
        // qalc versions differ in their save-settings handling. A non-directory
        // config/data/cache root prevents reads or writes of personal state.
        .env("XDG_CONFIG_HOME", "/dev/null")
        .env("XDG_DATA_HOME", "/dev/null")
        .env("XDG_CACHE_HOME", "/dev/null")
        .args(["--defaults", "-t", "-m", "100"]);
    if expression.units_enabled() {
        command.args([
            "--nocurrencies",
            "--nodatasets",
            "--novariables",
            "--nofunctions",
        ]);
        command.args(["-s", "prefixes off", "-s", "temperature calculation 1"]);
    } else {
        command.arg("-n");
    }
    for setting in [
        "save config no",
        "save mode no",
        "save definitions no",
        "clear history yes",
        "update exchange rates 0",
        "color 0",
        "unicode no",
        "decimal comma off",
        "fractions off",
    ] {
        command.args(["-s", setting]);
    }
    command.args(["--", &expression.source]);
    match capture(&mut command, b"", !expression.units_enabled()).await {
        Ok(bytes) => {
            // The conversion result must be finite and contain the exact target
            // unit, rather than an unevaluated or incompatible quantity.
            let output = String::from_utf8_lossy(&bytes);
            let Some(value) = expression.result(&output) else {
                return Batch {
                    status: if explicit {
                        if expression.units_enabled() {
                            "Expression has no finite real result in the requested unit".into()
                        } else {
                            "Expression has no finite real result".into()
                        }
                    } else {
                        String::new()
                    },
                    ..Batch::default()
                };
            };
            Batch {
                results: vec![ResultItem {
                    id: "value".into(),
                    label: value.clone(),
                    description: format!("Calculator · {query}"),
                    icon: Some("accessories-calculator".into()),
                    actions: vec![NamedAction {
                        label: "Copy result".into(),
                        action: Action::Copy { text: value },
                    }],
                }],
                status: String::new(),
            }
        }
        Err(error) => Batch {
            status: if explicit {
                format!("Calculator: {error} (requires qalc)")
            } else {
                String::new()
            },
            ..Batch::default()
        },
    }
}

#[derive(Serialize)]
struct Request<'a> {
    protocol_version: u8,
    query: &'a str,
    limit: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    protocol_version: u8,
    results: Vec<ResultItem>,
    #[serde(default)]
    more: bool,
}

fn decode(bytes: &[u8]) -> Result<Batch, String> {
    if bytes.len() > MAX_OUTPUT
        || !bytes.ends_with(b"\n")
        || bytes[..bytes.len().saturating_sub(1)].contains(&b'\n')
    {
        return Err("Expected a bounded JSON line ending in a newline".into());
    }
    let response: Response =
        serde_json::from_slice(bytes).map_err(|_| "Invalid launcher JSON response")?;
    if response.protocol_version != 1 || response.results.len() > RESULT_LIMIT {
        return Err("Unsupported protocol version or result limit".into());
    }
    let mut ids = HashSet::new();
    for result in &response.results {
        if result.id.is_empty()
            || result.id.len() > 128
            || !ids.insert(&result.id)
            || result.label.trim().is_empty()
            || result.label.len() > 512
            || result.description.len() > 2048
            || result.actions.is_empty()
            || result.actions.len() > 4
        {
            return Err("Invalid launcher result metadata".into());
        }
        if result
            .icon
            .as_ref()
            .is_some_and(|icon| !crate::icon_theme::is_valid_icon_name(icon))
        {
            return Err("Invalid launcher icon".into());
        }
        for named in &result.actions {
            if named.label.trim().is_empty() || named.label.len() > 64 {
                return Err("Invalid launcher action label".into());
            }
            match &named.action {
                Action::OpenUrl { url } if canopa_launcher::valid_url(url) => {}
                Action::Copy { text } if text.len() <= 16 * 1024 => {}
                Action::Open { path } if path.is_absolute() => {}
                Action::Launch { argv }
                    if !argv.is_empty()
                        && !argv[0].trim().is_empty()
                        && argv.len() <= 64
                        && argv
                            .iter()
                            .all(|arg| arg.len() <= 4096 && !arg.contains('\0')) => {}
                _ => return Err("Invalid launcher action".into()),
            }
        }
    }
    Ok(Batch {
        results: response.results,
        status: if response.more {
            "More matches; refine your search".into()
        } else {
            String::new()
        },
    })
}

pub(crate) async fn query_extension(extension: &Extension, query: &str) -> Batch {
    let result = async {
        if query.len() > MAX_QUERY_BYTES {
            return Err("Query is too long".into());
        }
        let (program, args) = extension
            .argv
            .split_first()
            .ok_or("Missing provider executable")?;
        let mut command = tokio::process::Command::new(program);
        command.args(args);
        let mut request = serde_json::to_vec(&Request {
            protocol_version: 1,
            query,
            limit: RESULT_LIMIT,
        })
        .map_err(|_| "Could not encode query")?;
        request.push(b'\n');
        decode(&capture(&mut command, &request, false).await?)
    }
    .await;
    result.unwrap_or_else(|error: String| Batch {
        status: format!("{}: {error}", extension.name),
        ..Batch::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn qalc_arithmetic_results_and_invalid_expressions() {
        if std::process::Command::new("qalc")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        for (expression, expected) in [
            ("12*(8+3)", "132"),
            ("-2*3", "-6"),
            ("0.1+0.2", "0.3"),
            ("1,5+2", "3.5"),
        ] {
            let batch = calculate(expression, true).await;
            assert_eq!(
                batch.results.first().map(|result| result.label.as_str()),
                Some(expected),
                "{expression}: {}",
                batch.status
            );
        }
        assert!(calculate("1/0", true).await.results.is_empty());
        assert!(calculate("firefox", false).await.status.is_empty());
    }

    #[test]
    fn extension_response_validates_version_identity_actions_and_framing() {
        let good = serde_json::json!({"protocol_version":1,"results":[{"id":"one","label":"Example","actions":[{"label":"Copy","action":{"type":"copy","text":"value"}}]}]});
        let encode = |value: &serde_json::Value| format!("{value}\n").into_bytes();
        assert_eq!(decode(&encode(&good)).unwrap().results.len(), 1);
        assert!(decode(good.to_string().as_bytes()).is_err());
        let mut bad = good.clone();
        bad["protocol_version"] = 2.into();
        assert!(decode(&encode(&bad)).is_err());
        let mut bad = good.clone();
        bad["results"][0]["actions"][0]["action"] =
            serde_json::json!({"type":"open","path":"relative"});
        assert!(decode(&encode(&bad)).is_err());
        let mut bad = good.clone();
        bad["results"]
            .as_array_mut()
            .unwrap()
            .push(good["results"][0].clone());
        assert!(decode(&encode(&bad)).is_err());
    }

    #[tokio::test]
    async fn real_extension_gets_query_and_returns_literal_actions() {
        let extension = Extension {
            id: "custom:fixture".into(), name: "Fixture".into(), keyword: "p".into(), global: false, argv: vec!["python3".into(), "-c".into(), "import sys,json; q=json.loads(sys.stdin.readline()); print(json.dumps({'protocol_version':q['protocol_version'],'results':[{'id':'x','label':q['query'],'actions':[{'label':'Copy','action':{'type':'copy','text':q['query']}}]}]}))".into()] };
        let batch = query_extension(&extension, "$(literal) 'quoted'").await;
        assert_eq!(batch.results[0].label, "$(literal) 'quoted'");
        assert!(batch.status.is_empty());
    }

    #[tokio::test]
    async fn provider_output_is_bounded_and_cancelled_descendants_are_terminated() {
        let mut flood = tokio::process::Command::new("python3");
        flood.args(["-c", "print('x'*300000)"]);
        assert!(
            capture(&mut flood, b"", false)
                .await
                .unwrap_err()
                .contains("exceeded")
        );

        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("child-pid");
        let mut command = tokio::process::Command::new("python3");
        command.args(["-c", "import pathlib,subprocess,sys,time; child=subprocess.Popen(['sleep','30']); pathlib.Path(sys.argv[1]).write_text(str(child.pid)); time.sleep(30)"]);
        command.arg(&marker);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(250),
                capture(&mut command, b"", false)
            )
            .await
            .is_err()
        );
        let pid = std::fs::read_to_string(&marker).unwrap();
        for _ in 0..50 {
            let state = std::fs::read_to_string(format!("/proc/{pid}/stat"));
            if state.as_ref().map_or(true, |stat| {
                stat.rsplit_once(") ")
                    .is_some_and(|(_, rest)| rest.starts_with(['Z', 'X']))
            }) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("cancelled provider left a live descendant {pid}");
    }
}
