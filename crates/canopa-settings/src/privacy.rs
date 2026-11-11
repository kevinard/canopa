//! The Privacy page's model: what applications were allowed or denied through
//! portals (the portal permission store), and what starts at login
//! (XDG autostart entries).
//!
//! xdg-desktop-portal records each grant; revoking one makes the portal ask
//! again. Canopa's own screen-sharing and remote-desktop dialogs do not
//! offer to remember a choice (they answer `persist_mode` 0), so there is no
//! Canopa sharing history to show; restore grants other backends stored are
//! listed and can be removed.
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_services::permission_store;
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const NOT_RUNNING: &str = "The portal permission store (xdg-permission-store) is not running.";

/// A kind of access the page lists, and where the portal records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Access {
    Location,
    Camera,
    Microphone,
    Speakers,
    Screenshot,
    Background,
    Notifications,
    ScreenCast,
    RemoteDesktop,
}

impl Access {
    pub const ALL: [Self; 9] = [
        Self::Location,
        Self::Camera,
        Self::Microphone,
        Self::Speakers,
        Self::Screenshot,
        Self::Background,
        Self::Notifications,
        Self::ScreenCast,
        Self::RemoteDesktop,
    ];

    pub fn table(self) -> &'static str {
        match self {
            Self::Location => "location",
            Self::Camera | Self::Microphone | Self::Speakers => "devices",
            Self::Screenshot => "screenshot",
            Self::Background => "background",
            Self::Notifications => "notifications",
            Self::ScreenCast => "screencast",
            Self::RemoteDesktop => "remote-desktop",
        }
    }

    /// The entry id, or `None` when each entry is a restore grant.
    pub fn id(self) -> Option<&'static str> {
        match self {
            Self::Location => Some("location"),
            Self::Camera => Some("camera"),
            Self::Microphone => Some("microphone"),
            Self::Speakers => Some("speakers"),
            Self::Screenshot => Some("screenshot"),
            Self::Background => Some("background"),
            Self::Notifications => Some("notification"),
            Self::ScreenCast | Self::RemoteDesktop => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Location => "Location",
            Self::Camera => "Camera",
            Self::Microphone => "Microphone",
            Self::Speakers => "Speakers",
            Self::Screenshot => "Screenshots",
            Self::Background => "Running in the background",
            Self::Notifications => "Notifications",
            Self::ScreenCast => "Screen sharing",
            Self::RemoteDesktop => "Remote desktop",
        }
    }

    /// Whether the grant is a yes/no the page can flip.
    pub fn switchable(self) -> bool {
        !matches!(
            self,
            Self::Location | Self::ScreenCast | Self::RemoteDesktop
        )
    }
}

/// One application's recorded answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub access: Access,
    /// The entry: the access's id, or a restore grant's token.
    pub id: String,
    /// The sandboxed application id, such as `org.mozilla.firefox`.
    pub app: String,
    pub permissions: Vec<String>,
}

impl Grant {
    /// Allowed, denied, or (for other values) neither.
    pub fn allowed(&self) -> Option<bool> {
        match self.permissions.first().map(String::as_str) {
            Some("yes") => Some(true),
            Some("no") => Some(false),
            // Location: an accuracy level, NONE when denied.
            Some("NONE") => Some(false),
            Some(_) if self.access == Access::Location => Some(true),
            Some(_) if !self.access.switchable() => Some(true),
            _ => None,
        }
    }
}

/// An entry in an autostart folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Autostart {
    /// The file name, such as `org.example.Chat.desktop`.
    pub id: String,
    pub name: String,
    /// Started at login now (not `Hidden`, and not hidden by a user entry).
    pub enabled: bool,
    /// Added by a sandboxed application through the Background portal.
    pub sandboxed: bool,
    /// Installed system-wide; turning it off writes a user override.
    pub system: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivacyState {
    /// `None` while the permission store is not available.
    pub grants: Option<Vec<Grant>>,
    pub autostart: Vec<Autostart>,
    /// Application names by application id, where installed.
    pub names: Arc<HashMap<String, String>>,
}

impl PrivacyState {
    pub fn name<'a>(&'a self, app: &'a str) -> &'a str {
        self.names.get(app).map_or(app, String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrivacyAction {
    /// Forget the answer; the portal asks again.
    Revoke(Grant),
    Set(Grant, bool),
    /// An autostart file id and whether it starts at login.
    Autostart(String, bool),
}

impl LiveAction for PrivacyAction {
    fn control(&self) -> String {
        match self {
            Self::Revoke(grant) | Self::Set(grant, _) => {
                format!("{}-{}-{}", grant.access.table(), grant.id, grant.app)
            }
            Self::Autostart(id, _) => format!("autostart-{id}"),
        }
    }
}

/// `[Desktop Entry]` keys of a desktop file.
fn entry_keys(text: &str) -> HashMap<String, String> {
    let mut keys = HashMap::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
        } else if inside && let Some((key, value)) = line.split_once('=') {
            keys.entry(key.trim().to_owned())
                .or_insert_with(|| value.trim().to_owned());
        }
    }
    keys
}

/// `text` with `Hidden` set to `hidden` in `[Desktop Entry]`, keeping the rest.
pub fn with_hidden(text: &str, hidden: bool) -> String {
    let mut out = Vec::new();
    let mut inside = false;
    let mut written = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if inside && !written && hidden {
                out.push("Hidden=true".to_owned());
                written = true;
            }
            inside = trimmed == "[Desktop Entry]";
        } else if inside && trimmed.split('=').next().map(str::trim) == Some("Hidden") {
            if hidden && !written {
                out.push("Hidden=true".to_owned());
                written = true;
            }
            continue;
        }
        out.push(line.to_owned());
    }
    if inside && !written && hidden {
        out.push("Hidden=true".to_owned());
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

fn read_entry(path: &Path) -> Option<String> {
    canopa_file::read_bounded(path, 256 * 1024)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn desktop_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_owned();
            (name.ends_with(".desktop") && entry.file_type().ok()?.is_file())
                .then(|| (name, entry.path()))
        })
        .take(256)
        .collect();
    files.sort();
    files
}

/// Autostart entries: the user's folder over the system folders.
pub fn autostart(user: &Path, system: &[PathBuf]) -> Vec<Autostart> {
    let mut found: Vec<Autostart> = Vec::new();
    let mut user_ids = Vec::new();
    for (id, path) in desktop_files(user) {
        let Some(text) = read_entry(&path) else {
            continue;
        };
        let keys = entry_keys(&text);
        user_ids.push(id.clone());
        found.push(Autostart {
            name: keys.get("Name").cloned().unwrap_or_else(|| id.clone()),
            enabled: keys.get("Hidden").map(String::as_str) != Some("true"),
            sandboxed: keys.contains_key("X-Flatpak"),
            system: false,
            id,
        });
    }
    for dir in system {
        for (id, path) in desktop_files(dir) {
            if found.iter().any(|entry| entry.id == id) {
                // A user entry with the same name overrides it.
                if let Some(entry) = found.iter_mut().find(|entry| entry.id == id)
                    && user_ids.contains(&id)
                {
                    entry.system = true;
                }
                continue;
            }
            let Some(text) = read_entry(&path) else {
                continue;
            };
            let keys = entry_keys(&text);
            found.push(Autostart {
                name: keys.get("Name").cloned().unwrap_or_else(|| id.clone()),
                enabled: keys.get("Hidden").map(String::as_str) != Some("true"),
                sandboxed: false,
                system: true,
                id,
            });
        }
    }
    found.sort_by_key(|entry| entry.name.to_lowercase());
    found
}

/// The permission store and autostart folders for the page.
#[derive(Clone)]
pub struct PrivacyService {
    session: Arc<tokio::sync::OnceCell<zbus::Connection>>,
    names: Arc<tokio::sync::OnceCell<Arc<HashMap<String, String>>>>,
    autostart_user: Option<PathBuf>,
    autostart_system: Vec<PathBuf>,
}

impl Default for PrivacyService {
    fn default() -> Self {
        let config = canopa_config::config_home().ok();
        let dirs = std::env::var_os("XDG_CONFIG_DIRS")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/etc/xdg".into());
        Self {
            session: Arc::default(),
            names: Arc::default(),
            autostart_user: config.map(|config| config.join("autostart")),
            autostart_system: std::env::split_paths(&dirs)
                .filter(|path| path.is_absolute())
                .map(|path| path.join("autostart"))
                .collect(),
        }
    }
}

fn failure(error: canopa_services::Error) -> ServiceError {
    use canopa_services::Error;
    match error {
        Error::Unavailable => ServiceError::Failed(NOT_RUNNING.into()),
        Error::NotAuthorized => ServiceError::Failed("The change was not allowed.".into()),
        Error::TimedOut => {
            ServiceError::Failed("The permission store did not answer in time.".into())
        }
        Error::Changed | Error::Invalid(_) => {
            ServiceError::Failed("The grant changed. Try again.".into())
        }
        Error::Refused(message) => ServiceError::Failed(message),
    }
}

impl PrivacyService {
    /// A service on an existing session connection and autostart folders (tests).
    pub fn with(session: zbus::Connection, user: PathBuf, system: Vec<PathBuf>) -> Self {
        Self {
            session: Arc::new(tokio::sync::OnceCell::new_with(Some(session))),
            names: Arc::new(tokio::sync::OnceCell::new_with(Some(Arc::default()))),
            autostart_user: Some(user),
            autostart_system: system,
        }
    }

    async fn session(&self) -> Option<zbus::Connection> {
        self.session
            .get_or_try_init(canopa_services::bus::session)
            .await
            .ok()
            .cloned()
    }

    async fn grants(
        &self,
        session: &zbus::Connection,
    ) -> Result<Vec<Grant>, canopa_services::Error> {
        let mut grants = Vec::new();
        for access in Access::ALL {
            let ids = match access.id() {
                Some(id) => vec![id.to_owned()],
                None => permission_store::list(session, access.table()).await?,
            };
            for id in ids {
                let mut apps: Vec<_> = permission_store::lookup(session, access.table(), &id)
                    .await?
                    .into_iter()
                    .filter(|(app, _)| !app.is_empty())
                    .collect();
                apps.sort();
                for (app, permissions) in apps {
                    grants.push(Grant {
                        access,
                        id: id.clone(),
                        app,
                        permissions,
                    });
                }
            }
        }
        Ok(grants)
    }

    async fn read(&self) -> PrivacyState {
        let grants = match self.session().await {
            Some(session) => self.grants(&session).await.ok(),
            None => None,
        };
        let (user, system) = (self.autostart_user.clone(), self.autostart_system.clone());
        let autostart = tokio::task::spawn_blocking(move || match user {
            Some(user) => autostart(&user, &system),
            None => Vec::new(),
        })
        .await
        .unwrap_or_default();
        let names = self
            .names
            .get_or_init(|| async {
                let names = tokio::task::spawn_blocking(|| {
                    canopa_apps::discover_installed_applications()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|app| (crate::notifications::app_identity(&app.id), app.name))
                        .collect::<HashMap<_, _>>()
                })
                .await
                .unwrap_or_default();
                Arc::new(names)
            })
            .await
            .clone();
        PrivacyState {
            grants,
            autostart,
            names,
        }
    }

    fn set_autostart(&self, id: &str, enabled: bool) -> Result<(), ServiceError> {
        if !id.ends_with(".desktop") || id.contains('/') || id.starts_with('.') {
            return Err(ServiceError::Failed("Unknown autostart entry.".into()));
        }
        let user = self
            .autostart_user
            .clone()
            .ok_or_else(|| ServiceError::Failed("There is no configuration folder.".into()))?;
        let path = user.join(id);
        let system = self
            .autostart_system
            .iter()
            .map(|dir| dir.join(id))
            .find(|path| path.is_file());
        let text = match read_entry(&path) {
            Some(text) => text,
            // Turning off a system entry writes a user override.
            None => match &system {
                Some(system) => read_entry(system).unwrap_or_default(),
                None => return Err(ServiceError::Failed("The entry is gone.".into())),
            },
        };
        let failed = |error: std::io::Error| {
            ServiceError::Failed(format!("The entry could not be saved: {error}"))
        };
        if enabled && system.is_some() && path.is_file() {
            // Removing the override brings back the system entry.
            let original = system.as_ref().and_then(|system| read_entry(system));
            if original
                .is_some_and(|original| with_hidden(&original, true) == with_hidden(&text, true))
            {
                return std::fs::remove_file(&path).map_err(failed);
            }
        }
        std::fs::create_dir_all(&user).map_err(failed)?;
        let mut file = tempfile::NamedTempFile::new_in(&user).map_err(failed)?;
        use std::io::Write;
        file.write_all(with_hidden(&text, !enabled).as_bytes())
            .map_err(failed)?;
        file.persist(&path).map_err(|error| failed(error.error))?;
        Ok(())
    }

    async fn run(&self, action: PrivacyAction) -> Result<(), ServiceError> {
        if let PrivacyAction::Autostart(id, enabled) = &action {
            let (service, id, enabled) = (self.clone(), id.clone(), *enabled);
            return tokio::task::spawn_blocking(move || service.set_autostart(&id, enabled))
                .await
                .map_err(|_| ServiceError::Failed("The entry could not be saved.".into()))?;
        }
        let session = self
            .session()
            .await
            .ok_or_else(|| ServiceError::Failed("The session bus is not reachable.".into()))?;
        match action {
            PrivacyAction::Revoke(grant) => match grant.access.id() {
                Some(_) => {
                    permission_store::delete_permission(
                        &session,
                        grant.access.table(),
                        &grant.id,
                        &grant.app,
                    )
                    .await
                }
                None => permission_store::delete(&session, grant.access.table(), &grant.id).await,
            },
            PrivacyAction::Set(grant, allowed) => {
                if !grant.access.switchable() {
                    return Err(ServiceError::Failed(
                        "This permission can only be revoked.".into(),
                    ));
                }
                permission_store::set_permission(
                    &session,
                    grant.access.table(),
                    &grant.id,
                    &grant.app,
                    &[if allowed { "yes" } else { "no" }],
                )
                .await
            }
            PrivacyAction::Autostart(..) => unreachable!("handled above"),
        }
        .map_err(failure)
    }
}

impl LiveService for PrivacyService {
    type State = PrivacyState;
    type Action = PrivacyAction;

    fn name(&self) -> &str {
        "The portal permission store"
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<PrivacyState, ServiceError>> {
        let service = self.clone();
        async move { Ok(service.read().await) }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<PrivacyState, ServiceError>> {
        // Grants change through the store's signal; autostart folders and a
        // store that starts later are read again every few seconds.
        let service = self.clone();
        futures_util::stream::once(async move {
            let changes = match service.session().await {
                Some(session) => permission_store::changes(&session)
                    .await
                    .map(|changes| changes.map(drop).boxed())
                    .unwrap_or_else(|_| futures_util::stream::pending().boxed()),
                None => futures_util::stream::pending().boxed(),
            };
            let ticks = futures_util::stream::unfold((), |()| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Some(((), ()))
            });
            futures_util::stream::select(changes, ticks.boxed())
                .then(move |()| {
                    let service = service.clone();
                    async move { Ok(service.read().await) }
                })
                .boxed()
        })
        .flatten()
        .boxed()
    }

    fn apply(
        &self,
        action: PrivacyAction,
    ) -> BoxFuture<'static, Result<Option<PrivacyState>, ServiceError>> {
        let service = self.clone();
        async move {
            service.run(action).await?;
            Ok(Some(service.read().await))
        }
        .boxed()
    }
}

/// What the page says when the store is missing.
pub const STORE_MISSING: &str = NOT_RUNNING;

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{TestBus, permission_store::FakePermissionStore};

    #[test]
    fn autostart_entries_and_overrides() {
        let root = tempfile::tempdir().unwrap();
        let (user, system) = (root.path().join("user"), root.path().join("system"));
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        std::fs::write(
            user.join("chat.desktop"),
            "[Desktop Entry]\nName=Chat\nX-Flatpak=org.example.Chat\n",
        )
        .unwrap();
        std::fs::write(
            system.join("agent.desktop"),
            "[Desktop Entry]\nName=Agent\nExec=agent\n",
        )
        .unwrap();
        let service = PrivacyService {
            session: Arc::default(),
            names: Arc::default(),
            autostart_user: Some(user.clone()),
            autostart_system: vec![system.clone()],
        };
        let entries = autostart(&user, std::slice::from_ref(&system));
        assert_eq!(entries.len(), 2);
        assert!(entries[0].system && entries[0].enabled && entries[0].name == "Agent");
        assert!(entries[1].sandboxed && !entries[1].system);

        service.set_autostart("agent.desktop", false).unwrap();
        let override_text = std::fs::read_to_string(user.join("agent.desktop")).unwrap();
        assert!(override_text.contains("Hidden=true") && override_text.contains("Exec=agent"));
        assert!(!autostart(&user, std::slice::from_ref(&system))[0].enabled);
        service.set_autostart("agent.desktop", true).unwrap();
        assert!(
            !user.join("agent.desktop").exists(),
            "the override is removed"
        );
        service.set_autostart("chat.desktop", false).unwrap();
        let chat = autostart(&user, &[])
            .into_iter()
            .find(|e| e.id == "chat.desktop")
            .unwrap();
        assert!(!chat.enabled && chat.sandboxed);
        assert!(service.set_autostart("../x.desktop", false).is_err());

        assert_eq!(
            with_hidden(
                "[Desktop Entry]\nName=A\nHidden=false\n[Desktop Action x]\nName=B\n",
                true
            ),
            "[Desktop Entry]\nName=A\nHidden=true\n[Desktop Action x]\nName=B\n"
        );
        assert_eq!(
            with_hidden("[Desktop Entry]\nName=A\nHidden=true\n", false),
            "[Desktop Entry]\nName=A\n"
        );
    }

    #[tokio::test]
    async fn grants_are_listed_changed_and_revoked() {
        let bus = TestBus::new();
        let root = tempfile::tempdir().unwrap();
        let service =
            PrivacyService::with(bus.connect().await, root.path().join("autostart"), vec![]);
        // Without the store the page still lists autostart entries.
        assert!(service.snapshot().await.unwrap().grants.is_none());
        let store = FakePermissionStore::serve(&bus).await;
        store.grant("devices", "camera", "org.example.Chat", &["yes"]);
        store.grant(
            "location",
            "location",
            "org.example.Maps",
            &["EXACT", "1700000000"],
        );
        store.grant("screencast", "token-1", "org.example.Meet", &["yes"]);
        let state = service.snapshot().await.unwrap();
        let grants = state.grants.unwrap();
        assert_eq!(grants.len(), 3);
        let camera = grants
            .iter()
            .find(|grant| grant.access == Access::Camera)
            .unwrap()
            .clone();
        assert_eq!(camera.allowed(), Some(true));
        let location = grants
            .iter()
            .find(|grant| grant.access == Access::Location)
            .unwrap()
            .clone();
        assert_eq!(location.allowed(), Some(true));
        let sharing = grants
            .iter()
            .find(|grant| grant.access == Access::ScreenCast)
            .unwrap()
            .clone();

        let state = service
            .apply(PrivacyAction::Set(camera.clone(), false))
            .await
            .unwrap()
            .unwrap();
        let denied = state
            .grants
            .unwrap()
            .into_iter()
            .find(|grant| grant.access == Access::Camera)
            .unwrap();
        assert_eq!(denied.allowed(), Some(false));
        assert!(matches!(
            service
                .apply(PrivacyAction::Set(location.clone(), false))
                .await,
            Err(ServiceError::Failed(_))
        ));
        for grant in [camera, location, sharing] {
            service.apply(PrivacyAction::Revoke(grant)).await.unwrap();
        }
        assert!(service.snapshot().await.unwrap().grants.unwrap().is_empty());
        let mut updates = service.updates();
        store.grant("screenshot", "screenshot", "org.example.Shot", &["no"]);
        let state = tokio::time::timeout(Duration::from_secs(10), updates.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(state.grants.unwrap().len(), 1);
    }
}
