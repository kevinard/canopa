//! AccountsService: user accounts, their names, type, language, picture and
//! automatic login.
//!
//! AccountsService and polkit decide who may change what: a person changes
//! their own name, language and picture; adding, removing and changing other
//! accounts needs an administrator, so polkit may ask for a password. This
//! client never handles password hashes: [`set_password_mode`] asks for a new
//! password at the next login instead.
//!
//! ```no_run
//! use canopa_services::accounts;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let users = accounts::read(&system).await?;
//! for user in &users.users {
//!     println!("{} ({}) admin={}", user.real_name, user.name, user.administrator);
//! }
//! let path = accounts::create_user(&system, "ada", "Ada Lovelace", false).await?;
//! accounts::set_password_mode(&system, path.as_str(), accounts::PasswordMode::SetAtLogin).await?;
//! # Ok(()) }
//! ```
use crate::{
    Error,
    bus::{DEADLINE, StateStream, call_interactive},
};
use std::collections::HashMap;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

pub const SERVICE: &str = "org.freedesktop.Accounts";
pub const PATH: &str = "/org/freedesktop/Accounts";
pub const INTERFACE: &str = "org.freedesktop.Accounts";
pub const USER: &str = "org.freedesktop.Accounts.User";
/// At most this many accounts are listed.
pub const MAX_USERS: usize = 128;

/// How the account's password is set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PasswordMode {
    #[default]
    Regular,
    /// The person chooses a password when they next log in.
    SetAtLogin,
    /// No password.
    None,
}

impl PasswordMode {
    fn from_code(code: i32) -> Self {
        match code {
            1 => Self::SetAtLogin,
            2 => Self::None,
            _ => Self::Regular,
        }
    }

    fn code(self) -> i32 {
        match self {
            Self::Regular => 0,
            Self::SetAtLogin => 1,
            Self::None => 2,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct User {
    pub path: OwnedObjectPath,
    pub uid: u64,
    pub name: String,
    pub real_name: String,
    pub administrator: bool,
    pub home: String,
    /// The language AccountsService records, such as `fr_FR.UTF-8`.
    pub language: String,
    /// The formats locale, when this AccountsService supports one.
    pub formats_locale: Option<String>,
    /// The picture, an absolute path; empty for none.
    pub icon_file: String,
    pub locked: bool,
    pub automatic_login: bool,
    pub password_mode: PasswordMode,
    pub system: bool,
}

impl User {
    /// The full name, else the user name.
    pub fn display_name(&self) -> &str {
        if self.real_name.trim().is_empty() {
            &self.name
        } else {
            &self.real_name
        }
    }
}

/// The people's accounts (not system accounts), sorted by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accounts {
    pub users: Vec<User>,
}

impl Accounts {
    pub fn user(&self, uid: u64) -> Option<&User> {
        self.users.iter().find(|user| user.uid == uid)
    }
}

fn user_from(path: OwnedObjectPath, all: &HashMap<String, OwnedValue>) -> User {
    let text = |name: &str| {
        all.get(name)
            .and_then(|value| <&str>::try_from(value).ok())
            .map(|value| crate::bus::bounded_text(value, 256).replace('\n', " "))
            .unwrap_or_default()
    };
    let flag = |name: &str| all.get(name).and_then(|v| bool::try_from(v).ok()) == Some(true);
    let int = |name: &str| {
        all.get(name)
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or(0)
    };
    User {
        uid: all
            .get("Uid")
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(u64::MAX),
        name: text("UserName"),
        real_name: text("RealName"),
        administrator: int("AccountType") == 1,
        home: text("HomeDirectory"),
        language: text("Language"),
        formats_locale: all
            .contains_key("FormatsLocale")
            .then(|| text("FormatsLocale")),
        icon_file: text("IconFile"),
        locked: flag("Locked"),
        automatic_login: flag("AutomaticLogin"),
        password_mode: PasswordMode::from_code(int("PasswordMode")),
        system: flag("SystemAccount"),
        path,
    }
}

async fn read_user(connection: &zbus::Connection, path: OwnedObjectPath) -> Result<User, Error> {
    let all = crate::bus::properties(connection, SERVICE, path.as_str(), USER).await?;
    Ok(user_from(path, &all))
}

async fn manager(connection: &zbus::Connection) -> Result<zbus::Proxy<'static>, Error> {
    Ok(zbus::Proxy::new_owned(connection.clone(), SERVICE, PATH, INTERFACE).await?)
}

async fn user_proxy(
    connection: &zbus::Connection,
    path: &str,
) -> Result<zbus::Proxy<'static>, Error> {
    let path = OwnedObjectPath::try_from(path).map_err(|_| Error::Invalid("account path"))?;
    Ok(zbus::Proxy::new_owned(connection.clone(), SERVICE, path, USER).await?)
}

/// The account of `uid`, even when AccountsService does not list it.
pub async fn find_by_id(connection: &zbus::Connection, uid: u64) -> Result<User, Error> {
    let path: OwnedObjectPath = tokio::time::timeout(
        DEADLINE,
        manager(connection)
            .await?
            .call("FindUserById", &(uid as i64)),
    )
    .await??;
    read_user(connection, path).await
}

/// The people's accounts, with `include` (the current user) even when it is
/// not in AccountsService's list.
pub async fn read_with(
    connection: &zbus::Connection,
    include: Option<u64>,
) -> Result<Accounts, Error> {
    let paths: Vec<OwnedObjectPath> = tokio::time::timeout(
        DEADLINE,
        manager(connection).await?.call("ListCachedUsers", &()),
    )
    .await??;
    let mut users = Vec::new();
    for path in paths.into_iter().take(MAX_USERS) {
        let user = read_user(connection, path).await?;
        if !user.system && user.uid != 0 {
            users.push(user);
        }
    }
    if let Some(uid) = include
        && !users.iter().any(|user| user.uid == uid)
        && let Ok(user) = find_by_id(connection, uid).await
    {
        users.push(user);
    }
    users.sort_by(|a, b| {
        (a.display_name().to_lowercase(), a.uid).cmp(&(b.display_name().to_lowercase(), b.uid))
    });
    Ok(Accounts { users })
}

pub async fn read(connection: &zbus::Connection) -> Result<Accounts, Error> {
    read_with(connection, None).await
}

/// The accounts, re-read on each AccountsService signal (accounts added,
/// removed or changed).
pub fn watch(connection: zbus::Connection, include: Option<u64>) -> StateStream<Accounts> {
    crate::bus::watch_service(connection, SERVICE, move |connection| async move {
        read_with(&connection, include).await
    })
}

/// A user name `useradd` accepts: a lowercase letter or `_`, then lowercase
/// letters, digits, `_` and `-`, at most 32 characters.
pub fn valid_user_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'_'))
        && name.len() <= 32
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
}

fn valid_text(text: &str) -> bool {
    text.chars().count() <= 128 && !text.chars().any(|c| c.is_control() || c == ':')
}

/// Add an account. Its password is unset; see [`set_password_mode`].
pub async fn create_user(
    connection: &zbus::Connection,
    name: &str,
    full_name: &str,
    administrator: bool,
) -> Result<OwnedObjectPath, Error> {
    if !valid_user_name(name) {
        return Err(Error::Invalid("user name"));
    }
    if !valid_text(full_name) {
        return Err(Error::Invalid("full name"));
    }
    call_interactive(
        &manager(connection).await?,
        "CreateUser",
        &(name, full_name, i32::from(administrator)),
    )
    .await
}

/// Remove an account, and its home folder and mail when `remove_files`.
pub async fn delete_user(
    connection: &zbus::Connection,
    uid: u64,
    remove_files: bool,
) -> Result<(), Error> {
    call_interactive(
        &manager(connection).await?,
        "DeleteUser",
        &(uid as i64, remove_files),
    )
    .await
}

pub async fn set_real_name(
    connection: &zbus::Connection,
    user: &str,
    name: &str,
) -> Result<(), Error> {
    if !valid_text(name) {
        return Err(Error::Invalid("full name"));
    }
    call_interactive(
        &user_proxy(connection, user).await?,
        "SetRealName",
        &(name,),
    )
    .await
}

/// Make the account an administrator (AccountsService adds it to the
/// administrator group, `wheel` on Arch) or a standard account.
pub async fn set_administrator(
    connection: &zbus::Connection,
    user: &str,
    administrator: bool,
) -> Result<(), Error> {
    call_interactive(
        &user_proxy(connection, user).await?,
        "SetAccountType",
        &(i32::from(administrator),),
    )
    .await
}

pub async fn set_language(
    connection: &zbus::Connection,
    user: &str,
    language: &str,
) -> Result<(), Error> {
    if !crate::locale::valid_locale(language) && !language.is_empty() {
        return Err(Error::Invalid("language"));
    }
    call_interactive(
        &user_proxy(connection, user).await?,
        "SetLanguage",
        &(language,),
    )
    .await
}

/// Only where [`User::formats_locale`] is `Some`.
pub async fn set_formats_locale(
    connection: &zbus::Connection,
    user: &str,
    locale: &str,
) -> Result<(), Error> {
    if !crate::locale::valid_locale(locale) && !locale.is_empty() {
        return Err(Error::Invalid("formats"));
    }
    call_interactive(
        &user_proxy(connection, user).await?,
        "SetFormatsLocale",
        &(locale,),
    )
    .await
}

/// Set the picture from an image file (AccountsService copies it), or remove
/// it with an empty path.
pub async fn set_icon_file(
    connection: &zbus::Connection,
    user: &str,
    file: &str,
) -> Result<(), Error> {
    if !file.is_empty() && (!file.starts_with('/') || file.len() > 4096 || file.contains('\0')) {
        return Err(Error::Invalid("picture"));
    }
    call_interactive(
        &user_proxy(connection, user).await?,
        "SetIconFile",
        &(file,),
    )
    .await
}

pub async fn set_automatic_login(
    connection: &zbus::Connection,
    user: &str,
    enabled: bool,
) -> Result<(), Error> {
    call_interactive(
        &user_proxy(connection, user).await?,
        "SetAutomaticLogin",
        &(enabled,),
    )
    .await
}

pub async fn set_password_mode(
    connection: &zbus::Connection,
    user: &str,
    mode: PasswordMode,
) -> Result<(), Error> {
    call_interactive(
        &user_proxy(connection, user).await?,
        "SetPasswordMode",
        &(mode.code(),),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ServiceState,
        testing::{TestBus, accounts::FakeAccounts},
    };
    use futures_util::StreamExt;
    use std::time::Duration;

    #[test]
    fn user_names_follow_useradd() {
        assert!(valid_user_name("ada") && valid_user_name("_x-1"));
        assert!(!valid_user_name("Ada") && !valid_user_name("1ada") && !valid_user_name(""));
        assert!(!valid_user_name(&"a".repeat(33)));
    }

    #[tokio::test]
    async fn lists_creates_changes_and_removes_accounts() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert_eq!(read(&client).await, Err(Error::Unavailable));
        let fake = FakeAccounts::serve(&bus).await;
        let accounts = read(&client).await.unwrap();
        assert_eq!(accounts.users.len(), 1, "system accounts are left out");
        let kevin = accounts.user(1000).unwrap().clone();
        assert_eq!(kevin.display_name(), "Kevin");
        assert!(kevin.administrator);
        assert_eq!(kevin.formats_locale.as_deref(), Some(""));
        let mut states = watch(client.clone(), Some(1000));
        let next = async |states: &mut StateStream<Accounts>| loop {
            if let ServiceState::Available(accounts) =
                tokio::time::timeout(Duration::from_secs(3), states.next())
                    .await
                    .unwrap()
                    .unwrap()
            {
                return accounts;
            }
        };
        next(&mut states).await;

        let path = create_user(&client, "ada", "Ada Lovelace", false)
            .await
            .unwrap();
        set_password_mode(&client, path.as_str(), PasswordMode::SetAtLogin)
            .await
            .unwrap();
        let accounts = loop {
            let accounts = next(&mut states).await;
            if accounts.users.len() == 2
                && accounts.users[0].password_mode == PasswordMode::SetAtLogin
            {
                break accounts;
            }
        };
        assert_eq!(accounts.users[0].name, "ada");
        assert!(!accounts.users[0].administrator);
        set_administrator(&client, path.as_str(), true)
            .await
            .unwrap();
        set_real_name(&client, path.as_str(), "Ada King")
            .await
            .unwrap();
        set_language(&client, path.as_str(), "fr_FR.UTF-8")
            .await
            .unwrap();
        set_formats_locale(&client, path.as_str(), "en_GB.UTF-8")
            .await
            .unwrap();
        set_icon_file(&client, path.as_str(), "/tmp/ada.png")
            .await
            .unwrap();
        set_automatic_login(&client, path.as_str(), true)
            .await
            .unwrap();
        let ada = find_by_id(&client, 1001).await.unwrap();
        assert!(ada.administrator && ada.automatic_login);
        assert_eq!(
            (
                ada.real_name.as_str(),
                ada.language.as_str(),
                ada.formats_locale.as_deref(),
                ada.icon_file.as_str()
            ),
            (
                "Ada King",
                "fr_FR.UTF-8",
                Some("en_GB.UTF-8"),
                "/tmp/ada.png"
            )
        );
        assert_eq!(
            create_user(&client, "Bad", "", false).await,
            Err(Error::Invalid("user name"))
        );
        assert!(matches!(
            create_user(&client, "ada", "", false).await,
            Err(Error::Refused(_))
        ));
        delete_user(&client, 1001, true).await.unwrap();
        assert_eq!(fake.state.lock().unwrap().removed, vec![(1001, true)]);
        assert_eq!(read(&client).await.unwrap().users.len(), 1);
    }
}
