//! The Users page's model: accounts through AccountsService and polkit, and
//! changing one's own password with `passwd` (ADR 0143).
//!
//! Settings never sees or computes a password hash. Another person's
//! password is reset by asking for a new one at their next login
//! (AccountsService's password mode). One's own password goes to `passwd`,
//! which verifies the current one through PAM and applies the system's
//! password rules, exactly as in a terminal.
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_services::{
    ServiceState,
    accounts::{self, Accounts, PasswordMode},
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

fn not_running() -> String {
    fl!("settings-users-not-running")
}

/// The rules a picture follows, the same as the login screen's (2 MiB, at
/// most 2048 pixels a side, PNG or JPEG).
pub const MAX_PICTURE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_PICTURE_SIDE: u32 = 2048;

/// What the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsersState {
    pub accounts: Accounts,
    /// The person using Settings.
    pub me: u64,
}

impl UsersState {
    pub fn me(&self) -> Option<&accounts::User> {
        self.accounts.user(self.me)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsersAction {
    RealName(u64, String),
    Administrator(u64, bool),
    /// A checked image file, or empty to remove the picture.
    Picture(u64, String),
    /// Ask for a new password at the next login.
    PasswordAtLogin(u64),
    Create {
        name: String,
        full_name: String,
        administrator: bool,
    },
    Delete {
        uid: u64,
        remove_files: bool,
    },
}

impl LiveAction for UsersAction {
    fn control(&self) -> String {
        match self {
            Self::RealName(uid, _) => format!("name-{uid}"),
            Self::Administrator(uid, _) => format!("admin-{uid}"),
            Self::Picture(uid, _) => format!("picture-{uid}"),
            Self::PasswordAtLogin(uid) => format!("password-{uid}"),
            Self::Create { .. } => "create".into(),
            Self::Delete { uid, .. } => format!("delete-{uid}"),
        }
    }
}

fn failure(error: canopa_services::Error) -> ServiceError {
    use canopa_services::Error;
    match error {
        Error::Unavailable => ServiceError::Unavailable(not_running()),
        Error::NotAuthorized => ServiceError::Failed(fl!("settings-error-not-authorized")),
        Error::TimedOut => ServiceError::Failed(fl!("settings-users-timed-out")),
        Error::Changed => ServiceError::Failed(fl!("settings-users-changed")),
        Error::Invalid(what) => ServiceError::Failed(crate::service_text::invalid(what)),
        Error::Refused(message) => ServiceError::Failed(message),
    }
}

/// A user name for a full name, as people expect: `Ada Lovelace` → `ada`.
pub fn suggested_name(full_name: &str) -> String {
    let first = full_name.split_whitespace().next().unwrap_or("");
    let mut name: String = first
        .chars()
        .filter_map(|c| {
            let c = c.to_ascii_lowercase();
            (c.is_ascii_lowercase() || c.is_ascii_digit()).then_some(c)
        })
        .take(32)
        .collect();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, 'u');
        name.truncate(32);
    }
    name
}

/// Check a picture against [`MAX_PICTURE_BYTES`] and [`MAX_PICTURE_SIDE`]
/// from its header, without decoding it.
pub fn check_picture(path: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|_| fl!("settings-users-picture-unreadable"))?;
    if !meta.is_file() {
        return Err(fl!("settings-users-picture-not-file"));
    }
    if meta.len() > MAX_PICTURE_BYTES {
        return Err(fl!(
            "settings-users-picture-too-large",
            size = canopa_i18n::format::size(MAX_PICTURE_BYTES)
        ));
    }
    let mut header = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(64 * 1024).read_to_end(&mut header))
        .map_err(|_| fl!("settings-users-picture-unreadable"))?;
    let (width, height) =
        image_size(&header).ok_or_else(|| fl!("settings-users-picture-format"))?;
    if width == 0 || height == 0 || width > MAX_PICTURE_SIDE || height > MAX_PICTURE_SIDE {
        return Err(fl!(
            "settings-users-picture-too-big",
            pixels = MAX_PICTURE_SIDE.to_string()
        ));
    }
    Ok(())
}

/// A PNG's or JPEG's size from its first bytes.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.get(12..16) == Some(b"IHDR") {
        let width = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
        let height = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
        return Some((width, height));
    }
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xff {
            return None;
        }
        let marker = bytes[at + 1];
        if marker == 0xff {
            at += 1;
            continue;
        }
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        // Start of frame markers carry the size; DHT, JPG and DAC do not.
        if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            let height = u16::from_be_bytes([*bytes.get(at + 5)?, *bytes.get(at + 6)?]);
            let width = u16::from_be_bytes([*bytes.get(at + 7)?, *bytes.get(at + 8)?]);
            return Some((u32::from(width), u32::from(height)));
        }
        if length < 2 {
            return None;
        }
        at += 2 + length;
    }
    None
}

/// Overwrite a secret before it is dropped.
pub fn wipe(secret: &mut String) {
    // SAFETY: zero bytes are valid UTF-8.
    unsafe { secret.as_mut_vec().fill(0) };
    secret.clear();
}

/// Change one's own password with `passwd` at `program`: it asks PAM for
/// the current password, then the new one twice.
pub async fn change_own_password(
    program: &Path,
    mut current: String,
    mut new: String,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let mut input = format!("{current}\n{new}\n{new}\n");
    wipe(&mut current);
    wipe(&mut new);
    let result = async {
        let mut child = tokio::process::Command::new(program)
            .env("LC_ALL", "C")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| fl!("settings-users-passwd-missing"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| fl!("settings-users-passwd-failed"))?;
        // passwd may stop reading early; its answer explains why.
        let _ = stdin.write_all(input.as_bytes()).await;
        drop(stdin);
        let output = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())
            .await
            .map_err(|_| fl!("settings-users-passwd-timed-out"))?
            .map_err(|_| fl!("settings-users-passwd-failed"))?;
        if output.status.success() {
            return Ok(());
        }
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Err(password_refusal(&text))
    }
    .await;
    wipe(&mut input);
    result
}

/// `passwd`'s refusal in the page's words.
pub fn password_refusal(output: &str) -> String {
    let lower = output.to_lowercase();
    if let Some(line) = output.lines().find(|line| line.contains("BAD PASSWORD")) {
        let reason = line
            .split_once(':')
            .map_or(line, |(_, reason)| reason)
            .trim();
        let reason: String = reason
            .chars()
            .filter(|c| !c.is_control())
            .take(160)
            .collect();
        // passwd runs in the C locale: its reason is English.
        fl!("settings-users-password-weak", reason = reason)
    } else if lower.contains("do not match") || lower.contains("don't match") {
        fl!("settings-users-password-mismatch")
    } else if lower.contains("authentication token manipulation error")
        || lower.contains("authentication failure")
        || lower.contains("current password")
    {
        fl!("settings-users-password-wrong")
    } else if lower.contains("too soon") || lower.contains("wait longer") {
        fl!("settings-users-password-too-soon")
    } else {
        fl!("settings-users-password-failed")
    }
}

/// AccountsService for the page.
#[derive(Clone)]
pub struct UsersService {
    system: Arc<tokio::sync::OnceCell<zbus::Connection>>,
    me: u64,
}

impl Default for UsersService {
    fn default() -> Self {
        Self {
            system: Arc::default(),
            // SAFETY: getuid has no preconditions.
            me: u64::from(unsafe { libc::getuid() }),
        }
    }
}

impl UsersService {
    /// A service on an existing connection, as user `me` (tests).
    pub fn with(system: zbus::Connection, me: u64) -> Self {
        Self {
            system: Arc::new(tokio::sync::OnceCell::new_with(Some(system))),
            me,
        }
    }

    async fn connection(&self) -> Result<zbus::Connection, ServiceError> {
        self.system
            .get_or_try_init(canopa_services::bus::system)
            .await
            .cloned()
            .map_err(|_| ServiceError::Unavailable(fl!("settings-error-bus")))
    }

    async fn read(&self) -> Result<UsersState, ServiceError> {
        let system = self.connection().await?;
        let accounts = accounts::read_with(&system, Some(self.me))
            .await
            .map_err(failure)?;
        Ok(UsersState {
            accounts,
            me: self.me,
        })
    }

    async fn path(&self, uid: u64) -> Result<String, ServiceError> {
        let state = self.read().await?;
        state
            .accounts
            .user(uid)
            .map(|user| user.path.to_string())
            .ok_or_else(|| ServiceError::Failed(fl!("settings-users-gone")))
    }

    async fn run(&self, action: UsersAction) -> Result<(), ServiceError> {
        let system = self.connection().await?;
        let result = match action {
            UsersAction::RealName(uid, name) => {
                let name = name.trim();
                accounts::set_real_name(&system, &self.path(uid).await?, name).await
            }
            UsersAction::Administrator(uid, administrator) => {
                if uid == self.me && !administrator {
                    let others = self
                        .read()
                        .await?
                        .accounts
                        .users
                        .iter()
                        .any(|user| user.uid != uid && user.administrator);
                    if !others {
                        return Err(ServiceError::Failed(fl!(
                            "settings-users-last-administrator"
                        )));
                    }
                }
                accounts::set_administrator(&system, &self.path(uid).await?, administrator).await
            }
            UsersAction::Picture(uid, file) => {
                if !file.is_empty() {
                    check_picture(Path::new(&file)).map_err(ServiceError::Failed)?;
                }
                accounts::set_icon_file(&system, &self.path(uid).await?, &file).await
            }
            UsersAction::PasswordAtLogin(uid) => {
                if uid == self.me {
                    return Err(ServiceError::Failed(fl!("settings-users-own-password")));
                }
                accounts::set_password_mode(
                    &system,
                    &self.path(uid).await?,
                    PasswordMode::SetAtLogin,
                )
                .await
            }
            UsersAction::Create {
                name,
                full_name,
                administrator,
            } => {
                let path =
                    accounts::create_user(&system, name.trim(), full_name.trim(), administrator)
                        .await
                        .map_err(failure)?;
                // The person chooses a password when they first log in.
                accounts::set_password_mode(&system, path.as_str(), PasswordMode::SetAtLogin).await
            }
            UsersAction::Delete { uid, remove_files } => {
                if uid == self.me {
                    return Err(ServiceError::Failed(fl!("settings-users-remove-self")));
                }
                accounts::delete_user(&system, uid, remove_files).await
            }
        };
        result.map_err(failure)
    }
}

impl LiveService for UsersService {
    type State = UsersState;
    type Action = UsersAction;

    fn name(&self) -> String {
        "AccountsService".into()
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<UsersState, ServiceError>> {
        let service = self.clone();
        async move { service.read().await }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<UsersState, ServiceError>> {
        let service = self.clone();
        futures_util::stream::once(async move {
            let system = match service.connection().await {
                Ok(system) => system,
                Err(error) => return futures_util::stream::iter([Err(error)]).boxed(),
            };
            let me = service.me;
            accounts::watch(system, Some(me))
                .map(move |state| match state {
                    ServiceState::Available(accounts) => Ok(UsersState { accounts, me }),
                    ServiceState::Unavailable => Err(ServiceError::Unavailable(not_running())),
                })
                .boxed()
        })
        .flatten()
        .boxed()
    }

    fn apply(
        &self,
        action: UsersAction,
    ) -> BoxFuture<'static, Result<Option<UsersState>, ServiceError>> {
        let service = self.clone();
        async move {
            service.run(action).await?;
            Ok(service.read().await.ok())
        }
        .boxed()
    }
}

/// Where `passwd` is.
pub fn passwd() -> PathBuf {
    "passwd".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{TestBus, accounts::FakeAccounts};

    #[test]
    fn names_pictures_and_refusals() {
        assert_eq!(suggested_name("Ada Lovelace"), "ada");
        assert_eq!(suggested_name("Élodie Martin"), "lodie");
        assert_eq!(suggested_name("2pac"), "u2pac");
        assert_eq!(suggested_name(""), "");
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(256u32.to_be_bytes());
        png.extend(128u32.to_be_bytes());
        assert_eq!(image_size(&png), Some((256, 128)));
        // A JPEG with an APP0 segment, then a baseline frame of 640 × 480.
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x01,
            0xe0, 0x02, 0x80,
        ];
        assert_eq!(image_size(&jpeg), Some((640, 480)));
        assert_eq!(image_size(b"GIF89a"), None);
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("me.png");
        std::fs::write(&file, &png).unwrap();
        assert_eq!(check_picture(&file), Ok(()));
        let mut big = png.clone();
        big[16..20].copy_from_slice(&4096u32.to_be_bytes());
        std::fs::write(&file, &big).unwrap();
        assert!(check_picture(&file).unwrap_err().contains("2048"));
        assert!(check_picture(dir.path()).is_err());

        assert_eq!(
            password_refusal("BAD PASSWORD: The password is shorter than 8 characters\n"),
            "The new password is too weak: The password is shorter than 8 characters."
        );
        assert_eq!(
            password_refusal(
                "passwd: Authentication token manipulation error\npasswd: password unchanged"
            ),
            "The current password is not correct."
        );
        let mut secret = "hunter2".to_owned();
        wipe(&mut secret);
        assert!(secret.is_empty());
    }

    #[tokio::test]
    async fn own_passwords_go_to_passwd() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("passwd");
        let seen = dir.path().join("seen");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nread current; read new; read again\nprintf '%s|%s|%s' \"$current\" \"$new\" \"$again\" > {}\n[ \"$current\" = old ] || {{ echo 'passwd: Authentication token manipulation error' >&2; exit 10; }}\n",
                seen.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        change_own_password(&program, "old".into(), "n3w-secret".into())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&seen).unwrap(),
            "old|n3w-secret|n3w-secret"
        );
        assert_eq!(
            change_own_password(&program, "wrong".into(), "x".into()).await,
            Err("The current password is not correct.".into())
        );
        assert!(
            change_own_password(&dir.path().join("missing"), "a".into(), "b".into())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn accounts_are_added_changed_and_removed() {
        let bus = TestBus::new();
        let service = UsersService::with(bus.connect().await, 1000);
        assert!(matches!(
            service.snapshot().await,
            Err(ServiceError::Unavailable(_))
        ));
        let fake = FakeAccounts::serve(&bus).await;
        let state = service.snapshot().await.unwrap();
        assert_eq!(state.me().unwrap().name, "kevin");

        let state = service
            .apply(UsersAction::Create {
                name: "ada".into(),
                full_name: "Ada Lovelace".into(),
                administrator: false,
            })
            .await
            .unwrap()
            .unwrap();
        let ada = state
            .accounts
            .users
            .iter()
            .find(|user| user.name == "ada")
            .unwrap()
            .clone();
        assert_eq!(ada.password_mode, PasswordMode::SetAtLogin);
        // The last administrator stays one.
        assert!(matches!(
            service.apply(UsersAction::Administrator(1000, false)).await,
            Err(ServiceError::Failed(message)) if message.contains("Another account")
        ));
        service
            .apply(UsersAction::Administrator(ada.uid, true))
            .await
            .unwrap();
        service
            .apply(UsersAction::RealName(ada.uid, " Ada King ".into()))
            .await
            .unwrap();
        assert!(matches!(
            service.apply(UsersAction::PasswordAtLogin(1000)).await,
            Err(ServiceError::Failed(_))
        ));
        assert!(matches!(
            service
                .apply(UsersAction::Picture(ada.uid, "/nonexistent.png".into()))
                .await,
            Err(ServiceError::Failed(_))
        ));
        let state = service.snapshot().await.unwrap();
        let ada = state.accounts.user(ada.uid).unwrap();
        assert!(ada.administrator);
        assert_eq!(ada.real_name, "Ada King");
        assert!(matches!(
            service
                .apply(UsersAction::Delete {
                    uid: 1000,
                    remove_files: false
                })
                .await,
            Err(ServiceError::Failed(_))
        ));
        service
            .apply(UsersAction::Delete {
                uid: ada.uid,
                remove_files: false,
            })
            .await
            .unwrap();
        assert_eq!(fake.state.lock().unwrap().removed, vec![(ada.uid, false)]);
    }
}
