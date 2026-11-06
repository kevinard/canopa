//! Durable runtime privacy switches, independent of shared appearance configuration.
mod dbus;
mod store;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use tokio::sync::{mpsc as events, oneshot, watch};
use zbus::{Connection, fdo};

pub const INTERFACE: &str = "org.freedesktop.impl.portal.Lockdown";
const CAPACITY: usize = 16;
type Values = [bool; 7];
type Reply = oneshot::Sender<Result<(), Error>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Property {
    Printing,
    SaveToDisk,
    ApplicationHandlers,
    Location,
    Camera,
    Microphone,
    SoundOutput,
}

/// The revision remembers a denial even when a watch receiver coalesces a rapid
/// disable/enable pair. An earlier grant cannot silently become valid again.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    values: Values,
    revisions: [Option<u64>; 7],
}
impl Snapshot {
    fn initial(values: Values) -> Self {
        Self {
            values,
            revisions: [Some(0); 7],
        }
    }
    fn changed(self, after: Values) -> Self {
        let mut revisions = self.revisions;
        for property in Property::ALL {
            let index = property as usize;
            if !self.values[index] && after[index] {
                revisions[index] = revisions[index].and_then(|value| value.checked_add(1));
            }
        }
        Self {
            values: after,
            revisions,
        }
    }
    pub fn disabled(&self, property: Property) -> bool {
        self.values[property as usize] || self.revisions[property as usize].is_none()
    }
    pub fn permits(&self, earlier: &Self, property: Property) -> bool {
        !self.disabled(property)
            && !earlier.disabled(property)
            && self.revisions[property as usize] == earlier.revisions[property as usize]
    }
}

impl Property {
    const ALL: [Self; 7] = [
        Self::Printing,
        Self::SaveToDisk,
        Self::ApplicationHandlers,
        Self::Location,
        Self::Camera,
        Self::Microphone,
        Self::SoundOutput,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Printing => "disable-printing",
            Self::SaveToDisk => "disable-save-to-disk",
            Self::ApplicationHandlers => "disable-application-handlers",
            Self::Location => "disable-location",
            Self::Camera => "disable-camera",
            Self::Microphone => "disable-microphone",
            Self::SoundOutput => "disable-sound-output",
        }
    }
    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == name)
    }
}

/// Errors deliberately contain no database, SQL or policy contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Location,
    Source,
    Changed,
    ReadOnly,
    Busy,
    Capacity,
    Stopped,
    Signal,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Location => "policy source requires an absolute state home or HOME",
            Self::Source => "policy source is invalid or unavailable",
            Self::Changed => "policy source was replaced or removed; reload required",
            Self::ReadOnly => "policy source is read-only",
            Self::Busy => "policy source is busy",
            Self::Capacity => "too many pending policy operations",
            Self::Stopped => "policy module is unavailable",
            Self::Signal => "policy committed but change publication failed",
        })
    }
}
impl std::error::Error for Error {}
impl From<Error> for fdo::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::ReadOnly => Self::AccessDenied(error.to_string()),
            Error::Capacity => Self::LimitsExceeded(error.to_string()),
            _ => Self::Failed(error.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Source {
    path: PathBuf,
    read_only: bool,
}
impl Source {
    pub fn file(path: PathBuf, read_only: bool) -> Result<Self, Error> {
        Ok(Self {
            path: std::path::absolute(path).map_err(|_| Error::Location)?,
            read_only,
        })
    }
    pub fn discover(read_only: bool) -> Result<Self, Error> {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|path| path.join(".local/state"))
            })
            .ok_or(Error::Location)?;
        Self::file(base.join("canopa/portal/lockdown.sqlite3"), read_only)
    }
}

enum Operation {
    Set(Property, bool),
    Reload,
    Stop,
}
struct Command {
    operation: Operation,
    reply: Reply,
}
struct Admission {
    sender: mpsc::SyncSender<Command>,
    closed: bool,
}
struct Change {
    before: Values,
    after: Values,
    acknowledged: mpsc::Sender<Result<(), Error>>,
}
struct Inner {
    admission: Mutex<Admission>,
    values: watch::Receiver<Snapshot>,
    changes: Mutex<Option<events::Receiver<Change>>>,
}

/// Open before registration, run exactly one relay after registration, then
/// reload or mutate through D-Bus. Shutdown drains admitted work before the relay stops.
#[derive(Clone)]
pub struct Lockdown(Arc<Inner>);
impl Lockdown {
    pub async fn open(source: Source) -> Result<Self, Error> {
        let (sender, commands) = mpsc::sync_channel(CAPACITY);
        let (values, observed) = watch::channel(Snapshot::initial([false; 7]));
        let (changes, changed) = events::channel(1);
        let (ready, readiness) = oneshot::channel();
        std::thread::Builder::new()
            .name("portal-policy".into())
            .spawn(move || {
                let mut store = match store::Store::open(source) {
                    Ok(store) => store,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                values.send_replace(Snapshot::initial(store.values()));
                if ready.send(Ok(())).is_err() {
                    return;
                }
                while let Ok(Command { operation, reply }) = commands.recv() {
                    let outcome = match operation {
                        Operation::Stop => {
                            // Runtime permits must observe source retirement
                            // before the shutdown acknowledgement can complete.
                            drop(values);
                            let _ = reply.send(Ok(()));
                            break;
                        }
                        Operation::Reload => store.reload(),
                        Operation::Set(property, value) => store.set(property, value),
                    };
                    let result = match outcome {
                        Err(error) => Err(error),
                        Ok(after) => {
                            let before = *values.borrow();
                            if before.values == after {
                                Ok(())
                            } else {
                                // This work and its publication outlive the initiating future.
                                values.send_replace(before.changed(after));
                                let (acknowledged, acknowledgment) = mpsc::channel();
                                if changes
                                    .blocking_send(Change {
                                        before: before.values,
                                        after,
                                        acknowledged,
                                    })
                                    .is_err()
                                {
                                    let _ = reply.send(Err(Error::Stopped));
                                    break;
                                }
                                acknowledgment.recv().unwrap_or(Err(Error::Stopped))
                            }
                        }
                    };
                    let _ = reply.send(result);
                }
            })
            .map_err(|_| Error::Stopped)?;
        tokio::time::timeout(Duration::from_secs(5), readiness)
            .await
            .map_err(|_| Error::Busy)?
            .map_err(|_| Error::Stopped)??;
        Ok(Self(Arc::new(Inner {
            admission: Mutex::new(Admission {
                sender,
                closed: false,
            }),
            values: observed,
            changes: Mutex::new(Some(changed)),
        })))
    }

    fn values(&self) -> Values {
        self.0.values.borrow().values
    }

    /// Runtime consumers enforce policy using the same committed source as the
    /// exported properties. A closed source retires pending work.
    pub fn observe(&self) -> watch::Receiver<Snapshot> {
        self.0.values.clone()
    }

    async fn request(&self, operation: Operation) -> Result<(), Error> {
        let (reply, result) = oneshot::channel();
        {
            let admission = self.0.admission.lock().map_err(|_| Error::Stopped)?;
            if admission.closed {
                return Err(Error::Stopped);
            }
            admission
                .sender
                .try_send(Command { operation, reply })
                .map_err(|error| match error {
                    mpsc::TrySendError::Full(_) => Error::Capacity,
                    mpsc::TrySendError::Disconnected(_) => Error::Stopped,
                })?;
        }
        result.await.map_err(|_| Error::Stopped)?
    }

    pub async fn reload(&self) -> Result<(), Error> {
        self.request(Operation::Reload).await
    }

    pub async fn relay(&self, connection: Connection) -> Result<(), Error> {
        let mut changes = self
            .0
            .changes
            .lock()
            .map_err(|_| Error::Stopped)?
            .take()
            .ok_or(Error::Stopped)?;
        while let Some(change) = changes.recv().await {
            let result = dbus::publish(&connection, change.before, change.after)
                .await
                .map_err(|_| Error::Signal);
            let _ = change.acknowledged.send(result);
            result?;
        }
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), Error> {
        let sender = {
            let mut admission = self.0.admission.lock().map_err(|_| Error::Stopped)?;
            if admission.closed {
                return Err(Error::Stopped);
            }
            admission.closed = true;
            admission.sender.clone()
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            let (reply, result) = oneshot::channel();
            let mut command = Command {
                operation: Operation::Stop,
                reply,
            };
            loop {
                match sender.try_send(command) {
                    Ok(()) => break,
                    Err(mpsc::TrySendError::Disconnected(_)) => return Err(Error::Stopped),
                    Err(mpsc::TrySendError::Full(pending)) => {
                        command = pending;
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                }
            }
            result.await.map_err(|_| Error::Stopped)?
        })
        .await
        .map_err(|_| Error::Busy)?
    }
}

#[cfg(test)]
mod tests;
