//! Native gesture coordination. File identity, action policy and mutation admission
//! remain behind OperationJobs; this adapter owns only live transport and views.
use super::*;
use crate::{
    file_clipboard::{LIMIT, Selection, URIS},
    operation_jobs::{self, DropActions, DropChoice, DropTarget, PreparedDrag, PreparedDrop},
    transfers::{DropPreference, TransferKind},
};
use iced::futures::channel::mpsc;
use iced::{Point, Rectangle, window::drag_drop as native};
use std::sync::Mutex;

#[path = "tab_drag.rs"]
pub(super) mod tab_drag;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Context {
    window: window::Id,
    tab: u64,
    revision: u64,
    epoch: u64,
}
impl Context {
    fn new(window: window::Id, tab: &Tab) -> Self {
        Self {
            window,
            tab: tab.id,
            revision: tab.revision,
            epoch: tab.list_epoch,
        }
    }
    fn current(&self, desktop: &Desktop) -> bool {
        desktop.active(self.window).is_some_and(|tab| {
            tab.id == self.tab
                && tab.revision == self.revision
                && tab.list_epoch == self.epoch
                && !tab.loading
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Row {
    context: Context,
    path: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    context: Context,
    directory: PathBuf,
    binding: Option<(PathBuf, PathBuf)>,
    priority: u8,
    device: Option<crate::local_volumes::DeviceToken>,
}
impl Target {
    fn current(&self, desktop: &Desktop) -> bool {
        self.context.current(desktop)
            && self.device.is_none_or(|token| {
                desktop
                    .device_runtime()
                    .and_then(|runtime| runtime.device_requests().root(token).ok())
                    .is_some_and(|root| root == self.directory)
            })
    }
    fn core(&self) -> DropTarget {
        DropTarget {
            directory: self.directory.clone(),
            binding: self
                .binding
                .as_ref()
                .map(|(directory, resolved)| operation_jobs::Source {
                    directory: directory.clone(),
                    resolved: resolved.clone(),
                }),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Region {
    source: Option<Row>,
    target: Option<Target>,
}
#[derive(Clone, Debug, PartialEq)]
struct Bounds {
    region: Region,
    bounds: Rectangle,
}
struct SceneData {
    current: Vec<Bounds>,
    staging: Vec<Bounds>,
    changed: mpsc::Sender<()>,
}
#[derive(Clone)]
pub(super) struct Scene(Arc<Mutex<SceneData>>);
impl Scene {
    fn new() -> (Self, activation_ui::Notices) {
        let (send, receive) = mpsc::channel(0);
        (
            Self(Arc::new(Mutex::new(SceneData {
                current: Vec::new(),
                staging: Vec::new(),
                changed: send,
            }))),
            activation_ui::Notices::new(receive),
        )
    }
    pub fn begin(&self) {
        self.0.lock().unwrap().staging.clear();
    }
    pub fn record(&self, region: Region, bounds: Rectangle) {
        let mut scene = self.0.lock().unwrap();
        if scene.staging.len() < 2048 {
            scene.staging.push(Bounds { region, bounds });
        }
    }
    pub fn commit(&self) {
        let mut scene = self.0.lock().unwrap();
        if scene.current != scene.staging {
            scene.current = std::mem::take(&mut scene.staging);
            let _ = scene.changed.try_send(());
        }
    }
    pub fn source(&self, point: Point) -> Option<Row> {
        self.0
            .lock()
            .unwrap()
            .current
            .iter()
            .rev()
            .filter(|region| region.bounds.contains(point))
            .find_map(|region| region.region.source.clone())
    }
    fn target(&self, point: Point) -> Option<(Target, Rectangle)> {
        self.0
            .lock()
            .unwrap()
            .current
            .iter()
            .filter(|region| region.bounds.contains(point))
            .filter_map(|region| {
                region
                    .region
                    .target
                    .as_ref()
                    .map(|target| (target, region.bounds))
            })
            .max_by_key(|(target, _)| target.priority)
            .map(|(target, bounds)| (target.clone(), bounds))
    }
}

/// A drag hovering a browser window, as its views show it.
pub(super) struct HoverFeedback {
    /// The target outlined under the pointer.
    pub bounds: Rectangle,
    pub pointer: Point,
    /// What a drop would do, for the status bar: "Move to Documents".
    pub label: String,
    /// The same with what is dragged, beside the pointer.
    pub badge: String,
    pub refused: bool,
    /// A drop here would copy or move: only then is the target outlined and
    /// the pointer labelled.
    pub acting: bool,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Enabled(window::Id, Option<native::DragDrop>),
    Changed(window::Id),
    Press(window::Id, Option<Row>),
    NativePress(window::Id),
    Start(window::Id),
    Release(window::Id),
    Prepared(u64, Result<PreparedDrag, String>),
    Bytes(u64, bool, Result<Option<Arc<[PathBuf]>>, String>),
    ForeignPrepared(u64, Result<PreparedDrag, String>),
    HoverPrepared(u64, u64, Result<PreparedDrop, String>),
    Admitted(
        u64,
        Arc<file_operations::Submission>,
        Option<Result<u64, String>>,
    ),
    /// A press on a tab of a window: its bounds and where in it it was pressed.
    TabPress(window::Id, u64, Rectangle, iced::Vector),
    /// That press moved far enough to drag the tab.
    TabStart(window::Id, u64),
    /// The dragged tab's picture, cut from its window's capture.
    TabPicture(u64, Arc<Option<native::Icon>>),
}
struct Host {
    scene: Scene,
    geometry: activation_ui::Notices,
    endpoint: Option<native::DragDrop>,
    notices: Option<activation_ui::Notices>,
    live: bool,
}
struct Gesture {
    serial: u64,
    origin: Arc<file_operations::Origin>,
    path: PathBuf,
    mode: SelectionMode,
    cancellation: Cancellation,
    prepared: Option<PreparedDrag>,
    native: Option<native::Origin>,
    requested: bool,
    deferred: bool,
    error: Option<String>,
}
struct Outgoing {
    source: native::Source,
    prepared: PreparedDrag,
    owner: window::Id,
}
struct Incoming {
    offer: native::Offer,
    owner: window::Id,
    local: Option<u64>,
    paths: Option<Arc<[PathBuf]>>,
    drag: Option<PreparedDrag>,
    cancellation: Cancellation,
    target: Option<Target>,
    bounds: Option<Rectangle>,
    hover: Option<PreparedDrop>,
    hover_cancel: Cancellation,
    hover_serial: u64,
    hover_pending: bool,
    choice: Option<DropChoice>,
    confirmed: bool,
    dropped: bool,
    awaiting: bool,
    job: Option<u64>,
    _work: Option<crate::local_activity::Work>,
}
#[derive(Default)]
pub(super) struct State {
    hosts: BTreeMap<window::Id, Host>,
    pointer: Option<Gesture>,
    outgoing: BTreeMap<u64, Outgoing>,
    incoming: BTreeMap<u64, Incoming>,
    serial: u64,
    pending: usize,
    suppress: Option<(window::Id, PathBuf)>,
    modifiers: keyboard::Modifiers,
    rejected: BTreeMap<u64, window::Id>,
    tab_gesture: Option<tab_drag::TabGesture>,
    tab_source: Option<tab_drag::TabSource>,
    tab_hovers: BTreeMap<window::Id, tab_drag::TabHover>,
}
impl State {
    pub fn open(&mut self, id: window::Id) {
        let (scene, geometry) = Scene::new();
        self.hosts.insert(
            id,
            Host {
                scene,
                geometry,
                endpoint: None,
                notices: None,
                live: true,
            },
        );
    }
    pub fn scene(&self, id: window::Id) -> Option<Scene> {
        self.hosts.get(&id).map(|host| host.scene.clone())
    }
    pub fn holds_application(&self) -> bool {
        self.pending > 0
            || self.pointer.is_some()
            || self.tab_source.is_some()
            || !self.outgoing.is_empty()
            || self.incoming.values().any(|offer| offer.dropped)
    }
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(
            self.hosts
                .iter()
                .flat_map(|(&id, host)| {
                    let event = |(id, ())| Message::Drag(Event::Changed(id));
                    [
                        host.geometry.subscription().with(id).map(event),
                        host.notices
                            .as_ref()
                            .map_or_else(Subscription::none, |notice| {
                                notice.subscription().with(id).map(event)
                            }),
                    ]
                })
                .collect::<Vec<_>>(),
        )
    }
    pub fn suppress_select(&mut self, id: window::Id, path: &Path) -> bool {
        if self
            .suppress
            .as_ref()
            .is_some_and(|(window, source)| *window == id && source == path)
        {
            self.suppress = None;
            true
        } else {
            false
        }
    }
    pub fn modifiers(&mut self, modifiers: keyboard::Modifiers) {
        self.modifiers = modifiers;
    }
    fn cancel_pointer(&mut self) {
        if let Some(gesture) = self.pointer.take() {
            gesture.cancellation.cancel();
            if let Some(drag) = gesture.prepared {
                drag.cancel();
            }
        }
    }
    pub fn close(&mut self, id: window::Id) {
        if let Some(host) = self.hosts.get_mut(&id) {
            host.live = false;
        }
        if self
            .pointer
            .as_ref()
            .is_some_and(|pointer| pointer.origin.window == id)
        {
            self.cancel_pointer();
        }
        for outgoing in self.outgoing.values().filter(|source| source.owner == id) {
            if !outgoing.source.snapshot().dropped {
                outgoing.source.cancel();
                outgoing.prepared.cancel();
            }
        }
    }
    fn preference(&self) -> DropPreference {
        if self.modifiers.control() {
            DropPreference::Copy
        } else if self.modifiers.shift() {
            DropPreference::Move
        } else {
            DropPreference::Automatic
        }
    }
    fn retire(&mut self, incoming: Incoming, success: bool) {
        if !incoming.offer.snapshot().dropped && !incoming.offer.snapshot().finished {
            self.rejected
                .insert(incoming.offer.snapshot().id, incoming.owner);
            let _ = incoming
                .offer
                .accept(None, native::Actions::default(), None);
        }
        incoming.hover_cancel.cancel();
        incoming.cancellation.cancel();
        if incoming.local.is_none()
            && let Some(drag) = incoming.drag
        {
            drag.cancel();
        }
        if incoming.offer.snapshot().dropped && !incoming.offer.snapshot().finished {
            let _ = incoming.offer.finish(success);
        }
    }
    pub fn feedback(&self, id: window::Id) -> Option<(Rectangle, String)> {
        self.hover_feedback(id)
            .map(|feedback| (feedback.bounds, feedback.label))
    }

    /// The drop target to outline: only one that a drop would act on.
    pub fn outlined(&self, id: window::Id) -> Option<Rectangle> {
        self.hover_feedback(id)
            .filter(|feedback| feedback.acting)
            .map(|feedback| feedback.bounds)
    }

    /// Whether `path` is being dragged out of this application right now.
    pub fn is_dragged(&self, path: &Path) -> bool {
        self.outgoing.values().any(|outgoing| {
            let snapshot = outgoing.source.snapshot();
            !snapshot.finished
                && !snapshot.dropped
                && outgoing
                    .prepared
                    .paths()
                    .iter()
                    .any(|dragged| dragged == path)
        })
    }

    /// What a drag over window `id` would do there, and where its pointer is.
    pub fn hover_feedback(&self, id: window::Id) -> Option<HoverFeedback> {
        self.incoming.values().find_map(|incoming| {
            if incoming.owner != id || incoming.dropped || incoming.offer.snapshot().left {
                return None;
            }
            let bounds = incoming.bounds?;
            let target = incoming.target.as_ref()?;
            let snapshot = incoming.offer.snapshot();
            let folder = target
                .directory
                .file_name()
                .unwrap_or(target.directory.as_os_str())
                .to_string_lossy()
                .into_owned();
            let items = incoming.paths.as_deref().map(|paths| match paths {
                [one] => one
                    .file_name()
                    .unwrap_or(one.as_os_str())
                    .to_string_lossy()
                    .into_owned(),
                many => fl!("files-status-items", count = many.len()),
            });
            let home = incoming.paths.as_deref().is_some_and(|paths| {
                !paths.is_empty()
                    && paths.iter().all(|path| {
                        path.parent() == Some(target.directory.as_path())
                            || *path == target.directory
                    })
            });
            let refused = incoming.hover_cancel.is_cancelled()
                || incoming.hover.is_some() && snapshot.selected.is_none();
            let label = if home {
                fl!("files-drop-already-there", folder = folder)
            } else if incoming.hover_cancel.is_cancelled() {
                fl!("files-drop-refused", folder = folder)
            } else if incoming.hover.is_some() {
                match snapshot.selected {
                    Some(native::Action::Copy) => fl!("files-drop-copy", folder = folder),
                    Some(native::Action::Move) => fl!("files-drop-move", folder = folder),
                    None => fl!("files-drop-refused", folder = folder),
                }
            } else {
                fl!("files-drop-checking", folder = folder)
            };
            Some(HoverFeedback {
                bounds,
                pointer: Point::new(snapshot.position.0 as f32, snapshot.position.1 as f32),
                badge: match items {
                    Some(items) if !home => {
                        fl!("files-drop-badge", action = label.clone(), items = items)
                    }
                    _ => label.clone(),
                },
                label,
                refused: refused || home,
                acting: !refused
                    && !home
                    && incoming.hover.is_some()
                    && snapshot.selected.is_some(),
            })
        })
    }
}
fn allowed(desktop: &Desktop, id: window::Id) -> bool {
    desktop.windows.contains_key(&id)
        && desktop
            .action_menu
            .as_ref()
            .is_none_or(|menu| menu.window() != id)
        && desktop
            .folder_options
            .as_ref()
            .is_none_or(|target| target.window != id)
        && desktop
            .active(id)
            .is_some_and(|tab| tab.location.is_none() && !tab.loading)
}
fn paths_task(id: u64, offer: native::Offer, final_read: bool) -> Task<Message> {
    Task::perform(
        async move {
            let bytes = offer
                .read(URIS.into(), LIMIT, Duration::from_secs(5))
                .await?;
            // A withdrawn native source can still produce a phantom Drop with
            // an empty pipe. It ends the gesture, without an operation error.
            if bytes.is_empty() {
                return Ok(None);
            }
            let paths = Selection::decode(URIS, &bytes, None)
                .map_err(|error| error.to_string())?
                .paths()
                .to_vec();
            Ok(Some(Arc::from(paths)))
        },
        move |result| Message::Drag(Event::Bytes(id, final_read, result)),
    )
}
async fn receive<T: Send + 'static>(
    receiver: futures_channel::oneshot::Receiver<Result<T, String>>,
) -> Result<T, String> {
    receiver
        .await
        .unwrap_or_else(|_| Err(fl!("files-drag-worker-stopped")))
}

impl State {
    fn event(&mut self, desktop: &mut Desktop, event: Event) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        match event {
            Event::Enabled(id, endpoint) => {
                if let Some(host) = self.hosts.get_mut(&id) {
                    host.notices = endpoint
                        .as_ref()
                        .and_then(native::DragDrop::take_notices)
                        .map(activation_ui::Notices::new);
                    host.endpoint = endpoint;
                }
            }
            Event::Changed(id) => {
                if !self.hosts.contains_key(&id) {
                    return tasks;
                }
            }
            Event::Press(id, row) => {
                self.cancel_pointer();
                self.suppress = None;
                let Some(row) = row.filter(|row| row.context.current(desktop)) else {
                    return tasks;
                };
                if !allowed(desktop, id)
                    || self
                        .hosts
                        .get(&id)
                        .is_none_or(|host| host.endpoint.is_none())
                {
                    return tasks;
                }
                let Some(service) = desktop.file_operations.service.clone() else {
                    return tasks;
                };
                let mode = desktop
                    .windows
                    .get(&id)
                    .map_or(SelectionMode::Replace, |window| {
                        selection_mode(window.modifiers)
                    });
                let Some(browser) = desktop.active_mut(id).and_then(|tab| tab.browser.as_mut())
                else {
                    return tasks;
                };
                let selected = browser.selected().contains(&row.path);
                let current = browser.lists(&row.path);
                if !selected && current {
                    browser.select(&row.path, mode);
                }
                let Some(origin) = file_operations::Origin::capture(desktop, id) else {
                    return tasks;
                };
                let paths = if selected || current {
                    origin.selected.clone()
                } else {
                    vec![row.path.clone()]
                };
                if paths.is_empty() {
                    return tasks;
                }
                self.serial += 1;
                let serial = self.serial;
                let cancellation = Cancellation::default();
                let error = match service.prepare_drag(paths, cancellation.clone()) {
                    Ok(receiver) => {
                        self.pending += 1;
                        tasks.push(Task::perform(receive(receiver), move |result| {
                            Message::Drag(Event::Prepared(serial, result))
                        }));
                        None
                    }
                    Err(error) => Some(error.to_string()),
                };
                self.pointer = Some(Gesture {
                    serial,
                    origin,
                    path: row.path.clone(),
                    mode,
                    cancellation,
                    prepared: None,
                    native: None,
                    requested: false,
                    deferred: selected || !current,
                    error,
                });
                self.suppress = Some((id, row.path));
            }
            Event::NativePress(id) => {
                // The global listener includes overlay-captured client presses.
                let native = self
                    .hosts
                    .get(&id)
                    .and_then(|host| host.endpoint.as_ref())
                    .and_then(native::DragDrop::capture_origin);
                if let Some(gesture) = self
                    .pointer
                    .as_mut()
                    .filter(|gesture| gesture.origin.window == id)
                {
                    gesture.native = native;
                } else if let Some(gesture) = self
                    .tab_gesture
                    .as_mut()
                    .filter(|gesture| gesture.window == id && gesture.native.is_none())
                {
                    gesture.native = native;
                }
            }

            Event::Start(id) => {
                if let Some(gesture) = self
                    .pointer
                    .as_mut()
                    .filter(|gesture| gesture.origin.window == id)
                {
                    gesture.requested = true;
                }
            }
            Event::Release(id) => {
                if self
                    .tab_gesture
                    .as_ref()
                    .is_some_and(|gesture| gesture.window == id)
                {
                    self.tab_gesture = None;
                }
                if self
                    .pointer
                    .as_ref()
                    .is_some_and(|gesture| gesture.origin.window == id)
                {
                    if let Some(gesture) = self.pointer.as_ref().filter(|gesture| {
                        !gesture.requested && gesture.deferred && gesture.origin.current(desktop)
                    }) {
                        let path = gesture.path.clone();
                        let mode = gesture.mode;
                        if let Some(browser) =
                            desktop.active_mut(id).and_then(|tab| tab.browser.as_mut())
                        {
                            if browser.lists(&path) {
                                browser.select(&path, mode);
                            } else {
                                tasks.push(desktop.update_browser(Message::Select(id, path)));
                            }
                        }
                    }
                    self.cancel_pointer();
                }
            }
            Event::Prepared(serial, result) => {
                self.pending = self.pending.saturating_sub(1);
                match result {
                    Ok(drag) => {
                        if let Some(pointer) = self.pointer.as_mut().filter(|pointer| {
                            pointer.serial == serial && pointer.origin.current(desktop)
                        }) {
                            pointer.prepared = Some(drag);
                        } else {
                            drag.cancel();
                        }
                    }
                    Err(error) => {
                        if let Some(pointer) = self
                            .pointer
                            .as_mut()
                            .filter(|pointer| pointer.serial == serial)
                        {
                            pointer.error = Some(error);
                        }
                    }
                }
            }
            Event::Bytes(id, final_read, result) => {
                self.pending = self.pending.saturating_sub(1);
                if let Some(incoming) = self.incoming.get_mut(&id) {
                    match result {
                        Ok(None) => {
                            incoming.cancellation.cancel();
                            incoming.choice = None;
                        }
                        Ok(Some(paths)) if final_read => {
                            if incoming.paths.as_deref() == Some(paths.as_ref()) {
                                incoming.confirmed = true;
                            } else {
                                incoming.choice = None;
                                desktop.file_operations.error = Some(fl!("files-drag-changed"));
                            }
                        }
                        Ok(Some(paths)) => {
                            incoming.paths = Some(paths.clone());
                            if let Some(service) = &desktop.file_operations.service {
                                match service
                                    .prepare_drag(paths.to_vec(), incoming.cancellation.clone())
                                {
                                    Ok(receiver) => {
                                        self.pending += 1;
                                        tasks.push(Task::perform(
                                            receive(receiver),
                                            move |result| {
                                                Message::Drag(Event::ForeignPrepared(id, result))
                                            },
                                        ));
                                    }
                                    Err(error) => {
                                        incoming.cancellation.cancel();
                                        desktop.file_operations.error = Some(error.to_string());
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            incoming.cancellation.cancel();
                            incoming.choice = None;
                            desktop.file_operations.error =
                                Some(fl!("files-drag-read-failed", error = error.to_string()));
                        }
                    }
                }
            }
            Event::ForeignPrepared(id, result) => {
                self.pending = self.pending.saturating_sub(1);
                match result {
                    Ok(drag) => {
                        if let Some(incoming) = self.incoming.get_mut(&id) {
                            incoming.drag = Some(drag);
                        } else {
                            drag.cancel();
                        }
                    }
                    Err(error) => {
                        if let Some(incoming) = self.incoming.get_mut(&id) {
                            incoming.cancellation.cancel();
                            desktop.file_operations.error = Some(error);
                        }
                    }
                }
            }
            Event::HoverPrepared(id, serial, result) => {
                self.pending = self.pending.saturating_sub(1);
                if let Some(incoming) = self.incoming.get_mut(&id).filter(|incoming| {
                    incoming.hover_serial == serial && !incoming.hover_cancel.is_cancelled()
                }) {
                    incoming.hover_pending = false;
                    match result {
                        Ok(hover) => incoming.hover = Some(hover),
                        Err(_) => {
                            incoming.hover_cancel.cancel();
                        }
                    }
                }
            }
            event @ (Event::TabPress(..) | Event::TabStart(..) | Event::TabPicture(..)) => {
                tasks.extend(self.tab_event(desktop, event));
            }
            Event::Admitted(id, submission, result) => {
                if let Some(incoming) = self.incoming.get_mut(&id) {
                    incoming.awaiting = false;
                    incoming.job = result
                        .as_ref()
                        .and_then(|result| result.as_ref().ok())
                        .copied();
                    if incoming.job.is_none() {
                        let incoming = self.incoming.remove(&id).unwrap();
                        self.retire(incoming, false);
                    }
                }
                tasks.push(
                    desktop
                        .file_operation_event(file_operations::Event::Admitted(submission, result)),
                );
            }
        }
        tasks
    }
    fn poll(&mut self, desktop: &mut Desktop) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        if let Some(error) = self
            .pointer
            .as_ref()
            .filter(|gesture| gesture.requested)
            .and_then(|gesture| gesture.error.clone())
        {
            desktop.file_operations.error = Some(error);
            self.cancel_pointer();
        }
        if self.pointer.as_ref().is_some_and(|gesture| {
            !gesture.origin.current(desktop) || !allowed(desktop, gesture.origin.window)
        }) {
            self.cancel_pointer();
        }
        if self.pointer.as_ref().is_some_and(|gesture| {
            gesture.requested && gesture.prepared.is_some() && gesture.native.is_some()
        }) {
            let gesture = self.pointer.take().unwrap();
            let prepared = gesture.prepared.unwrap();
            let endpoint = self
                .hosts
                .get(&gesture.origin.window)
                .and_then(|host| host.endpoint.as_ref());
            let content = Selection::new(prepared.paths().to_vec(), TransferKind::Copy)
                .and_then(|selection| selection.encode(URIS));
            match endpoint
                .zip(content.ok())
                .ok_or_else(|| fl!("files-drag-origin-unavailable"))
                .and_then(|(endpoint, bytes)| {
                    endpoint.start_with_icon(
                        gesture.native.unwrap(),
                        vec![native::Content {
                            mime: URIS.into(),
                            bytes: bytes.into(),
                        }],
                        native::Actions {
                            copy: true,
                            move_files: true,
                        },
                        drag_icon(desktop, gesture.origin.window, prepared.paths()),
                    )
                }) {
                Ok(source) => {
                    self.outgoing.insert(
                        source.snapshot().id,
                        Outgoing {
                            source,
                            prepared,
                            owner: gesture.origin.window,
                        },
                    );
                }
                Err(error) => {
                    prepared.cancel();
                    desktop.file_operations.error = Some(error);
                }
            }
        }
        let mut tab_offers = Vec::new();
        for (&owner, host) in &self.hosts {
            if let Some(endpoint) = &host.endpoint {
                for offer in endpoint.offers() {
                    let snap = offer.snapshot();
                    if self.incoming.contains_key(&snap.id)
                        || self.rejected.contains_key(&snap.id)
                        || snap.finished
                    {
                        continue;
                    }
                    if host.live && offer.has_format(tab_drag::MIME) {
                        tab_offers.push((owner, offer.clone()));
                        continue;
                    }
                    if !host.live || !offer.has_format(URIS) || self.incoming.len() >= 32 {
                        if snap.dropped {
                            let _ = offer.finish(false);
                        } else {
                            let _ = offer.accept(None, native::Actions::default(), None);
                        }
                        continue;
                    }
                    let local = snap.local_source;
                    let drag = local
                        .and_then(|source| self.outgoing.get(&source))
                        .map(|source| source.prepared.clone());
                    if local.is_some() && drag.is_none() {
                        let _ = offer.accept(None, native::Actions::default(), None);
                        continue;
                    }
                    let paths = drag.as_ref().map(|drag| Arc::from(drag.paths()));
                    let incoming = Incoming {
                        offer: offer.clone(),
                        owner,
                        local,
                        paths,
                        drag,
                        cancellation: Cancellation::default(),
                        target: None,
                        bounds: None,
                        hover: None,
                        hover_cancel: Cancellation::default(),
                        hover_serial: 0,
                        hover_pending: false,
                        choice: None,
                        confirmed: local.is_some(),
                        dropped: false,
                        awaiting: false,
                        job: None,
                        _work: None,
                    };
                    self.incoming.insert(snap.id, incoming);
                    if local.is_none() {
                        self.pending += 1;
                        tasks.push(paths_task(snap.id, offer, false));
                    }
                }
            }
        }
        tasks.extend(self.poll_tabs(desktop, tab_offers));
        for source in self.outgoing.values() {
            let snapshot = source.source.snapshot();
            if snapshot.finished && !snapshot.dropped {
                source.prepared.cancel();
            }
        }
        let preference = self.preference();
        let ids: Vec<_> = self.incoming.keys().copied().collect();
        for id in ids {
            let mut incoming = self.incoming.remove(&id).unwrap();
            let snap = incoming.offer.snapshot();
            if snap.finished
                || (!snap.dropped
                    && (snap.left
                        || !self
                            .hosts
                            .get(&incoming.owner)
                            .is_some_and(|host| host.live)))
                || incoming.cancellation.is_cancelled()
            {
                self.retire(incoming, false);
                continue;
            }
            let hit = self
                .hosts
                .get(&incoming.owner)
                .and_then(|host| {
                    host.scene
                        .target(Point::new(snap.position.0 as f32, snap.position.1 as f32))
                })
                .filter(|(target, _)| target.current(desktop) && allowed(desktop, incoming.owner));
            if !incoming.dropped && !snap.dropped {
                incoming.bounds = hit.as_ref().map(|(_, bounds)| *bounds);
                if incoming.target.as_ref() != hit.as_ref().map(|(target, _)| target) {
                    incoming.hover_cancel.cancel();
                    incoming.hover_cancel = Cancellation::default();
                    incoming.hover = None;
                    incoming.hover_serial += 1;
                    incoming.target = hit.as_ref().map(|(target, _)| target.clone());
                    incoming.bounds = hit.as_ref().map(|(_, bounds)| *bounds);
                    incoming.hover_pending = false;
                }
                if incoming.hover.is_none()
                    && !incoming.hover_pending
                    && !incoming.hover_cancel.is_cancelled()
                    && let (Some(target), Some(drag), Some(service)) = (
                        &incoming.target,
                        &incoming.drag,
                        &desktop.file_operations.service,
                    )
                {
                    incoming.hover_pending = true;
                    match service.prepare_drop(
                        drag.clone(),
                        target.core(),
                        incoming.hover_cancel.clone(),
                    ) {
                        Ok(receiver) => {
                            self.pending += 1;
                            let serial = incoming.hover_serial;
                            tasks.push(Task::perform(receive(receiver), move |result| {
                                Message::Drag(Event::HoverPrepared(id, serial, result))
                            }));
                        }
                        Err(error) => {
                            incoming.hover_cancel.cancel();
                            incoming.hover_pending = false;
                            desktop.file_operations.error = Some(error.to_string());
                        }
                    }
                }
                let choice = incoming.hover.as_ref().and_then(|hover| {
                    hover.choose(
                        preference,
                        DropActions {
                            copy: snap.actions.copy,
                            move_files: snap.actions.move_files,
                        },
                    )
                });
                let native_action = choice.as_ref().map(|choice| match choice.kind() {
                    TransferKind::Copy => native::Action::Copy,
                    TransferKind::Move => native::Action::Move,
                });
                let actions = match preference {
                    DropPreference::Copy => native::Actions {
                        copy: snap.actions.copy,
                        move_files: false,
                    },
                    DropPreference::Move => native::Actions {
                        copy: false,
                        move_files: snap.actions.move_files,
                    },
                    DropPreference::Automatic => snap.actions,
                };
                let _ = incoming.offer.accept(
                    native_action.map(|_| URIS.into()),
                    if native_action.is_some() {
                        actions
                    } else {
                        native::Actions::default()
                    },
                    native_action,
                );
            }
            if snap.dropped && !incoming.dropped {
                incoming.dropped = true;
                incoming._work = desktop
                    .activity
                    .as_ref()
                    .map(crate::local_activity::LocalActivity::work);
                let final_kind = match snap.selected {
                    Some(native::Action::Copy) => Some((DropPreference::Copy, TransferKind::Copy)),
                    Some(native::Action::Move) => Some((DropPreference::Move, TransferKind::Move)),
                    None => None,
                };
                let valid = incoming.target.as_ref() == hit.as_ref().map(|(target, _)| target)
                    && incoming.target.is_some();
                incoming.choice = if valid {
                    final_kind.and_then(|(force, kind)| {
                        let forced = match preference {
                            DropPreference::Copy => kind == TransferKind::Copy,
                            DropPreference::Move => kind == TransferKind::Move,
                            DropPreference::Automatic => true,
                        };
                        forced
                            .then(|| {
                                incoming.hover.as_ref().and_then(|hover| {
                                    hover.choose(
                                        force,
                                        DropActions {
                                            copy: snap.actions.copy,
                                            move_files: snap.actions.move_files,
                                        },
                                    )
                                })
                            })
                            .flatten()
                    })
                } else {
                    None
                };
                if incoming.choice.is_none() {
                    self.retire(incoming, false);
                    continue;
                }
                if incoming.local.is_none() {
                    self.pending += 1;
                    tasks.push(paths_task(id, incoming.offer.clone(), true));
                }
            }
            if incoming.dropped
                && incoming.choice.is_none()
                && !incoming.awaiting
                && incoming.job.is_none()
            {
                self.retire(incoming, false);
                continue;
            }
            if incoming.dropped
                && incoming.confirmed
                && !incoming.awaiting
                && incoming.job.is_none()
            {
                let choice = incoming.choice.take().unwrap();
                incoming.awaiting = true;
                let submission = file_operations::Submission {
                    origin: None,
                    owner: incoming.owner,
                    dialog: None,
                    affected: incoming
                        .paths
                        .as_ref()
                        .map_or_else(Vec::new, |paths| paths.to_vec()),
                    reveal: None,
                    refresh_all: true,
                };
                tasks.push(desktop.submit_drag_drop(id, choice, submission));
            }
            let backend = desktop
                .file_operations
                .service
                .as_ref()
                .map(operation_jobs::OperationJobs::snapshot);
            if incoming.job.is_some()
                && backend.as_ref().is_some_and(|snapshot| {
                    !snapshot.jobs.iter().any(|job| Some(job.id) == incoming.job)
                })
            {
                desktop.file_operations.error = Some(fl!("files-drop-status-gone"));
                self.retire(incoming, false);
                continue;
            }
            if let Some(job) = incoming
                .job
                .and_then(|job| {
                    backend.as_ref().and_then(|snapshot| {
                        snapshot.jobs.iter().find(|snapshot| snapshot.id == job)
                    })
                })
                .filter(|job| {
                    matches!(
                        job.phase,
                        operation_jobs::Phase::Finished
                            | operation_jobs::Phase::Failed
                            | operation_jobs::Phase::Stopped
                            | operation_jobs::Phase::Interrupted
                            | operation_jobs::Phase::StartSaveFailed
                            | operation_jobs::Phase::SaveFailed
                    )
                })
            {
                let success = job.phase == operation_jobs::Phase::Finished
                    && job.transfer.as_ref().is_some_and(|report| {
                        !report.cancelled
                            && report.failures.is_empty()
                            && report.skipped.is_empty()
                            && report.remaining.is_empty()
                    });
                self.retire(incoming, success);
                continue;
            }
            self.incoming.insert(id, incoming);
        }
        let retired: Vec<_> = self
            .outgoing
            .iter()
            .filter(|(id, source)| {
                source.source.snapshot().finished
                    && !self
                        .incoming
                        .values()
                        .any(|incoming| incoming.local == Some(**id))
            })
            .map(|(&id, _)| id)
            .collect();
        for id in retired {
            if let Some(source) = self.outgoing.remove(&id) {
                source.prepared.cancel();
            }
        }
        let live_offers: std::collections::BTreeSet<_> = self
            .hosts
            .values()
            .filter(|host| host.live)
            .filter_map(|host| host.endpoint.as_ref())
            .flat_map(native::DragDrop::offers)
            .map(|offer| offer.snapshot().id)
            .collect();
        self.rejected.retain(|id, _| live_offers.contains(id));
        self.hosts.retain(|id, host| {
            host.live
                || self.outgoing.values().any(|source| source.owner == *id)
                || self.incoming.values().any(|incoming| incoming.owner == *id)
        });
        tasks
    }
}

impl Desktop {
    pub(super) fn drag_event(&mut self, event: Event) -> Task<Message> {
        let mut state = std::mem::take(&mut self.drag);
        let mut tasks = state.event(self, event);
        tasks.extend(state.poll(self));
        self.drag = state;
        tasks.push(self.exit_if_empty());
        Task::batch(tasks)
    }
    pub(super) fn poll_drag(&mut self) -> Task<Message> {
        let mut state = std::mem::take(&mut self.drag);
        let tasks = state.poll(self);
        self.drag = state;
        Task::batch(tasks)
    }
    pub(super) fn cancel_drag(&mut self) -> bool {
        let tab = self.drag.cancel_tab_drag();
        let active = tab
            || self.drag.pointer.is_some()
            || self.drag.outgoing.values().any(|source| {
                !source.source.snapshot().dropped && !source.source.snapshot().finished
            });
        self.drag.cancel_pointer();
        for source in self.drag.outgoing.values() {
            if !source.source.snapshot().dropped {
                source.prepared.cancel();
                source.source.cancel();
            }
        }
        active
    }
}

/// The picture under the pointer while files are dragged out of `window`: the
/// first dragged item's icon or thumbnail, with up to two more stacked behind
/// it for a larger selection. Drawn at twice the logical size for sharp output
/// on scaled displays.
fn drag_icon(desktop: &Desktop, window: window::Id, paths: &[PathBuf]) -> Option<native::Icon> {
    const SCALE: u32 = 2;
    const SIDE: u32 = 48 * SCALE;
    const STEP: u32 = 7 * SCALE;
    let browser = desktop.active(window)?.browser.as_ref()?;
    let entries: Vec<_> = browser
        .visible_entries()
        .into_iter()
        .filter(|entry| paths.contains(&entry.path))
        .take(3)
        .collect();
    if entries.is_empty() {
        return None;
    }
    let layers = entries.len() as u32;
    let side = SIDE + STEP * (layers - 1);
    let mut canvas = ::image::RgbaImage::new(side, side);
    let mut drawn = false;
    // The first item is in front, at the top left; the others peek out behind.
    for (index, entry) in entries.iter().enumerate().rev() {
        let Some((width, height, pixels)) = desktop.icons.picture(entry) else {
            continue;
        };
        let Some(source) = ::image::RgbaImage::from_raw(width, height, pixels.to_vec()) else {
            continue;
        };
        let fit = (f64::from(SIDE) / f64::from(width.max(height))).min(4.0);
        let (fitted_width, fitted_height) = (
            ((f64::from(width) * fit).round() as u32).clamp(1, SIDE),
            ((f64::from(height) * fit).round() as u32).clamp(1, SIDE),
        );
        let fitted = ::image::imageops::resize(
            &source,
            fitted_width,
            fitted_height,
            ::image::imageops::FilterType::Triangle,
        );
        let offset = STEP * index as u32;
        drawn = true;
        ::image::imageops::overlay(
            &mut canvas,
            &fitted,
            i64::from(offset + (SIDE - fitted_width) / 2),
            i64::from(offset + (SIDE - fitted_height) / 2),
        );
    }
    if !drawn {
        return None;
    }
    // Slightly see-through, so the drop target shows under it.
    for pixel in canvas.pixels_mut() {
        pixel[3] = (u16::from(pixel[3]) * 220 / 255) as u8;
    }
    let logical = (side / SCALE) as i32;
    Some(native::Icon {
        width: side,
        height: side,
        scale: SCALE,
        rgba: Arc::from(canvas.into_raw()),
        hotspot: (logical / 2, logical / 2),
    })
}

pub(super) fn row_region(
    desktop: &Desktop,
    id: window::Id,
    tab: &Tab,
    entry: &crate::browse::Entry,
    column: &Path,
) -> Option<(Scene, Region)> {
    let scene = desktop.drag.scene(id)?;
    let context = Context::new(id, tab);
    let binding = directory_binding(tab, column);
    Some((
        scene,
        Region {
            source: Some(Row {
                context: context.clone(),
                path: entry.path.clone(),
            }),
            target: entry.is_directory().then(|| Target {
                context,
                directory: entry.path.clone(),
                binding,
                priority: 3,
                device: None,
            }),
        },
    ))
}
fn directory_binding(tab: &Tab, path: &Path) -> Option<(PathBuf, PathBuf)> {
    let browser = tab.browser.as_ref()?;
    let directory = if path == browser.path() {
        Some(browser.directory())
    } else {
        tab.columns
            .get(path)
            .and_then(|snapshot| snapshot.result.as_ref())
            .and_then(|result| result.as_ref().ok())
    }?;
    Some((directory.path.clone(), directory.resolved_path.clone()))
}
pub(super) fn target_region<'a>(
    desktop: &Desktop,
    id: window::Id,
    tab: &Tab,
    path: PathBuf,
    binding: Option<(PathBuf, PathBuf)>,
    priority: u8,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    match desktop.drag.scene(id) {
        Some(scene) => super::drag_geometry::mark(
            scene,
            Region {
                source: None,
                target: Some(Target {
                    context: Context::new(id, tab),
                    directory: path,
                    binding,
                    priority,
                    device: None,
                }),
            },
            content,
        ),
        None => content,
    }
}
pub(super) fn column_binding(tab: &Tab, path: &Path) -> Option<(PathBuf, PathBuf)> {
    directory_binding(tab, path)
}

pub(super) fn device_region<'a>(
    desktop: &Desktop,
    id: window::Id,
    token: crate::local_volumes::DeviceToken,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let target = desktop
        .device_runtime()
        .and_then(|runtime| runtime.device_requests().root(token).ok())
        .zip(desktop.active(id));
    match (desktop.drag.scene(id), target) {
        (Some(scene), Some((root, tab))) => super::drag_geometry::mark(
            scene,
            Region {
                source: None,
                target: Some(Target {
                    context: Context::new(id, tab),
                    directory: root,
                    binding: None,
                    priority: 2,
                    device: Some(token),
                }),
            },
            content,
        ),
        _ => content,
    }
}
