//! A fake AccountsService with an administrator `kevin` (uid 1000) and a
//! system account. New accounts get uids from 1001.
use super::TestBus;
use crate::accounts::{PATH, SERVICE};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use zbus::{
    ObjectServer, fdo,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedObjectPath},
};

#[derive(Clone, Debug, Default)]
pub struct UserData {
    pub name: String,
    pub real_name: String,
    pub account_type: i32,
    pub language: String,
    pub formats: String,
    pub icon: String,
    pub automatic_login: bool,
    pub password_mode: i32,
    pub system: bool,
}

/// What clients did.
#[derive(Default)]
pub struct AccountsState {
    pub users: BTreeMap<u64, UserData>,
    /// Removed uids and whether their files went too.
    pub removed: Vec<(u64, bool)>,
}

type Shared = Arc<Mutex<AccountsState>>;

fn path(uid: u64) -> OwnedObjectPath {
    OwnedObjectPath::try_from(format!("/org/freedesktop/Accounts/User{uid}")).unwrap()
}

struct Accounts(Shared);
struct User(Shared, u64);

#[zbus::interface(name = "org.freedesktop.Accounts")]
impl Accounts {
    fn list_cached_users(&self) -> Vec<OwnedObjectPath> {
        self.0
            .lock()
            .unwrap()
            .users
            .keys()
            .map(|uid| path(*uid))
            .collect()
    }
    fn find_user_by_id(&self, uid: i64) -> fdo::Result<OwnedObjectPath> {
        let uid = uid as u64;
        if self.0.lock().unwrap().users.contains_key(&uid) {
            Ok(path(uid))
        } else {
            Err(fdo::Error::Failed("Failed to look up user".into()))
        }
    }
    async fn create_user(
        &self,
        name: &str,
        real_name: &str,
        account_type: i32,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<OwnedObjectPath> {
        let uid = {
            let mut state = self.0.lock().unwrap();
            if state.users.values().any(|user| user.name == name) {
                return Err(fdo::Error::Failed(format!(
                    "A user with name '{name}' already exists"
                )));
            }
            let uid = state
                .users
                .keys()
                .copied()
                .filter(|uid| *uid >= 1000)
                .max()
                .unwrap_or(999)
                + 1;
            state.users.insert(
                uid,
                UserData {
                    name: name.into(),
                    real_name: real_name.into(),
                    account_type,
                    formats: String::new(),
                    ..Default::default()
                },
            );
            uid
        };
        server.at(path(uid), User(self.0.clone(), uid)).await?;
        Self::user_added(&emitter, path(uid).as_ref()).await?;
        Ok(path(uid))
    }
    async fn delete_user(
        &self,
        uid: i64,
        remove_files: bool,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let uid = uid as u64;
        {
            let mut state = self.0.lock().unwrap();
            if state.users.remove(&uid).is_none() {
                return Err(fdo::Error::Failed("No user with uid found".into()));
            }
            state.removed.push((uid, remove_files));
        }
        server.remove::<User, _>(path(uid)).await?;
        Self::user_deleted(&emitter, path(uid).as_ref()).await?;
        Ok(())
    }
    #[zbus(signal)]
    async fn user_added(emitter: &SignalEmitter<'_>, user: ObjectPath<'_>) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn user_deleted(emitter: &SignalEmitter<'_>, user: ObjectPath<'_>) -> zbus::Result<()>;
}

impl User {
    fn with<T>(&self, read: impl FnOnce(&UserData) -> T) -> T {
        read(
            self.0
                .lock()
                .unwrap()
                .users
                .get(&self.1)
                .expect("a known user"),
        )
    }
    async fn change(
        &self,
        emitter: &SignalEmitter<'_>,
        write: impl FnOnce(&mut UserData),
    ) -> fdo::Result<()> {
        if let Some(user) = self.0.lock().unwrap().users.get_mut(&self.1) {
            write(user);
        }
        Self::changed(emitter).await?;
        Ok(())
    }
}

#[zbus::interface(name = "org.freedesktop.Accounts.User")]
impl User {
    async fn set_real_name(
        &self,
        name: String,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.change(&e, |user| user.real_name = name).await
    }
    async fn set_account_type(
        &self,
        kind: i32,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.change(&e, |user| user.account_type = kind).await
    }
    async fn set_language(
        &self,
        language: String,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.change(&e, |user| user.language = language).await
    }
    async fn set_formats_locale(
        &self,
        formats: String,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.change(&e, |user| user.formats = formats).await
    }
    async fn set_icon_file(
        &self,
        icon: String,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.change(&e, |user| user.icon = icon).await
    }
    async fn set_automatic_login(
        &self,
        enabled: bool,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.change(&e, |user| user.automatic_login = enabled).await
    }
    async fn set_password_mode(
        &self,
        mode: i32,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.change(&e, |user| user.password_mode = mode).await
    }
    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
    #[zbus(property)]
    fn uid(&self) -> u64 {
        self.1
    }
    #[zbus(property)]
    fn user_name(&self) -> String {
        self.with(|user| user.name.clone())
    }
    #[zbus(property)]
    fn real_name(&self) -> String {
        self.with(|user| user.real_name.clone())
    }
    #[zbus(property)]
    fn account_type(&self) -> i32 {
        self.with(|user| user.account_type)
    }
    #[zbus(property)]
    fn home_directory(&self) -> String {
        self.with(|user| format!("/home/{}", user.name))
    }
    #[zbus(property)]
    fn language(&self) -> String {
        self.with(|user| user.language.clone())
    }
    #[zbus(property)]
    fn formats_locale(&self) -> String {
        self.with(|user| user.formats.clone())
    }
    #[zbus(property)]
    fn icon_file(&self) -> String {
        self.with(|user| user.icon.clone())
    }
    #[zbus(property)]
    fn locked(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn automatic_login(&self) -> bool {
        self.with(|user| user.automatic_login)
    }
    #[zbus(property)]
    fn password_mode(&self) -> i32 {
        self.with(|user| user.password_mode)
    }
    #[zbus(property)]
    fn system_account(&self) -> bool {
        self.with(|user| user.system)
    }
}

/// AccountsService on a private bus.
pub struct FakeAccounts {
    pub connection: zbus::Connection,
    pub state: Shared,
}

impl FakeAccounts {
    pub async fn serve(bus: &TestBus) -> Self {
        let mut users = BTreeMap::new();
        users.insert(
            1000,
            UserData {
                name: "kevin".into(),
                real_name: "Kevin".into(),
                account_type: 1,
                ..Default::default()
            },
        );
        users.insert(
            970,
            UserData {
                name: "colord".into(),
                system: true,
                ..Default::default()
            },
        );
        let state: Shared = Arc::new(Mutex::new(AccountsState {
            users,
            removed: Vec::new(),
        }));
        let mut builder = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(PATH, Accounts(state.clone()))
            .unwrap();
        for uid in [970, 1000] {
            builder = builder
                .serve_at(path(uid), User(state.clone(), uid))
                .unwrap();
        }
        let connection = builder.name(SERVICE).unwrap().build().await.unwrap();
        Self { connection, state }
    }
}
