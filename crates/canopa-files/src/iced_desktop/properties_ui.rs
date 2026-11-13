//! Present prepared metadata; filesystem inspection stays in the shared core.
use super::*;
use gio::prelude::FileExt;
use iced::{
    Length,
    widget::{column, row, scrollable, text},
};

#[derive(Clone, Debug)]
pub(super) enum Event {
    Opened(window::Id),
    Ready(window::Id, Arc<Result<Vec<Item>, String>>),
    /// A browser's side panel finished reading `paths`.
    PanelReady(window::Id, Vec<PathBuf>, Arc<Result<Vec<Item>, String>>),
    Copy(window::Id),
    /// Change one class's access to `path` to the given mode bits.
    SetAccess(PathBuf, u32),
    AccessSet(PathBuf, Result<(), String>),
    ClosePanel(window::Id),
    /// Show or hide Access in browser `id`'s side panel.
    TogglePanelAccess(window::Id),
    /// Show or hide the numeric mode and identities for window or panel `id`.
    ToggleAdvanced(window::Id),
    /// Choose the application that opens `path`, from window `owner`.
    ChangeOpener(window::Id, PathBuf),
    /// The name field of browser `id`'s side panel was edited.
    NameEdited(window::Id, String),
    /// Rename the side panel's item to the edited name.
    Rename(window::Id),
    /// The rename of the panel's item was prepared for the edited name.
    RenamePrepared(
        window::Id,
        Arc<super::file_operations::Origin>,
        String,
        Result<crate::operation_jobs::PreparedRename, String>,
    ),
}

pub(super) struct Window {
    pub(super) paths: Vec<PathBuf>,
    cancellation: Cancellation,
    result: Option<Arc<Result<Vec<Item>, String>>>,
    /// Why the last access change failed, shown beside the access table.
    access_error: Option<String>,
    /// The panel shows Access only on request; the window always shows it.
    pub(super) access_open: bool,
    /// Numeric modes and identities appear only on request.
    advanced_open: bool,
    /// The panel's name field while it differs from the item's name.
    name_draft: Option<String>,
    /// Why the last rename from the panel was refused.
    name_error: Option<String>,
    /// A rename from the panel is being prepared.
    renaming: bool,
}
impl Window {
    /// Properties for `paths`, before they have been read.
    pub(super) fn pending(paths: Vec<PathBuf>) -> Self {
        Self {
            paths,
            cancellation: Cancellation::default(),
            result: None,
            access_error: None,
            access_open: false,
            advanced_open: false,
            name_draft: None,
            name_error: None,
            renaming: false,
        }
    }
}
impl Window {
    /// Forget an unsubmitted name edit; whether there was one.
    pub(super) fn discard_name_edit(&mut self) -> bool {
        let edited = self.name_draft.is_some() || self.name_error.is_some();
        self.name_draft = None;
        self.name_error = None;
        edited
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
impl Window {
    pub fn title(&self) -> String {
        let title = if self.result.is_none() {
            fl!("files-properties-title-reading")
        } else {
            fl!("files-properties-title", count = self.paths.len())
        };
        fl!("files-window-title", title = title)
    }
}

#[derive(Debug)]
pub(super) struct Item {
    filesystem: crate::file_properties::Item,
    kind: Option<String>,
    owner: Option<String>,
    group: Option<String>,
    /// The listing entry that selects the same thumbnail or type icon as the browser.
    entry: Option<crate::browse::Entry>,
    /// Pixel dimensions read from an image header; the image is never decoded.
    dimensions: Option<(u32, u32)>,
    /// The default application's name for this type.
    opener: Option<String>,
}

impl Item {
    fn describe(filesystem: crate::file_properties::Item) -> Self {
        let content_type = filesystem.metadata.as_ref().ok().and_then(|metadata| {
            if metadata.kind != crate::browse::EntryKind::File {
                return None;
            }
            let (content_type, _) = gio::content_type_guess(Some(&filesystem.path), None);
            (!gio::content_type_is_unknown(&content_type)).then_some(content_type)
        });
        let kind = content_type
            .as_ref()
            .map(|content_type| gio::content_type_get_description(content_type).into());
        let dimensions = content_type
            .as_ref()
            .and_then(|content_type| gio::content_type_get_mime_type(content_type))
            .filter(|mime| mime.starts_with("image/"))
            .and_then(|_| ::image::image_dimensions(&filesystem.path).ok());
        let entry = crate::browse::Entry::read(&filesystem.path).ok();
        let opener = content_type
            .as_ref()
            .and_then(|content_type| gio::AppInfo::default_for_type(content_type, false))
            .map(|app| gio::prelude::AppInfoExt::display_name(&app).to_string());
        let identities = filesystem.metadata.as_ref().ok().and_then(|metadata| {
            gio::File::for_path(&filesystem.path)
                .query_info(
                    "owner::user,owner::group,unix::uid,unix::gid",
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                )
                .ok()
                .filter(|info| {
                    info.has_attribute("unix::uid")
                        && info.has_attribute("unix::gid")
                        && info.attribute_uint32("unix::uid") == metadata.owner
                        && info.attribute_uint32("unix::gid") == metadata.group
                })
        });
        let attribute = |name| {
            identities
                .as_ref()
                .and_then(|info| info.attribute_string(name))
                .map(Into::into)
        };
        Self {
            filesystem,
            kind,
            owner: attribute("owner::user"),
            group: attribute("owner::group"),
            entry,
            dimensions,
            opener,
        }
    }
}

/// Read and describe `paths`, off the renderer thread.
fn read(paths: &[PathBuf], cancellation: &Cancellation) -> Result<Vec<Item>, String> {
    let items =
        crate::file_properties::read(paths, cancellation).map_err(|error| error.to_string())?;
    items
        .into_iter()
        .map(|item| {
            if cancellation.is_cancelled() {
                return Err(fl!("files-properties-cancelled"));
            }
            Ok(Item::describe(item))
        })
        .collect()
}

/// One access level a person can choose for a class of users.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Level {
    label: String,
    bits: u32,
}
impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// The levels offered for a folder or a file, as other file managers name them.
/// A file keeps its execute bit; only reading and writing are offered.
fn levels(directory: bool, current: u32) -> Vec<Level> {
    let level = |label, bits| Level { label, bits };
    if directory {
        vec![
            level(fl!("files-permission-folder-none"), 0),
            level(fl!("files-permission-folder-list"), 4),
            level(fl!("files-permission-folder-access"), 5),
            level(fl!("files-permission-folder-change"), 7),
        ]
    } else {
        let execute = current & 1;
        vec![
            level(fl!("files-permission-file-none"), execute),
            level(fl!("files-permission-file-read"), 4 | execute),
            level(fl!("files-permission-file-write"), 6 | execute),
        ]
    }
}

fn permission_label(mode: u32) -> String {
    match mode & 7 {
        0 => fl!("files-permission-none"),
        1 => fl!("files-permission-x"),
        2 => fl!("files-permission-w"),
        3 => fl!("files-permission-wx"),
        4 => fl!("files-permission-r"),
        5 => fl!("files-permission-rx"),
        6 => fl!("files-permission-rw"),
        _ => fl!("files-permission-rwx"),
    }
}

fn control(id: window::Id, index: u8) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-properties-{id:?}-{index}"))
}
fn scroll(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-properties-{id:?}-scroll"))
}
fn uri(path: &Path) -> String {
    url::Url::from_file_path(path).map_or_else(|_| display_location(path), |url| url.to_string())
}

/// Read `window`'s paths on a worker, reporting through `event`.
fn read_task(
    window: &Window,
    event: impl FnOnce(Arc<Result<Vec<Item>, String>>) -> Event + Send + 'static,
) -> Task<Message> {
    let paths = window.paths.clone();
    let cancellation = window.cancellation.clone();
    Task::perform(async move { read(&paths, &cancellation) }, move |result| {
        Message::Properties(event(Arc::new(result)))
    })
}

fn timestamp(time: std::time::SystemTime) -> Option<chrono::DateTime<chrono::Utc>> {
    let (seconds, nanos) = match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => (
            i64::try_from(duration.as_secs()).ok()?,
            duration.subsec_nanos(),
        ),
        Err(error) => {
            let duration = error.duration();
            let seconds = i64::try_from(duration.as_secs()).ok()?.checked_neg()?;
            if duration.subsec_nanos() == 0 {
                (seconds, 0)
            } else {
                (
                    seconds.checked_sub(1)?,
                    1_000_000_000 - duration.subsec_nanos(),
                )
            }
        }
    };
    chrono::DateTime::from_timestamp(seconds, nanos)
}

fn modified(time: std::time::SystemTime) -> String {
    use chrono::Datelike;
    timestamp(time).map_or_else(
        || fl!("files-date-out-of-range"),
        |utc| {
            if (1..=9999).contains(&utc.year()) {
                canopa_i18n::format::date_time(utc.with_timezone(&chrono::Local).naive_local())
            } else {
                utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            }
        },
    )
}

impl Desktop {
    pub(super) fn show_properties(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        // A fixed size lets compositors float Properties like other inspectors
        // instead of tiling a short card across a full column.
        let size = if paths.len() == 1 {
            Size::new(480.0, 664.0)
        } else {
            Size::new(620.0, 520.0)
        };
        let settings = window::Settings {
            size,
            min_size: Some(size),
            max_size: Some(size),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let (id, opened) = window::open(settings);
        let window = Window::pending(paths);
        let task = read_task(&window, move |result| Event::Ready(id, result));
        self.properties.insert(id, window);
        Task::batch([
            opened.map(move |_| Message::Properties(Event::Opened(id))),
            task,
        ])
    }

    /// Properties for browser `id`'s selection, or its folder: in a window or
    /// in the side panel, as the person chose.
    pub(super) fn properties_command(&mut self, id: window::Id) -> Task<Message> {
        if self.configuration.settings.properties == crate::interface::PropertiesDisplay::Panel {
            return self.toggle_properties_panel(id);
        }
        let Some(browser) = self.active(id).and_then(|tab| tab.browser.as_ref()) else {
            return Task::none();
        };
        let mut paths: Vec<PathBuf> = browser.selected().iter().take(64).cloned().collect();
        if paths.is_empty() {
            paths.push(browser.path().to_owned());
        }
        self.show_properties(paths)
    }

    /// Show or hide the side panel of browser `id`.
    pub(super) fn toggle_properties_panel(&mut self, id: window::Id) -> Task<Message> {
        if let Some(window) = self.windows.get_mut(&id) {
            window.properties = if window.properties.is_some() {
                None
            } else {
                Some(Window::pending(Vec::new()))
            };
        }
        Task::none()
    }

    /// Keep each open side panel on its browser's selection, or on the current
    /// folder when nothing is selected. Reading happens off the renderer thread.
    pub(super) fn sync_properties_panels(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        for (id, window) in &mut self.windows {
            let Some(panel) = &window.properties else {
                continue;
            };
            let Some(browser) = window
                .tabs
                .get(window.active)
                .and_then(|tab| tab.browser.as_ref())
            else {
                continue;
            };
            let mut paths: Vec<PathBuf> = browser.selected().iter().take(64).cloned().collect();
            if paths.is_empty() {
                paths.push(browser.path().to_owned());
            }
            if panel.paths == paths {
                continue;
            }
            let id = *id;
            let mut next = Window::pending(paths.clone());
            next.access_open = panel.access_open;
            next.advanced_open = panel.advanced_open;
            tasks.push(read_task(&next, move |result| {
                Event::PanelReady(id, paths.clone(), result)
            }));
            window.properties = Some(next);
        }
        Task::batch(tasks)
    }

    pub(super) fn close_properties(&mut self, id: window::Id) -> Task<Message> {
        if self.properties.remove(&id).is_none() {
            return Task::none();
        }
        Task::batch([window::close(id), self.exit_if_empty()])
    }

    pub(super) fn properties_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Opened(id) => iced::widget::operation::focus(control(id, 1)),
            Event::Ready(id, result) => {
                if let Some(properties) = self.properties.get_mut(&id) {
                    properties.result = Some(result);
                }
                Task::none()
            }
            Event::PanelReady(id, paths, result) => {
                if let Some(panel) = self
                    .windows
                    .get_mut(&id)
                    .and_then(|window| window.properties.as_mut())
                    .filter(|panel| panel.paths == paths)
                {
                    panel.result = Some(result);
                }
                Task::none()
            }
            Event::Copy(id) => self
                .properties
                .get(&id)
                .map_or_else(Task::none, |properties| {
                    iced::clipboard::write(
                        properties
                            .paths
                            .iter()
                            .map(|p| uri(p))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                }),
            Event::SetAccess(path, mode) => {
                let done = path.clone();
                Task::perform(
                    async move {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                            .map_err(|error| error.to_string())
                    },
                    move |result| Message::Properties(Event::AccessSet(done.clone(), result)),
                )
            }
            Event::AccessSet(path, result) => {
                // Every view of the item reads it again, so each shows what
                // the filesystem now holds, whether or not the change applied.
                let error = result.err();
                let mut tasks = Vec::new();
                for (id, window) in &mut self.properties {
                    if window.paths.contains(&path) {
                        let id = *id;
                        let mut next = Window::pending(window.paths.clone());
                        next.access_error = error.clone();
                        tasks.push(read_task(&next, move |result| Event::Ready(id, result)));
                        *window = next;
                    }
                }
                for (id, window) in &mut self.windows {
                    if let Some(panel) = window
                        .properties
                        .as_mut()
                        .filter(|panel| panel.paths.contains(&path))
                    {
                        let id = *id;
                        let paths = panel.paths.clone();
                        let mut next = Window::pending(paths.clone());
                        next.access_error = error.clone();
                        tasks.push(read_task(&next, move |result| {
                            Event::PanelReady(id, paths.clone(), result)
                        }));
                        *panel = next;
                    }
                }
                Task::batch(tasks)
            }
            Event::TogglePanelAccess(id) => {
                if let Some(panel) = self
                    .windows
                    .get_mut(&id)
                    .and_then(|window| window.properties.as_mut())
                {
                    panel.access_open = !panel.access_open;
                }
                Task::none()
            }
            Event::ToggleAdvanced(id) => {
                let window = match self.properties.get_mut(&id) {
                    Some(window) => Some(window),
                    None => self
                        .windows
                        .get_mut(&id)
                        .and_then(|window| window.properties.as_mut()),
                };
                if let Some(window) = window {
                    window.advanced_open = !window.advanced_open;
                }
                Task::none()
            }
            Event::ChangeOpener(owner, path) => self.show_open_with_chooser(owner, vec![path]),
            Event::NameEdited(id, name) => {
                if let Some(panel) = self.panel_mut(id).filter(|panel| !panel.renaming) {
                    panel.name_draft = Some(name);
                    panel.name_error = None;
                }
                Task::none()
            }
            Event::Rename(id) => self.rename_from_panel(id),
            Event::RenamePrepared(id, origin, name, result) => {
                let Some(panel) = self.panel_mut(id) else {
                    return Task::none();
                };
                panel.renaming = false;
                let source = origin.selected.first().cloned();
                if !origin.current(self) || source.is_none() {
                    return Task::none();
                }
                let source = source.unwrap();
                let prepared = match result {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        if let Some(panel) = self.panel_mut(id) {
                            panel.name_error = Some(error);
                        }
                        return Task::none();
                    }
                };
                let destination = source.with_file_name(&name);
                let result = self
                    .file_operations
                    .service
                    .as_ref()
                    .ok_or_else(|| std::io::Error::other(fl!("files-operation-queue-unavailable")))
                    .and_then(|service| service.submit_rename(prepared, name.into()));
                if let Some(panel) = self.panel_mut(id) {
                    panel.name_draft = None;
                }
                self.await_file_admission(
                    result,
                    super::file_operations::Submission {
                        origin: Some(origin),
                        owner: id,
                        dialog: None,
                        affected: vec![destination.clone(), source],
                        reveal: Some(destination),
                        refresh_all: false,
                    },
                )
            }
            Event::ClosePanel(id) => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.properties = None;
                }
                Task::none()
            }
        }
    }

    fn panel_mut(&mut self, id: window::Id) -> Option<&mut Window> {
        self.windows
            .get_mut(&id)
            .and_then(|window| window.properties.as_mut())
    }

    /// Rename the one selected item shown in browser `id`'s side panel, through
    /// the same prepared, undoable operation as the Rename dialog.
    fn rename_from_panel(&mut self, id: window::Id) -> Task<Message> {
        let Some(panel) = self
            .windows
            .get(&id)
            .and_then(|window| window.properties.as_ref())
        else {
            return Task::none();
        };
        let (Some(name), [path]) = (panel.name_draft.clone(), panel.paths.as_slice()) else {
            return Task::none();
        };
        let path = path.clone();
        if panel.renaming || path.file_name() == Some(std::ffi::OsStr::new(&name)) {
            if let Some(panel) = self.panel_mut(id) {
                panel.name_draft = None;
            }
            return Task::none();
        }
        let invalid =
            name.is_empty() || matches!(name.as_str(), "." | "..") || name.contains(['/', '\0']);
        let origin = super::file_operations::Origin::capture(self, id)
            .filter(|origin| origin.selected == [path.clone()]);
        let refusal = if invalid {
            Some(fl!("files-name-invalid"))
        } else if !self.file_operations.ready() || origin.is_none() {
            Some(fl!("files-operation-queue-unavailable"))
        } else {
            None
        };
        if let Some(refusal) = refusal {
            if let Some(panel) = self.panel_mut(id) {
                panel.name_error = Some(refusal);
            }
            return Task::none();
        }
        let origin = origin.unwrap();
        let result = self
            .file_operations
            .service
            .as_ref()
            .unwrap()
            .prepare_rename(path, Some(origin.binding()));
        match result {
            Ok(receiver) => {
                if let Some(panel) = self.panel_mut(id) {
                    panel.renaming = true;
                    panel.name_error = None;
                }
                Task::perform(
                    async move {
                        receiver
                            .await
                            .unwrap_or_else(|_| Err(fl!("files-rename-worker-stopped")))
                    },
                    move |result| {
                        Message::Properties(Event::RenamePrepared(
                            id,
                            origin.clone(),
                            name.clone(),
                            result,
                        ))
                    },
                )
            }
            Err(error) => {
                if let Some(panel) = self.panel_mut(id) {
                    panel.name_error = Some(error.to_string());
                }
                Task::none()
            }
        }
    }

    pub(super) fn properties_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        use keyboard::key::Named;
        if matches!(&key, keyboard::Key::Named(Named::Escape))
            || modifiers.control()
                && matches!(&key, keyboard::Key::Character(c) if c.eq_ignore_ascii_case("w"))
        {
            return self.close_properties(id);
        }
        if modifiers.control()
            && matches!(&key, keyboard::Key::Character(c) if c.eq_ignore_ascii_case("c"))
        {
            return self.properties_event(Event::Copy(id));
        }
        match key {
            keyboard::Key::Named(Named::Home) => iced::widget::operation::scroll_to(
                scroll(id),
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
            ),
            keyboard::Key::Named(Named::End) => iced::widget::operation::snap_to_end(scroll(id)),
            keyboard::Key::Named(Named::PageDown) => iced::widget::operation::scroll_by(
                scroll(id),
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: 300.0 },
            ),
            keyboard::Key::Named(Named::PageUp) => iced::widget::operation::scroll_by(
                scroll(id),
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: -300.0 },
            ),
            _ => Task::none(),
        }
    }
}

pub(super) fn view<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    properties: &'a Window,
) -> Element<'a, Message> {
    iced::widget::responsive(move |size| {
        window_contents(desktop, id, properties, size.width >= 550.0)
    })
    .into()
}

/// Each item's facts and access, shared by the Properties window and panel.
fn sections<'a>(
    desktop: &'a Desktop,
    owner: window::Id,
    properties: &'a Window,
    wide: bool,
    panel: Option<window::Id>,
) -> Element<'a, Message> {
    let compact = panel.is_some();
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().label_size.px();
    let body_size = appearance.typography().body_size.px();
    let muted = chrome::ink(appearance, canopa_theme::ColorRole::Muted);
    // Keys share one column across the facts and Access, wide enough for
    // "Owner · name", so every value and control starts at the same edge.
    let key_width = if compact {
        78.0
    } else if wide {
        136.0
    } else {
        120.0
    };
    let value_size = if compact { size + 1.0 } else { body_size };
    let label = |value: String| {
        text(value)
            .font(font)
            .size(body_size)
            .wrapping(text::Wrapping::WordOrGlyph)
    };
    // The panel uses the window's label and value rows, a step smaller.
    let fact = move |key: String, value: String| -> Element<'a, Message> {
        row![
            text(key)
                .font(font)
                .size(value_size)
                .color(muted)
                .width(key_width),
            text(value)
                .font(font)
                .size(value_size)
                .width(Length::Fill)
                .wrapping(text::Wrapping::WordOrGlyph)
        ]
        .spacing(if compact { 8 } else { 12 })
        .into()
    };
    let single = properties.paths.len() == 1;
    let mut facts = column![].spacing(12);
    match properties.result.as_deref() {
        None => {
            facts = facts.push(chrome::section_card(
                appearance,
                label(fl!("files-properties-reading")).color(muted),
            ))
        }
        Some(Err(error)) => {
            facts = facts.push(chrome::section_card(appearance, label(error.clone())))
        }
        Some(Ok(items)) => {
            for item in items {
                let path = &item.filesystem.path;
                let name = path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy();
                let location = super::question_dialog::display_location(
                    &desktop.home,
                    path.parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or(path),
                );
                let mut details = column![].spacing(10);
                if !single {
                    details = details.push(chrome::section_title(appearance, name.into_owned()));
                }
                let mut access = None;
                match &item.filesystem.metadata {
                    Err(error) => {
                        details = details
                            .push(fact(fl!("files-location"), location))
                            .push(fact(fl!("files-unavailable"), error.clone()))
                    }
                    Ok(metadata) => {
                        details =
                            details.push(fact(fl!("files-column-kind"), kind(item, metadata)));
                        if metadata.kind == crate::browse::EntryKind::File {
                            details = details.push(fact(
                                fl!("files-column-size"),
                                fl!(
                                    "files-properties-size-detail",
                                    size = super::listing_ui::size(metadata.bytes),
                                    exact = fl!("files-size-bytes", bytes = metadata.bytes)
                                ),
                            ));
                        }
                        if let Some((width, height)) = item.dimensions {
                            details = details.push(fact(
                                fl!("files-properties-dimensions"),
                                fl!("files-properties-pixels", width = width, height = height),
                            ));
                        }
                        if let Some(target) = &metadata.link_target {
                            details = details.push(fact(
                                fl!("files-properties-link-target"),
                                target.to_string_lossy().into_owned(),
                            ));
                        }
                        if let Some(opener) = &item.opener {
                            let change = Message::Properties(Event::ChangeOpener(
                                owner,
                                item.filesystem.path.clone(),
                            ));
                            // An outlined button as tall as the access
                            // fields below, rounded like them.
                            let button = iced::widget::button(
                                text(fl!("files-properties-change"))
                                    .font(font)
                                    .size(size + 1.0),
                            )
                            .on_press(change)
                            .padding([5, 12])
                            .style(|theme, status| {
                                let mut style = canopa_ui::action_style(
                                    theme,
                                    status,
                                    canopa_ui::Action::Normal,
                                    false,
                                );
                                style.border.radius = 8.0.into();
                                style
                            });
                            let opens = fact(fl!("files-properties-opens-with"), opener.clone());
                            // In the narrow panel the application itself is the
                            // button that changes it, so the facts keep one column.
                            details = details.push(if compact {
                                let change = Message::Properties(Event::ChangeOpener(
                                    owner,
                                    item.filesystem.path.clone(),
                                ));
                                let chooser = iced::widget::button(
                                    row![
                                        text(opener.clone())
                                            .font(font)
                                            .size(value_size)
                                            .wrapping(text::Wrapping::WordOrGlyph),
                                        desktop.icons.chrome(chrome::Icon::Down, 12.0, 0.7),
                                    ]
                                    .spacing(6)
                                    .align_y(iced::Alignment::Center),
                                )
                                .on_press(change)
                                .padding([3, 8])
                                .style(|theme, status| {
                                    let mut style = canopa_ui::action_style(
                                        theme,
                                        status,
                                        canopa_ui::Action::Normal,
                                        false,
                                    );
                                    style.border.radius = 7.0.into();
                                    style
                                });
                                let _ = (opens, button);
                                Element::from(
                                    row![
                                        text(fl!("files-properties-opens-with"))
                                            .font(font)
                                            .size(value_size)
                                            .color(muted)
                                            .width(key_width),
                                        chooser,
                                    ]
                                    .spacing(8)
                                    .align_y(iced::Alignment::Center),
                                )
                            } else {
                                row![iced::widget::container(opens).width(Length::Fill), button]
                                    .align_y(iced::Alignment::Center)
                                    .into()
                            });
                        }
                        // One item's heading names its folder; the row gives the exact path.
                        let location = if single {
                            let folder = path
                                .parent()
                                .filter(|parent| !parent.as_os_str().is_empty())
                                .unwrap_or(path);
                            // Home abbreviates to ~ as in a shell; other paths stay absolute.
                            match folder.strip_prefix(&desktop.home) {
                                Ok(relative) if relative.as_os_str().is_empty() => "~".to_owned(),
                                Ok(relative) => format!("~/{}", relative.to_string_lossy()),
                                Err(_) => folder.to_string_lossy().into_owned(),
                            }
                        } else {
                            location
                        };
                        details = details
                            .push(fact(fl!("files-location"), location))
                            .push(fact(
                                fl!("files-column-modified"),
                                metadata
                                    .modified
                                    .map(modified)
                                    .unwrap_or_else(|| fl!("files-unavailable")),
                            ));
                        if let Some(created) = metadata.created {
                            details = details
                                .push(fact(fl!("files-properties-created"), modified(created)));
                        }
                        access = Some(match panel {
                            // In the panel Access is one click away, keeping the
                            // everyday facts in view.
                            Some(window) if !properties.access_open => {
                                access_disclosure(desktop, window, false)
                            }
                            Some(window) => column![
                                access_disclosure(desktop, window, true),
                                access_section(
                                    desktop,
                                    item,
                                    metadata,
                                    key_width,
                                    true,
                                    (owner, properties.advanced_open),
                                    properties.access_error.as_deref(),
                                )
                            ]
                            .spacing(10),
                            None => access_section(
                                desktop,
                                item,
                                metadata,
                                key_width,
                                false,
                                (owner, properties.advanced_open),
                                properties.access_error.as_deref(),
                            ),
                        });
                    }
                }
                let card: Element<'a, Message> = match access {
                    Some(access) if wide && !single => chrome::section_card(
                        appearance,
                        row![
                            iced::widget::container(details).width(Length::FillPortion(3)),
                            iced::widget::container(access).width(Length::FillPortion(2)),
                        ]
                        .spacing(18),
                    )
                    .into(),
                    Some(access) if single => column![
                        chrome::section_card(appearance, details),
                        chrome::section_card(appearance, access)
                    ]
                    .spacing(12)
                    .into(),
                    Some(access) => chrome::section_card(
                        appearance,
                        details
                            .push(iced::widget::rule::horizontal(1).style(chrome::divider))
                            .push(access),
                    )
                    .into(),
                    None => chrome::section_card(appearance, details).into(),
                };
                facts = facts.push(card);
            }
        }
    }
    let _ = size;
    facts.into()
}

/// Who may do what with the item. The owner chooses each class's level from
/// the levels other file managers offer; anyone else reads them.
fn access_section<'a>(
    desktop: &'a Desktop,
    item: &'a Item,
    metadata: &'a crate::file_properties::Metadata,
    key_width: f32,
    stacked: bool,
    (owner, advanced): (window::Id, bool),
    error: Option<&'a str>,
) -> iced::widget::Column<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().label_size.px();
    let body_size = appearance.typography().body_size.px();
    let muted = chrome::ink(appearance, canopa_theme::ColorRole::Muted);
    let directory = metadata.kind == crate::browse::EntryKind::Directory;
    let editable = (directory || metadata.kind == crate::browse::EntryKind::File)
        && metadata.owner == rustix::process::getuid().as_raw();
    let path = item.filesystem.path.clone();
    let line = |who: String, identity: Option<String>, shift: u32| {
        let current = (metadata.permissions >> shift) & 7;
        let value: Element<'a, Message> = if editable {
            let choices = levels(directory, current);
            let selected = choices.iter().find(|level| level.bits == current).cloned();
            let path = path.clone();
            let permissions = metadata.permissions;
            let list = iced::widget::pick_list(choices, selected, move |level: Level| {
                let mode = (permissions & !(7 << shift)) | (level.bits << shift);
                Message::Properties(Event::SetAccess(path.clone(), mode))
            })
            .placeholder(permission_label(current))
            .font(font)
            .text_size(body_size)
            .padding(iced::Padding {
                top: 7.0,
                bottom: 7.0,
                left: 12.0,
                right: 32.0,
            })
            .handle(iced::widget::pick_list::Handle::None)
            .width(Length::Fill)
            .style(move |theme, status| {
                // Rounded like the other controls, with the accent on hover.
                let mut style = iced::widget::pick_list::default(theme, status);
                style.background = chrome::ink(appearance, canopa_theme::ColorRole::Raised).into();
                style.border.radius = 10.0.into();
                style.border.color = match status {
                    iced::widget::pick_list::Status::Active => {
                        chrome::ink(appearance, canopa_theme::ColorRole::Border).scale_alpha(0.6)
                    }
                    _ => chrome::ink(appearance, canopa_theme::ColorRole::Accent),
                };
                style
            })
            .menu_style(move |theme| {
                let mut style = iced::widget::overlay::menu::default(theme);
                style.border.radius = 10.0.into();
                style.background = chrome::ink(appearance, canopa_theme::ColorRole::Raised).into();
                style.selected_background =
                    chrome::ink(appearance, canopa_theme::ColorRole::AccentSurface).into();
                style.selected_text_color = chrome::ink(appearance, canopa_theme::ColorRole::Text);
                style
            });
            // The same thin chevron as the app's other disclosures; it lets
            // clicks through to the list beneath it.
            iced::widget::stack![
                list,
                iced::widget::container(desktop.icons.chrome(chrome::Icon::Down, 14.0, 0.75))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_right(Length::Fill)
                    .center_y(Length::Fill)
                    .padding(iced::Padding {
                        right: 11.0,
                        ..Default::default()
                    }),
            ]
            .into()
        } else {
            text(permission_label(current))
                .font(font)
                .size(body_size)
                .width(Length::Fill)
                .into()
        };
        // The class and its account read as one label: "Owner · kevinard".
        let who = match identity {
            Some(identity) => fl!(
                "files-properties-access-identity",
                who = who,
                identity = identity
            ),
            None => who,
        };
        let who = text(who).font(font).color(muted);
        let line: Element<'a, Message> = if stacked {
            column![who.size(size), value].spacing(4).into()
        } else {
            row![who.size(body_size).width(key_width), value]
                .spacing(12)
                .align_y(iced::Alignment::Center)
                .into()
        };
        line
    };
    let title: Element<'a, Message> = if stacked {
        iced::widget::space().height(0).into()
    } else {
        chrome::section_title(appearance, fl!("files-properties-access")).into()
    };
    let mut access = column![
        title,
        line(fl!("files-properties-owner"), item.owner.clone(), 6),
        line(fl!("files-properties-group"), item.group.clone(), 3),
        line(fl!("files-properties-others"), None, 0),
    ]
    .spacing(10);
    if metadata.kind == crate::browse::EntryKind::File {
        access = access.push(executable_toggle(
            desktop,
            &item.filesystem.path,
            metadata,
            if stacked { size + 1.0 } else { body_size },
        ));
    }
    // Numeric modes and identities are for those who ask for them.
    let toggle = Message::Properties(Event::ToggleAdvanced(owner));
    access = access.push(
        canopa_ui::styled_action(
            // A disclosure: its chevron turns down while the details show.
            row![
                desktop.icons.chrome(
                    if advanced {
                        chrome::Icon::Down
                    } else {
                        chrome::Icon::Forward
                    },
                    14.0,
                    0.85
                ),
                text(fl!("files-properties-advanced"))
                    .font(font)
                    .size(body_size)
                    .style(canopa_ui::link),
                iced::widget::space().width(Length::Fill),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
            Some(toggle),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .padding([3, 0])
        .width(Length::Fill),
    );
    if advanced {
        access = access.push(
            text(fl!(
                "files-properties-mode",
                mode = format!("{:04o}", metadata.permissions),
                uid = metadata.owner.to_string(),
                gid = metadata.group.to_string()
            ))
            .font(font)
            .size(size + 1.0)
            .color(muted),
        );
    }
    if let Some(error) = error {
        access = access.push(
            text(error.to_owned())
                .font(font)
                .size(size + 1.0)
                .color(chrome::ink(appearance, canopa_theme::ColorRole::Danger))
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    }
    access
}

/// The panel's Access heading, which shows or hides the access table.
fn access_disclosure<'a>(
    desktop: &'a Desktop,
    window: window::Id,
    open: bool,
) -> iced::widget::Column<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let mut bold = canopa_ui::font(appearance);
    bold.weight = iced::font::Weight::Semibold;
    let toggle = Message::Properties(Event::TogglePanelAccess(window));
    column![
        canopa_ui::styled_action(
            row![
                text(fl!("files-properties-access"))
                    .font(bold)
                    .size(appearance.typography().body_size.px() + 1.0)
                    .width(Length::Fill),
                desktop.icons.chrome(
                    if open {
                        chrome::Icon::Up
                    } else {
                        chrome::Icon::Down
                    },
                    16.0,
                    0.86
                ),
            ]
            .align_y(iced::Alignment::Center),
            Some(toggle),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .padding([2, 0])
        .width(Length::Fill)
    ]
}

/// The item's picture and name, as both the window and the panel open.
fn header<'a>(
    desktop: &'a Desktop,
    properties: &'a Window,
    side: f32,
) -> (Element<'a, Message>, String, String) {
    let appearance = &desktop.configuration.appearance;
    let first = properties
        .result
        .as_deref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|items| items.first());
    let single = properties.paths.len() == 1;
    let (title, copy_label) = if let [path] = properties.paths.as_slice() {
        (
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            fl!("files-properties-copy-location"),
        )
    } else {
        (
            fl!("files-status-items", count = properties.paths.len()),
            fl!("files-properties-copy-locations"),
        )
    };
    let symbol: Element<'a, Message> = match first.and_then(|item| item.entry.as_ref()) {
        Some(entry) if single => iced::widget::container(desktop.icons.file(entry, side))
            .center_x(side + 8.0)
            .center_y(side + 8.0)
            .into(),
        _ => iced::widget::container(desktop.icons.chrome(
            if single {
                chrome::Icon::Documents
            } else {
                chrome::Icon::Info
            },
            (side * 0.45).round(),
            1.0,
        ))
        .center_x(side + 8.0)
        .center_y(side + 8.0)
        .style(move |_| chrome::symbol_tile(appearance, chrome::Tone::Accent))
        .into(),
    };
    (symbol, title, copy_label)
}

fn window_contents<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    properties: &'a Window,
    wide: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().label_size.px();
    let body_size = appearance.typography().body_size.px();
    let muted = chrome::ink(appearance, canopa_theme::ColorRole::Muted);
    let action = |index, title: String, message: Message, treatment| {
        super::focus::button(
            control(id, index),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(title).font(font).size(body_size),
                Some(message),
                treatment,
                false,
                appearance,
            )
            .padding(chrome::DIALOG_BUTTON_PADDING),
        )
    };
    let (symbol, title, copy_label) = header(desktop, properties, 48.0);
    let mut title_font = font;
    title_font.weight = iced::font::Weight::Semibold;
    // Like the other Files dialogs: the item, then what this window is.
    let heading = row![
        symbol,
        column![
            text(title)
                .font(title_font)
                .size(appearance.typography().title_size.px())
                .wrapping(text::Wrapping::WordOrGlyph),
            text(fl!("files-properties"))
                .font(font)
                .size(size + 1.0)
                .color(muted),
        ]
        .spacing(2)
        .width(Length::Fill),
        close_button(desktop, id),
    ]
    .spacing(14)
    .align_y(iced::Alignment::Center);
    // Properties closes like Preferences and Quick Look, from its heading;
    // the footer keeps the item's own action.
    let actions = canopa_ui::direction::row([
        iced::widget::space().width(Length::Fill).into(),
        action(
            0,
            copy_label,
            Message::Properties(Event::Copy(id)),
            canopa_ui::Action::Normal,
        ),
    ])
    .spacing(8);
    let body = iced::widget::container(
        column![
            heading,
            scrollable(sections(desktop, id, properties, wide, None))
                .style(canopa_ui::scrollbar)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new()
                        .width(6)
                        .scroller_width(6)
                        .spacing(8),
                ))
                .id(scroll(id))
                .width(Length::Fill)
                .height(Length::Fill),
            actions,
        ]
        .spacing(16),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| chrome::window_surface(appearance, true));
    super::focus::group(
        body,
        vec![control(id, 0), control(id, 1)],
        Vec::new(),
        Some(control(id, 1)),
    )
}

/// The side panel's width beside a browser listing.
pub(super) const PANEL_WIDTH: f32 = 320.0;

/// Properties beside browser `id`'s listing, following its selection: a large
/// picture and the same facts and access as the Properties window.
pub(super) fn panel<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    properties: &'a Window,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().label_size.px();
    let muted = chrome::ink(appearance, canopa_theme::ColorRole::Muted);
    let mut bold = font;
    bold.weight = iced::font::Weight::Semibold;
    let (symbol, title, _) = header(desktop, properties, 80.0);
    // One item shows its picture as large as the panel allows, so the panel
    // previews it as well as describing it.
    let picture = (desktop.properties_width(id) - 28.0).max(120.0);
    let symbol: Element<'a, Message> = match properties
        .result
        .as_deref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|items| items.first())
        .and_then(|item| item.entry.as_ref())
    {
        Some(entry) if properties.paths.len() == 1 => {
            let _ = symbol;
            desktop
                .icons
                .file_in(entry, picture, (picture * 0.62).clamp(96.0, 220.0))
        }
        _ => symbol,
    };
    let close = Message::Properties(Event::ClosePanel(id));
    let top = row![
        chrome::section(appearance, fl!("files-properties")),
        iced::widget::space().width(Length::Fill),
        iced::widget::tooltip(
            canopa_ui::styled_action(
                iced::widget::container(desktop.icons.chrome(chrome::Icon::Close, 14.0, 0.86))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
                Some(close),
                canopa_ui::Action::Quiet,
                false,
                appearance,
            )
            .width(26)
            .height(26)
            .padding(3)
            .style(chrome::close_style(appearance)),
            text(fl!("files-properties-hide-panel"))
                .font(font)
                .size(size),
            iced::widget::tooltip::Position::Bottom,
        )
        .padding(6)
        .style(super::chrome::tooltip_style),
    ]
    .align_y(iced::Alignment::Center);
    let identity = column![
        iced::widget::container(symbol).center_x(Length::Fill),
        name_field(desktop, id, properties, title, bold),
    ]
    .spacing(10);
    let _ = muted;
    let body = column![
        top,
        identity,
        sections(desktop, id, properties, false, Some(id))
    ]
    .spacing(14)
    .padding([12, 14]);
    iced::widget::container(
        scrollable(body)
            .style(canopa_ui::scrollbar)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(6)
                    .scroller_width(6)
                    .spacing(2),
            )),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The panel's item name: a field that renames the one selected item when
/// Enter is pressed, or plain text for the open folder and several items.
fn name_field<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    properties: &'a Window,
    title: String,
    font: iced::Font,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().body_size.px() + 2.0;
    let current_folder = desktop
        .active(id)
        .and_then(|tab| tab.browser.as_ref())
        .map(Browser::path);
    let renameable = match (properties.paths.as_slice(), properties.result.as_deref()) {
        ([path], Some(Ok(items))) => {
            Some(path.as_path()) != current_folder
                && items
                    .first()
                    .is_some_and(|item| item.filesystem.metadata.is_ok())
                && desktop.file_operations.ready()
        }
        _ => false,
    };
    if !renameable {
        return text(title)
            .font(font)
            .size(size)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .wrapping(text::Wrapping::WordOrGlyph)
            .into();
    }
    let value = properties.name_draft.clone().unwrap_or(title);
    let mut input = iced::widget::text_input(&fl!("files-column-name"), &value)
        .id(iced::widget::Id::from(format!(
            "files-properties-name-{id:?}"
        )))
        .font(font)
        .size(size)
        .align_x(iced::alignment::Horizontal::Center)
        .padding([4, 8])
        // A title until pointed at or focused, then a field.
        .style(move |theme, status| {
            let mut style = super::view::input_style(appearance, theme, status);
            if matches!(status, iced::widget::text_input::Status::Active) {
                style.border.color = iced::Color::TRANSPARENT;
                style.background = iced::Color::TRANSPARENT.into();
            }
            style
        });
    if !properties.renaming {
        input = input
            .on_input(move |name| Message::Properties(Event::NameEdited(id, name)))
            .on_submit(Message::Properties(Event::Rename(id)));
    }
    let node = super::accessibility_ui::Node::new(super::accessibility_ui::Role::TextInput)
        .key("files-properties-name")
        .label(fl!("files-name-full-name"))
        .value(value.clone())
        .invalid(properties.name_error.is_some())
        .read_only(properties.renaming)
        .on_text(move |name| Message::Properties(Event::NameEdited(id, name)));
    let mut field = column![super::accessibility_ui::semantic(node, input)].spacing(4);
    if let Some(error) = &properties.name_error {
        field = field.push(
            text(error.clone())
                .font(canopa_ui::font(appearance))
                .size(appearance.typography().label_size.px() + 1.0)
                .color(chrome::ink(appearance, canopa_theme::ColorRole::Danger))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    } else if properties.name_draft.is_some() {
        field = field.push(
            text(fl!("files-properties-rename-hint"))
                .font(canopa_ui::font(appearance))
                .size(appearance.typography().label_size.px())
                .color(chrome::ink(appearance, canopa_theme::ColorRole::Muted))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        );
    }
    field.into()
}

/// "Allow executing as a program" for a regular file: its owner turns the
/// execute permission on for each class that may read the file, or off for all.
fn executable_toggle<'a>(
    desktop: &'a Desktop,
    path: &Path,
    metadata: &crate::file_properties::Metadata,
    size: f32,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let permissions = metadata.permissions;
    let executable = permissions & 0o111 != 0;
    let mode = if executable {
        permissions & !0o111
    } else {
        permissions | ((permissions & 0o444) >> 2) | 0o100
    };
    let owner = metadata.owner == rustix::process::getuid().as_raw();
    let message = owner.then(|| Message::Properties(Event::SetAccess(path.to_owned(), mode)));
    let toggle = message.clone();
    let mut check = iced::widget::checkbox(executable)
        .label(fl!("files-properties-executable"))
        .font(canopa_ui::font(appearance))
        .text_size(size)
        .size(16)
        .style(canopa_ui::checkbox_style);
    if let Some(toggle) = toggle {
        check = check.on_toggle(move |_| toggle.clone());
    }
    super::accessibility_ui::check_box("files-properties-executable", executable, message, check)
}

fn kind(item: &Item, metadata: &crate::file_properties::Metadata) -> String {
    match metadata.kind {
        crate::browse::EntryKind::Directory => fl!("files-kind-folder"),
        crate::browse::EntryKind::File => {
            item.kind.clone().unwrap_or_else(|| fl!("files-kind-file"))
        }
        crate::browse::EntryKind::Symlink => fl!("files-kind-symlink"),
        crate::browse::EntryKind::Other => fl!("files-kind-special-file"),
    }
}

/// The heading's round close button, the window's default control.
fn close_button(desktop: &Desktop, id: window::Id) -> Element<'_, Message> {
    let appearance = &desktop.configuration.appearance;
    let message = Some(Message::Close(id));
    let button = canopa_ui::styled_action(
        iced::widget::container(desktop.icons.chrome(chrome::Icon::Close, 16.0, 0.86))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
        message.clone(),
        canopa_ui::Action::Quiet,
        false,
        appearance,
    )
    .width(32)
    .height(32)
    .padding(4)
    .style(chrome::close_style(appearance));
    super::accessibility_ui::named_button(
        super::accessibility_ui::focus_key(&control(id, 1)),
        fl!("files-close-window"),
        message.clone(),
        super::focus::control(control(id, 1), message, appearance, button),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn access_levels_keep_a_file_execute_bit_and_name_folder_levels() {
        let file: Vec<u32> = levels(false, 5).iter().map(|level| level.bits).collect();
        assert_eq!(file, [1, 5, 7]);
        let plain: Vec<u32> = levels(false, 6).iter().map(|level| level.bits).collect();
        assert_eq!(plain, [0, 4, 6]);
        let folder: Vec<u32> = levels(true, 7).iter().map(|level| level.bits).collect();
        assert_eq!(folder, [0, 4, 5, 7]);
    }

    #[test]
    fn property_dates_preserve_pre_epoch_fraction_and_refuse_overflow_without_panicking() {
        let before = std::time::UNIX_EPOCH - Duration::from_nanos(1);
        let parsed = timestamp(before).unwrap();
        assert_eq!(parsed.timestamp(), -1);
        assert_eq!(parsed.timestamp_subsec_nanos(), 999_999_999);
        let huge = std::time::UNIX_EPOCH
            .checked_add(Duration::from_secs(i64::MAX as u64))
            .unwrap();
        assert_eq!(modified(huge), "Outside supported date range");
    }
}
