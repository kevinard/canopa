//! Shell-owned clipboard recording and explicitly saved snippets.
use iced::futures::SinkExt;
use std::{
    io::{self, Read, Write},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::io::{AsyncReadExt, BufReader};

use canopa_launcher::clipboard::{Edit, Options, Saved, TEXT_BYTES, Text};

mod image_data;
pub use image_data::Image;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    CopyHistory(u64),
    CopySnippet(u64),
    Forget(u64),
    Clear,
    DraftFromHistory(u64),
    DraftFromClipboard,
    Rename(u64),
    Save(Edit),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u64,
    pub content: Content,
    received: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    Text(Text),
    Image(Image),
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub saved: Arc<Saved>,
    pub loaded: bool,
    pub history: Vec<Entry>,
    pub status: [Option<String>; 2],
    pub load_error: Option<String>,
}

#[derive(Debug)]
pub struct Runtime {
    pub view: Arc<Snapshot>,
    pub saving: bool,
    saved_epoch: u64,
    epochs: [u64; 2],
    next_id: u64,
    capture_since: Instant,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            view: Arc::new(Snapshot::default()),
            saving: false,
            saved_epoch: 0,
            epochs: [0; 2],
            next_id: 0,
            capture_since: Instant::now(),
        }
    }
}

impl Runtime {
    pub fn load(&mut self, change: SavedChange) {
        if change.epoch != self.saved_epoch {
            return;
        }
        match change.result {
            Ok(saved) => self.commit(saved),
            Err(error) => Arc::make_mut(&mut self.view).load_error = Some(error),
        }
    }

    pub fn commit_save(&mut self, saved: Saved) {
        // Restart the reader so a queued snapshot cannot undo a local save.
        self.saved_epoch = self.saved_epoch.wrapping_add(1);
        self.commit(saved);
    }

    fn commit(&mut self, saved: Saved) {
        if self.view.loaded && self.view.load_error.is_none() && *self.view.saved == saved {
            return;
        }
        let view = Arc::make_mut(&mut self.view);
        view.loaded = true;
        view.load_error = None;
        let previous = view.saved.options();
        let next = saved.options();
        for kind in [Kind::Text, Kind::Image] {
            if kind.enabled(previous) != kind.enabled(next) {
                let index = kind as usize;
                self.epochs[index] = self.epochs[index].wrapping_add(1);
                view.status[index] = None;
            }
        }
        if !next.recording {
            view.history.clear();
        } else if !next.images {
            view.history
                .retain(|entry| matches!(entry.content, Content::Text(_)));
        }
        trim_history(&mut view.history, next);
        view.saved = Arc::new(saved);
    }

    pub fn clear(&mut self) {
        Arc::make_mut(&mut self.view).history.clear();
        self.capture_since = Instant::now();
    }

    pub fn forget(&mut self, id: u64) {
        Arc::make_mut(&mut self.view)
            .history
            .retain(|entry| entry.id != id);
        self.capture_since = Instant::now();
    }

    pub fn observe(&mut self, event: Observation) {
        let options = self.view.saved.options();
        let status = event.key.kind as usize;
        if !event.key.kind.enabled(options) || self.epochs[status] != event.key.epoch {
            return;
        }
        match event.kind {
            Observed::Content { content, received } if received >= self.capture_since => {
                let view = Arc::make_mut(&mut self.view);
                view.status[status] = None;
                let history = &mut view.history;
                let entry = match history.iter().position(|entry| entry.content == content) {
                    Some(position) if history[position].received >= received => return,
                    Some(position) => Entry {
                        received,
                        ..history.remove(position)
                    },
                    None => {
                        self.next_id += 1;
                        Entry {
                            id: self.next_id,
                            content,
                            received,
                        }
                    }
                };
                // Thumbnail decoding can finish after a newer text observation.
                let position = history.partition_point(|entry| entry.received > received);
                history.insert(position, entry);
                trim_history(history, options);
            }
            Observed::Status(value) if self.view.status[status] != value => {
                Arc::make_mut(&mut self.view).status[status] = value;
            }
            _ => {}
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Observation> {
        let options = self.view.saved.options();
        if !options.recording {
            return iced::Subscription::none();
        }
        let kinds = [Kind::Text, Kind::Image];
        iced::Subscription::batch(kinds.into_iter().filter(|kind| kind.enabled(options)).map(
            |kind| {
                iced::Subscription::run_with(
                    Watch {
                        epoch: self.epochs[kind as usize],
                        kind,
                    },
                    watch,
                )
            },
        ))
    }
}

fn trim_history(history: &mut Vec<Entry>, options: Options) {
    const IMAGE_MEMORY: usize = 32 * 1024 * 1024;
    let mut used = 0;
    history.retain(|entry| {
        let Content::Image(image) = &entry.content else {
            return true;
        };
        if used + image.memory() > IMAGE_MEMORY {
            return false;
        }
        used += image.memory();
        true
    });
    history.truncate(options.max_entries);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Text,
    Image,
}

impl Kind {
    fn enabled(self, options: Options) -> bool {
        options.recording && (self == Self::Text || options.images)
    }

    fn maximum(self) -> usize {
        match self {
            Self::Text => TEXT_BYTES,
            Self::Image => image_data::MAX_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Hash)]
struct Watch {
    epoch: u64,
    kind: Kind,
}

#[derive(Clone, Debug)]
pub struct Observation {
    key: Watch,
    kind: Observed,
}

#[derive(Clone, Debug)]
enum Observed {
    Content { content: Content, received: Instant },
    Status(Option<String>),
}

fn watch(key: &Watch) -> impl iced::futures::Stream<Item = Observation> + use<> {
    let key = *key;
    iced::stream::channel(
        2,
        move |mut out: iced::futures::channel::mpsc::Sender<Observation>| async move {
            loop {
                let Err(error) = watch_once(key, &mut out).await else {
                    return;
                };
                if out
                    .send(Observation {
                        key,
                        kind: Observed::Status(Some(error)),
                    })
                    .await
                    .is_err()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(4)).await;
            }
        },
    )
}

async fn watch_once(
    key: Watch,
    out: &mut iced::futures::channel::mpsc::Sender<Observation>,
) -> Result<(), String> {
    use std::process::Stdio;
    let (mime, flag) = match key.kind {
        Kind::Text => ("text", "--clipboard-frame"),
        Kind::Image => ("image", "--clipboard-image-frame"),
    };
    let exe =
        canopa_file::executable::installed().map_err(|_| fl!("shell-clipboard-reader-failed"))?;
    let mut child = tokio::process::Command::new("wl-paste")
        .args([
            "--type",
            mime,
            "--no-newline",
            "--watch",
            "timeout",
            "--foreground",
            "2s",
        ])
        .arg(exe)
        .arg(flag)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .process_group(0)
        .spawn()
        .map_err(|_| fl!("shell-clipboard-wl-paste-failed"))?;
    let _group = crate::command::ProcessGroup::new(child.id());
    let mut reader = BufReader::new(
        child
            .stdout
            .take()
            .ok_or_else(|| fl!("shell-clipboard-watcher-unreadable"))?,
    );
    let stopped = || match key.kind {
        Kind::Text => fl!("shell-clipboard-text-watcher-stopped"),
        Kind::Image => fl!("shell-clipboard-image-watcher-stopped"),
    };
    loop {
        let payload = tokio::select! {
            _ = child.wait() => return Err(stopped()),
            result = read_frame(&mut reader, key.kind.maximum()) => result.map_err(|_| stopped())?,
        };
        let received = Instant::now();
        let content = if payload.is_empty() {
            None
        } else {
            match key.kind {
                Kind::Text => String::from_utf8(payload)
                    .ok()
                    .and_then(Text::new)
                    .map(Content::Text),
                Kind::Image => tokio::task::spawn_blocking(move || Image::decode(payload))
                    .await
                    .ok()
                    .flatten()
                    .map(Content::Image),
            }
        };
        let kind = match content {
            Some(content) => Observed::Content { content, received },
            None => Observed::Status(None),
        };
        if out.send(Observation { key, kind }).await.is_err() {
            return Ok(());
        }
    }
}

async fn read_frame(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
    maximum: usize,
) -> io::Result<Vec<u8>> {
    let length = reader.read_u32().await? as usize;
    if length > maximum {
        return Err(io::Error::other(
            "Clipboard frame exceeds the capture budget",
        ));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).await?;
    Ok(bytes)
}

/// wl-paste gives each invocation one selection on stdin and waits for it to finish.
pub fn write_frame(kind: Kind) -> io::Result<()> {
    let mut bytes = Vec::new();
    if std::env::var("CLIPBOARD_STATE").as_deref() == Ok("data") {
        io::stdin()
            .take(kind.maximum() as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > kind.maximum() {
            bytes.clear();
        }
    }
    let mut stdout = io::stdout().lock();
    stdout.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stdout.write_all(&bytes)?;
    stdout.flush()
}

#[derive(Clone, Debug)]
pub struct SavedChange {
    epoch: u64,
    result: Result<Saved, String>,
}

impl Runtime {
    #[cfg(test)]
    pub fn saved_subscription(&self) -> iced::Subscription<SavedChange> {
        iced::Subscription::none()
    }

    #[cfg(not(test))]
    pub fn saved_subscription(&self) -> iced::Subscription<SavedChange> {
        iced::Subscription::run_with(self.saved_epoch, saved_changes)
    }
}

#[cfg(not(test))]
fn saved_changes(epoch: &u64) -> impl iced::futures::Stream<Item = SavedChange> + use<> {
    use std::os::unix::fs::MetadataExt;
    let epoch = *epoch;
    iced::stream::channel(
        1,
        move |mut out: iced::futures::channel::mpsc::Sender<SavedChange>| async move {
            let path = match canopa_launcher::clipboard::path() {
                Ok(path) => path,
                Err(error) => {
                    let _ = out
                        .send(SavedChange {
                            epoch,
                            result: Err(error),
                        })
                        .await;
                    return;
                }
            };
            let mut previous = None;
            loop {
                let path = path.clone();
                let next = tokio::task::spawn_blocking(move || {
                    let fingerprint = std::fs::symlink_metadata(&path)
                        .map(|meta| {
                            (
                                meta.dev(),
                                meta.ino(),
                                meta.len(),
                                meta.ctime(),
                                meta.ctime_nsec(),
                            )
                        })
                        .map_err(|error| error.kind());
                    if previous == Some(fingerprint) {
                        return None;
                    }
                    Some((fingerprint, Saved::load(&path)))
                })
                .await;
                match next {
                    Ok(Some((fingerprint, result))) => {
                        previous = Some(fingerprint);
                        if out.send(SavedChange { epoch, result }).await.is_err() {
                            return;
                        }
                    }
                    Ok(None) => {}
                    Err(_) => {
                        let _ = out
                            .send(SavedChange {
                                epoch,
                                result: Err(fl!("shell-clipboard-preferences-stopped")),
                            })
                            .await;
                        return;
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Text {
        Text::new(value.to_owned()).unwrap()
    }

    #[test]
    fn pending_saved_read_cannot_undo_local_save() {
        let mut runtime = Runtime::default();
        let older = SavedChange {
            epoch: runtime.saved_epoch,
            result: Ok(Saved::default()),
        };
        let mut enabled = Saved::default();
        enabled.edit(Edit::Recording(true)).unwrap();
        runtime.commit_save(enabled);
        runtime.load(older);
        assert!(runtime.view.saved.options().recording);
        runtime.load(SavedChange {
            epoch: runtime.saved_epoch,
            result: Ok(Saved::default()),
        });
        assert!(!runtime.view.saved.options().recording);
    }

    #[test]
    fn clear_and_disable_retire_pending_observations() {
        let mut runtime = Runtime::default();
        let mut enabled = Saved::default();
        enabled.edit(Edit::Recording(true)).unwrap();
        runtime.commit(enabled.clone());
        let received = Instant::now();
        let original_epoch = runtime.epochs[Kind::Text as usize];
        let event = |received| Observation {
            key: Watch {
                epoch: original_epoch,
                kind: Kind::Text,
            },
            kind: Observed::Content {
                content: Content::Text(text("copied")),
                received,
            },
        };
        runtime.observe(event(received));
        runtime.observe(event(received));
        assert_eq!(runtime.view.history.len(), 1);
        runtime.clear();
        runtime.observe(event(
            runtime
                .capture_since
                .checked_sub(Duration::from_nanos(1))
                .unwrap(),
        ));
        assert!(runtime.view.history.is_empty());
        runtime.observe(event(Instant::now()));
        assert_eq!(runtime.view.history.len(), 1);
        runtime.commit(Saved::default());
        runtime.commit(enabled);
        runtime.observe(event(Instant::now()));
        assert!(runtime.view.history.is_empty());
    }
    #[test]
    fn late_image_does_not_displace_newer_text_or_reappear_after_opt_out() {
        let mut runtime = Runtime::default();
        let mut saved = Saved::default();
        let options = Options {
            recording: true,
            images: true,
            max_entries: 1,
        };
        saved.set_options(options);
        runtime.commit(saved.clone());

        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(4, 2)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let image = Image::decode(bytes.into_inner()).unwrap();
        let image_received = Instant::now();
        let text_epoch = runtime.epochs[Kind::Text as usize];
        runtime.observe(Observation {
            key: Watch {
                epoch: text_epoch,
                kind: Kind::Text,
            },
            kind: Observed::Content {
                content: Content::Text(text("newer text")),
                received: image_received + Duration::from_millis(1),
            },
        });
        let observed_image = Observation {
            key: Watch {
                epoch: runtime.epochs[Kind::Image as usize],
                kind: Kind::Image,
            },
            kind: Observed::Content {
                content: Content::Image(image),
                received: image_received,
            },
        };
        runtime.observe(observed_image.clone());
        assert!(matches!(runtime.view.history[0].content, Content::Text(_)));
        assert_eq!(runtime.view.history.len(), 1);

        saved.set_options(Options {
            images: false,
            ..options
        });
        runtime.commit(saved.clone());
        saved.set_options(Options {
            max_entries: 2,
            ..options
        });
        runtime.commit(saved);
        runtime.observe(observed_image);
        assert_eq!(runtime.epochs[Kind::Text as usize], text_epoch);
        assert_eq!(runtime.view.history.len(), 1);
    }
}
