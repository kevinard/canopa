//! Event-driven user discovery and AccountsService reader. NSS runs in an isolated helper.
//! Authentication stays in greetd.
use crate::config::valid_user;
use anyhow::{Context, Result, ensure};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use zbus::{Connection, Proxy, proxy::CacheProperties, zvariant::OwnedObjectPath};

const SERVICE: &str = "org.freedesktop.Accounts";
const ROOT: &str = "/org/freedesktop/Accounts";
const MAX_USERS: usize = 128;
const DEADLINE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum Source {
    #[default]
    Auto,
    Accountsservice,
    Nss,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Account {
    pub username: String,
    pub display_name: String,
    pub uid: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_file: Option<std::path::PathBuf>,
}
impl Account {
    pub fn manual(username: &str) -> Self {
        Self {
            username: username.into(),
            display_name: username.into(),
            uid: None,
            icon_file: None,
        }
    }
    pub(crate) fn discovered(
        username: String,
        real_name: String,
        uid: u64,
        system: bool,
    ) -> Option<Self> {
        if system || uid == 0 || uid >= u32::MAX as u64 || !valid_user(&username) {
            return None;
        }
        let real_name = real_name.trim();
        let display_name = if real_name.is_empty()
            || real_name.len() > 128
            || real_name.chars().any(char::is_control)
        {
            username.clone()
        } else {
            real_name.into()
        };
        Some(Self {
            username,
            display_name,
            uid: Some(uid),
            icon_file: None,
        })
    }
    pub fn chooser_label(&self) -> String {
        if self.display_name == self.username || self.username.chars().count() > 24 {
            return self.username.clone();
        }
        let budget = 37usize
            .saturating_sub(self.username.chars().count())
            .min(16);
        let mut name = self.display_name.chars().take(budget).collect::<String>();
        if name != self.display_name {
            name.push('…');
        }
        format!("{name} ({})", self.username)
    }
}

pub fn merge(configured: &[String], mut detected: Vec<Account>) -> Vec<Account> {
    detected.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
            .then(a.username.cmp(&b.username))
    });
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for name in configured {
        if valid_user(name) && seen.insert(name.clone()) {
            result.push(
                detected
                    .iter()
                    .find(|a| &a.username == name)
                    .cloned()
                    .unwrap_or_else(|| Account::manual(name)),
            );
        }
    }
    result.extend(
        detected
            .into_iter()
            .filter(|a| seen.insert(a.username.clone()))
            .take(MAX_USERS.saturating_sub(result.len())),
    );
    result
}

async fn proxy<'a>(
    connection: &'a Connection,
    destination: &'a str,
    path: &'a str,
    interface: &'a str,
) -> zbus::Result<Proxy<'a>> {
    zbus::proxy::Builder::new(connection)
        .destination(destination)?
        .path(path)?
        .interface(interface)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}
async fn user(
    connection: Connection,
    owner: String,
    path: OwnedObjectPath,
    avatars: bool,
) -> Result<Option<Account>> {
    let proxy = proxy(
        &connection,
        &owner,
        path.as_str(),
        "org.freedesktop.Accounts.User",
    )
    .await?;
    // Individual getters deliberately avoid GetAll (password hints, home paths, email, etc.).
    let (uid, username, system): (u64, String, bool) = tokio::try_join!(
        proxy.get_property("Uid"),
        proxy.get_property("UserName"),
        proxy.get_property("SystemAccount")
    )?;
    if system || uid == 0 {
        return Ok(None);
    }
    let real_name: String = proxy.get_property("RealName").await.unwrap_or_default();
    let mut account = Account::discovered(username, real_name, uid, system);
    if avatars && let Some(account) = &mut account {
        let icon: String = proxy.get_property("IconFile").await.unwrap_or_default();
        let path = std::path::PathBuf::from(icon);
        if crate::avatars::valid_path(&path) {
            account.icon_file = Some(path);
        }
    }
    Ok(account)
}
async fn read(connection: &Connection, avatars: bool) -> Result<Vec<Account>> {
    let manager = proxy(connection, SERVICE, ROOT, SERVICE).await?;
    let reply = manager.call_method("ListCachedUsers", &()).await?;
    let owner = reply
        .header()
        .sender()
        .context("AccountsService reply has no sender")?
        .to_string();
    let paths: Vec<OwnedObjectPath> = reply.body().deserialize()?;
    ensure!(
        paths.len() <= MAX_USERS,
        "AccountsService user list exceeds budget"
    );
    let mut unique = HashSet::new();
    let mut paths = paths.into_iter().filter(|p| {
        p.as_str().starts_with("/org/freedesktop/Accounts/User") && unique.insert(p.clone())
    });
    let mut tasks = tokio::task::JoinSet::new();
    let mut result = Vec::new();
    let mut uids = HashSet::new();
    loop {
        while tasks.len() < 8 {
            let Some(path) = paths.next() else {
                break;
            };
            tasks.spawn(user(connection.clone(), owner.clone(), path, avatars));
        }
        let Some(value) = tasks.join_next().await else {
            break;
        };
        if let Ok(Ok(Some(account))) = value
            && uids.insert(account.uid)
        {
            result.push(account);
        }
    }
    Ok(merge(&[], result))
}
pub async fn discover(avatars: bool) -> Result<Vec<Account>> {
    tokio::time::timeout(DEADLINE, async {
        read(&Connection::system().await?, avatars).await
    })
    .await
    .context("AccountsService discovery timed out")?
}
async fn selected(
    source: Source,
    policy: &crate::nss::Policy,
    cancelled: &AtomicBool,
    avatars: bool,
) -> Result<Vec<Account>> {
    match source {
        Source::Accountsservice => discover(avatars).await,
        Source::Nss => crate::nss::discover(policy, cancelled),
        Source::Auto => match discover(avatars).await {
            Ok(accounts) => Ok(accounts),
            Err(_) => crate::nss::discover(policy, cancelled),
        },
    }
}
pub fn list(
    configured: &[String],
    enabled: bool,
    source: Source,
    policy: &crate::nss::Policy,
    avatars: bool,
) -> Vec<Account> {
    let detected = if enabled {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(anyhow::Error::from)
            .and_then(|rt| rt.block_on(selected(source, policy, &AtomicBool::new(false), avatars)))
            .unwrap_or_else(|_| {
                eprintln!(
                    "greeter: user discovery unavailable; manual username entry remains available"
                );
                Vec::new()
            })
    } else {
        Vec::new()
    };
    merge(configured, detected)
}

pub struct Worker {
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    cancelled: Arc<AtomicBool>,
    refresh: tokio::sync::watch::Sender<u64>,
}
impl Worker {
    pub fn start(
        source: Source,
        policy: crate::nss::Policy,
        avatars: bool,
        mut notify: impl FnMut(Vec<Account>) + Send + 'static,
    ) -> Self {
        let (cancel, mut stopped) = tokio::sync::oneshot::channel();
        let (refresh, mut requested) = tokio::sync::watch::channel(0u64);
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let thread = std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(async {
                use futures_util::StreamExt;
                use canopa_file::watch::{Watcher,Target};
                let mut files=Watcher::new(vec![Target::File("/etc/passwd".into()),Target::File("/etc/nsswitch.conf".into()),Target::File("/etc/login.defs".into())])
                    .ok().and_then(|watcher|{
                        // SAFETY: Watcher owns its inotify descriptor and never replaces it.
                        unsafe{tokio::io::unix::AsyncFd::register(watcher)}.ok()
                    });
                let mut last=None;
                loop {
                    let connection=if matches!(source,Source::Nss) {None}else {
                        tokio::time::timeout(DEADLINE,Connection::system()).await.ok().and_then(Result::ok)
                    };
                    let mut signals=None;let mut owners=None;
                    if let Some(conn)=&connection {
                        let changes=zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender(SERVICE).unwrap().path_namespace(ROOT).unwrap().build();
                        signals=zbus::MessageStream::for_match_rule(changes,conn,Some(64)).await.ok();
                        let owner=zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender("org.freedesktop.DBus").unwrap().interface("org.freedesktop.DBus").unwrap().member("NameOwnerChanged").unwrap().arg(0,SERVICE).unwrap().build();
                        owners=zbus::MessageStream::for_match_rule(owner,conn,Some(8)).await.ok();
                    }
                    let mut reconnect=connection.is_none() && !matches!(source,Source::Nss);
                    loop {
                        let result=tokio::select! {biased;
                            _=&mut stopped=>return,
                            result=selected(source,&policy,&flag,avatars)=>result,
                        };
                        if !flag.load(Ordering::Acquire) && let Ok(accounts)=result && last.as_ref()!=Some(&accounts) {
                            last=Some(accounts.clone());notify(accounts);
                        }
                        let broken=tokio::select! {biased;
                            _=&mut stopped=>return,
                            result=requested.changed()=>{if result.is_err(){return;}false},
                            result=async{match &mut files {Some(f)=>f.readable_mut().await, None=>std::future::pending().await}}=>{
                                match result {
                                    Ok(mut ready)=>{
                                        let changed=ready.get_inner_mut().changed().unwrap_or(false);
                                        if ready.get_inner().drained() {ready.clear_ready();}
                                        if !changed {continue;}
                                    }
                                    Err(_)=>{files=None;}
                                }
                                false
                            },
                            message=async{match &mut signals {Some(s)=>s.next().await,None=>std::future::pending().await}}=>{let broken=message.is_none();if broken {signals=None;}broken},
                            message=async{match &mut owners {Some(s)=>s.next().await,None=>std::future::pending().await}}=>{let broken=message.is_none();if broken {owners=None;}broken},
                            _=async{if reconnect {tokio::time::sleep(Duration::from_secs(5)).await;}else{std::future::pending().await}}=>true,
                        };
                        if broken {break;}
                        reconnect=connection.is_none() && !matches!(source,Source::Nss);
                    }
                }
            });
        });
        Self {
            cancel: Some(cancel),
            thread: Some(thread),
            cancelled,
            refresh,
        }
    }
    pub fn refresh(&self) {
        self.refresh
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_uses_accountsservice_system_classification_without_a_local_uid_threshold() {
        assert!(Account::discovered("root".into(), "Root".into(), 0, false).is_none());
        assert!(Account::discovered("daemon".into(), "Daemon".into(), 42, true).is_none());
        assert!(Account::discovered("bad user".into(), "Bad".into(), 1000, false).is_none());
        let user =
            Account::discovered("enterprise".into(), "Enterprise User".into(), 500, false).unwrap();
        assert_eq!(user.uid, Some(500));
        assert_eq!(
            Account::discovered("alice".into(), "Bad\nName".into(), 1000, false)
                .unwrap()
                .display_name,
            "alice"
        );
        assert_eq!(
            Account::discovered("bob".into(), "".into(), 1001, false)
                .unwrap()
                .display_name,
            "bob"
        );
    }
    #[test]
    fn merge_keeps_manual_order_deduplicates_and_bounds_the_chooser() {
        let alice =
            Account::discovered("alice".into(), "Alice Example".into(), 1000, false).unwrap();
        let bob = Account::discovered("bob".into(), "Bob Builder".into(), 1001, false).unwrap();
        let users = merge(
            &["bob".into(), "manual".into()],
            vec![alice.clone(), bob, alice],
        );
        assert_eq!(
            users
                .iter()
                .map(|a| a.username.as_str())
                .collect::<Vec<_>>(),
            ["bob", "manual", "alice"]
        );
        assert_eq!(users[0].display_name, "Bob Builder");
        assert_eq!(users[1].uid, None);
        assert_eq!(
            merge(
                &[],
                (1..300)
                    .map(|n| Account::manual(&format!("user{n}")))
                    .collect()
            )
            .len(),
            MAX_USERS
        );
    }
}
