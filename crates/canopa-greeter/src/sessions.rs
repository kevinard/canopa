//! Bounded, command-free discovery of login session desktop entries.
use crate::config::{Session, check_system_source};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::{BTreeMap, HashSet},
    ffi::CString,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
};

const MAX_FILES: usize = 256;
const MAX_BYTES: u64 = 64 * 1024;
const MAX_SESSIONS: usize = 128;
const SEARCH_PATH: [&str; 6] = [
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/local/sbin",
    "/usr/sbin",
    "/sbin",
];

#[derive(Clone, Debug)]
pub struct Discovery {
    pub enabled: bool,
    pub wayland_dirs: Vec<PathBuf>,
    pub x11_dirs: Vec<PathBuf>,
    pub x11_wrapper: Option<Vec<String>>,
}
impl Default for Discovery {
    fn default() -> Self {
        Self {
            enabled: true,
            wayland_dirs: vec![
                "/usr/local/share/wayland-sessions".into(),
                "/usr/share/wayland-sessions".into(),
            ],
            x11_dirs: vec![
                "/usr/local/share/xsessions".into(),
                "/usr/share/xsessions".into(),
            ],
            x11_wrapper: None,
        }
    }
}
impl Discovery {
    pub fn scan(&self, require_system_ownership: bool) -> Vec<Session> {
        if !self.enabled {
            return Vec::new();
        }
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|k| std::env::var(k).ok())
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut budget = MAX_FILES;
        let kinds = [(&self.wayland_dirs, "wayland"), (&self.x11_dirs, "x11")];
        for (directories, kind) in kinds {
            if kind == "x11" && self.x11_wrapper.is_none() {
                continue;
            }
            for dir in directories {
                let files = match paths(dir) {
                    Ok(files) => files,
                    Err(_) => {
                        eprintln!("greeter: cannot scan session directory {}", dir.display());
                        continue;
                    }
                };
                for path in files {
                    if budget == 0 {
                        break;
                    }
                    budget -= 1;
                    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    if stem.is_empty()
                        || stem.len() > if kind == "x11" { 60 } else { 64 }
                        || !stem
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
                    {
                        continue;
                    }
                    let id = if kind == "x11" {
                        format!("x11:{stem}")
                    } else {
                        stem.to_owned()
                    };
                    // A higher-precedence hidden, invalid or unavailable file masks lower copies.
                    if !seen.insert(id.clone()) {
                        continue;
                    }
                    let entry = (|| -> Result<Option<Session>> {
                        if require_system_ownership {
                            check_system_source(&path)?;
                        }
                        let file = OpenOptions::new()
                            .read(true)
                            .custom_flags(libc::O_NONBLOCK)
                            .open(&path)?;
                        ensure!(
                            file.metadata()?.is_file(),
                            "session entry is not a regular file"
                        );
                        let mut body = String::new();
                        file.take(MAX_BYTES + 1).read_to_string(&mut body)?;
                        ensure!(
                            body.len() as u64 <= MAX_BYTES,
                            "session entry exceeds budget"
                        );
                        let mut session = parse(&body, &path, &id, stem, kind, &locale)?;
                        if let Some(session) = &mut session {
                            if kind == "x11" {
                                let mut argv = self.x11_wrapper.clone().unwrap();
                                let program = argv
                                    .first()
                                    .and_then(|v| executable(v))
                                    .context("X11 wrapper is unavailable")?;
                                argv[0] = program
                                    .to_str()
                                    .context("X11 wrapper is not UTF-8")?
                                    .to_owned();
                                argv.append(&mut session.argv);
                                session.argv = argv;
                            }
                            session.validate()?;
                        }
                        Ok(session)
                    })();
                    match entry {
                        Ok(Some(session)) if result.len() < MAX_SESSIONS => result.push(session),
                        Ok(_) => {}
                        Err(error) => eprintln!(
                            "greeter: skipping session entry {}: {error:#}",
                            path.display()
                        ),
                    }
                }
            }
        }
        result.sort_by(|a, b| {
            a.label
                .to_lowercase()
                .cmp(&b.label.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        result
    }
}

fn paths(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut files = Vec::new();
    for (index, entry) in entries.take(MAX_FILES + 1).enumerate() {
        ensure!(index < MAX_FILES, "session directory exceeds budget");
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "desktop") {
            files.push(path);
        }
    }
    ensure!(files.len() <= MAX_FILES, "session directory exceeds budget");
    files.sort();
    Ok(files)
}

fn executable(value: &str) -> Option<PathBuf> {
    fn available(path: &Path) -> bool {
        let Ok(cpath) = CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        path.is_file() && unsafe { libc::access(cpath.as_ptr(), libc::X_OK) } == 0
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return available(path).then(|| path.to_owned());
    }
    if value.is_empty() || value.contains('/') {
        return None;
    }
    let directories = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_else(|| SEARCH_PATH.iter().map(PathBuf::from).collect());
    directories
        .into_iter()
        .filter(|p| p.is_absolute())
        .map(|p| p.join(value))
        .find(|p| available(p))
}

fn fields(body: &str) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    let (mut active, mut found) = (false, false);
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            active = line == "[Desktop Entry]";
            if active {
                ensure!(!found, "duplicate desktop entry group");
                found = true;
            }
            continue;
        }
        if active {
            let (key, value) = line
                .split_once('=')
                .context("invalid desktop entry field")?;
            ensure!(
                result
                    .insert(key.trim().to_owned(), value.trim().to_owned())
                    .is_none(),
                "duplicate desktop entry key"
            );
        }
    }
    ensure!(found, "missing desktop entry group");
    Ok(result)
}
fn unescape(value: &str) -> Result<String> {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        result.push(if c == '\\' {
            match chars.next() {
                Some('s') => ' ',
                Some('n') => '\n',
                Some('t') => '\t',
                Some('r') => '\r',
                Some('\\') => '\\',
                _ => bail!("invalid desktop entry escape"),
            }
        } else {
            c
        });
    }
    ensure!(!result.contains('\0'), "NUL in desktop entry");
    Ok(result)
}
fn flag(fields: &BTreeMap<String, String>, key: &str) -> Result<bool> {
    match fields.get(key).map(String::as_str) {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        _ => bail!("invalid desktop boolean"),
    }
}
fn label(fields: &BTreeMap<String, String>, locale: &str) -> Result<String> {
    let (base, modifier) = locale
        .split_once('@')
        .map_or((locale, None), |(b, m)| (b, Some(m)));
    let base = base.split('.').next().unwrap_or(base);
    let language = base.split('_').next().unwrap_or(base);
    let mut keys = Vec::new();
    if let Some(m) = modifier {
        keys.push(format!("Name[{base}@{m}]"));
    }
    keys.push(format!("Name[{base}]"));
    if let Some(m) = modifier {
        keys.push(format!("Name[{language}@{m}]"));
    }
    keys.push(format!("Name[{language}]"));
    keys.push("Name".into());
    ensure!(fields.contains_key("Name"), "missing default session name");
    unescape(
        keys.iter()
            .find_map(|k| fields.get(k))
            .context("missing session name")?,
    )
}

fn parse(
    body: &str,
    path: &Path,
    id: &str,
    stem: &str,
    kind: &str,
    locale: &str,
) -> Result<Option<Session>> {
    let fields = fields(body)?;
    ensure!(
        fields.get("Type").is_some_and(|v| v == "Application"),
        "unsupported desktop entry type"
    );
    if flag(&fields, "Hidden")?
        || flag(&fields, "NoDisplay")?
        || flag(&fields, "Terminal")?
        || flag(&fields, "DBusActivatable")?
    {
        return Ok(None);
    }
    if let Some(value) = fields.get("TryExec")
        && executable(&unescape(value)?).is_none()
    {
        return Ok(None);
    }
    let label = label(&fields, locale)?;
    let command = unescape(fields.get("Exec").context("missing session command")?)?;
    let icon = fields.get("Icon").map(|v| unescape(v)).transpose()?;
    let mut argv = exec(&command, &label, path, icon.as_deref())?;
    ensure!(
        !argv.is_empty() && !argv[0].contains('='),
        "invalid session program"
    );
    let Some(program) = executable(&argv[0]) else {
        return Ok(None);
    };
    argv[0] = program
        .to_str()
        .context("session executable is not UTF-8")?
        .to_owned();
    if let Some(cwd) = fields
        .get("Path")
        .map(|v| unescape(v))
        .transpose()?
        .filter(|v| !v.is_empty())
    {
        ensure!(
            Path::new(&cwd).is_absolute() && !cwd.chars().any(char::is_control),
            "invalid session working directory"
        );
        let mut command = vec![
            "/usr/bin/env".to_owned(),
            format!("--chdir={cwd}"),
            "--".to_owned(),
        ];
        command.append(&mut argv);
        argv = command;
    }
    let desktops = fields
        .get("DesktopNames")
        .map(|v| v.trim_end_matches(';'))
        .filter(|v| !v.is_empty())
        .unwrap_or(stem);
    ensure!(
        desktops.split(';').all(|v| !v.is_empty()
            && v.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))),
        "invalid desktop identifiers"
    );
    let session = Session {
        id: id.to_owned(),
        label: if kind == "x11" {
            format!("{label} (X11)")
        } else {
            label
        },
        argv,
        env: vec![
            format!("XDG_SESSION_TYPE={kind}"),
            format!("XDG_SESSION_DESKTOP={stem}"),
            format!("XDG_CURRENT_DESKTOP={}", desktops.replace(';', ":")),
        ],
        desktop_file: Some(path.to_owned()),
    };
    session.validate()?;
    Ok(Some(session))
}

/// Desktop-entry quoting is deliberately distinct from shell quoting.
fn exec(value: &str, label: &str, path: &Path, icon: Option<&str>) -> Result<Vec<String>> {
    let mut chars = value.chars().peekable();
    let mut result = Vec::new();
    let mut file_codes = 0;
    let mut first = true;
    while chars.peek().is_some() {
        while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }
        let quoted = chars.peek() == Some(&'"');
        if quoted {
            chars.next();
        }
        let mut token = String::new();
        let mut closed = !quoted;
        while let Some(c) = chars.next() {
            if quoted && c == '"' {
                closed = true;
                break;
            }
            if !quoted && c.is_ascii_whitespace() {
                break;
            }
            if c == '\\' && quoted {
                let c = chars.next().context("trailing command escape")?;
                ensure!(
                    matches!(c, '"' | '`' | '$' | '\\'),
                    "invalid command escape"
                );
                token.push(c);
            } else {
                ensure!(!c.is_control(), "control character in command");
                ensure!(
                    if quoted {
                        !matches!(c, '$' | '`')
                    } else {
                        !"\"'\\><~|&;$*?#()`".contains(c)
                    },
                    "unquoted command character"
                );
                token.push(c);
            }
        }
        ensure!(
            closed && (!quoted || chars.peek().is_none_or(|c| c.is_ascii_whitespace())),
            "invalid command quoting"
        );
        ensure!(
            !first || !token.replace("%%", "").contains('%'),
            "field code in session executable"
        );
        first = false;
        if !quoted && matches!(token.as_str(), "%F" | "%U" | "%i") {
            if token == "%i" {
                if let Some(icon) = icon.filter(|v| !v.is_empty()) {
                    result.extend(["--icon".to_owned(), icon.to_owned()]);
                }
            } else {
                file_codes += 1;
            }
            continue;
        }
        let mut expanded = String::new();
        let mut codes = token.chars();
        let mut removed = false;
        while let Some(c) = codes.next() {
            if c != '%' {
                expanded.push(c);
                continue;
            }
            let code = codes.next().context("trailing command field code")?;
            ensure!(!quoted || code == '%', "field code in quoted argument");
            match code {
                '%' => expanded.push('%'),
                'c' => expanded.push_str(label),
                'k' => expanded.push_str(path.to_str().context("desktop file path is not UTF-8")?),
                'f' | 'u' => {
                    file_codes += 1;
                    removed = true;
                }
                'd' | 'D' | 'n' | 'N' | 'v' | 'm' => {
                    removed = true;
                }
                _ => bail!("unsupported command field code"),
            }
        }
        if !removed || !expanded.is_empty() {
            result.push(expanded);
        }
    }
    ensure!(file_codes <= 1, "multiple file field codes");
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_exec_rejects_invalid_quoting_and_unknown_or_ambiguous_field_codes() {
        for command in [
            "/bin/true %Z",
            "/bin/true %",
            "/bin/true \"%c\"",
            "/bin/true x%F",
            "/bin/true %f %U",
            "/bin/true \"unfinished",
            "/bin/true \"arg\"tail",
            "/bin/true 'shell quoting'",
            "/bin/true $(touch /tmp/marker)",
            "/bin/true \"$HOME\"",
            "%f /bin/true",
        ] {
            assert!(
                exec(command, "Desktop", Path::new("/tmp/example.desktop"), None).is_err(),
                "{command}"
            );
        }
        assert_eq!(
            exec(
                "/bin/true \"\" %f %d %i %% prefix%c",
                "Desktop Name",
                Path::new("/tmp/example.desktop"),
                None
            )
            .unwrap(),
            ["/bin/true", "", "%", "prefixDesktop Name"]
        );
    }
    #[test]
    fn localized_names_follow_posix_locale_fallback_without_environment_mutation() {
        let data=fields("[Desktop Entry]\nName=Default\nName[sr]=Language\nName[sr@Latn]=Modifier\nName[sr_RS]=Country\nName[sr_RS@Latn]=Full\n").unwrap();
        for (locale, expected) in [
            ("sr_RS.UTF-8@Latn", "Full"),
            ("sr_RS.UTF-8", "Country"),
            ("sr_XX@Latn", "Modifier"),
            ("sr_XX", "Language"),
            ("C.UTF-8", "Default"),
        ] {
            assert_eq!(label(&data, locale).unwrap(), expected);
        }
    }
    #[test]
    fn parser_honors_only_the_main_group_and_rejects_duplicate_main_groups() {
        let text = "[Desktop Entry]\nType=Application\nName=Main\nExec=/usr/bin/true\n[Desktop Action Other]\nName=Action\nExec=/no/such/action\n";
        let result = parse(
            text,
            Path::new("/tmp/main.desktop"),
            "main",
            "main",
            "wayland",
            "C",
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.label, "Main");
        assert_eq!(result.argv, ["/usr/bin/true"]);
        assert!(fields(&(text.to_owned() + "[Desktop Entry]\nName=Duplicate\n")).is_err());
    }
}
