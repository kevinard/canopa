//! Iced application windows. Filesystem snapshots stay in the shared core and
//! are prepared on workers; UI messages carry a tab identity and scan revision.
mod action_editor;
mod actions_ui;
mod activation_ui;
mod catalog_notices;
mod chrome;
mod clipboard_ui;
mod configuration_ui;
mod delete_dialog;
#[cfg(test)]
mod delete_tests;
mod destination_browser;
mod device_question;
mod devices_ui;
mod drag_geometry;
mod drag_ui;
mod file_icon;
mod file_operation_rows;
mod file_operations;
mod focus;
mod folder_views;
#[cfg(test)]
mod folder_watch_tests;
mod folder_watch_ui;
mod icons_ui;
mod jobs_ui;
mod listing_ui;
mod loading;
mod media_ui;
mod name_dialog;
mod operation_details;
#[cfg(test)]
mod operation_tests;
mod preferences_editor;
mod preferences_view;
mod preview_highlights;
mod preview_ui;
mod properties_ui;
mod question_dialog;
mod recovery_destination;
mod recovery_ui;
mod row_state;
mod scrolling;
#[cfg(test)]
mod search_tests;
mod search_ui;
#[cfg(test)]
mod tests;
mod toolbar;
mod toolbar_layout;
mod transfer_dialog;
mod trash_restore;
mod trash_ui;
mod view;

use crate::{
    Cancellation,
    browse::{Browser, Directory, HistoryDirection, SelectionMode},
    configuration::{ConfigurationJobs, ConfigurationSnapshot},
    interface::{Command, KeyChord, SidebarMode},
    personal_folders::PersonalFolders,
    preferences::PreferenceStore,
};
use iced::{Element, Event, Size, Subscription, Task, Theme, keyboard, window};
use std::{
    collections::BTreeMap,
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

pub fn run(
    command: crate::activation::Command,
    selection: Option<crate::activation::Configuration>,
) -> io::Result<std::process::ExitCode> {
    use crate::activation::{Claim, Instance};
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::other("an absolute HOME is required"))?;
    let directory = crate::application::state_directory(&home);
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR is required for Files activation"))?;
    let can_open = ["WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY"]
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()));
    let instance = match Instance::launch(&directory, &runtime, selection, command, can_open)? {
        Claim::Primary(instance) => instance,
        Claim::Forwarded | Claim::Absent => return Ok(std::process::ExitCode::SUCCESS),
    };
    let mut application =
        crate::application::Application::start(&directory, &instance.configuration().request())?;
    if let Err(error) = application.serve_file_manager(instance.clone()) {
        eprintln!("canopa-files: FileManager1 is unavailable: {error}");
    }
    // Iced and native clipboard transport are first initialized for an actual
    // Browser. Resident launch waiting itself requires no display connection.
    while !instance.wait_for_open() {
        if !application.holds_application() && instance.try_exit() {
            finish_application(&application)?;
            return Ok(std::process::ExitCode::SUCCESS);
        }
    }
    let configuration = application.configuration_snapshot();
    let preference_path = directory.join("views.json");
    let preferences = PreferenceStore::load(&preference_path, configuration.settings.view.clone())?;
    let personal_config = canopa_config::config_home_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        Some(home.as_os_str()),
    )
    .map_err(io::Error::other)?;
    let personal_home = home.clone();
    let personal_folders = std::thread::Builder::new()
        .name("files-personal-folders".into())
        .spawn(move || PersonalFolders::load(&personal_home, &personal_config))?
        .join()
        .map_err(|_| io::Error::other("personal-folder worker stopped"))??;
    let configuration_frontend = application.configuration.clone();
    let operations_frontend = application.operations.clone();
    let actions_frontend = application.actions.clone();
    let playback_frontend = application.playback.clone();
    let activity_frontend = application.activity().clone();
    let frontend_instance = instance.clone();
    let font = canopa_ui::font(&configuration.appearance);
    iced::daemon(
        move || {
            let mut openings = frontend_instance.take_openings().into_iter();
            let first = openings
                .next()
                .expect("the host waits for an accepted Browser opening");
            let (mut desktop, task) = Desktop::boot(
                None,
                configuration.clone(),
                preferences.clone(),
                home.clone(),
                preference_path.clone(),
                personal_folders.clone(),
                configuration_frontend.clone(),
            );
            desktop.activation_notices = frontend_instance
                .take_notices()
                .map(activation_ui::Notices::new);
            desktop.instance = Some(frontend_instance.clone());
            desktop.activity_notices = activity_frontend
                .take_notices()
                .map(activation_ui::Notices::new);
            desktop.activity = Some(activity_frontend.clone());
            desktop.media = media_ui::Host::attach(playback_frontend.clone());
            let mut tasks = vec![task];
            desktop.file_clipboard = clipboard_ui::State::start();
            desktop.folder_watch = folder_watch_ui::State::start();
            desktop.jobs = jobs_ui::Jobs::attach(actions_frontend.clone());
            tasks.push(desktop.attach_file_operations(operations_frontend.clone()));
            tasks.push(desktop.open_requested(first));
            for opening in openings {
                tasks.push(desktop.open_requested(opening));
            }
            (desktop, Task::batch(tasks))
        },
        Desktop::update,
        Desktop::view,
    )
    .title(Desktop::title)
    .theme(|desktop: &Desktop, _| desktop.theme.clone())
    .subscription(Desktop::subscription)
    .default_font(font)
    .run()
    .map_err(io::Error::other)?;
    finish_application(&application)?;
    // Queue shutdown acknowledgements precede releasing instance ownership.
    drop(application);
    drop(instance);
    Ok(std::process::ExitCode::SUCCESS)
}

fn finish_application(application: &crate::application::Application) -> io::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(application.shutdown())
}

struct Desktop {
    activity: Option<crate::local_activity::LocalActivity>,
    activity_notices: Option<activation_ui::Notices>,
    instance: Option<crate::activation::Instance>,
    activation_notices: Option<activation_ui::Notices>,
    windows: BTreeMap<window::Id, BrowserWindow>,
    previews: BTreeMap<window::Id, preview_ui::PreviewWindow>,
    properties: BTreeMap<window::Id, properties_ui::Window>,
    media: media_ui::Host,
    configuration: ConfigurationSnapshot,
    preferences: PreferenceStore,
    home: PathBuf,
    personal_folders: PersonalFolders,
    theme: Theme,
    next_tab: u64,
    configuration_worker: Option<ConfigurationJobs>,
    configuration_notices: Option<configuration_ui::Notices>,
    configuration_revision: u64,
    configuration_applied: u64,
    configuration_error: Option<String>,
    action_editor: Option<action_editor::Editor>,
    preferences_editor: Option<preferences_editor::Editor>,
    configuration_request: u64,
    configuration_pending: Option<(u64, window::Id, action_editor::Operation)>,
    jobs: jobs_ui::Jobs,
    operations_window: Option<window::Id>,
    action_menu: Option<actions_ui::Menu>,
    icons: icons_ui::Icons,
    folder_writer: folder_views::Writer,
    folder_options: Option<folder_views::Target>,
    folder_watch: folder_watch_ui::State,
    file_operations: file_operations::State,
    file_clipboard: clipboard_ui::State,
    drag: drag_ui::State,
    devices: devices_ui::State,
    focused_window: Option<window::Id>,
    trash_window: Option<trash_ui::TrashWindow>,
    trash_restore: Option<trash_restore::Dialog>,
    delete_dialog: Option<delete_dialog::Dialog>,
    recovery_window: Option<recovery_ui::RecoveryWindow>,
    recovery_destination: Option<recovery_destination::Dialog>,
}

struct BrowserWindow {
    tabs: Vec<Tab>,
    active: usize,
    width: f32,
    size_confirmed: bool,
    modifiers: keyboard::Modifiers,
}

struct Tab {
    id: u64,
    browser: Option<Browser>,
    columns: BTreeMap<PathBuf, loading::ColumnSnapshot>,
    next_column_request: u64,
    scan_cancel: Cancellation,
    target: PathBuf,
    initial_selection: Vec<PathBuf>,
    preview_after_load: Option<PathBuf>,
    revision: u64,
    loading: bool,
    error: Option<String>,
    location: Option<String>,
    prefix: String,
    last_typing: Option<Instant>,
    viewport_height: f32,
    viewport_width: f32,
    reveal_on_show: bool,
    search: Option<search_ui::SearchSession>,
    search_serial: u64,
    list_epoch: u64,
    search_worker: Option<search_ui::Worker>,
    menu_after_load: Option<u64>,
    needs_refresh: bool,
    /// Outside changes arrived while this tab could not rescan.
    watch_stale: bool,
    watch_scanning: bool,
    watch_request: u64,
    watch_cancel: Cancellation,
    /// The displayed error came from a watch rescan and clears on success.
    watch_error: bool,
}

impl Tab {
    fn path(&self) -> &Path {
        self.browser
            .as_ref()
            .map_or(self.target.as_path(), Browser::path)
    }

    fn column_paths(&self, width: f32, sidebar: SidebarMode) -> Vec<PathBuf> {
        self.browser.as_ref().map_or_else(Vec::new, |browser| {
            if browser.search().is_some()
                || browser.view().mode != crate::preferences::ViewMode::Columns
            {
                return vec![browser.path().to_owned()];
            }
            browser.column_paths_for_content(
                width
                    - if sidebar_visible(sidebar, width) {
                        chrome::SIDEBAR_WIDTH + 1.0
                    } else {
                        0.0
                    },
            )
        })
    }
}

impl Drop for Tab {
    fn drop(&mut self) {
        self.scan_cancel.cancel();
        self.watch_cancel.cancel();
    }
}

impl Tab {
    /// A full scan supersedes outstanding watch rescans of this tab.
    fn supersede_watch(&mut self) {
        self.watch_cancel.cancel();
        self.watch_scanning = false;
        self.watch_stale = false;
        if std::mem::take(&mut self.watch_error) {
            self.error = None;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Navigation {
    Visit,
    History(HistoryDirection),
    Refresh,
}

#[derive(Clone, Debug)]
struct Scan {
    directory: Directory,
    selected: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug)]
struct ScrollUpdate {
    offset: f32,
    rendered: Size,
    measured: Size,
}

#[derive(Clone, Debug)]
enum Message {
    Properties(properties_ui::Event),
    ActivationPoll,
    MediaPoll,
    ActivityPoll,
    Device(devices_ui::Event),
    Focus(window::Id, bool),
    FileOperation(file_operations::Event),
    FileClipboard(clipboard_ui::Event),
    Drag(drag_ui::Event),
    Opened(window::Id),
    Close(window::Id),
    Closed(window::Id),
    Resized(window::Id, Size),
    Modifiers(window::Id, keyboard::Modifiers),
    Key(
        window::Id,
        keyboard::Key,
        keyboard::Modifiers,
        Option<String>,
    ),
    Command(window::Id, Command),
    Loaded(window::Id, u64, u64, Navigation, Arc<Result<Scan, String>>),
    ColumnLoaded {
        window: window::Id,
        tab: u64,
        revision: u64,
        path: PathBuf,
        request: u64,
        result: Arc<Result<Directory, String>>,
    },
    FoldersChanged,
    WatchLoaded {
        window: window::Id,
        tab: u64,
        revision: u64,
        request: u64,
        result: Arc<Result<Directory, (bool, String)>>,
    },
    Select(window::Id, PathBuf),
    Activate(window::Id, PathBuf),
    Go(window::Id, PathBuf),
    SwitchTab(window::Id, usize),
    Location(window::Id, String),
    SubmitLocation(window::Id),
    Scroll(window::Id, u64, u64, PathBuf, ScrollUpdate),
    Viewport(window::Id, u64, u64, PathBuf, Size),
    Error(window::Id, u64, String),
    Search(search_ui::Event),
    Preview(preview_ui::Event),
    ConfigurationPoll,
    ActionEditor(action_editor::Event),
    Preferences(preferences_editor::Event),
    CapturedKey(window::Id, keyboard::Key, keyboard::Modifiers),
    Job(jobs_ui::Event),
    NewBrowser(window::Id),
    Action(actions_ui::Event),
    IconsReady(Arc<icons_ui::Prepared>),
    FolderView(folder_views::Event),
    Trash(trash_ui::Event),
    Recovery(recovery_ui::Event),
    RecoveryDestination(recovery_destination::Event),
    TrashRestore(trash_restore::Event),
    DeleteDialog(delete_dialog::Event),
}

impl Desktop {
    fn boot(
        paths: Option<Vec<PathBuf>>,
        configuration: ConfigurationSnapshot,
        preferences: PreferenceStore,
        home: PathBuf,
        preference_path: PathBuf,
        personal_folders: PersonalFolders,
        configuration_worker: Result<ConfigurationJobs, String>,
    ) -> (Self, Task<Message>) {
        let theme = canopa_ui::theme(configuration.appearance.clone());
        let configuration_error = configuration_worker.as_ref().err().cloned();
        let configuration_worker = configuration_worker.ok();
        let configuration_notices = configuration_worker
            .as_ref()
            .and_then(ConfigurationJobs::take_notices)
            .map(configuration_ui::Notices::new);
        let mut desktop = Self {
            activity: None,
            activity_notices: None,
            instance: None,
            activation_notices: None,
            windows: BTreeMap::new(),
            previews: BTreeMap::new(),
            properties: BTreeMap::new(),
            media: media_ui::Host::default(),
            configuration,
            preferences,
            home,
            personal_folders,
            theme,
            next_tab: 0,
            configuration_worker,
            configuration_notices,
            configuration_revision: 0,
            configuration_applied: 0,
            configuration_error,
            action_editor: None,
            preferences_editor: None,
            configuration_request: 0,
            configuration_pending: None,
            jobs: jobs_ui::Jobs::default(),
            operations_window: None,
            action_menu: None,
            icons: icons_ui::Icons::default(),
            folder_writer: folder_views::Writer::new(preference_path),
            folder_options: None,
            folder_watch: folder_watch_ui::State::default(),
            file_operations: file_operations::State::default(),
            file_clipboard: clipboard_ui::State::default(),
            drag: drag_ui::State::default(),
            devices: devices_ui::State::default(),
            focused_window: None,
            trash_window: None,
            trash_restore: None,
            delete_dialog: None,
            recovery_window: None,
            recovery_destination: None,
        };
        let icons = desktop.icons.refresh(&desktop.configuration);
        let initial = paths.map_or_else(Task::none, |paths| {
            desktop.new_window(if paths.is_empty() {
                vec![desktop.home.clone()]
            } else {
                paths
            })
        });
        let task = Task::batch([initial, icons]);
        (desktop, task)
    }

    fn new_tab(&mut self, path: PathBuf) -> Tab {
        self.next_tab += 1;
        Tab {
            id: self.next_tab,
            browser: None,
            columns: BTreeMap::new(),
            next_column_request: 0,
            scan_cancel: Cancellation::default(),
            target: path,
            initial_selection: Vec::new(),
            preview_after_load: None,
            revision: 0,
            loading: false,
            error: None,
            location: None,
            prefix: String::new(),
            last_typing: None,
            viewport_height: 0.0,
            viewport_width: 0.0,
            reveal_on_show: false,
            search: None,
            search_serial: 0,
            list_epoch: 0,
            search_worker: None,
            menu_after_load: None,
            needs_refresh: false,
            watch_stale: false,
            watch_scanning: false,
            watch_request: 0,
            watch_cancel: Cancellation::default(),
            watch_error: false,
        }
    }

    fn new_window(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        self.new_target_window(
            paths
                .into_iter()
                .map(|directory| crate::presentation::BrowserTarget {
                    directory,
                    selected: Vec::new(),
                })
                .collect(),
        )
    }

    fn new_target_window(
        &mut self,
        targets: Vec<crate::presentation::BrowserTarget>,
    ) -> Task<Message> {
        self.new_target_window_with_preview(targets, false)
    }
    fn new_target_window_with_preview(
        &mut self,
        targets: Vec<crate::presentation::BrowserTarget>,
        preview: bool,
    ) -> Task<Message> {
        let settings = window::Settings {
            size: Size::new(1000.0, 640.0),
            min_size: Some(Size::new(280.0, 220.0)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let (id, opened) = window::open(settings);
        let tabs = targets
            .into_iter()
            .map(|target| {
                let mut tab = self.new_tab(target.directory);
                tab.preview_after_load = if preview {
                    target.selected.first().cloned()
                } else {
                    None
                };
                tab.initial_selection = target.selected;
                tab
            })
            .collect();
        self.drag.open(id);
        self.windows.insert(
            id,
            BrowserWindow {
                tabs,
                active: 0,
                width: 1000.0,
                size_confirmed: false,
                modifiers: keyboard::Modifiers::default(),
            },
        );
        let target = self.active(id).unwrap().target.clone();
        Task::batch([
            opened.then(move |window| {
                window::drag_drop::enable(window)
                    .map(move |endpoint| Message::Drag(drag_ui::Event::Enabled(window, endpoint)))
            }),
            self.navigate(id, target, Navigation::Visit, None),
            self.present_initial_recovery(),
        ])
    }

    fn active(&self, id: window::Id) -> Option<&Tab> {
        let window = self.windows.get(&id)?;
        window.tabs.get(window.active)
    }

    fn active_mut(&mut self, id: window::Id) -> Option<&mut Tab> {
        let window = self.windows.get_mut(&id)?;
        window.tabs.get_mut(window.active)
    }

    fn navigate(
        &mut self,
        window: window::Id,
        target: PathBuf,
        navigation: Navigation,
        selected: Option<PathBuf>,
    ) -> Task<Message> {
        let home = self.home.clone();
        let Some(tab) = self.active_mut(window) else {
            return Task::none();
        };
        let initial_selection = if tab.revision == 0 && tab.target == target && selected.is_none() {
            std::mem::take(&mut tab.initial_selection)
        } else {
            tab.initial_selection.clear();
            Vec::new()
        };
        if tab.revision != 0 {
            tab.preview_after_load = None;
        }
        tab.dismiss_search();
        tab.revision += 1;
        tab.scan_cancel.cancel();
        tab.scan_cancel = Cancellation::default();
        let cancellation = tab.scan_cancel.clone();
        tab.columns.clear();
        tab.loading = true;
        tab.error = None;
        tab.location = None;
        tab.menu_after_load = None;
        tab.target = target.clone();
        tab.needs_refresh = false;
        tab.supersede_watch();
        tab.prefix.clear();
        let (tab_id, revision) = (tab.id, tab.revision);
        let base = tab
            .browser
            .as_ref()
            .map(|browser| browser.path().to_owned());
        Task::perform(
            async move {
                scan(target, &home, base.as_deref(), selected, &cancellation)
                    .map(|mut scan| {
                        if !initial_selection.is_empty() {
                            scan.selected = initial_selection;
                        }
                        scan
                    })
                    .map_err(|error| error.to_string())
            },
            move |result| Message::Loaded(window, tab_id, revision, navigation, Arc::new(result)),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = match message {
            Message::Properties(event) => self.properties_event(event),
            Message::Drag(event) => self.drag_event(event),
            Message::Key(_, keyboard::Key::Named(keyboard::key::Named::Escape), _, _)
                if self.cancel_drag() =>
            {
                Task::none()
            }
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self.properties.contains_key(&id) =>
            {
                self.properties_key(id, key, modifiers)
            }
            Message::Close(id) | Message::Closed(id) if self.properties.contains_key(&id) => {
                self.close_properties(id)
            }
            Message::Device(event) => self.device_event(event),
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self
                    .devices
                    .dialog
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.device_question_key(id, key, modifiers)
            }
            Message::Close(id)
                if self
                    .devices
                    .dialog
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.device_question_event(devices_ui::Event::CancelQuestion(id))
            }
            Message::Closed(id)
                if self
                    .devices
                    .dialog
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.dismiss_device_question(id)
            }
            Message::Recovery(event) => self.recovery_event(event),
            Message::RecoveryDestination(event) => self.recovery_destination_event(event),
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self
                    .recovery_destination
                    .as_ref()
                    .is_some_and(|d| d.window == id) =>
            {
                self.recovery_destination_key(id, key, modifiers)
            }
            Message::Close(id) | Message::Closed(id)
                if self
                    .recovery_destination
                    .as_ref()
                    .is_some_and(|d| d.window == id) =>
            {
                self.close_recovery_destination(id)
            }
            Message::Key(id, key, modifiers, _)
                if self
                    .recovery_window
                    .as_ref()
                    .is_some_and(|w| w.window == id) =>
            {
                self.recovery_key(id, key, modifiers)
            }
            Message::Close(id) | Message::Closed(id)
                if self
                    .recovery_window
                    .as_ref()
                    .is_some_and(|w| w.window == id) =>
            {
                self.close_recovery(id)
            }
            Message::DeleteDialog(event) => self.delete_dialog_event(event),
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self
                    .delete_dialog
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.delete_dialog_key(id, key, modifiers)
            }
            Message::Close(id) | Message::Closed(id)
                if self
                    .delete_dialog
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.close_delete_dialog(id)
            }
            Message::Trash(event) => self.trash_event(event),
            Message::TrashRestore(event) => self.trash_restore_event(event),
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self
                    .trash_restore
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.trash_restore_key(id, key, modifiers)
            }
            Message::Close(id) | Message::Closed(id)
                if self
                    .trash_restore
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.close_trash_restore(id)
            }
            Message::Key(id, key, modifiers, _)
                if self
                    .trash_window
                    .as_ref()
                    .is_some_and(|trash| trash.window == id) =>
            {
                self.trash_key(id, key, modifiers)
            }
            Message::Close(id) | Message::Closed(id)
                if self
                    .trash_window
                    .as_ref()
                    .is_some_and(|trash| trash.window == id) =>
            {
                self.close_trash(id)
            }
            Message::ActivationPoll => self.activation_poll(),
            Message::ActivityPoll => self.exit_if_empty(),
            Message::MediaPoll => self.media_poll(),
            Message::FileOperation(event) => self.file_operation_event(event),
            Message::FileClipboard(event) => self.clipboard_event(event),
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self
                    .file_operations
                    .details
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.operation_details_key(id, key, modifiers)
            }
            Message::Close(id) | Message::Closed(id)
                if self
                    .file_operations
                    .details
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.close_operation_details(id)
            }
            Message::Focus(id, focused) => {
                if focused {
                    self.focused_window = Some(id);
                } else if self.focused_window == Some(id) {
                    self.focused_window = None;
                }
                let close = if !focused {
                    self.action_event(actions_ui::Event::Close(id))
                } else {
                    Task::none()
                };
                Task::batch([
                    close,
                    self.auto_file_question(),
                    self.auto_device_question(),
                ])
            }
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self
                    .file_operations
                    .name
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.name_key(id, key, modifiers)
            }
            Message::Close(id)
                if self
                    .file_operations
                    .name
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.close_name_dialog(id, false)
            }
            Message::Closed(id)
                if self
                    .file_operations
                    .name
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.close_name_dialog(id, true)
            }
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self
                    .file_operations
                    .question
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.file_question_key(id, key, modifiers)
            }
            Message::Close(id) | Message::Closed(id)
                if self
                    .file_operations
                    .question
                    .as_ref()
                    .is_some_and(|dialog| dialog.window == id) =>
            {
                self.close_file_question(id)
            }
            Message::IconsReady(prepared) => self.icons.completed(&prepared),
            Message::FolderView(event) => self.folder_view_event(event),
            Message::Action(event) => self.action_event(event),
            Message::Job(event) => self.job_event(event),
            Message::Key(id, key, modifiers, _) if self.operations_window == Some(id) => {
                self.operations_key(id, key, modifiers)
            }
            Message::Close(id)
                if self.operations_window == Some(id)
                    && self.windows.is_empty()
                    && self
                        .file_operations
                        .snapshot
                        .jobs
                        .iter()
                        .any(|job| job.phase.needs_save()) =>
            {
                self.file_operations.error =
                    Some("Save the operation result before closing the last window.".into());
                Task::none()
            }
            Message::Closed(id)
                if self.operations_window == Some(id)
                    && self.windows.is_empty()
                    && self
                        .file_operations
                        .snapshot
                        .jobs
                        .iter()
                        .any(|job| job.phase.needs_save()) =>
            {
                let close_details = self.close_operation_details_parent(id);
                self.operations_window = None;
                Task::batch([close_details, self.show_operations()])
            }
            Message::Close(id) | Message::Closed(id) if self.operations_window == Some(id) => {
                let close_details = self.close_operation_details_parent(id);
                let device_question = self.close_device_parent(id);
                self.operations_window = None;
                Task::batch([
                    close_details,
                    device_question,
                    window::close(id),
                    self.exit_if_empty(),
                ])
            }
            Message::ActionEditor(event) => self.action_editor_event(event),
            Message::Preferences(event) => self.preferences_event(event),
            Message::Key(id, key, modifiers, _) | Message::CapturedKey(id, key, modifiers)
                if self
                    .preferences_editor
                    .as_ref()
                    .is_some_and(|editor| editor.window == id) =>
            {
                self.preferences_key(id, key, modifiers)
            }
            Message::Close(id)
                if self
                    .preferences_editor
                    .as_ref()
                    .is_some_and(|editor| editor.window == id) =>
            {
                self.close_preferences(id, false)
            }
            Message::Closed(id)
                if self
                    .preferences_editor
                    .as_ref()
                    .is_some_and(|editor| editor.window == id) =>
            {
                self.close_preferences(id, true)
            }
            Message::Key(id, key, modifiers, _)
                if self
                    .action_editor
                    .as_ref()
                    .is_some_and(|editor| editor.window == id) =>
            {
                self.action_editor_key(id, key, modifiers, false)
            }
            Message::CapturedKey(id, key, modifiers)
                if self
                    .action_editor
                    .as_ref()
                    .is_some_and(|editor| editor.window == id) =>
            {
                self.action_editor_key(id, key, modifiers, true)
            }
            Message::Close(id)
                if self
                    .action_editor
                    .as_ref()
                    .is_some_and(|editor| editor.window == id) =>
            {
                self.close_action_editor(id, false)
            }
            Message::Closed(id)
                if self
                    .action_editor
                    .as_ref()
                    .is_some_and(|editor| editor.window == id) =>
            {
                self.close_action_editor(id, true)
            }
            Message::Preview(event) => self.preview_event(event),
            Message::Key(id, key, modifiers, text) if self.previews.contains_key(&id) => {
                self.preview_key(id, key, modifiers, text)
            }
            Message::Close(id) if self.previews.contains_key(&id) => self.dismiss_preview(id),
            Message::Closed(id) if self.previews.contains_key(&id) => {
                self.previews.remove(&id);
                Task::none()
            }
            Message::Close(id) | Message::Closed(id) => {
                // Observe already received drops while their rendered context is
                // still present. Their final reads/admission outlive this window.
                let drop = self.poll_drag();
                self.drag.close(id);
                let close = self.close_source_previews(id);
                let files = self.close_operation_source(id);
                let device_question = self.close_device_parent(id);
                Task::batch([
                    drop,
                    close,
                    files,
                    device_question,
                    self.update_browser(message),
                ])
            }
            other => self.update_browser(other),
        };
        self.sync_action_menu();
        self.sync_folder_options();
        self.sync_folder_watch();
        let drag = self.poll_drag();
        Task::batch([task, self.sync_previews(), drag])
    }

    fn update_browser(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ActivationPoll
            | Message::Device(_)
            | Message::ActivityPoll
            | Message::MediaPoll
            | Message::Focus(..)
            | Message::FileOperation(_)
            | Message::Recovery(_)
            | Message::RecoveryDestination(_)
            | Message::Trash(_)
            | Message::TrashRestore(_)
            | Message::DeleteDialog(_)
            | Message::FileClipboard(_)
            | Message::Properties(_)
            | Message::Drag(_) => {}
            Message::ConfigurationPoll => return self.configuration_poll(),
            Message::ActionEditor(_) | Message::Preferences(_) | Message::CapturedKey(..) => {}
            Message::Job(_) => {}
            Message::Action(_) | Message::IconsReady(_) | Message::FolderView(_) => {}
            Message::NewBrowser(id) => {
                if self.operations_window == Some(id) {
                    return self.new_window(vec![self.home.clone()]);
                }
            }
            Message::Opened(_id) => {}
            Message::Preview(_) => {}
            Message::Close(id) => {
                let browser = self.windows.remove(&id).is_some();
                let tail = if browser
                    && self.windows.is_empty()
                    && (self.jobs.has_active() || self.file_operations.has_active())
                    && self.operations_window.is_none()
                    && self.trash_window.is_none()
                    && self.recovery_window.is_none()
                {
                    self.show_operations()
                } else {
                    self.exit_if_empty()
                };
                return Task::batch([window::close(id), tail]);
            }
            Message::Closed(id) => {
                let browser = self.windows.remove(&id).is_some();
                return if browser
                    && self.windows.is_empty()
                    && (self.jobs.has_active() || self.file_operations.has_active())
                    && self.operations_window.is_none()
                    && self.trash_window.is_none()
                    && self.recovery_window.is_none()
                {
                    self.show_operations()
                } else {
                    self.exit_if_empty()
                };
            }
            Message::Resized(id, size) => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.width = size.width;
                    window.size_confirmed = true;
                }
                // Column count changes reorder the ordinary Row's widget tree.
                // Restore from the Browser model into the rebuilt current list.
                return Task::batch([
                    self.restore_scroll(id),
                    self.load_columns(id),
                    self.present_requested_preview(id),
                ]);
            }
            Message::Modifiers(id, modifiers) => {
                self.drag.modifiers(modifiers);
                if let Some(window) = self.windows.get_mut(&id) {
                    window.modifiers = modifiers;
                }
            }
            Message::Key(id, key, modifiers, text) => return self.key(id, key, modifiers, text),
            Message::Command(id, command) => return self.command(id, command),
            Message::Search(event) => return self.search_event(event),
            Message::Go(id, path) => return self.navigate(id, path, Navigation::Visit, None),
            Message::Loaded(id, tab_id, revision, navigation, result) => {
                let appearance = self.configuration.appearance.clone();
                let Some(window) = self.windows.get_mut(&id) else {
                    return Task::none();
                };
                let active_tab = window.tabs.get(window.active).map(|tab| tab.id) == Some(tab_id);
                let Some(tab) = window
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.id == tab_id && tab.revision == revision)
                else {
                    return Task::none();
                };
                tab.loading = false;
                if !active_tab {
                    tab.preview_after_load = None;
                }
                match Arc::try_unwrap(result).unwrap_or_else(|shared| (*shared).clone()) {
                    Ok(scan) => {
                        tab.list_epoch += 1;
                        let reveal = !scan.selected.is_empty()
                            || tab.browser.as_ref().is_some_and(|browser| {
                                browser.path().parent() == Some(scan.directory.path.as_path())
                            });
                        let view = self.preferences.for_folder(&scan.directory.path).clone();
                        if let Some(browser) = &mut tab.browser {
                            match navigation {
                                Navigation::Visit => browser.visit(scan.directory),
                                Navigation::History(direction) => {
                                    browser.visit_history(scan.directory, direction);
                                }
                                Navigation::Refresh => {
                                    browser.refresh(scan.directory);
                                }
                            }
                        } else {
                            tab.browser = Some(Browser::new(scan.directory));
                        }
                        let browser = tab.browser.as_mut().unwrap();
                        let _ = browser.set_view(view);
                        if !scan.selected.is_empty() {
                            browser.reveal_selection(scan.selected);
                        }
                        tab.target = browser.path().to_owned();
                        tab.columns.clear();
                        tab.reveal_on_show = reveal;
                        if reveal {
                            listing_ui::Geometry::new(
                                &appearance,
                                browser.view(),
                                false,
                                tab.viewport_width,
                            )
                            .reveal(browser, tab.viewport_height);
                        }
                        let scroll = iced::widget::operation::scroll_to(
                            view::scroll_id(tab.id, browser.path(), tab.list_epoch),
                            iced::widget::scrollable::AbsoluteOffset {
                                x: Some(0.0),
                                y: Some(browser.scroll()),
                            },
                        );
                        let menu = tab.menu_after_load.take() == Some(revision) && active_tab;
                        let menu = if menu {
                            self.action_event(actions_ui::Event::Menu(id))
                        } else {
                            Task::none()
                        };
                        let refresh = if active_tab
                            && self.active(id).is_some_and(|tab| tab.needs_refresh)
                        {
                            self.refresh_operation_browser(id, None)
                        } else if active_tab && self.active(id).is_some_and(|tab| tab.watch_stale) {
                            self.watch_rescan(id, tab_id)
                        } else {
                            Task::none()
                        };
                        return Task::batch([
                            scroll,
                            self.load_columns(id),
                            menu,
                            refresh,
                            self.present_requested_preview(id),
                        ]);
                    }
                    Err(error) => {
                        tab.preview_after_load = None;
                        tab.menu_after_load = None;
                        tab.error = Some(error);
                        return self.load_columns(id);
                    }
                }
            }
            Message::ColumnLoaded {
                window,
                tab,
                revision,
                path,
                request,
                result,
            } => {
                self.finish_column(window, tab, revision, path, request, result);
            }
            Message::FoldersChanged => return self.folders_changed(),
            Message::WatchLoaded {
                window,
                tab,
                revision,
                request,
                result,
            } => return self.finish_watch_rescan(window, tab, revision, request, result),
            Message::Select(id, path) => {
                if self.drag.suppress_select(id, &path) {
                    return Task::none();
                }
                let mode = self
                    .windows
                    .get(&id)
                    .map_or(SelectionMode::Replace, |window| {
                        selection_mode(window.modifiers)
                    });
                let Some(tab) = self.active_mut(id) else {
                    return Task::none();
                };
                if let Some(browser) = tab.browser.as_mut() {
                    if browser.search().is_some() || path.parent() == Some(browser.path()) {
                        browser.select(&path, mode);
                    } else if path.starts_with(browser.path()) {
                        return self.navigate(id, path, Navigation::Visit, None);
                    } else if let Some(parent) = path.parent() {
                        return self.navigate(id, parent.to_owned(), Navigation::Visit, Some(path));
                    }
                }
            }
            Message::Activate(id, path) => return self.open(id, path),
            Message::SwitchTab(id, index) => {
                let Some(window) = self.windows.get_mut(&id) else {
                    return Task::none();
                };
                if index >= window.tabs.len() {
                    return Task::none();
                }
                if window.active != index {
                    window.tabs[window.active].preview_after_load = None;
                    window.tabs[window.active].columns.clear();
                    window.active = index;
                }
                let tab = &window.tabs[index];
                if tab.needs_refresh {
                    return self.command(id, Command::Refresh);
                }
                if tab.browser.is_none() && !tab.loading {
                    let target = tab.target.clone();
                    return self.navigate(id, target, Navigation::Visit, None);
                }
                let stale = if tab.watch_stale && !tab.watch_scanning {
                    let tab = tab.id;
                    self.watch_rescan(id, tab)
                } else {
                    Task::none()
                };
                return Task::batch([self.restore_scroll(id), self.load_columns(id), stale]);
            }
            Message::Location(id, value) => {
                if let Some(tab) = self.active_mut(id) {
                    tab.location = Some(value);
                }
            }
            Message::SubmitLocation(id) => {
                if let Some(value) = self.active(id).and_then(|tab| tab.location.clone()) {
                    return self.navigate(id, PathBuf::from(value), Navigation::Visit, None);
                }
            }
            Message::Scroll(id, tab_id, epoch, path, scroll) => {
                if let Some(tab) = self.windows.get_mut(&id).and_then(|window| {
                    window
                        .tabs
                        .iter_mut()
                        .find(|tab| tab.id == tab_id && tab.list_epoch == epoch)
                }) {
                    let viewport = Size::new(tab.viewport_width, tab.viewport_height);
                    if let Some(browser) = tab
                        .browser
                        .as_mut()
                        .filter(|browser| browser.path() == path)
                    {
                        // Layout may report its old native offset before or
                        // after Viewport remaps the model. Admit feedback only
                        // when producer, native bounds and model agree.
                        if scroll.rendered == viewport && scroll.measured == viewport {
                            browser.set_scroll(scroll.offset);
                        }
                    } else if let Some(column) = tab.columns.get_mut(&path)
                        && scroll.offset.is_finite()
                    {
                        column.scroll = scroll.offset.max(0.0);
                    }
                }
            }
            Message::Viewport(id, tab_id, epoch, path, size) => {
                let active = self.active(id).is_some_and(|tab| tab.id == tab_id);
                let appearance = &self.configuration.appearance;
                let mut reveal = false;
                let mut remapped = false;
                if let Some(tab) = self.windows.get_mut(&id).and_then(|window| {
                    window
                        .tabs
                        .iter_mut()
                        .find(|tab| tab.id == tab_id && tab.list_epoch == epoch)
                }) && tab.path() == path
                    && size.width.is_finite()
                    && size.height.is_finite()
                {
                    if let Some(browser) = tab.browser.as_mut() {
                        let old = listing_ui::Geometry::new(
                            appearance,
                            browser.view(),
                            browser.search().is_some(),
                            tab.viewport_width,
                        );
                        let new = listing_ui::Geometry::new(
                            appearance,
                            browser.view(),
                            browser.search().is_some(),
                            size.width,
                        );
                        let was_visible = browser
                            .visible_entries()
                            .iter()
                            .position(|entry| Some(entry.path.as_path()) == browser.focused())
                            .is_some_and(|index| {
                                old.visible(index, browser.scroll(), tab.viewport_height)
                            });
                        let reflow = old.columns != new.columns || old.height != new.height;
                        if tab.viewport_width > 0.0 && reflow {
                            let row = (browser.scroll() / old.height).floor() as usize;
                            let remainder =
                                (browser.scroll() % old.height) * new.height / old.height;
                            browser.set_scroll(new.top(row * old.columns) + remainder);
                            remapped = true;
                        }
                        // A Search bar with no results still displays the real
                        // folder. Its height must not move that folder's saved
                        // scroll merely because the text field was opened.
                        let empty_search = tab.search.is_some() && browser.search().is_none();
                        reveal = active
                            && (std::mem::take(&mut tab.reveal_on_show)
                                || !empty_search
                                    && was_visible
                                    && (size.height < tab.viewport_height || reflow));
                    }
                    tab.viewport_height = size.height.max(0.0);
                    tab.viewport_width = size.width.max(0.0);
                }
                if reveal || remapped {
                    if reveal {
                        let _model_update = self.reveal_focus(id);
                    }
                    return self.restore_scroll(id);
                }
            }
            Message::Error(id, tab_id, error) => {
                if let Some(tab) = self
                    .windows
                    .get_mut(&id)
                    .and_then(|window| window.tabs.iter_mut().find(|tab| tab.id == tab_id))
                {
                    tab.error = Some(error);
                }
            }
        }
        Task::none()
    }

    fn exit_if_empty(&self) -> Task<Message> {
        if self.windows.is_empty()
            && self.properties.is_empty()
            && self.previews.is_empty()
            && self.trash_window.is_none()
            && self.recovery_window.is_none()
            && self.trash_restore.is_none()
            && self.delete_dialog.is_none()
            && self.recovery_destination.is_none()
            && self.action_editor.is_none()
            && self.preferences_editor.is_none()
            && self.file_operations.name.is_none()
            && self.file_operations.question.is_none()
            && self.file_operations.details.is_none()
            && self.devices.dialog.is_none()
            && self.configuration_pending.is_none()
            && self
                .configuration_worker
                .as_ref()
                .is_none_or(|worker| worker.snapshot().pending == 0)
            && self.operations_window.is_none()
            && !self.jobs.holds_application()
            && !self.file_operations.holds_application()
            && !self.file_clipboard.holds_application()
            && !self.drag.holds_application()
            && !self.folder_writer.holds_application()
            && !self.media.holds_application()
            && self
                .activity
                .as_ref()
                .is_none_or(|activity| !activity.holds_application())
            && self
                .instance
                .as_ref()
                .is_none_or(crate::activation::Instance::try_exit)
        {
            iced::exit()
        } else {
            Task::none()
        }
    }

    fn restore_scroll(&self, id: window::Id) -> Task<Message> {
        let Some(tab) = self.active(id) else {
            return Task::none();
        };
        let Some(browser) = &tab.browser else {
            return Task::none();
        };
        iced::widget::operation::scroll_to(
            view::scroll_id(tab.id, browser.path(), tab.list_epoch),
            iced::widget::scrollable::AbsoluteOffset {
                x: None,
                y: Some(browser.scroll()),
            },
        )
    }

    fn reveal_focus(&mut self, id: window::Id) -> Task<Message> {
        let Some(tab) = self.active(id) else {
            return Task::none();
        };
        let geometry = listing_ui::geometry(self, tab);
        let Some(tab) = self.active_mut(id) else {
            return Task::none();
        };
        let Some(browser) = tab.browser.as_mut() else {
            return Task::none();
        };
        if !geometry.reveal(browser, tab.viewport_height) {
            return Task::none();
        }
        self.restore_scroll(id)
    }

    fn open(&mut self, id: window::Id, path: PathBuf) -> Task<Message> {
        let tab_id = self.active(id).map(|tab| tab.id);
        if tab_id.is_none()
            && self
                .trash_window
                .as_ref()
                .is_none_or(|trash| trash.window != id)
            && self.recovery_window.as_ref().is_none_or(|w| w.window != id)
        {
            return Task::none();
        }
        let recovery = self
            .recovery_window
            .as_ref()
            .is_some_and(|w| w.window == id);
        let is_directory = self
            .active(id)
            .and_then(|tab| tab.browser.as_ref())
            .and_then(|browser| {
                browser
                    .visible_entries()
                    .into_iter()
                    .find(|entry| entry.path == path)
            })
            .is_some_and(|entry| entry.is_directory());
        if is_directory {
            self.navigate(id, path, Navigation::Visit, None)
        } else {
            Task::perform(
                async move {
                    std::process::Command::new("gio")
                        .arg("open")
                        .arg(path)
                        .status()
                        .map_err(|error| error.to_string())
                        .and_then(|status| {
                            if status.success() {
                                Ok(())
                            } else {
                                Err(format!("Opening the file failed: {status}"))
                            }
                        })
                },
                move |result| match result {
                    Ok(()) => Message::Opened(id),
                    Err(error) => match tab_id {
                        Some(tab_id) => Message::Error(id, tab_id, error),
                        None if recovery => {
                            Message::Recovery(recovery_ui::Event::OpenFailed(id, error))
                        }
                        None => Message::Trash(trash_ui::Event::OpenFailed(id, error)),
                    },
                },
            )
        }
    }

    fn command(&mut self, id: window::Id, command: Command) -> Task<Message> {
        match command {
            Command::Recovery => return self.show_recovery(),
            Command::ShowTrash => return self.show_trash(),
            Command::Trash => return self.trash_selection(id),
            Command::DeletePermanently => return self.confirm_delete_selection(id),
            Command::Copy | Command::Cut => return self.copy_files(id, command == Command::Cut),
            Command::Paste => return self.paste_files(id),
            Command::NewFolder => return self.show_name_dialog(id, false),
            Command::Rename => return self.show_name_dialog(id, true),
            Command::Undo => {
                if !self.file_operations.ready() || self.file_operations.has_active() {
                    return self.show_operations();
                }
                if let Some(undo) = &self.file_operations.snapshot.undo {
                    return self.file_operation_event(file_operations::Event::Undo(id, undo.id));
                }
                return Task::none();
            }
            _ => {}
        }
        if command == Command::Preferences || command == Command::BookmarkFolder {
            let folder = self
                .active(id)
                .map(|tab| tab.path().to_owned())
                .unwrap_or_else(|| self.home.clone());
            return self.show_preferences(folder, command == Command::BookmarkFolder);
        }
        if command == Command::CustomActions {
            return self.show_action_editor();
        }
        if command == Command::Operations {
            return self.show_operations();
        }
        if command == Command::QuickLook {
            return self.toggle_preview(id);
        }
        if matches!(command, Command::Columns | Command::List | Command::Icons) {
            let mode = match command {
                Command::List => crate::preferences::ViewMode::List,
                Command::Icons => crate::preferences::ViewMode::Icons,
                _ => crate::preferences::ViewMode::Columns,
            };
            return self.change_folder_mode(id, mode);
        }
        let Some(tab) = self.active(id) else {
            return Task::none();
        };
        match command {
            Command::Search => return self.open_search(id),
            Command::SearchSubfolders
            | Command::StopSearch
            | Command::MoreSearchResults
            | Command::RevealSearchResult
            | Command::SearchProblems => {
                let Some(session) = &tab.search else {
                    return Task::none();
                };
                let target = search_ui::Target {
                    window: id,
                    tab: tab.id,
                    instance: session.instance,
                };
                let event = match command {
                    Command::SearchSubfolders => {
                        search_ui::Event::Recursive(target, !session.recursive)
                    }
                    Command::StopSearch => search_ui::Event::Stop(target),
                    Command::MoreSearchResults => search_ui::Event::More(target),
                    Command::RevealSearchResult => search_ui::Event::Reveal(target),
                    _ => search_ui::Event::ToggleFailures(target),
                };
                return self.search_event(event);
            }
            Command::Parent => {
                if let Some(parent) = tab.browser.as_ref().and_then(Browser::parent) {
                    return self.navigate(id, parent.to_owned(), Navigation::Visit, None);
                }
            }
            Command::Back | Command::Forward => {
                if command == Command::Back && tab.search.is_some() {
                    return self.close_search(id);
                }
                let direction = if command == Command::Back {
                    HistoryDirection::Back
                } else {
                    HistoryDirection::Forward
                };
                if let Some(target) = tab
                    .browser
                    .as_ref()
                    .and_then(|browser| browser.history_target(direction))
                    .map(Path::to_owned)
                {
                    return self.navigate(id, target, Navigation::History(direction), None);
                }
            }
            Command::Refresh => {
                if tab
                    .search
                    .as_ref()
                    .is_some_and(|search| !search.query.is_empty())
                {
                    return self.restart_search(id);
                }
                return self.navigate(id, tab.path().to_owned(), Navigation::Refresh, None);
            }
            Command::EnterFolder | Command::Open => {
                if let Some(browser) = &tab.browser
                    && let Some(path) = browser.focused().map(Path::to_owned)
                {
                    let directory = browser
                        .visible_entries()
                        .into_iter()
                        .any(|entry| entry.path == path && entry.is_directory());
                    if command == Command::Open || directory {
                        return self.open(id, path);
                    }
                }
            }
            Command::Location => {
                let value = display_location(tab.path());
                let close = self.close_search(id);
                self.active_mut(id).unwrap().location = Some(value);
                return close;
            }
            Command::ShowHidden => {
                if let Some(browser) = self.active_mut(id).and_then(|tab| tab.browser.as_mut()) {
                    browser.set_show_hidden(!browser.show_hidden());
                }
                if self.active(id).is_some_and(|tab| tab.search.is_some()) {
                    return self.restart_search(id);
                }
            }
            Command::SelectAll => {
                if let Some(browser) = self.active_mut(id).and_then(|tab| tab.browser.as_mut()) {
                    browser.select_all();
                }
            }
            Command::NewWindow => return self.new_window(vec![tab.path().to_owned()]),
            Command::NewTab => {
                let path = tab.path().to_owned();
                let tab = self.new_tab(path.clone());
                let window = self.windows.get_mut(&id).unwrap();
                window.tabs[window.active].columns.clear();
                window.tabs[window.active].preview_after_load = None;
                window.tabs.push(tab);
                window.active = window.tabs.len() - 1;
                return self.navigate(id, path, Navigation::Visit, None);
            }
            Command::CloseTab => {
                let window = self.windows.get_mut(&id).unwrap();
                if window.tabs.len() == 1 {
                    return self.update(Message::Close(id));
                }
                window.tabs.remove(window.active);
                window.active = window.active.min(window.tabs.len() - 1);
                let index = window.active;
                return self.update(Message::SwitchTab(id, index));
            }
            Command::NextTab | Command::PreviousTab => {
                let window = self.windows.get(&id).unwrap();
                let count = window.tabs.len();
                let next = (window.active
                    + if command == Command::NextTab {
                        1
                    } else {
                        count - 1
                    })
                    % count;
                return self.update(Message::SwitchTab(id, next));
            }
            _ => {}
        }
        Task::none()
    }

    fn key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
        text: Option<String>,
    ) -> Task<Message> {
        if self.active(id).is_none() {
            return Task::none();
        }
        if self
            .action_menu
            .as_ref()
            .is_some_and(|menu| menu.window() == id)
        {
            return self.menu_key(id, key, modifiers);
        }
        if self
            .folder_options
            .as_ref()
            .is_some_and(|target| target.window == id)
        {
            if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                self.folder_options = None;
            }
            return Task::none();
        }
        let name = key_name(&key);
        if let Some(chord) = key_chord(&key, modifiers)
            && let Some(command) = self.configuration.settings.command_for(&chord)
        {
            return self.command(id, command);
        }
        if let Some(chord) = key_chord(&key, modifiers)
            && let Some(task) = self.action_shortcut(id, &chord)
        {
            return task;
        }
        if (name == "Menu" || name == "F10" && modifiers.shift())
            && !modifiers.control()
            && !modifiers.alt()
            && !modifiers.logo()
            && self.active(id).is_some_and(|tab| tab.location.is_none())
        {
            return self.action_event(actions_ui::Event::Menu(id));
        }
        if name == "Escape" {
            if self
                .active(id)
                .is_some_and(|tab| tab.location.is_none() && tab.search.is_some())
            {
                return self.close_search(id);
            }
            if let Some(tab) = self.active_mut(id) {
                tab.location = None;
            }
            return Task::none();
        }
        if matches!(
            name.as_str(),
            "Up" | "Down" | "Home" | "End" | "Page_Up" | "Page_Down"
        ) && !modifiers.alt()
            && !modifiers.logo()
        {
            if let Some(tab) = self.active(id).filter(|tab| tab.search.is_some()) {
                let target = search_ui::Target {
                    window: id,
                    tab: tab.id,
                    instance: tab.search.as_ref().unwrap().instance,
                };
                return iced::widget::operation::is_focused(view::search_id(id, tab.id)).map(
                    move |focused| {
                        Message::Search(search_ui::Event::NavigationKey(
                            target,
                            name.clone(),
                            modifiers,
                            focused,
                        ))
                    },
                );
            }
            return self.move_rows(id, &name, modifiers);
        }
        let Some(tab) = self.active_mut(id) else {
            return Task::none();
        };
        if tab.location.is_some() {
            return Task::none();
        }
        let Some(browser) = tab.browser.as_mut() else {
            return Task::none();
        };
        if !modifiers.control()
            && !modifiers.alt()
            && !modifiers.logo()
            && let Some(text) = text.filter(|text| !text.chars().any(char::is_control))
        {
            let now = Instant::now();
            if tab
                .last_typing
                .is_none_or(|last| now.duration_since(last) > Duration::from_millis(700))
            {
                tab.prefix.clear();
            }
            tab.last_typing = Some(now);
            tab.prefix.push_str(&text);
            browser.select_prefix(&tab.prefix);
            return self.reveal_focus(id);
        }
        Task::none()
    }

    fn move_rows(
        &mut self,
        id: window::Id,
        name: &str,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        let Some(tab) = self.active(id) else {
            return Task::none();
        };
        let geometry = listing_ui::geometry(self, tab);
        let Some(tab) = self.active_mut(id).filter(|tab| tab.location.is_none()) else {
            return Task::none();
        };
        let Some(browser) = tab.browser.as_mut() else {
            return Task::none();
        };
        let entries = browser.visible_entries();
        if entries.is_empty() {
            return Task::none();
        }
        let current = entries
            .iter()
            .position(|entry| Some(entry.path.as_path()) == browser.focused());
        let stride = geometry.columns as isize;
        let page = (tab.viewport_height / geometry.height).floor().max(1.0) as isize * stride;
        let delta = match name {
            "Up" => -stride,
            "Down" => stride,
            "Page_Up" => -page,
            "Page_Down" => page,
            _ => 0,
        };
        let index = match name {
            "Home" => 0,
            "End" => entries.len() - 1,
            _ => current
                .map_or(0, |index| index.saturating_add_signed(delta))
                .min(entries.len() - 1),
        };
        let path = entries[index].path.clone();
        if modifiers.control() && !modifiers.shift() {
            browser.focus_entry(&path, false);
        } else {
            browser.select(&path, selection_mode(modifiers));
        }
        tab.prefix.clear();
        self.reveal_focus(id)
    }

    fn title(&self, id: window::Id) -> String {
        if let Some(properties) = self.properties.get(&id) {
            return properties.title();
        }
        if let Some(dialog) = self
            .devices
            .dialog
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return format!("{} — Canopa Files", dialog.title());
        }
        if self
            .recovery_window
            .as_ref()
            .is_some_and(|w| w.window == id)
        {
            return "Recovery — Canopa Files".into();
        }
        if let Some(dialog) = self
            .recovery_destination
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return if dialog.browser.open() {
                "Choose a folder — Canopa Files"
            } else {
                "Recover a copy — Canopa Files"
            }
            .into();
        }
        if self
            .trash_window
            .as_ref()
            .is_some_and(|trash| trash.window == id)
        {
            return "Trash — Canopa Files".into();
        }
        if let Some(dialog) = self
            .delete_dialog
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return format!("{} — Canopa Files", dialog.title());
        }
        if let Some(dialog) = self
            .trash_restore
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return if dialog.browser.open() {
                "Choose a folder — Canopa Files"
            } else {
                "Restore to folder — Canopa Files"
            }
            .into();
        }
        if let Some(dialog) = self
            .file_operations
            .name
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return format!("{} — Canopa Files", dialog.title());
        }
        if let Some(dialog) = self
            .file_operations
            .question
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return format!("{} — Canopa Files", dialog.title());
        }
        if self
            .preferences_editor
            .as_ref()
            .is_some_and(|editor| editor.window == id)
        {
            return "Preferences — Canopa Files".into();
        }
        if self.operations_window == Some(id) {
            return "Operations — Canopa Files".into();
        }
        if self
            .file_operations
            .details
            .as_ref()
            .is_some_and(|dialog| dialog.window == id)
        {
            return "Transfer details — Canopa Files".into();
        }
        if self
            .action_editor
            .as_ref()
            .is_some_and(|editor| editor.window == id)
        {
            return "Custom Actions — Canopa Files".into();
        }
        if let Some(preview) = self.previews.get(&id) {
            return preview_ui::title(preview);
        }
        let name = self
            .active(id)
            .map(|tab| {
                tab.path()
                    .file_name()
                    .unwrap_or(tab.path().as_os_str())
                    .to_string_lossy()
            })
            .unwrap_or_default();
        if let Some(tab) = self.active(id)
            && let Some(search) = tab
                .search
                .as_ref()
                .filter(|search| !search.query.is_empty())
        {
            let state = if search.stopping {
                "Stopping search"
            } else if search.running {
                "Searching"
            } else if tab
                .browser
                .as_ref()
                .and_then(Browser::search)
                .is_some_and(|results| results.cancelled)
            {
                "Stopped search"
            } else {
                "Search"
            };
            return format!("{state}: {} — {name} — Canopa Files", search.query);
        }
        format!("{name} — Canopa Files")
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.media.subscription(),
            self.activation_notices
                .as_ref()
                .map_or_else(Subscription::none, |notices| {
                    notices.subscription().map(|()| Message::ActivationPoll)
                }),
            self.activity_notices
                .as_ref()
                .map_or_else(Subscription::none, |notices| {
                    notices.subscription().map(|()| Message::ActivityPoll)
                }),
            self.file_operations.subscription(),
            self.folder_watch.subscription(),
            self.trash_window
                .as_ref()
                .map_or_else(Subscription::none, trash_ui::TrashWindow::subscription),
            self.recovery_window.as_ref().map_or_else(
                Subscription::none,
                recovery_ui::RecoveryWindow::subscription,
            ),
            self.jobs.subscription(),
            self.file_clipboard.subscription(),
            self.drag.subscription(),
            self.configuration_notices
                .as_ref()
                .map_or_else(Subscription::none, configuration_ui::Notices::subscription),
            window::close_events().map(Message::Closed),
            iced::event::listen_with(|event, status, id| match event {
                Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => Some(Message::Drag(drag_ui::Event::NativePress(id))),
                Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => Some(Message::Drag(drag_ui::Event::Release(id))),
                Event::Window(window::Event::CloseRequested) => Some(Message::Close(id)),
                Event::Window(window::Event::Resized(size)) => Some(Message::Resized(id, size)),
                Event::Window(window::Event::Focused) => Some(Message::Focus(id, true)),
                Event::Window(window::Event::Unfocused) => Some(Message::Focus(id, false)),
                Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                    Some(Message::Modifiers(id, modifiers))
                }
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text,
                    ..
                // TextInput captures Escape to unfocus itself. The containing
                // location editor must also close so browsing keys work again.
                }) if status == iced::event::Status::Ignored
                    || matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape | keyboard::key::Named::Tab)) => Some(Message::Key(
                    id,
                    key,
                    modifiers,
                    text.map(|text| text.to_string()),
                )),
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => Some(Message::CapturedKey(id, key, modifiers)),
                _ => None,
            }),
        ])
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        if let Some(properties) = self.properties.get(&id) {
            return properties_ui::view(self, id, properties);
        }
        if let Some(dialog) = self
            .devices
            .dialog
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return device_question::view(self, dialog);
        }
        if let Some(w) = self.recovery_window.as_ref().filter(|w| w.window == id) {
            return recovery_ui::view(self, w);
        }
        if let Some(d) = self
            .recovery_destination
            .as_ref()
            .filter(|d| d.window == id)
        {
            return recovery_destination::view(self, d);
        }
        if let Some(trash) = self
            .trash_window
            .as_ref()
            .filter(|trash| trash.window == id)
        {
            return trash_ui::view(self, trash);
        }
        if let Some(dialog) = self
            .trash_restore
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return trash_restore::view(self, dialog);
        }
        if let Some(dialog) = self
            .delete_dialog
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return delete_dialog::view(self, dialog);
        }
        if let Some(dialog) = self
            .file_operations
            .name
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return name_dialog::view(self, dialog);
        }
        if let Some(dialog) = self
            .file_operations
            .question
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return question_dialog::view(self, dialog);
        }
        if let Some(editor) = self
            .preferences_editor
            .as_ref()
            .filter(|editor| editor.window == id)
        {
            return preferences_view::view(self, editor);
        }
        if self.operations_window == Some(id) {
            return jobs_ui::view(self);
        }
        if let Some(dialog) = self
            .file_operations
            .details
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            return operation_details::view(self, dialog);
        }
        if let Some(editor) = self
            .action_editor
            .as_ref()
            .filter(|editor| editor.window == id)
        {
            return action_editor::view(self, editor);
        }
        if let Some(preview) = self.previews.get(&id) {
            preview_ui::view(self, id, preview)
        } else {
            actions_ui::overlay(self, id, view::browser(self, id))
        }
    }
}

fn selection_mode(modifiers: keyboard::Modifiers) -> SelectionMode {
    if modifiers.shift() {
        SelectionMode::Extend
    } else if modifiers.control() {
        SelectionMode::Toggle
    } else {
        SelectionMode::Replace
    }
}

fn sidebar_visible(mode: SidebarMode, width: f32) -> bool {
    match mode {
        SidebarMode::Auto => width >= 960.0,
        SidebarMode::Always => width >= 560.0,
        SidebarMode::Never => false,
    }
}

fn scan(
    target: PathBuf,
    home: &Path,
    base: Option<&Path>,
    selected: Option<PathBuf>,
    cancellation: &Cancellation,
) -> io::Result<Scan> {
    if cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "folder scan cancelled",
        ));
    }
    let mut target = local_path(&target, home, base)?;
    let selected = if crate::browse::Entry::read(&target)?.is_directory() {
        selected.into_iter().collect()
    } else {
        let file = target;
        target = file
            .parent()
            .ok_or_else(|| io::Error::other("file has no parent"))?
            .to_owned();
        vec![file]
    };
    let directory = Directory::read_cancellable(&target, cancellation)?;
    Ok(Scan {
        directory,
        selected,
    })
}

fn local_path(path: &Path, home: &Path, base: Option<&Path>) -> io::Result<PathBuf> {
    let bytes = path.as_os_str().as_bytes();
    let path = if let Some(uri) = bytes.strip_prefix(b"file://") {
        let slash = uri
            .iter()
            .position(|byte| *byte == b'/')
            .ok_or_else(|| io::Error::other("invalid local file URI"))?;
        if !uri[..slash].is_empty() && !uri[..slash].eq_ignore_ascii_case(b"localhost")
            || uri[slash..].iter().any(|byte| matches!(byte, b'?' | b'#'))
        {
            return Err(io::Error::other("only local file locations are supported"));
        }
        let mut decoded = Vec::new();
        let mut offset = slash;
        while offset < uri.len() {
            let byte = if uri[offset] == b'%' {
                let pair = uri
                    .get(offset + 1..offset + 3)
                    .ok_or_else(|| io::Error::other("invalid file URI escape"))?;
                let value = std::str::from_utf8(pair)
                    .ok()
                    .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                    .ok_or_else(|| io::Error::other("invalid file URI escape"))?;
                offset += 3;
                value
            } else {
                let value = uri[offset];
                offset += 1;
                value
            };
            if byte == 0 {
                return Err(io::Error::other("invalid file URI path"));
            }
            decoded.push(byte);
        }
        PathBuf::from(std::ffi::OsString::from_vec(decoded))
    } else if bytes == b"~" {
        home.to_owned()
    } else if let Some(tail) = bytes.strip_prefix(b"~/") {
        home.join(std::ffi::OsStr::from_bytes(tail))
    } else {
        path.to_owned()
    };
    std::path::absolute(if path.is_relative() {
        base.unwrap_or(Path::new(".")).join(path)
    } else {
        path
    })
}

fn display_location(path: &Path) -> String {
    if let Some(path) = path.to_str() {
        return path.to_owned();
    }
    let mut uri = String::from("file://");
    for byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            uri.push(char::from(*byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

fn key_name(key: &keyboard::Key) -> String {
    match key {
        keyboard::Key::Named(named) => match named {
            keyboard::key::Named::ArrowLeft => "Left".into(),
            keyboard::key::Named::ArrowRight => "Right".into(),
            keyboard::key::Named::ArrowUp => "Up".into(),
            keyboard::key::Named::ArrowDown => "Down".into(),
            keyboard::key::Named::Enter => "Return".into(),
            keyboard::key::Named::PageUp => "Page_Up".into(),
            keyboard::key::Named::PageDown => "Page_Down".into(),
            keyboard::key::Named::ContextMenu => "Menu".into(),
            other => format!("{other:?}"),
        },
        keyboard::Key::Character(value) => value.to_string(),
        _ => String::new(),
    }
}

fn key_chord(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<KeyChord> {
    let mut chord = String::new();
    for (active, name) in [
        (modifiers.control(), "Control"),
        (modifiers.shift(), "Shift"),
        (modifiers.alt(), "Alt"),
        (modifiers.logo(), "Super"),
    ] {
        if active {
            chord.push_str(&format!("<{name}>"));
        }
    }
    chord.push_str(&key_name(key));
    KeyChord::parse(&chord).ok()
}
