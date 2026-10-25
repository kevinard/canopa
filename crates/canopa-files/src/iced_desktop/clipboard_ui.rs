//! File exchange keeps the user's source observation, independently of widgets.
use super::*;
use crate::{
    file_clipboard::{
        Selection,
        wayland::{Clipboard, Intent, Received},
    },
    operation_jobs::PreparedSelection,
    storage,
    transfers::{TransferKind, TransferRequest},
};
use iced::futures::StreamExt;
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub(super) enum CopyOrigin {
    Browser(Arc<file_operations::Origin>),
    Trash { window: window::Id, path: PathBuf },
}
impl CopyOrigin {
    fn paths(&self) -> Vec<PathBuf> {
        match self {
            Self::Browser(origin) => origin.selected.clone(),
            Self::Trash { path, .. } => vec![path.clone()],
        }
    }
    fn current(&self, desktop: &Desktop) -> bool {
        match self {
            Self::Browser(origin) => origin.current(desktop),
            Self::Trash { window, path } => desktop.trash_window.as_ref().is_some_and(|trash| {
                trash.window == *window && trash.selected.as_ref() == Some(path)
            }),
        }
    }
}

#[derive(Clone)]
pub(super) struct Source {
    serial: u64,
    pub origin: CopyOrigin,
    prepared: PreparedSelection,
}

pub(super) struct Paste {
    origin: Arc<file_operations::Origin>,
    owned: Option<(Intent, Arc<Source>)>,
    external: storage::Snapshot,
}

#[derive(Clone, Debug)]
pub(super) struct Cut {
    intent: Intent,
    revision: u64,
    origin: CopyOrigin,
    serial: u64,
}
impl std::fmt::Debug for Paste {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Paste destination")
            .field("origin", &self.origin)
            .finish()
    }
}
impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clipboard source")
            .field("selection", &self.prepared.snapshot().selection)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Changed,
    Prepared(u64, CopyOrigin, Result<PreparedSelection, String>),
    Published(Arc<Source>, Result<Intent, String>),
    Read(Arc<Paste>, Result<Received, String>),
    CutUpdated(Cut, Result<bool, String>),
}

#[derive(Clone)]
struct Notices(Arc<Mutex<Option<iced::futures::channel::mpsc::Receiver<()>>>>);
impl PartialEq for Notices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Notices {}
impl std::hash::Hash for Notices {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
    }
}

#[derive(Default)]
pub(super) struct State {
    service: Option<Clipboard>,
    notices: Option<Notices>,
    owned: Option<(Intent, Arc<Source>)>,
    pending: usize,
    error: Option<String>,
    latest: u64,
    cut_revision: u64,
}
impl State {
    pub fn start() -> Self {
        match Clipboard::start() {
            Ok(service) => Self {
                notices: service
                    .take_notices()
                    .map(|receiver| Notices(Arc::new(Mutex::new(Some(receiver))))),
                service: Some(service),
                ..Self::default()
            },
            Err(error) => Self {
                error: Some(error.to_string()),
                ..Self::default()
            },
        }
    }
    pub fn ready(&self) -> bool {
        self.service
            .as_ref()
            .is_some_and(|service| service.snapshot().ready)
    }
    pub fn can_paste(&self) -> bool {
        self.pending == 0
            && self
                .owned
                .as_ref()
                .is_none_or(|(_, source)| !source.prepared.snapshot().busy)
            && self.service.as_ref().is_some_and(|service| {
                let snapshot = service.snapshot();
                snapshot.ready && snapshot.has_files
            })
    }
    pub fn holds_application(&self) -> bool {
        self.pending > 0
            || self
                .service
                .as_ref()
                .is_some_and(|service| service.snapshot().owned.is_some())
    }
    pub fn subscription(&self) -> Subscription<Message> {
        self.notices
            .as_ref()
            .map_or_else(Subscription::none, |notices| {
                Subscription::run_with(notices.clone(), |key| {
                    let receiver = key.0.lock().unwrap_or_else(|e| e.into_inner()).take();
                    iced::futures::stream::unfold(receiver, |receiver| async move {
                        let mut receiver = receiver?;
                        receiver.next().await?;
                        Some((Message::FileClipboard(Event::Changed), Some(receiver)))
                    })
                })
            })
    }
}

impl Desktop {
    pub(super) fn sync_clipboard_cut(&mut self) -> Task<Message> {
        if self.file_clipboard.pending != 0 {
            return Task::none();
        }
        let Some((intent, source)) = self.file_clipboard.owned.clone() else {
            return Task::none();
        };
        let snapshot = source.prepared.snapshot();
        if snapshot.revision <= self.file_clipboard.cut_revision {
            return Task::none();
        }
        let Some(service) = self
            .file_clipboard
            .service
            .clone()
            .filter(|service| service.snapshot().owned == Some(intent))
        else {
            return Task::none();
        };
        // A failed transport update is shown to the user; do not retry it in a
        // tight event loop. A newer Copy/Cut still has its own native identity.
        self.file_clipboard.cut_revision = snapshot.revision;
        self.file_clipboard.pending += 1;
        let cut = Cut {
            intent,
            revision: snapshot.revision,
            origin: source.origin.clone(),
            serial: source.serial,
        };
        Task::perform(
            async move {
                service
                    .replace_cut(intent, snapshot.selection)
                    .map_err(|error| error.to_string())?
                    .await
                    .map_err(|_| {
                        "The file clipboard stopped before updating the Cut selection".to_string()
                    })?
            },
            move |result| Message::FileClipboard(Event::CutUpdated(cut.clone(), result)),
        )
    }

    pub(super) fn copy_files(&mut self, window: window::Id, cut: bool) -> Task<Message> {
        let Some(origin) = file_operations::Origin::capture(self, window)
            .filter(|origin| !origin.selected.is_empty())
        else {
            return Task::none();
        };
        self.copy_selection(CopyOrigin::Browser(origin), cut)
    }

    pub(super) fn copy_trash(&mut self, window: window::Id) -> Task<Message> {
        let Some(trash) = self
            .trash_window
            .as_ref()
            .filter(|trash| trash.window == window && !trash.snapshot.loading)
        else {
            return Task::none();
        };
        let Some(item) = trash.selected() else {
            return Task::none();
        };
        self.copy_selection(
            CopyOrigin::Trash {
                window,
                path: item.path.clone(),
            },
            false,
        )
    }

    fn copy_selection(&mut self, origin: CopyOrigin, cut: bool) -> Task<Message> {
        if !self.file_clipboard.ready() || self.file_clipboard.pending >= 8 {
            let message = self.file_clipboard.error.clone().unwrap_or_else(|| {
                "The file clipboard is starting or unavailable. Try again.".into()
            });
            self.copy_error(&origin, message);
            return Task::none();
        };
        let Some(storage) = self
            .file_operations
            .storage
            .as_ref()
            .filter(|_| self.file_operations.storage_error.is_none())
        else {
            self.copy_error(
                &origin,
                "Source storage is not ready. Try again after it becomes available.".into(),
            );
            return Task::none();
        };
        let Some(operations) = self.file_operations.service.clone() else {
            return self.show_operations();
        };
        let mut placement = storage.snapshot();
        if let CopyOrigin::Browser(origin) = &origin {
            placement.bind_folder(origin.directory.clone(), origin.resolved.clone());
        }
        let Some(serial) = self.file_clipboard.latest.checked_add(1) else {
            self.copy_error(
                &origin,
                "Clipboard identities exhausted. Restart Files.".into(),
            );
            return Task::none();
        };
        self.file_clipboard.latest = serial;
        let kind = if cut {
            TransferKind::Move
        } else {
            TransferKind::Copy
        };
        let previous = self
            .file_clipboard
            .owned
            .as_ref()
            .filter(|(intent, _)| {
                self.file_clipboard
                    .service
                    .as_ref()
                    .is_some_and(|service| service.snapshot().owned == Some(*intent))
            })
            .map(|(_, source)| source.prepared.clone());
        self.file_clipboard.pending += 1;
        let captured = origin.clone();
        Task::perform(
            async move {
                let selection =
                    Selection::new(captured.paths(), kind).map_err(|error| error.to_string())?;
                match previous {
                    Some(previous) => operations.replace_clipboard(previous, selection, placement),
                    None => operations.prepare_clipboard(selection, placement),
                }
                .map_err(|error| error.to_string())?
                .await
                .map_err(|_| {
                    "The operation worker stopped before preparing the selection".to_string()
                })?
            },
            move |result| Message::FileClipboard(Event::Prepared(serial, origin.clone(), result)),
        )
    }

    pub(super) fn paste_files(&mut self, window: window::Id) -> Task<Message> {
        let Some(origin) = file_operations::Origin::capture(self, window) else {
            return Task::none();
        };
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        if !self.file_clipboard.can_paste() {
            self.clipboard_error(&origin, "The clipboard contains no ready file selection. Wait for Copy or Cut to finish, then try again.".into());
            return Task::none();
        }
        let Some(storage) = self.file_operations.storage.as_ref() else {
            return self.show_operations();
        };
        let context = Arc::new(Paste {
            origin,
            owned: self.file_clipboard.owned.clone(),
            external: storage.snapshot(),
        });
        let service = self.file_clipboard.service.as_ref().unwrap();
        match service.read() {
            Ok(receiver) => {
                self.file_clipboard.pending += 1;
                Task::perform(
                    async move {
                        receiver
                            .await
                            .map_err(|_| "The file clipboard stopped before replying".to_string())?
                    },
                    move |result| Message::FileClipboard(Event::Read(context.clone(), result)),
                )
            }
            Err(error) => {
                self.clipboard_error(&context.origin, error.to_string());
                Task::none()
            }
        }
    }

    fn copy_error(&mut self, origin: &CopyOrigin, error: String) {
        match origin {
            CopyOrigin::Browser(origin) => self.clipboard_error(origin, error),
            CopyOrigin::Trash { window, .. } => {
                self.file_clipboard.error = Some(error.clone());
                if let Some(trash) = self
                    .trash_window
                    .as_mut()
                    .filter(|trash| trash.window == *window)
                {
                    trash.error = Some(error);
                }
            }
        }
    }

    fn clipboard_error(&mut self, origin: &file_operations::Origin, error: String) {
        self.file_clipboard.error = Some(error.clone());
        if let Some(tab) = self
            .windows
            .get_mut(&origin.window)
            .and_then(|window| window.tabs.iter_mut().find(|tab| tab.id == origin.tab))
        {
            tab.error = Some(error);
        }
    }

    pub(super) fn clipboard_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Prepared(serial, origin, result) => {
                self.file_clipboard.pending = self.file_clipboard.pending.saturating_sub(1);
                if serial == self.file_clipboard.latest {
                    let publication = result.and_then(|prepared| {
                        let selection = prepared.snapshot().selection.ok_or_else(|| {
                            "The prepared selection is no longer available".to_string()
                        })?;
                        let source = Arc::new(Source {
                            serial,
                            origin: origin.clone(),
                            prepared,
                        });
                        let receiver = self
                            .file_clipboard
                            .service
                            .as_ref()
                            .ok_or_else(|| "The file clipboard is unavailable".to_string())?
                            .publish(selection)
                            .map_err(|error| error.to_string())?;
                        Ok((source, receiver))
                    });
                    match publication {
                        Ok((source, receiver)) => {
                            self.file_clipboard.pending += 1;
                            return Task::perform(
                                async move {
                                    receiver.await.map_err(|_| "The file clipboard stopped before accepting the selection".to_string())?
                                },
                                move |result| {
                                    Message::FileClipboard(Event::Published(source.clone(), result))
                                },
                            );
                        }
                        Err(error) => self.copy_error(&origin, error),
                    }
                }
            }
            Event::Changed => {
                if let Some(service) = &self.file_clipboard.service {
                    let snapshot = service.snapshot();
                    if self
                        .file_clipboard
                        .owned
                        .as_ref()
                        .is_some_and(|(token, _)| snapshot.owned != Some(*token))
                    {
                        self.file_clipboard.owned = None;
                    }
                    self.file_clipboard.error = snapshot.error;
                }
            }
            Event::Published(source, result) => {
                self.file_clipboard.pending = self.file_clipboard.pending.saturating_sub(1);
                match result {
                    Ok(token)
                        if self
                            .file_clipboard
                            .service
                            .as_ref()
                            .is_some_and(|service| service.snapshot().owned == Some(token)) =>
                    {
                        if source.serial == self.file_clipboard.latest
                            && source.origin.current(self)
                            && let CopyOrigin::Browser(origin) = &source.origin
                            && let Some(tab) = self.active_mut(origin.window)
                        {
                            tab.error = None;
                        }
                        self.file_clipboard.cut_revision = 0;
                        self.file_clipboard.owned = Some((token, source));
                        self.file_clipboard.error = None;
                    }
                    Ok(_) => {}
                    Err(error) if source.serial == self.file_clipboard.latest => {
                        self.copy_error(&source.origin, error)
                    }
                    Err(_) => {}
                }
            }
            Event::Read(context, result) => {
                self.file_clipboard.pending = self.file_clipboard.pending.saturating_sub(1);
                let result = result.and_then(|received| {
                    let prepared = match received.owned {
                        Some(intent) => Some(context.owned.as_ref()
                            .filter(|(token, _)| *token == intent)
                            .map(|(_, source)| source.prepared.clone())
                            .ok_or_else(|| "The original selection is unavailable. Copy or Cut the files again.".to_string())?),
                        None => None,
                    };
                    Ok((received.selection, prepared))
                });
                match result {
                    Ok((selection, prepared)) => {
                        let origin = context.origin.clone();
                        let submission = file_operations::Submission {
                            owner: origin.window,
                            origin: Some(origin.clone()),
                            dialog: None,
                            affected: Vec::new(),
                            reveal: None,
                            refresh_all: true,
                        };
                        if let Some(prepared) = prepared {
                            return self.submit_clipboard_paste(prepared, submission);
                        }
                        let request = TransferRequest {
                            kind: selection.kind(),
                            sources: selection.paths().to_vec(),
                            destination: origin.directory.clone(),
                        };
                        return self.submit_file_transfer(
                            request,
                            context.external.clone(),
                            submission,
                        );
                    }
                    Err(error) => self.clipboard_error(&context.origin, error),
                }
            }
            Event::CutUpdated(cut, result) => {
                self.file_clipboard.pending = self.file_clipboard.pending.saturating_sub(1);
                if let Err(error) = result
                    && cut.revision == self.file_clipboard.cut_revision
                    && cut.serial == self.file_clipboard.latest
                    && self
                        .file_clipboard
                        .service
                        .as_ref()
                        .is_some_and(|service| service.snapshot().owned == Some(cut.intent))
                {
                    self.copy_error(&cut.origin, format!("The move finished, but its Cut selection could not be updated: {error}. Copy or Cut the remaining files again."));
                }
            }
        }
        let update = self.sync_clipboard_cut();
        Task::batch([update, self.exit_if_empty()])
    }
}
