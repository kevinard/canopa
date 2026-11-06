//! Private, bounded selection history. One worker serializes load, record and clear.
use iced::futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};
const LIMIT: usize = 256;
const BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    key: String,
    count: u32,
    last: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    version: u8,
    serial: u64,
    entries: Vec<Entry>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            version: 1,
            serial: 0,
            entries: vec![],
        }
    }
}
fn valid_key(key: &str) -> bool {
    key.len() <= 4096
        && ["app:", "file:", "window-app:"]
            .iter()
            .any(|prefix| key.starts_with(prefix))
}
impl Snapshot {
    fn record(&mut self, key: String) {
        if !valid_key(&key) {
            return;
        }
        if self.serial == u64::MAX {
            self.entries.sort_by_key(|entry| entry.last);
            for (i, entry) in self.entries.iter_mut().enumerate() {
                entry.last = i as u64;
            }
            self.serial = self.entries.len() as u64;
        }
        self.serial += 1;
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            entry.count = entry.count.saturating_add(1);
            entry.last = self.serial;
        } else {
            self.entries.push(Entry {
                key,
                count: 1,
                last: self.serial,
            });
        }
        self.entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.last));
        self.entries.truncate(LIMIT);
    }
    pub fn boost(&self, key: Option<&str>) -> u64 {
        key.and_then(|key| self.entries.iter().find(|entry| entry.key == key))
            .map(|entry| {
                u64::from(entry.count.min(20)) * 3 + 20
                    - self.serial.saturating_sub(entry.last).min(20)
            })
            .unwrap_or(0)
    }
    fn load(path: &std::path::Path) -> Result<Self, String> {
        let bytes = match canopa_file::private_state::read(path, BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(_) => return Err("Launcher history could not be read".into()),
        };
        let snapshot: Self =
            serde_json::from_slice(&bytes).map_err(|_| "Launcher history is invalid")?;
        let mut keys = std::collections::HashSet::new();
        if snapshot.version != 1
            || snapshot.entries.len() > LIMIT
            || snapshot.entries.iter().any(|entry| {
                !valid_key(&entry.key) || entry.last > snapshot.serial || !keys.insert(&entry.key)
            })
        {
            return Err("Launcher history is invalid".into());
        }
        Ok(snapshot)
    }
    fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let bytes =
            serde_json::to_vec(self).map_err(|_| "Launcher history could not be encoded")?;
        canopa_file::private_state::write(path, &bytes)
            .map_err(|_| "Launcher history could not be saved".into())
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Updated {
        snapshot: Arc<Snapshot>,
        error: Option<String>,
    },
    Cleared {
        snapshot: Arc<Snapshot>,
        session: u64,
        generation: u64,
        result: Result<(), String>,
    },
}
enum Command {
    Record(String),
    Clear(u64, u64),
}
#[derive(Clone)]
struct Receiver(Arc<Mutex<Option<iced::futures::channel::mpsc::Receiver<Event>>>>);
impl Hash for Receiver {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

pub struct Runtime {
    pub snapshot: Arc<Snapshot>,
    pub error: Option<String>,
    sender: Option<mpsc::SyncSender<Command>>,
    receiver: Option<Receiver>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            snapshot: Arc::new(Snapshot::default()),
            error: None,
            sender: None,
            receiver: None,
        }
    }
}
impl Runtime {
    pub fn start(&mut self) {
        #[cfg(not(test))]
        if self.sender.is_none() {
            let path = std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .filter(|path| path.is_absolute())
                        .map(|home| home.join(".local/state"))
                })
                .map(|root| root.join("canopa/shell/launcher-history.json"));
            self.start_at(path);
        }
    }
    fn start_at(&mut self, path: Option<PathBuf>) {
        let (sender, commands) = mpsc::sync_channel(64);
        let (mut output, receiver) = iced::futures::channel::mpsc::channel(8);
        if std::thread::Builder::new()
            .name("launcher-history".into())
            .spawn(move || {
                let loaded = path
                    .as_ref()
                    .map_or_else(|| Ok(Snapshot::default()), |path| Snapshot::load(path));
                let (mut snapshot, error) = match loaded {
                    Ok(snapshot) => (snapshot, None),
                    Err(error) => (Snapshot::default(), Some(error)),
                };
                if iced::futures::executor::block_on(output.send(Event::Updated {
                    snapshot: Arc::new(snapshot.clone()),
                    error,
                }))
                .is_err()
                {
                    return;
                }
                while let Ok(command) = commands.recv() {
                    let clear = match command {
                        Command::Record(key) => {
                            snapshot.record(key);
                            None
                        }
                        Command::Clear(session, generation) => {
                            snapshot = Snapshot::default();
                            Some((session, generation))
                        }
                    };
                    let result = path.as_ref().map_or(Ok(()), |path| snapshot.save(path));
                    let event = match clear {
                        Some((session, generation)) => Event::Cleared {
                            snapshot: Arc::new(snapshot.clone()),
                            session,
                            generation,
                            result,
                        },
                        None => Event::Updated {
                            snapshot: Arc::new(snapshot.clone()),
                            error: result.err(),
                        },
                    };
                    if iced::futures::executor::block_on(output.send(event)).is_err() {
                        break;
                    }
                }
            })
            .is_ok()
        {
            self.sender = Some(sender);
            self.receiver = Some(Receiver(Arc::new(Mutex::new(Some(receiver)))));
        } else {
            self.error = Some("Launcher history worker is unavailable".into());
        }
    }
    pub fn record(&mut self, key: String) {
        if !valid_key(&key) {
            return;
        }
        if let Some(sender) = &self.sender {
            if sender.try_send(Command::Record(key)).is_err() {
                self.error = Some("Launcher history is busy".into());
            }
        } else {
            let mut snapshot = (*self.snapshot).clone();
            snapshot.record(key);
            self.snapshot = Arc::new(snapshot);
        }
    }
    pub fn clear(&mut self, session: u64, generation: u64) -> Result<Option<Event>, String> {
        self.start();
        if let Some(sender) = &self.sender {
            sender
                .try_send(Command::Clear(session, generation))
                .map_err(|_| "Launcher history is busy")?;
            Ok(None)
        } else if let Some(error) = &self.error {
            Err(error.clone())
        } else {
            Ok(Some(Event::Cleared {
                snapshot: Arc::new(Snapshot::default()),
                session,
                generation,
                result: Ok(()),
            }))
        }
    }
    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::Updated { snapshot, error } => {
                self.snapshot = snapshot.clone();
                self.error = error.clone();
            }
            Event::Cleared {
                snapshot, result, ..
            } => {
                self.snapshot = snapshot.clone();
                self.error = result.as_ref().err().cloned();
            }
        }
    }
    pub fn subscription(&self) -> iced::Subscription<Event> {
        self.receiver
            .as_ref()
            .map_or_else(iced::Subscription::none, |receiver| {
                iced::Subscription::run_with(receiver.clone(), |key| {
                    let receiver = key
                        .0
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .take();
                    iced::futures::stream::unfold(receiver, |receiver| async {
                        let mut receiver = receiver?;
                        let event = receiver.next().await?;
                        Some((event, Some(receiver)))
                    })
                })
            })
    }
}

pub(crate) fn key(action: &super::Action) -> Option<String> {
    use super::Action;
    match action {
        Action::Application { id }
        | Action::OpenWith {
            application: id, ..
        } => Some(format!("app:{id}")),
        Action::FocusWindow {
            application: Some(id),
            ..
        } => Some(format!("app:{id}")),
        Action::Open { path }
        | Action::Reveal { path }
        | Action::ChooseApplication { path }
        | Action::Preview { path } => {
            Some(format!("file:{}", crate::app::launcher::file_uri(path)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_serializes_record_and_durable_clear() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.json");
        let mut runtime = Runtime::default();
        runtime.start_at(Some(path.clone()));
        let mut receiver = runtime
            .receiver
            .as_ref()
            .unwrap()
            .0
            .lock()
            .unwrap()
            .take()
            .unwrap();
        iced::futures::executor::block_on(async {
            runtime.apply(&receiver.next().await.unwrap());
            runtime.record("app:editor".into());
            runtime.clear(4, 7).unwrap();
            runtime.apply(&receiver.next().await.unwrap());
            assert!(runtime.snapshot.boost(Some("app:editor")) > 0);
            let cleared = receiver.next().await.unwrap();
            assert!(matches!(
                cleared,
                Event::Cleared {
                    session: 4,
                    generation: 7,
                    result: Ok(()),
                    ..
                }
            ));
            runtime.apply(&cleared);
        });
        assert!(runtime.snapshot.entries.is_empty());
        assert!(Snapshot::load(&path).unwrap().entries.is_empty());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    #[test]
    fn invalid_history_cannot_supply_queries_or_duplicates() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.json");
        for input in [
            r#"{"version":2,"serial":0,"entries":[]}"#,
            r#"{"version":1,"serial":1,"entries":[{"key":"copy:secret","count":1,"last":1}]}"#,
            r#"{"version":1,"serial":1,"entries":[{"key":"app:a","count":1,"last":2}]}"#,
        ] {
            canopa_file::private_state::write(&path, input.as_bytes()).unwrap();
            assert!(Snapshot::load(&path).is_err());
        }
    }
    #[test]
    fn history_is_bounded_private_persistent_and_can_be_cleared() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("private/history.json");
        let mut snapshot = Snapshot::default();
        snapshot.record("app:editor".into());
        snapshot.record("app:editor".into());
        snapshot.record("app:browser".into());
        snapshot.record("copy:private contents".into());
        assert!(snapshot.boost(Some("app:editor")) > snapshot.boost(Some("app:browser")));
        snapshot.save(&path).unwrap();
        let loaded = Snapshot::load(&path).unwrap();
        assert_eq!(loaded.entries.len(), 2);
        for i in 0..300 {
            snapshot.record(format!("file:{i}"));
        }
        assert_eq!(snapshot.entries.len(), LIMIT);
        Snapshot::default().save(&path).unwrap();
        assert!(Snapshot::load(&path).unwrap().entries.is_empty());
        assert!(
            key(&super::super::Action::Copy {
                text: "secret".into()
            })
            .is_none()
        );
    }
}
