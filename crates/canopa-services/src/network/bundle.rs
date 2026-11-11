//! One live daemon bundle for status and Wi-Fi control, shared by host and provider.
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::watch;

pub const NM: &str = "org.freedesktop.NetworkManager";
pub const NETWORKD: &str = "org.freedesktop.network1";
pub const IWD: &str = "net.connman.iwd";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    NetworkManager,
    Networkd,
    Iwd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pub kind: Kind,
    pub unique: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bundle {
    pub status: Option<Owner>,
    pub control: Option<Owner>,
}

impl Bundle {
    pub async fn resolve(conn: &zbus::Connection) -> zbus::Result<Self> {
        let dbus = zbus::fdo::DBusProxy::new(conn).await?;
        async fn owner(
            dbus: &zbus::fdo::DBusProxy<'_>,
            name: &str,
            kind: Kind,
        ) -> zbus::Result<Option<Owner>> {
            match dbus.get_name_owner(name.try_into().unwrap()).await {
                Ok(owner) => Ok(Some(Owner {
                    kind,
                    unique: owner.to_string(),
                })),
                Err(zbus::fdo::Error::NameHasNoOwner(_)) => Ok(None),
                Err(error) => Err(error.into()),
            }
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            if let Some(nm) = owner(&dbus, NM, Kind::NetworkManager).await? {
                return Ok(Self {
                    status: Some(nm.clone()),
                    control: Some(nm),
                });
            }
            let status = owner(&dbus, NETWORKD, Kind::Networkd).await?;
            let control = owner(&dbus, IWD, Kind::Iwd).await?;
            Ok(Self { status, control })
        })
        .await
        .map_err(|_| zbus::Error::Failure("network ownership query timed out".into()))?
    }
}

struct Worker(tokio::task::JoinHandle<()>);
impl Drop for Worker {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Subscription is installed before discovery. Clones share an owned watcher;
/// dropping the last consumer ends it even while the bus is otherwise idle.
#[derive(Clone)]
pub struct Watch {
    rx: watch::Receiver<Bundle>,
    _worker: Arc<Worker>,
}
impl Watch {
    pub async fn new(conn: &zbus::Connection) -> zbus::Result<Self> {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender("org.freedesktop.DBus")?
            .interface("org.freedesktop.DBus")?
            .member("NameOwnerChanged")?
            .build();
        let mut changes = zbus::MessageStream::for_match_rule(rule, conn, Some(64)).await?;
        let (tx, rx) = watch::channel(Bundle::resolve(conn).await?);
        let conn = conn.clone();
        let worker = tokio::spawn(async move {
            while let Some(Ok(message)) = changes.next().await {
                let Ok((name, _, _)) = message.body().deserialize::<(String, String, String)>()
                else {
                    continue;
                };
                if ![NM, NETWORKD, IWD].contains(&name.as_str()) {
                    continue;
                }
                let bundle = Bundle::resolve(&conn).await.unwrap_or_default();
                tx.send_if_modified(|current| {
                    if *current == bundle {
                        false
                    } else {
                        *current = bundle;
                        true
                    }
                });
            }
            tx.send_replace(Bundle::default());
        });
        Ok(Self {
            rx,
            _worker: Arc::new(Worker(worker)),
        })
    }
    pub fn current(&self) -> Bundle {
        self.rx.borrow().clone()
    }
    pub async fn changed(&mut self) -> Result<(), watch::error::RecvError> {
        self.rx.changed().await
    }
}
