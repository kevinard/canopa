//! Source-owned floating previews. Preparation never runs on the UI thread.
use super::*;
use crate::preview::{PreparedPreview, PreviewContent, PreviewRequest};
use iced::futures::channel::oneshot;
use iced::{
    Length,
    widget::{column, container, image, row, scrollable, text, text_editor},
};

const MINIMUM_ZOOM: f32 = 0.1;
const MAXIMUM_ZOOM: f32 = 8.0;

#[derive(Debug)]
pub(super) struct Prepared {
    preview: PreparedPreview,
    highlighting: preview_highlights::Prepared,
}
impl Prepared {
    fn new(preview: PreparedPreview, cancellation: &Cancellation) -> io::Result<Self> {
        let highlighting = preview_highlights::Prepared::new(&preview.content, cancellation)?;
        Ok(Self {
            preview,
            highlighting,
        })
    }
}

fn insert_text(content: &mut text_editor::Content, text: &str) {
    // The locked editor inserts multiline pastes in reverse, shifting the line
    // array for each line. Append small groups to keep initialization linear.
    // Use its own line iterator so paired CR/LF endings stay intact at a boundary.
    let mut start = 0;
    for (index, (range, ending)) in
        iced::advanced::graphics::text::cosmic_text::LineIter::new(text).enumerate()
    {
        if (index + 1) % 128 == 0 {
            let end = range.end + ending.as_str().len();
            content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(text[start..end].to_owned()),
            )));
            start = end;
        }
    }
    if start < text.len() {
        content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
            Arc::new(text[start..].to_owned()),
        )));
    }
    content.move_to(text_editor::Cursor {
        position: text_editor::Position { line: 0, column: 0 },
        selection: None,
    });
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TargetContext {
    Browser { tab: u64, navigation: u64 },
    Trash(crate::trash::PayloadVersion),
    Recovery(crate::operations::RecoveryVersion),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    context: TargetContext,
    path: PathBuf,
    name_hint: Option<std::ffi::OsString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Open,
    ZoomOut,
    Fit,
    Actual,
    ZoomIn,
    Previous,
    Next,
    Play,
    Seek,
    Mute,
    Volume,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Opened(window::Id, Option<window::Id>),
    Ready(window::Id, u64, Option<Arc<Result<Prepared, String>>>),
    Activate(window::Id, u64, Control),
    Viewport(window::Id, u64, Size),
    TextLaidOut(window::Id, u64),
    Text(window::Id, u64, text_editor::Action),
    Media(window::Id, u64, media_ui::Event),
}

pub(super) struct PreviewWindow {
    pub source: window::Id,
    target: Option<Target>,
    revision: u64,
    cancellation: Cancellation,
    worker: Option<Worker>,
    result: Option<Arc<Result<Prepared, String>>>,
    bitmap: Option<image::Handle>,
    text: Option<text_editor::Content>,
    pending_text: Option<Arc<String>>,
    loading: bool,
    page: u32,
    zoom: Option<f32>,
    focused: Option<Control>,
    viewport: Size,
    media: Option<media_ui::State>,
}

impl Drop for PreviewWindow {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

struct Job {
    request: PreviewRequest,
    cancellation: Cancellation,
    sender: oneshot::Sender<Arc<Result<Prepared, String>>>,
}

#[derive(Default)]
struct WorkerState {
    pending: Option<Job>,
    active: Option<Cancellation>,
    stopped: bool,
}

/// One reader and one replaceable pending preparation per preview window.
struct Worker {
    state: Arc<(std::sync::Mutex<WorkerState>, std::sync::Condvar)>,
}

impl Worker {
    fn new() -> io::Result<Self> {
        let state = Arc::new((
            std::sync::Mutex::new(WorkerState::default()),
            std::sync::Condvar::new(),
        ));
        let background = state.clone();
        std::thread::Builder::new()
            .name("files-preview".into())
            .spawn(move || {
                loop {
                    let job = {
                        let (lock, available) = &*background;
                        let mut state = lock
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        while state.pending.is_none() && !state.stopped {
                            state = available
                                .wait(state)
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                        }
                        if state.stopped {
                            return;
                        }
                        let job = state.pending.take().unwrap();
                        state.active = Some(job.cancellation.clone());
                        job
                    };
                    let prepared = job
                        .request
                        .prepare(&job.cancellation)
                        .and_then(|preview| Prepared::new(preview, &job.cancellation))
                        .map_err(|error| error.to_string());
                    let _ = job.sender.send(Arc::new(prepared));
                    background
                        .0
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .active = None;
                }
            })?;
        Ok(Self { state })
    }

    fn submit(
        &self,
        request: PreviewRequest,
        cancellation: Cancellation,
    ) -> oneshot::Receiver<Arc<Result<Prepared, String>>> {
        let (sender, receiver) = oneshot::channel();
        let (lock, available) = &*self.state;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(active) = &state.active {
            active.cancel();
        }
        if let Some(old) = state.pending.replace(Job {
            request,
            cancellation,
            sender,
        }) {
            old.cancellation.cancel();
        }
        available.notify_one();
        receiver
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, available) = &*self.state;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.stopped = true;
        if let Some(active) = &state.active {
            active.cancel();
        }
        if let Some(pending) = state.pending.take() {
            pending.cancellation.cancel();
        }
        available.notify_one();
    }
}

fn root_id(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-preview-{id:?}"))
}

impl PreviewWindow {
    fn scale(&self) -> f32 {
        self.zoom.unwrap_or_else(|| match self.content() {
            Some(PreviewContent::Image(bitmap) | PreviewContent::Pdf { bitmap, .. }) => {
                (self.viewport.width / bitmap.width.max(1) as f32)
                    .min(self.viewport.height / bitmap.height.max(1) as f32)
            }
            _ => 1.0,
        })
    }

    fn content(&self) -> Option<&PreviewContent> {
        self.result
            .as_deref()
            .and_then(|result| result.as_ref().ok())
            .map(|prepared| &prepared.preview.content)
    }

    fn controls(&self) -> Vec<Control> {
        let mut controls = if self.target.is_some() {
            vec![Control::Open]
        } else {
            vec![]
        };
        if self.bitmap.is_some() {
            if self.scale() > MINIMUM_ZOOM {
                controls.push(Control::ZoomOut);
            }
            controls.extend([Control::Fit, Control::Actual]);
            if self.scale() < MAXIMUM_ZOOM {
                controls.push(Control::ZoomIn);
            }
        }
        if let Some(PreviewContent::Pdf { page, pages, .. }) = self.content() {
            if *page > 1 {
                controls.push(Control::Previous);
            }
            if page < pages {
                controls.push(Control::Next);
            }
        }
        if let Some(media) = &self.media {
            controls.extend(media.controls());
        }
        controls
    }

    fn request(&mut self, id: window::Id) -> Task<Message> {
        self.media = None;
        self.cancellation.cancel();
        self.cancellation = Cancellation::default();
        self.revision += 1;
        let revision = self.revision;
        let Some(target) = &self.target else {
            self.loading = false;
            return Task::none();
        };
        let worker = match &self.worker {
            Some(worker) => worker,
            None => match Worker::new() {
                Ok(worker) => {
                    self.worker = Some(worker);
                    self.worker.as_ref().unwrap()
                }
                Err(error) => {
                    self.loading = false;
                    self.result = Some(Arc::new(Err(error.to_string())));
                    return Task::none();
                }
            },
        };
        let mut request = PreviewRequest::new(&target.path);
        request.page = self.page;
        request.name_hint = target.name_hint.clone();
        let receiver = worker.submit(request, self.cancellation.clone());
        self.loading = true;
        Task::perform(async move { receiver.await.ok() }, move |result| {
            Message::Preview(Event::Ready(id, revision, result))
        })
    }
}

impl Desktop {
    pub(super) fn media_poll(&mut self) -> Task<Message> {
        for preview in self.previews.values_mut() {
            if let Some(media) = &mut preview.media {
                media.refresh();
            }
            if preview
                .focused
                .is_some_and(|control| !preview.controls().contains(&control))
            {
                preview.focused = None;
            }
        }
        // The final worker notice arrives after its real decoder/reader teardown.
        self.exit_if_empty()
    }

    fn preview_source_exists(&self, source: window::Id) -> bool {
        self.windows.contains_key(&source)
            || self
                .trash_window
                .as_ref()
                .is_some_and(|trash| trash.window == source)
            || self
                .recovery_window
                .as_ref()
                .is_some_and(|w| w.window == source)
    }

    /// User dismissal returns to its live owner only while this preview owns
    /// keyboard focus. Queue activation before retiring its input surface.
    pub(super) fn dismiss_preview(&mut self, id: window::Id) -> Task<Message> {
        let Some(preview) = self.previews.remove(&id) else {
            return Task::none();
        };
        let focus = if self.focused_window == Some(id) && self.preview_source_exists(preview.source)
        {
            window::gain_focus(preview.source)
        } else {
            Task::none()
        };
        focus.chain(window::close(id))
    }

    pub(super) fn toggle_preview(&mut self, source: window::Id) -> Task<Message> {
        if let Some(id) = self
            .previews
            .iter()
            .find_map(|(id, preview)| (preview.source == source).then_some(*id))
        {
            return self.dismiss_preview(id);
        }
        if !self.preview_source_exists(source) {
            return Task::none();
        }
        let settings = window::Settings {
            size: Size::new(760.0, 600.0),
            min_size: Some(Size::new(320.0, 240.0)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let (id, opened) = window::open_transient(source, settings);
        self.previews.insert(
            id,
            PreviewWindow {
                source,
                target: None,
                revision: 0,
                cancellation: Cancellation::default(),
                worker: None,
                result: None,
                bitmap: None,
                text: None,
                pending_text: None,
                loading: false,
                page: 1,
                zoom: None,
                focused: None,
                viewport: Size::new(760.0, 500.0),
                media: None,
            },
        );
        opened.map(move |opened| Message::Preview(Event::Opened(id, opened)))
    }

    pub(super) fn sync_previews(&mut self) -> Task<Message> {
        let changes: Vec<_> = self
            .previews
            .iter()
            .map(|(id, preview)| {
                let target = self
                    .active(preview.source)
                    .filter(|tab| !tab.loading)
                    .and_then(|tab| {
                        let browser = tab.browser.as_ref()?;
                        let path = browser
                            .focused()
                            .filter(|path| browser.selected().contains(*path))
                            .map(Path::to_owned)
                            .or_else(|| browser.selected().first().cloned())?;
                        Some(Target {
                            context: TargetContext::Browser {
                                tab: tab.id,
                                navigation: tab.revision,
                            },
                            path,
                            name_hint: None,
                        })
                    })
                    .or_else(|| {
                        let trash = self
                            .trash_window
                            .as_ref()
                            .filter(|trash| trash.window == preview.source)?;
                        let item = trash.selected()?;
                        Some(Target {
                            context: TargetContext::Trash(item.version()),
                            path: item.path.clone(),
                            name_hint: Some(item.name().to_owned()),
                        })
                    })
                    .or_else(|| {
                        let recovery = self
                            .recovery_window
                            .as_ref()
                            .filter(|w| w.window == preview.source)?;
                        let item = recovery.selected()?;
                        Some(Target {
                            context: TargetContext::Recovery(item.version()?),
                            path: item.entry.as_ref()?.path.clone(),
                            name_hint: Some(item.name().to_owned()),
                        })
                    });
                (*id, target)
            })
            .collect();
        let mut tasks = Vec::new();
        for (id, target) in changes {
            let preview = self.previews.get_mut(&id).unwrap();
            if preview.target != target {
                preview.target = target;
                preview.result = None;
                preview.bitmap = None;
                preview.text = None;
                preview.pending_text = None;
                preview.page = 1;
                preview.zoom = None;
                preview.focused = None;
                tasks.push(preview.request(id));
            }
        }
        Task::batch(tasks)
    }

    pub(super) fn close_source_previews(&mut self, source: window::Id) -> Task<Message> {
        let ids: Vec<_> = self
            .previews
            .iter()
            .filter_map(|(id, preview)| (preview.source == source).then_some(*id))
            .collect();
        Task::batch(ids.into_iter().map(|id| {
            self.previews.remove(&id);
            window::close(id)
        }))
    }

    pub(super) fn preview_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Opened(id, opened) => {
                let source = self.previews.get(&id).map(|preview| preview.source);
                if opened.is_none() {
                    self.previews.remove(&id);
                    if let Some(tab) = source.and_then(|source| self.active_mut(source)) {
                        tab.error =
                            Some("Could not open a floating preview for this browser".into());
                    } else if let Some(trash) = self
                        .trash_window
                        .as_mut()
                        .filter(|trash| Some(trash.window) == source)
                    {
                        trash.error = Some("Could not open a floating preview for Trash".into());
                    } else if let Some(recovery) = self
                        .recovery_window
                        .as_mut()
                        .filter(|w| Some(w.window) == source)
                    {
                        recovery.error =
                            Some("Could not open a floating preview for Recovery".into());
                    }
                } else if source.is_none_or(|source| !self.preview_source_exists(source)) {
                    self.previews.remove(&id);
                    return window::close(id);
                }
            }
            Event::Ready(id, revision, result) => {
                let Some(preview) = self
                    .previews
                    .get_mut(&id)
                    .filter(|preview| preview.revision == revision)
                else {
                    return Task::none();
                };
                let result =
                    result.unwrap_or_else(|| Arc::new(Err("Preview preparation stopped".into())));
                preview.loading = false;
                preview.bitmap = None;
                preview.text = None;
                preview.pending_text = None;
                preview.media = None;
                if let Ok(prepared) = &*result {
                    match &prepared.preview.content {
                        PreviewContent::Image(bitmap) | PreviewContent::Pdf { bitmap, .. } => {
                            preview.bitmap = Some(image::Handle::from_rgba(
                                bitmap.width,
                                bitmap.height,
                                iced::advanced::graphics::core::Bytes::from_owner(
                                    bitmap.rgba.clone(),
                                ),
                            ));
                        }
                        PreviewContent::Text { text, .. } => {
                            // with_text shapes the entire buffer before it has a
                            // viewport. Lay out an empty editor first, then insert
                            // the bounded text without shaping invisible lines.
                            preview.text = Some(text_editor::Content::new());
                            preview.pending_text = Some(Arc::new(text.clone()));
                        }
                        PreviewContent::Audio(source) | PreviewContent::Video(source) => {
                            preview.media = Some(self.media.open(source.clone()));
                        }
                        _ => {}
                    }
                }
                preview.result = Some(result);
                if preview
                    .focused
                    .is_some_and(|focused| !preview.controls().contains(&focused))
                {
                    preview.focused = None;
                }
            }
            Event::TextLaidOut(id, revision) => {
                if let Some(preview) = self
                    .previews
                    .get_mut(&id)
                    .filter(|preview| preview.revision == revision)
                    && let Some(text) = preview.pending_text.take()
                    && let Some(content) = &mut preview.text
                {
                    insert_text(content, &text);
                }
            }
            Event::Media(id, revision, event) => {
                if let Some(preview) = self
                    .previews
                    .get_mut(&id)
                    .filter(|p| p.revision == revision)
                    && let Some(media) = &mut preview.media
                    && let Some(control) = media.event(event)
                {
                    preview.focused = Some(control);
                }
            }
            Event::Text(id, revision, action) => {
                if let Some(preview) = self.previews.get_mut(&id).filter(|preview| {
                    preview.revision == revision && preview.pending_text.is_none()
                }) && !action.is_edit()
                    && let Some(content) = &mut preview.text
                {
                    preview.focused = None;
                    content.perform(action);
                }
            }
            Event::Viewport(id, revision, size) => {
                if let Some(preview) = self
                    .previews
                    .get_mut(&id)
                    .filter(|preview| preview.revision == revision)
                {
                    preview.viewport = size;
                }
            }
            Event::Activate(id, revision, control) => {
                let Some(preview) = self
                    .previews
                    .get_mut(&id)
                    .filter(|preview| preview.revision == revision)
                else {
                    return Task::none();
                };
                if !preview.controls().contains(&control) {
                    return Task::none();
                }
                match control {
                    Control::Open => {
                        if let Some(target) = &preview.target {
                            let (source, path) = (preview.source, target.path.clone());
                            return self.open(source, path);
                        }
                    }
                    Control::Fit => preview.zoom = None,
                    Control::Actual => preview.zoom = Some(1.0),
                    Control::ZoomOut => {
                        preview.zoom = Some((preview.scale() / 1.25).max(MINIMUM_ZOOM))
                    }
                    Control::ZoomIn => {
                        preview.zoom = Some((preview.scale() * 1.25).min(MAXIMUM_ZOOM))
                    }
                    Control::Previous | Control::Next => {
                        let Some(PreviewContent::Pdf { pages, .. }) = preview.content() else {
                            return Task::none();
                        };
                        preview.page = if control == Control::Previous {
                            preview.page.saturating_sub(1).max(1)
                        } else {
                            preview.page.saturating_add(1).min(*pages)
                        };
                        return preview.request(id);
                    }
                    Control::Play | Control::Seek | Control::Mute | Control::Volume => {
                        preview.focused = Some(control);
                        if let Some(media) = &mut preview.media {
                            media.activate(control);
                        }
                    }
                }
            }
        }
        Task::none()
    }

    pub(super) fn preview_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
        text: Option<String>,
    ) -> Task<Message> {
        let Some(preview) = self.previews.get_mut(&id) else {
            return Task::none();
        };
        let source = preview.source;
        let revision = preview.revision;
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || (modifiers.control() && key.as_ref() == keyboard::Key::Character("w"))
        {
            return self.dismiss_preview(id);
        }
        if key == keyboard::Key::Named(keyboard::key::Named::Tab) {
            let controls = preview.controls();
            if !controls.is_empty() {
                let index = preview
                    .focused
                    .and_then(|focused| controls.iter().position(|control| *control == focused));
                let next = if modifiers.shift() {
                    index.map_or(controls.len() - 1, |i| {
                        (i + controls.len() - 1) % controls.len()
                    })
                } else {
                    index.map_or(0, |i| (i + 1) % controls.len())
                };
                preview.focused = Some(controls[next]);
            }
            use iced::advanced::widget::{operate, operation};
            return operate(operation::scope(
                root_id(id),
                operation::focusable::unfocus(),
            ));
        }
        let space = key == keyboard::Key::Named(keyboard::key::Named::Space)
            || key.as_ref() == keyboard::Key::Character(" ");
        if let Some(control) = preview.focused
            && let Some(media) = &mut preview.media
            && media.key(control, &key, modifiers.shift())
        {
            return Task::none();
        }
        if space || key == keyboard::Key::Named(keyboard::key::Named::Enter) {
            if let Some(control) = preview.focused {
                return self.preview_event(Event::Activate(id, revision, control));
            }
            if space {
                return self.dismiss_preview(id);
            }
        }
        if matches!(preview.content(), Some(PreviewContent::Pdf { .. })) {
            if key == keyboard::Key::Named(keyboard::key::Named::PageUp) {
                return self.preview_event(Event::Activate(id, revision, Control::Previous));
            }
            if key == keyboard::Key::Named(keyboard::key::Named::PageDown) {
                return self.preview_event(Event::Activate(id, revision, Control::Next));
            }
        }
        if self
            .trash_window
            .as_ref()
            .is_some_and(|trash| trash.window == source)
        {
            self.trash_key(source, key, modifiers)
        } else if self
            .recovery_window
            .as_ref()
            .is_some_and(|w| w.window == source)
        {
            self.recovery_key(source, key, modifiers)
        } else {
            self.key(source, key, modifiers, text)
        }
    }
}

pub(super) fn title(preview: &PreviewWindow) -> String {
    preview.target.as_ref().map_or_else(
        || "Quick Look — Canopa Files".into(),
        |target| {
            format!(
                "Quick Look: {} — Canopa Files",
                target
                    .name_hint
                    .as_deref()
                    .or_else(|| target.path.file_name())
                    .unwrap_or(target.path.as_os_str())
                    .to_string_lossy()
            )
        },
    )
}

pub(super) fn view<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    preview: &'a PreviewWindow,
) -> Element<'a, Message> {
    use canopa_theme::UiKind;
    let appearance = &desktop.configuration.appearance;
    let body_size = appearance.typography().body_size.px();
    let controls = preview.controls();
    let button = |label: &'static str, control: Control| {
        canopa_ui::styled_action(
            text(label).size(appearance.typography().label_size.px()),
            controls
                .contains(&control)
                .then_some(Message::Preview(Event::Activate(
                    id,
                    preview.revision,
                    control,
                ))),
            canopa_ui::Action::Quiet,
            preview.focused == Some(control),
            appearance,
        )
    };
    let mut header = row![button("Open", Control::Open)].spacing(6);
    if preview.bitmap.is_some() {
        header = header
            .push(button("−", Control::ZoomOut))
            .push(button("Fit", Control::Fit))
            .push(button("100%", Control::Actual))
            .push(button("+", Control::ZoomIn));
    }
    if let Some(PreviewContent::Pdf { page, pages, .. }) = preview.content() {
        header = header
            .push(button("Previous", Control::Previous))
            .push(text(format!("{page} / {pages}")))
            .push(button("Next", Control::Next));
    }
    let content: Element<'a, Message> = if let Some(media) = &preview.media {
        media.body(
            matches!(preview.content(), Some(PreviewContent::Video(_))),
            body_size,
        )
    } else if let Some(bitmap) = &preview.bitmap {
        if let Some(zoom) = preview.zoom {
            let (width, height) = match preview.content() {
                Some(PreviewContent::Image(bitmap) | PreviewContent::Pdf { bitmap, .. }) => {
                    (bitmap.width, bitmap.height)
                }
                _ => (1, 1),
            };
            scrollable(
                image(bitmap.clone())
                    .width(width as f32 * zoom)
                    .height(height as f32 * zoom),
            )
            .direction(scrollable::Direction::Both {
                vertical: scrollable::Scrollbar::new(),
                horizontal: scrollable::Scrollbar::new(),
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            image(bitmap.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Contain)
                .into()
        }
    } else if let Some(content) = &preview.text {
        let revision = preview.revision;
        iced::widget::sensor(
            text_editor(content)
                .font(iced::Font::MONOSPACE)
                .size(body_size)
                .height(Length::Fill)
                .highlight_with::<preview_highlights::Highlighter>(
                    preview
                        .result
                        .as_deref()
                        .and_then(|result| result.as_ref().ok())
                        .map(|prepared| prepared.highlighting.clone())
                        .unwrap_or_default()
                        .settings(&desktop.theme),
                    preview_highlights::format,
                )
                .style(preview_highlights::style)
                .on_action(move |action| Message::Preview(Event::Text(id, revision, action))),
        )
        .key((id, revision))
        .on_show(move |_| Message::Preview(Event::TextLaidOut(id, revision)))
        .into()
    } else {
        let notice = if preview.loading {
            "Preparing preview…".into()
        } else if let Some(result) = &preview.result {
            match result.as_ref() {
                Err(error) => format!("Preview unavailable: {error}"),
                Ok(prepared) => match &prepared.preview.content {
                    PreviewContent::Folder { items } => format!("Folder · {items} items"),
                    PreviewContent::Audio(_) | PreviewContent::Video(_) => {
                        "Media preview is unavailable. Open in your usual application.".into()
                    }
                    _ => "No preview available. Open in the normal application.".into(),
                },
            }
        } else {
            "Select a file to preview".into()
        };
        container(text(notice).size(body_size))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };
    let information = preview
        .result
        .as_deref()
        .and_then(|result| result.as_ref().ok())
        .map_or_else(String::new, |prepared| {
            let extra = if matches!(
                &prepared.preview.content,
                PreviewContent::Text {
                    truncated: true,
                    ..
                }
            ) {
                " · Text truncated"
            } else {
                ""
            };
            format!(
                "{} bytes{extra}{}",
                prepared.preview.size,
                if preview.loading {
                    " · Preparing…"
                } else {
                    ""
                }
            )
        });
    let appearance = appearance.clone();
    let revision = preview.revision;
    let content = iced::widget::sensor(content)
        .key((id, revision))
        .on_show(move |size| Message::Preview(Event::Viewport(id, revision, size)))
        .on_resize(move |size| Message::Preview(Event::Viewport(id, revision, size)));
    let mut body = column![container(header.wrap()).padding(8), content];
    if let Some(media) = &preview.media {
        body = body.push(media.transport(
            id,
            revision,
            preview.focused,
            &desktop.configuration.appearance,
        ));
    }
    body = body.push(container(text(information).size(body_size)).padding(8));
    container(body)
        .id(root_id(id))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)))
        .into()
}

#[cfg(test)]
mod tests {
    use super::super::tests::{desktop, loaded};
    use super::*;
    use std::fs;

    fn setup(root: &Path) -> (Desktop, window::Id, window::Id) {
        let (mut desktop, source) = desktop(root, vec![root.to_owned()]);
        let tab = desktop.active(source).unwrap().id;
        loaded(&mut desktop, source, tab, 1, root);
        let _task = desktop.update(Message::Select(source, root.join("a.txt")));
        let _task = desktop.update(Message::Command(source, Command::QuickLook));
        let id = *desktop.previews.keys().next().unwrap();
        (desktop, source, id)
    }

    fn deliver(desktop: &mut Desktop, id: window::Id, revision: u64, path: &Path) {
        deliver_pending(desktop, id, revision, path);
        let _task = desktop.update(Message::Preview(Event::TextLaidOut(id, revision)));
    }

    fn deliver_pending(desktop: &mut Desktop, id: window::Id, revision: u64, path: &Path) {
        let prepared = PreviewRequest::new(path)
            .prepare(&Cancellation::default())
            .unwrap();
        let _task = desktop.update(Message::Preview(Event::Ready(
            id,
            revision,
            Some(Arc::new(Ok(Prepared::new(
                prepared,
                &Cancellation::default(),
            )
            .unwrap()))),
        )));
    }

    #[test]
    fn text_waits_for_its_own_layout_and_old_layouts_cannot_populate_a_new_selection() {
        let root = tempfile::tempdir().unwrap();
        let body = "A large preview remains selectable.\n".repeat(30_000);
        fs::write(root.path().join("a.txt"), &body).unwrap();
        fs::write(root.path().join("b.txt"), "Next selection").unwrap();
        let (mut desktop, source, id) = setup(root.path());
        let revision = desktop.previews[&id].revision;
        deliver_pending(&mut desktop, id, revision, &root.path().join("a.txt"));
        assert!(desktop.previews[&id].text.as_ref().unwrap().is_empty());
        assert!(desktop.previews[&id].pending_text.is_some());
        let _task = desktop.update(Message::Preview(Event::Viewport(
            id,
            revision,
            Size::new(760.0, 500.0),
        )));
        assert!(desktop.previews[&id].text.as_ref().unwrap().is_empty());
        let _task = desktop.update(Message::Preview(Event::TextLaidOut(id, revision)));
        assert!(desktop.previews[&id].pending_text.is_none());
        let Some(PreviewContent::Text { text, .. }) = desktop.previews[&id].content() else {
            panic!("expected prepared text");
        };
        assert_eq!(desktop.previews[&id].text.as_ref().unwrap().text(), *text);

        let _task = desktop.update(Message::Select(source, root.path().join("b.txt")));
        let current = desktop.previews[&id].revision;
        deliver_pending(&mut desktop, id, current, &root.path().join("b.txt"));
        let _task = desktop.update(Message::Preview(Event::TextLaidOut(id, revision)));
        assert!(desktop.previews[&id].text.as_ref().unwrap().is_empty());
        let _task = desktop.update(Message::Preview(Event::TextLaidOut(id, current)));
        assert_eq!(
            desktop.previews[&id]
                .text
                .as_ref()
                .unwrap()
                .text()
                .trim_end(),
            "Next selection"
        );
        desktop
            .previews
            .get_mut(&id)
            .unwrap()
            .text
            .as_mut()
            .unwrap()
            .move_to(text_editor::Cursor {
                position: text_editor::Position { line: 0, column: 3 },
                selection: None,
            });
        let _task = desktop.update(Message::Preview(Event::TextLaidOut(id, current)));
        assert_eq!(
            desktop.previews[&id]
                .text
                .as_ref()
                .unwrap()
                .cursor()
                .position
                .column,
            3
        );
    }

    #[test]
    fn text_insertion_preserves_unicode_and_line_endings_across_groups() {
        let body: String = (0..390)
            .map(|index| {
                format!(
                    "Été 日本語 {index}{}",
                    ["\r\n", "\n\r", "\r", "\n"][index % 4]
                )
            })
            .collect();
        let mut content = text_editor::Content::new();
        insert_text(&mut content, &body);
        assert_eq!(content.text(), body);
        assert_eq!(content.cursor().position.line, 0);
        assert_eq!(content.cursor().position.column, 0);
        assert!(content.cursor().selection.is_none());
    }

    #[test]
    fn preview_follows_only_its_source_and_rejects_old_results_after_selection_and_close() {
        let root = tempfile::tempdir().unwrap();
        for name in ["a.txt", "b.txt"] {
            fs::write(root.path().join(name), name).unwrap();
        }
        let (mut desktop, source, id) = setup(root.path());
        let first = desktop.previews[&id].revision;
        let cancellation = desktop.previews[&id].cancellation.clone();
        deliver(&mut desktop, id, first, &root.path().join("a.txt"));
        let _task = desktop.new_window(vec![root.path().to_owned()]);
        let other = *desktop
            .windows
            .keys()
            .find(|candidate| **candidate != source)
            .unwrap();
        let tab = desktop.active(other).unwrap().id;
        loaded(&mut desktop, other, tab, 1, root.path());
        let _task = desktop.update(Message::Select(other, root.path().join("b.txt")));
        assert_eq!(desktop.previews[&id].revision, first);
        assert_eq!(
            desktop.previews[&id].target.as_ref().unwrap().path,
            root.path().join("a.txt")
        );
        let raw = root
            .path()
            .join(std::ffi::OsString::from_vec(b"raw-\xfe.txt".to_vec()));
        fs::write(&raw, b"original raw bytes").unwrap();
        let _task = desktop.navigate(source, root.path().to_owned(), Navigation::Refresh, None);
        let tab = desktop.active(source).unwrap();
        let (tab_id, revision) = (tab.id, tab.revision);
        loaded(&mut desktop, source, tab_id, revision, root.path());
        let _task = desktop.update(Message::Select(source, raw.clone()));
        let current = desktop.previews[&id].revision;
        assert!(cancellation.is_cancelled());
        assert!(current > first);
        deliver(&mut desktop, id, first, &root.path().join("a.txt"));
        assert!(desktop.previews[&id].result.is_none());
        deliver(&mut desktop, id, current, &raw);
        assert_eq!(desktop.previews[&id].target.as_ref().unwrap().path, raw);
        assert!(
            matches!(desktop.previews[&id].content(), Some(PreviewContent::Text { text, .. }) if text == "original raw bytes")
        );
        let cancellation = desktop.previews[&id].cancellation.clone();
        let _task = desktop.update(Message::Close(source));
        assert!(cancellation.is_cancelled());
        assert!(desktop.previews.is_empty());
        assert!(desktop.windows.contains_key(&other));
        deliver(&mut desktop, id, current, &root.path().join("b.txt"));
        assert!(desktop.previews.is_empty());
    }

    #[test]
    fn closing_and_reopening_preview_preserves_browsing_and_does_not_adopt_old_callbacks() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.txt"), b"selected").unwrap();
        let (mut desktop, source, id) = setup(root.path());
        let revision = desktop.previews[&id].revision;
        desktop
            .active_mut(source)
            .unwrap()
            .browser
            .as_mut()
            .unwrap()
            .set_scroll(80.0);
        let _task = desktop.update(Message::Key(
            id,
            keyboard::Key::Character(" ".into()),
            keyboard::Modifiers::default(),
            Some(" ".into()),
        ));
        assert!(desktop.previews.is_empty());
        let browser = desktop.active(source).unwrap().browser.as_ref().unwrap();
        assert_eq!(browser.focused(), Some(root.path().join("a.txt").as_path()));
        assert_eq!(browser.scroll(), 80.0);
        let _task = desktop.update(Message::Command(source, Command::QuickLook));
        let new_id = *desktop.previews.keys().next().unwrap();
        assert_ne!(id, new_id);
        deliver(&mut desktop, id, revision, &root.path().join("a.txt"));
        let _task = desktop.update(Message::Preview(Event::Opened(id, Some(id))));
        assert!(desktop.previews[&new_id].result.is_none());
        let _task = desktop.update(Message::Preview(Event::Opened(new_id, None)));
        assert!(desktop.previews.is_empty());
        assert!(
            desktop
                .active(source)
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("floating preview")
        );
    }

    #[test]
    fn controls_are_revision_guarded_zoom_starts_from_fit_and_space_activates_focused_controls() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.txt"), b"selected").unwrap();
        let (mut desktop, _source, id) = setup(root.path());
        let revision = desktop.previews[&id].revision;
        let result = PreparedPreview {
            path: root.path().join("a.txt"),
            size: 1,
            content: PreviewContent::Image(crate::preview::Bitmap {
                width: 1000,
                height: 1000,
                rgba: Arc::from(vec![0u8; 4_000_000]),
            }),
        };
        let _task = desktop.update(Message::Preview(Event::Ready(
            id,
            revision,
            Some(Arc::new(Ok(Prepared::new(
                result,
                &Cancellation::default(),
            )
            .unwrap()))),
        )));
        let _task = desktop.update(Message::Preview(Event::Viewport(
            id,
            revision,
            Size::new(500.0, 500.0),
        )));
        let _task = desktop.update(Message::Preview(Event::Activate(
            id,
            revision,
            Control::ZoomOut,
        )));
        assert_eq!(desktop.previews[&id].zoom, Some(0.4));
        let _task = desktop.update(Message::Preview(Event::Activate(
            id,
            revision - 1,
            Control::Actual,
        )));
        assert_eq!(desktop.previews[&id].zoom, Some(0.4));
        let _task = desktop.update(Message::Key(
            id,
            keyboard::Key::Named(keyboard::key::Named::Tab),
            keyboard::Modifiers::default(),
            None,
        ));
        assert_eq!(desktop.previews[&id].focused, Some(Control::Open));
        let _task = desktop.update(Message::Key(
            id,
            keyboard::Key::Character(" ".into()),
            keyboard::Modifiers::default(),
            Some(" ".into()),
        ));
        assert!(desktop.previews.contains_key(&id));
        assert_eq!(desktop.previews[&id].focused, Some(Control::Open));
    }

    #[test]
    fn fitted_scales_outside_manual_limits_cannot_reverse_zoom_direction() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.txt"), b"selected").unwrap();
        let (mut desktop, _source, id) = setup(root.path());
        let revision = desktop.previews[&id].revision;
        for (pixels, viewport, control) in
            [(1, 500.0, Control::ZoomIn), (1000, 50.0, Control::ZoomOut)]
        {
            let prepared = PreparedPreview {
                path: root.path().join("a.txt"),
                size: 1,
                content: PreviewContent::Image(crate::preview::Bitmap {
                    width: pixels,
                    height: pixels,
                    rgba: Arc::from(vec![0u8; pixels as usize * pixels as usize * 4]),
                }),
            };
            let _task = desktop.update(Message::Preview(Event::Ready(
                id,
                revision,
                Some(Arc::new(Ok(Prepared::new(
                    prepared,
                    &Cancellation::default(),
                )
                .unwrap()))),
            )));
            let _task = desktop.update(Message::Preview(Event::Viewport(
                id,
                revision,
                Size::new(viewport, viewport),
            )));
            assert!(
                !desktop.previews[&id].controls().contains(&control),
                "a fitted image beyond the zoom limit must disable {control:?}"
            );
            let before = desktop.previews[&id].scale();
            let _task = desktop.update(Message::Preview(Event::Activate(id, revision, control)));
            assert_eq!(desktop.previews[&id].scale(), before);
        }
    }

    #[test]
    fn text_is_read_only_and_switching_tabs_changes_the_preview_source() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.txt"), b"selected").unwrap();
        let (mut desktop, source, id) = setup(root.path());
        let revision = desktop.previews[&id].revision;
        let first_tab = desktop.active(source).unwrap().id;
        deliver(&mut desktop, id, revision, &root.path().join("a.txt"));
        let _task = desktop.update(Message::Preview(Event::Text(
            id,
            revision,
            text_editor::Action::Edit(text_editor::Edit::Insert('x')),
        )));
        assert_eq!(
            desktop.previews[&id]
                .text
                .as_ref()
                .unwrap()
                .text()
                .trim_end(),
            "selected"
        );
        let _task = desktop.update(Message::Command(source, Command::NewTab));
        assert!(desktop.previews[&id].target.is_none());
        assert!(desktop.previews[&id].text.is_none());
        let tab = desktop.active(source).unwrap();
        let (tab_id, navigation) = (tab.id, tab.revision);
        loaded(&mut desktop, source, tab_id, navigation, root.path());
        let _task = desktop.update(Message::Select(source, root.path().join("a.txt")));
        assert!(
            !matches!(desktop.previews[&id].target.as_ref().unwrap().context, TargetContext::Browser { tab, .. } if tab == first_tab)
        );
        let _task = desktop.update(Message::SwitchTab(source, 0));
        assert!(
            matches!(desktop.previews[&id].target.as_ref().unwrap().context, TargetContext::Browser { tab, .. } if tab == first_tab)
        );
    }

    fn wait_media(
        desktop: &mut Desktop,
        id: window::Id,
        predicate: impl Fn(&crate::playback::Snapshot) -> bool,
    ) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            let _task = desktop.update(Message::MediaPoll);
            let state = &desktop.previews[&id].media.as_ref().unwrap().snapshot;
            if predicate(state) {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline
                    && !matches!(state.phase, crate::playback::Phase::Failed(_)),
                "media controller did not settle: {:?}, position={:?}, seeking={}, muted={}, volume={}",
                state.phase,
                state.position,
                state.seeking,
                state.muted,
                state.volume
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn wait_media_retired(desktop: &mut Desktop) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while desktop.media.holds_application() {
            assert!(std::time::Instant::now() < deadline, "media did not retire");
            let _task = desktop.update(Message::MediaPoll);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn consecutive_media_controls_preserve_intent_before_the_next_snapshot() {
        use crate::playback::Phase;
        use keyboard::key::Named;
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.txt"), "Text selection").unwrap();
        let audio = root.path().join("b.wav");
        // Twenty seconds of actual PCM silence, represented by a private sparse
        // file. Seeking still traverses the real prepared reader and decoder.
        let size = 20 * 48_000 * 2u32;
        let mut header = b"RIFF".to_vec();
        header.extend((36 + size).to_le_bytes());
        header.extend(b"WAVEfmt ");
        header.extend(16u32.to_le_bytes());
        header.extend(1u16.to_le_bytes());
        header.extend(1u16.to_le_bytes());
        header.extend(48_000u32.to_le_bytes());
        header.extend(96_000u32.to_le_bytes());
        header.extend(2u16.to_le_bytes());
        header.extend(16u16.to_le_bytes());
        header.extend(b"data");
        header.extend(size.to_le_bytes());
        fs::write(&audio, header).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&audio)
            .unwrap()
            .set_len(u64::from(size) + 44)
            .unwrap();
        let (mut desktop, source, id) = setup(root.path());
        let _task = desktop.update(Message::Select(source, audio.clone()));
        let revision = desktop.previews[&id].revision;
        deliver(&mut desktop, id, revision, &audio);
        wait_media(&mut desktop, id, |s| s.phase == Phase::Paused && s.seekable);
        let before = desktop.previews[&id]
            .media
            .as_ref()
            .unwrap()
            .snapshot
            .revision;
        for _ in 0..2 {
            let _task = desktop.update(Message::Preview(Event::Activate(
                id,
                revision,
                Control::Mute,
            )));
        }
        wait_media(&mut desktop, id, |s| s.revision > before && !s.muted);
        let key = |desktop: &mut Desktop, named| {
            let _task = desktop.update(Message::Key(
                id,
                keyboard::Key::Named(named),
                keyboard::Modifiers::default(),
                None,
            ));
        };
        let _task = desktop.update(Message::Preview(Event::Media(
            id,
            revision,
            media_ui::Event::VolumeFocus,
        )));
        key(&mut desktop, Named::Home);
        key(&mut desktop, Named::ArrowRight);
        key(&mut desktop, Named::ArrowRight);
        wait_media(&mut desktop, id, |s| (s.volume - 0.1).abs() < 0.0001);
        let _task = desktop.update(Message::Preview(Event::Activate(
            id,
            revision,
            Control::Seek,
        )));
        key(&mut desktop, Named::Home);
        key(&mut desktop, Named::ArrowRight);
        key(&mut desktop, Named::ArrowRight);
        wait_media(&mut desktop, id, |s| {
            !s.seeking && (s.position.as_secs_f64() - 10.0).abs() < 0.01
        });
        assert_eq!(
            desktop.previews[&id].media.as_ref().unwrap().snapshot.phase,
            Phase::Paused
        );
        let _task = desktop.update(Message::Close(source));
        wait_media_retired(&mut desktop);
    }

    #[test]
    fn media_controls_use_the_backend_and_old_source_events_cannot_restart_it() {
        use crate::playback::Phase;
        use keyboard::key::Named;
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.txt"), "Text selection").unwrap();
        let video = root.path().join("b.webm");
        fs::write(
            &video,
            include_bytes!("../../tests/fixtures/preview-colors.webm"),
        )
        .unwrap();
        let (mut desktop, source, id) = setup(root.path());
        assert!(!desktop.media.holds_application());
        let _task = desktop.update(Message::Select(source, video.clone()));
        let revision = desktop.previews[&id].revision;
        deliver(&mut desktop, id, revision, &video);
        assert!(desktop.media.holds_application());
        wait_media(&mut desktop, id, |s| {
            s.phase == Phase::Paused && s.frame.is_some() && s.seekable
        });
        assert_eq!(desktop.previews[&id].focused, None);
        std::thread::sleep(std::time::Duration::from_millis(150));
        wait_media(&mut desktop, id, |s| s.phase == Phase::Paused);
        assert!(
            desktop.previews[&id]
                .media
                .as_ref()
                .unwrap()
                .snapshot
                .position
                < std::time::Duration::from_millis(50)
        );

        let key = |desktop: &mut Desktop, named| {
            let _task = desktop.update(Message::Key(
                id,
                keyboard::Key::Named(named),
                keyboard::Modifiers::default(),
                None,
            ));
        };
        key(&mut desktop, Named::Tab);
        key(&mut desktop, Named::Tab);
        assert_eq!(desktop.previews[&id].focused, Some(Control::Play));
        key(&mut desktop, Named::Space);
        wait_media(&mut desktop, id, |s| {
            s.phase == Phase::Playing && s.position > std::time::Duration::from_millis(100)
        });
        key(&mut desktop, Named::Space);
        wait_media(&mut desktop, id, |s| s.phase == Phase::Paused);
        key(&mut desktop, Named::Tab);
        assert_eq!(desktop.previews[&id].focused, Some(Control::Seek));
        key(&mut desktop, Named::ArrowRight);
        wait_media(&mut desktop, id, |s| {
            !s.seeking
                && s.frame
                    .as_ref()
                    .is_some_and(|f| f.timestamp.is_some_and(|t| t.as_secs() >= 5))
        });
        assert_eq!(
            desktop
                .active(source)
                .unwrap()
                .browser
                .as_ref()
                .unwrap()
                .focused(),
            Some(video.as_path())
        );
        key(&mut desktop, Named::Home);
        wait_media(&mut desktop, id, |s| {
            !s.seeking
                && s.frame
                    .as_ref()
                    .is_some_and(|f| f.timestamp == Some(std::time::Duration::ZERO))
        });
        key(&mut desktop, Named::Tab);
        key(&mut desktop, Named::Space);
        wait_media(&mut desktop, id, |s| s.muted);
        key(&mut desktop, Named::Tab);
        key(&mut desktop, Named::Home);
        wait_media(&mut desktop, id, |s| s.volume == 0.0);
        key(&mut desktop, Named::End);
        wait_media(&mut desktop, id, |s| s.volume == 1.0);
        assert!(desktop.previews.contains_key(&id));
        let _view = view(&desktop, id, &desktop.previews[&id]);
        drop(_view);

        let _task = desktop.update(Message::Select(source, root.path().join("a.txt")));
        assert!(desktop.previews[&id].media.is_none());
        assert_eq!(desktop.previews[&id].focused, None);
        wait_media_retired(&mut desktop);
        // Old preparation and slider messages have no authority over this source.
        deliver(&mut desktop, id, revision, &video);
        let _task = desktop.update(Message::Preview(Event::Media(
            id,
            revision,
            media_ui::Event::Volume(0.2),
        )));
        assert!(desktop.previews[&id].media.is_none());
        assert!(!desktop.media.holds_application());

        let _task = desktop.update(Message::Select(source, video.clone()));
        let current = desktop.previews[&id].revision;
        deliver(&mut desktop, id, current, &video);
        wait_media(&mut desktop, id, |s| s.phase == Phase::Paused);
        // Default Space still closes Quick Look; a decoder never owns this key.
        key(&mut desktop, Named::Space);
        assert!(desktop.previews.is_empty());
        wait_media_retired(&mut desktop);
        let _task = desktop.toggle_preview(source);
        let new_id = *desktop.previews.keys().next().unwrap();
        let _task = desktop.sync_previews();
        let current = desktop.previews[&new_id].revision;
        deliver(&mut desktop, new_id, current, &video);
        let _task = desktop.update(Message::Close(source));
        assert!(desktop.previews.is_empty());
        wait_media_retired(&mut desktop);
    }
}
