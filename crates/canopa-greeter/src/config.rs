use anyhow::{Context, Result, bail, ensure};
use canopa_config::{Component, Document, Request};
use canopa_session_ui::settings::Settings;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Session {
    pub id: String,
    pub label: String,
    pub argv: Vec<String>,
    pub env: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desktop_file: Option<PathBuf>,
}
impl Session {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty() && self.id.len() <= 64,
            "invalid session id"
        );
        ensure!(
            !self.argv.is_empty()
                && !self.argv[0].is_empty()
                && self.argv.len() <= 64
                && self
                    .argv
                    .iter()
                    .all(|v| v.len() <= 4096 && !v.contains('\0')),
            "invalid session command"
        );
        ensure!(
            !self.label.is_empty()
                && self.label.len() <= 128
                && !self.label.chars().any(char::is_control),
            "invalid session label"
        );
        ensure!(
            self.env.len() <= 64 && self.env.iter().all(|v| valid_env(v)),
            "invalid session environment"
        );
        let mut keys = std::collections::HashSet::new();
        ensure!(
            self.env
                .iter()
                .all(|v| keys.insert(v.split_once('=').unwrap().0)),
            "duplicate session environment key"
        );
        let frame = serde_json::json!({"type":"start_session","cmd":[self.shell_command()?],"env":self.env});
        ensure!(
            serde_json::to_vec(&frame)?.len() <= 8192,
            "session request exceeds backend budget (8 KiB)"
        );
        Ok(())
    }
    /// greetd joins its command array for /bin/sh. Quote logical argv at that boundary.
    pub fn shell_command(&self) -> Result<String> {
        ensure!(
            self.argv.first().is_some_and(|arg| !arg.is_empty()),
            "empty session command"
        );
        Ok(shlex::try_join(self.argv.iter().map(String::as_str))?)
    }
}
#[derive(Clone, Debug)]
pub struct Config {
    pub view: Settings,
    pub users: Vec<String>,
    pub discover_users: bool,
    pub user_source: crate::accounts::Source,
    pub nss: crate::nss::Policy,
    pub avatars: crate::avatars::Policy,
    pub default_user: String,
    pub remember_selection: bool,
    pub power_controls: bool,
    pub sessions: Vec<Session>,
    pub discovery: crate::sessions::Discovery,
    pub system_sources: bool,
}
impl Config {
    pub fn system_request() -> Request {
        let path = PathBuf::from("/etc/canopa/greeter.kdl");
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Request::Defaults,
            _ => Request::common(path),
        }
    }
    pub fn load(request: &Request) -> Result<(Self, Document)> {
        let document = Document::load(request, Component::Greeter, "")?;
        let mut users = Vec::new();
        let mut discover_users = true;
        let mut user_source = crate::accounts::Source::Auto;
        let mut nss = crate::nss::Policy::default();
        let mut avatars = crate::avatars::Policy::default();
        let mut default_user = String::new();
        let mut remember_selection = true;
        let mut power_controls = true;
        let mut sessions = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut view = None;
        let mut discovery = crate::sessions::Discovery::default();
        for node in document.selected().nodes() {
            let name = node.name().value();
            ensure!(node.ty().is_none(), "annotated greeter setting");
            if name != "session" && name != "user-avatar" {
                ensure!(seen.insert(name), "duplicate greeter setting");
            }
            match name {
                "user-avatar" => {
                    ensure!(
                        node.children().is_none() && node.entries().len() == 2,
                        "user-avatar takes username and path"
                    );
                    let username = entry_string(&node.entries()[0])?;
                    let path = avatar_path(
                        &entry_string(&node.entries()[1])?,
                        document.info().path.as_deref(),
                    )?;
                    ensure!(
                        valid_user(&username)
                            && avatars.users.len() < 64
                            && avatars.users.insert(username, path).is_none(),
                        "invalid, duplicate or too many user avatars"
                    );
                }
                "avatar-dir" => {
                    ensure!(node.children().is_none(), "avatar-dir takes a path");
                    avatars.directory = avatar_path(&one(node)?, document.info().path.as_deref())?;
                }
                "user-source" => {
                    ensure!(node.children().is_none(), "user-source takes a string");
                    user_source = match one(node)?.as_str() {
                        "auto" => crate::accounts::Source::Auto,
                        "accountsservice" => crate::accounts::Source::Accountsservice,
                        "nss" => crate::accounts::Source::Nss,
                        _ => bail!("invalid user discovery source"),
                    };
                }
                "nss-uid-min" | "nss-uid-max" => {
                    ensure!(
                        node.children().is_none() && node.entries().len() == 1,
                        "NSS UID bound takes an integer"
                    );
                    let entry = &node.entries()[0];
                    ensure!(
                        entry.name().is_none() && entry.ty().is_none(),
                        "expected an integer"
                    );
                    let value = u32::try_from(
                        entry
                            .value()
                            .as_integer()
                            .context("NSS UID bound takes an integer")?,
                    )
                    .context("invalid NSS UID bound")?;
                    if name == "nss-uid-min" {
                        nss.uid_min = Some(value);
                    } else {
                        nss.uid_max = Some(value);
                    }
                }
                "discover-sessions"
                | "discover-users"
                | "nss-include-nologin"
                | "user-avatars"
                | "remember-selection"
                | "power-controls" => {
                    ensure!(
                        node.children().is_none() && node.entries().len() == 1,
                        "discovery setting takes a boolean"
                    );
                    let entry = &node.entries()[0];
                    ensure!(
                        entry.name().is_none() && entry.ty().is_none(),
                        "expected a boolean"
                    );
                    let enabled = entry
                        .value()
                        .as_bool()
                        .context("discovery setting takes a boolean")?;
                    if name == "power-controls" {
                        power_controls = enabled;
                    } else if name == "remember-selection" {
                        remember_selection = enabled;
                    } else if name == "user-avatars" {
                        avatars.enabled = enabled;
                    } else if name == "nss-include-nologin" {
                        nss.include_nologin = enabled;
                    } else if name == "discover-users" {
                        discover_users = enabled;
                    } else {
                        discovery.enabled = enabled;
                    }
                }
                "session-dirs" | "xsession-dirs" => {
                    ensure!(node.children().is_none(), "session directories take paths");
                    let dirs = node
                        .entries()
                        .iter()
                        .map(entry_string)
                        .map(|v| v.map(PathBuf::from))
                        .collect::<Result<Vec<_>>>()?;
                    ensure!(
                        dirs.len() <= 8 && dirs.iter().all(|p| p.is_absolute()),
                        "session directories require at most eight absolute paths"
                    );
                    if name == "session-dirs" {
                        discovery.wayland_dirs = dirs;
                    } else {
                        discovery.x11_dirs = dirs;
                    }
                }
                "x11-wrapper" => {
                    ensure!(
                        node.entries().is_empty(),
                        "x11-wrapper requires an argv block"
                    );
                    let block = node
                        .children()
                        .context("x11-wrapper requires an argv block")?;
                    ensure!(block.nodes().len() == 1, "x11-wrapper requires argv");
                    let argv = &block.nodes()[0];
                    ensure!(
                        argv.name().value() == "argv"
                            && argv.ty().is_none()
                            && argv.children().is_none(),
                        "x11-wrapper requires argv"
                    );
                    let command = argv
                        .entries()
                        .iter()
                        .map(entry_string)
                        .collect::<Result<Vec<_>>>()?;
                    ensure!(
                        !command.is_empty()
                            && !command[0].is_empty()
                            && command.len() <= 16
                            && command.iter().all(|v| v.len() <= 4096 && !v.contains('\0')),
                        "invalid X11 wrapper"
                    );
                    Session {
                        id: "x11-wrapper".into(),
                        label: "X11 wrapper".into(),
                        argv: command.clone(),
                        env: Vec::new(),
                        desktop_file: None,
                    }
                    .validate()?;
                    discovery.x11_wrapper = Some(command);
                }
                "view" => {
                    ensure!(node.entries().is_empty(), "view takes a block");
                    view = Some(node.children().context("view takes a block")?);
                }
                "users" => {
                    ensure!(node.children().is_none(), "users takes names");
                    for entry in node.entries() {
                        users.push(entry_string(entry)?);
                    }
                }
                "default-user" => {
                    ensure!(node.children().is_none(), "default-user takes a string");
                    default_user = one(node)?;
                }
                "session" => {
                    let id = one(node)?;
                    ensure!(
                        !id.is_empty()
                            && id.len() <= 64
                            && !sessions.iter().any(|s: &Session| s.id == id),
                        "invalid or duplicate session id"
                    );
                    let mut session = Session {
                        id: id.clone(),
                        label: id,
                        argv: Vec::new(),
                        env: Vec::new(),
                        desktop_file: None,
                    };
                    let mut fields = std::collections::HashSet::new();
                    for field in node.children().context("session requires a block")?.nodes() {
                        ensure!(
                            field.ty().is_none() && fields.insert(field.name().value()),
                            "duplicate or annotated session field"
                        );
                        match field.name().value() {
                            "label" => {
                                ensure!(field.children().is_none(), "label takes a string");
                                session.label = one(field)?;
                            }
                            "command" => {
                                if let Some(block) = field.children() {
                                    ensure!(
                                        field.entries().is_empty() && block.nodes().len() == 1,
                                        "command requires argv"
                                    );
                                    let argv = &block.nodes()[0];
                                    ensure!(
                                        argv.name().value() == "argv"
                                            && argv.ty().is_none()
                                            && argv.children().is_none(),
                                        "command requires argv"
                                    );
                                    session.argv = argv
                                        .entries()
                                        .iter()
                                        .map(entry_string)
                                        .collect::<Result<_>>()?;
                                } else {
                                    session.argv = shlex::split(&one(field)?)
                                        .context("unmatched command quote")?;
                                }
                            }
                            "env" => {
                                ensure!(field.children().is_none(), "env requires assignments");
                                session.env = field
                                    .entries()
                                    .iter()
                                    .map(entry_string)
                                    .collect::<Result<_>>()?;
                            }
                            _ => bail!("unknown session setting"),
                        }
                    }
                    session.validate()?;
                    sessions.push(session);
                    ensure!(sessions.len() <= 32, "too many sessions");
                }
                _ => bail!("unknown greeter setting"),
            }
        }
        ensure!(
            users.len() <= 64
                && users.iter().all(|u| valid_user(u))
                && (default_user.is_empty() || valid_user(&default_user)),
            "invalid greeter user"
        );
        let mut unique = std::collections::HashSet::new();
        ensure!(
            users.iter().all(|u| unique.insert(u)),
            "duplicate greeter user"
        );
        let empty = kdl::KdlDocument::new();
        nss.validate()?;
        let mut presentation = Settings::parse(
            view.unwrap_or(&empty),
            document.appearance(),
            document.info().path.as_deref(),
            Component::Greeter,
        )?;
        // No user-session defaults or wallpaper socket are consulted before login.
        if view.is_none_or(|v| v.get("widgets").is_none()) {
            presentation.widgets = vec!["avatar".into(), "keyboard".into(), "battery".into()];
        }
        if !avatars.enabled {
            presentation.avatar = None;
            presentation.widgets.retain(|w| w != "avatar");
        }
        ensure!(
            view.and_then(|v| v.get("use-wallpaper"))
                .and_then(|n| n.entries().first())
                .and_then(|e| e.value().as_bool())
                != Some(true),
            "greeter background must be configured explicitly"
        );
        presentation.use_wallpaper = false;
        ensure!(
            !presentation.widgets.iter().any(|v| v == "media"),
            "media is a user-session widget"
        );
        Ok((
            Self {
                view: presentation,
                users,
                discover_users,
                user_source,
                nss,
                avatars,
                default_user,
                remember_selection,
                power_controls,
                sessions,
                discovery,
                system_sources: true,
            },
            document,
        ))
    }
    /// Resolve once at frontend startup; schema validation itself never scans the host.
    pub fn resolve_sessions(&mut self, require_system_ownership: bool) {
        self.system_sources = require_system_ownership;
        if !self.discovery.enabled {
            return;
        }
        self.sessions
            .retain(|session| session.desktop_file.is_none());
        let known: std::collections::HashSet<_> =
            self.sessions.iter().map(|s| s.id.clone()).collect();
        let detected = self.discovery.scan(require_system_ownership);
        let remaining = 128usize.saturating_sub(self.sessions.len());
        self.sessions.extend(
            detected
                .into_iter()
                .filter(|s| !known.contains(&s.id))
                .take(remaining),
        );
    }
    pub fn avatars_enabled(&self) -> bool {
        self.avatars.enabled
            && self.view.widgets.iter().any(|w| w == "avatar")
            && self
                .view
                .layout
                .regions
                .iter()
                .any(|r| r.widgets.iter().any(|w| w == "avatar"))
    }
}
fn avatar_path(text: &str, source: Option<&Path>) -> Result<PathBuf> {
    let path = PathBuf::from(text);
    ensure!(!text.is_empty(), "empty avatar path");
    let path = if path.is_absolute() {
        path
    } else {
        source
            .and_then(Path::parent)
            .unwrap_or(Path::new("/etc/canopa"))
            .join(path)
    };
    ensure!(crate::avatars::valid_path(&path), "invalid avatar path");
    Ok(path)
}
pub fn valid_user(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && !value
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == ':' || c == '/' || c == '\0')
}
fn valid_env(v: &str) -> bool {
    let Some((key, value)) = v.split_once('=') else {
        return false;
    };
    !key.is_empty()
        && key
            .bytes()
            .enumerate()
            .all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        && value.len() <= 4096
        && !value.contains('\0')
        && key != "GREETD_SOCK"
}
fn entry_string(e: &kdl::KdlEntry) -> Result<String> {
    ensure!(
        e.name().is_none() && e.ty().is_none(),
        "expected unannotated string"
    );
    Ok(e.value().as_string().context("expected string")?.to_owned())
}
fn one(node: &kdl::KdlNode) -> Result<String> {
    ensure!(node.entries().len() == 1, "expected one string");
    entry_string(&node.entries()[0])
}
/// Default deployment uses a root-owned, non-writable source; explicit paths are
/// useful for isolated development and still run with the greeter's own privileges.
pub fn check_system_source(path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let canonical = std::fs::canonicalize(path)?;
    for path in path.ancestors().chain(canonical.ancestors()) {
        let meta = std::fs::metadata(path)?;
        ensure!(
            meta.uid() == 0 && meta.mode() & 0o022 == 0,
            "greeter system configuration must be root-owned and not group/world writable"
        );
    }
    Ok(())
}
