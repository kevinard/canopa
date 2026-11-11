//! Ordinary settings window with shared controls and bounded background work.
mod about;
mod bluetooth;
mod date_time;
mod default_apps;
mod focus;
mod launcher;
#[cfg(debug_assertions)]
mod live_example;
mod live_page;
mod navigation;
mod network;
mod notifications;
mod power;
mod printers;
mod privacy;
mod profiles;
mod region;
mod shortcuts;
mod sound;
mod users;
mod widget_layouts;
use crate::{
    display::{Monitor, TrialHandle},
    file_page::{FileDraft, FileModel},
    input::Input,
    instance::{Instance, Request},
    niri::Client,
    power::PowerChoices,
    preferences::{IdleDraft, Personalization},
    registry::Document,
    session::Paths,
    storage::{Snapshot, companion},
};
use anyhow::{Context, Result, ensure};
use canopa_ui::{
    accessibility::{self, Node, semantic},
    direction,
};
use iced::{
    Alignment, Element, Font, Length, Subscription, Task, Theme,
    font::Weight,
    widget::{
        checkbox, column, container, pick_list, row, rule, scrollable, slider, space, text,
        text_input,
    },
};
use niri_ipc::{Output, Transform};
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    Page,
    registry::{self, Apply, Kind},
};
/// The window's Wayland app id and desktop entry name.
const APPLICATION_ID: &str = "org.canopa.Settings";
#[derive(Clone, Debug)]
enum Message {
    LauncherLoaded(Result<crate::launcher::Model, String>),
    LauncherStored(Result<crate::launcher::Model, String>),
    LauncherCatalog(Result<canopa_launcher::Catalog, String>),
    LauncherRefresh,
    ClipboardLoaded(Result<crate::launcher::ClipboardEditor, String>),
    ClipboardStored(Result<crate::launcher::ClipboardEditor, String>),
    ClipboardRecording(bool),
    ClipboardImages(bool),
    ClipboardLimit(String),
    ClipboardSave,
    LauncherEnabled(String, bool),
    LauncherKeyword(String, String),
    LauncherTemplate(String),
    LauncherSearch(usize, bool, String),
    LauncherRemoveSearch(usize),
    LauncherAddSearch,
    ShortcutsLoaded(Result<crate::shortcuts::Editor, String>),
    ShortcutsStored(Result<crate::shortcuts::Editor, String>),
    ShortcutSearch(String),
    ShortcutKey(usize, String),
    ResetShortcuts,
    BackendChanged,
    ReloadSaved,
    Page(Page),
    /// Move through the sidebar order (Ctrl+Page Down / Page Up).
    StepPage(isize),
    Search(String),
    /// Open the best search match.
    SearchSubmit,
    Refresh,
    Outputs(std::result::Result<HashMap<String, Output>, String>),
    DisplayProfiles(profiles::Action),
    WidgetLayouts(widget_layouts::Action),
    OpenLockWidgetEditor,
    Enabled(usize, bool),
    /// A display's mode, by its index in the display's mode list.
    Mode(usize, usize),
    Scale(usize, f64),
    Rotation(usize, Transform),
    Position(usize, bool, String),
    InputText(u8, String),
    InputNumber(u8, f64),
    InputFlag(u8, bool),
    PersonalText(u8, String),
    Light(bool),
    TextScale(f64),
    ReducedMotion(bool),
    HighContrast(bool),
    /// Apply a Niri cursor size at once (`None`: Niri's default).
    CursorSize(Option<u16>),
    CursorSizeStored(Result<Snapshot, String>),
    CompositorMotion(bool),
    CompositorMotionStored(Result<bool, String>),
    Dim(f64),
    IdleTime(bool, usize, String),
    IdleAction(&'static str),
    Profile(String),
    UseProfile,
    IdleStatus(std::result::Result<String, String>),
    Save,
    Stored(std::result::Result<(Page, Snapshot), String>),
    TrialStarted(std::result::Result<TrialHandle, String>),
    Decision(bool),
    TrialFinished(std::result::Result<String, String>),
    Tick,
    Wallpaper,
    Finished(std::result::Result<String, String>),
    WallpaperFinished(std::result::Result<(), String>),
    /// A later launch or a D-Bus caller asks for this window.
    Activated(Request),
    /// Whether the compositor focused the window.
    Raised(bool),
    Power(crate::live::Event<crate::power::PowerService>),
    Sound(crate::live::Event<crate::sound::SoundService>),
    Network(crate::live::Event<crate::network::NetworkService>),
    Bluetooth(crate::live::Event<crate::bluetooth::BluetoothService>),
    BluetoothForm(bluetooth::Edit),
    Notifications(crate::live::Event<crate::notifications::NotificationService>),
    NotificationsForm(notifications::Edit),
    DefaultApps(crate::live::Event<crate::default_apps::DefaultAppsService>),
    DefaultAppsSearch(String),
    DateTime(crate::live::Event<crate::date_time::DateTimeService>),
    DateTimeForm(date_time::Edit),
    About(crate::live::Event<crate::about::AboutService>),
    AboutForm(about::Edit),
    Region(crate::live::Event<crate::region::RegionService>),
    Users(crate::live::Event<crate::users::UsersService>),
    UsersForm(users::Edit),
    Privacy(crate::live::Event<crate::privacy::PrivacyService>),
    Printers(crate::live::Event<crate::printers::PrintersService>),
    NetworkForm(network::Edit),
    /// The default input's peak level, while the Sound page is shown.
    Level(Option<f32>),
    AlertSounds(bool),
    AlertSoundsStored(Result<(Snapshot, Snapshot), String>),
    LidClose(canopa_services::power_switches::LidAction),
    /// Leave the power button to logind (`true`) or to Niri's suspend.
    PowerKey(bool),
    PowerKeyStored(Result<Snapshot, String>),
    /// The lid choice saved, then the idle timeouts if they changed.
    PowerStored(Result<(Snapshot, Result<Option<Snapshot>, String>), String>),
    #[cfg(debug_assertions)]
    LiveExample(crate::live::Event<crate::live::fake::Service>),
    #[cfg(debug_assertions)]
    Simulate(live_example::Simulate),
    Accessibility(accessibility::Event<Message>),
}
impl accessibility::Message for Message {
    fn accessibility(event: accessibility::Event<Self>) -> Self {
        Self::Accessibility(event)
    }
}
/// The accessibility root's key; it names the window among AT-SPI windows.
const ACCESSIBILITY_ROOT: &str = "settings";
struct State {
    paths: Paths,
    launcher: crate::launcher::Model,
    clipboard: Option<crate::launcher::ClipboardEditor>,
    clipboard_notice: String,
    shortcuts: Option<Result<crate::shortcuts::Editor, String>>,
    shortcut_query: String,
    compositor_motion: Option<bool>,
    page: Page,
    search: String,
    notice: String,
    busy: bool,
    refreshing: bool,
    outputs: HashMap<String, Output>,
    monitors: Vec<Monitor>,
    positions: Vec<(String, String)>,
    display_dirty: bool,
    display_profiles: profiles::Editor,
    widget_layouts: widget_layouts::View,
    input: FileDraft<Input>,
    personal: FileDraft<Personalization>,
    idle: FileDraft<IdleDraft>,
    power_choices: FileDraft<PowerChoices>,
    power: power::PowerPage,
    sound: sound::SoundPage,
    network: network::NetworkPage,
    network_forms: network::NetworkForms,
    bluetooth: bluetooth::BluetoothPage,
    /// The PIN or passkey being typed for a pairing question.
    bluetooth_entry: String,
    notifications: notifications::NotificationsPage,
    /// Installed applications, read when the Notifications page opens.
    notification_apps: Option<Vec<notifications::App>>,
    notification_query: String,
    default_apps: default_apps::DefaultAppsPage,
    default_apps_query: String,
    date_time: date_time::DateTimePage,
    date_time_forms: date_time::DateTimeForms,
    about: about::AboutPage,
    /// The device name being typed; `None` shows the current one.
    about_name: Option<String>,
    region: region::RegionPage,
    users: users::UsersPage,
    users_forms: users::UsersForms,
    privacy: privacy::PrivacyPage,
    printers: printers::PrintersPage,
    input_level: Option<f32>,
    alert_sounds: Option<bool>,
    profile: String,
    idle_status: String,
    idle_available: bool,
    trial: Option<TrialHandle>,
    remaining: u64,
    #[cfg(debug_assertions)]
    live_example: live_example::Example,
    activations: Option<Activations>,
    /// The AT-SPI bridge, created by the running window only.
    accessibility: Option<accessibility::Bridge<Message>>,
}
fn task<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
    done: impl FnOnce(std::result::Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(work).await {
                Ok(result) => result.map_err(|e| format!("{e:#}")),
                Err(error) => Err(error.to_string()),
            }
        },
        done,
    )
}
impl State {
    fn action<'a>(
        &self,
        key: impl Into<String>,
        label: impl Into<String>,
        message: Option<Message>,
        active: bool,
    ) -> Element<'a, Message> {
        button(
            key,
            label,
            message,
            if active {
                canopa_ui::Action::Normal
            } else {
                canopa_ui::Action::Quiet
            },
            Length::Shrink,
            self.busy,
        )
    }

    fn boot() -> (Self, Task<Message>) {
        Self::boot_with(Paths::discover().expect("settings require a configuration home"))
    }

    fn load_motion(paths: &Paths) -> Option<bool> {
        paths
            .active()
            .then(|| {
                Snapshot::load(paths.niri())
                    .and_then(|source| crate::niri::reduced_motion(&source))
                    .ok()
            })
            .flatten()
    }

    fn boot_with(paths: Paths) -> (Self, Task<Message>) {
        let (input, input_error) = FileDraft::load(&paths);
        let (personal, personal_error) = FileDraft::load(&paths);
        // Unreadable idle policy shows the defaults without a notice, as before.
        let (idle, _) = FileDraft::load(&paths);
        let (power_choices, _) = FileDraft::load(&paths);
        let compositor_motion = Self::load_motion(&paths);
        let notice = personal_error.or(input_error).unwrap_or_default();
        let profile_task = profiles::Editor::load(&paths);
        let state = Self {
            paths,
            launcher: Default::default(),
            clipboard: None,
            clipboard_notice: String::new(),
            shortcuts: None,
            shortcut_query: String::new(),
            compositor_motion,
            page: Page::Displays,
            search: String::new(),
            notice,
            busy: false,
            refreshing: true,
            outputs: HashMap::new(),
            monitors: vec![],
            positions: vec![],
            display_dirty: false,
            display_profiles: profiles::Editor::default(),
            widget_layouts: widget_layouts::View::default(),
            input,
            personal,
            idle,
            power_choices,
            power: live_page::LivePage::new(Default::default()),
            sound: live_page::LivePage::new(Default::default()),
            network: live_page::LivePage::new(Default::default()),
            network_forms: Default::default(),
            bluetooth: live_page::LivePage::new(Default::default()),
            bluetooth_entry: String::new(),
            notifications: live_page::LivePage::new(Default::default()),
            notification_apps: None,
            notification_query: String::new(),
            default_apps: live_page::LivePage::new(Default::default()),
            default_apps_query: String::new(),
            date_time: live_page::LivePage::new(Default::default()),
            date_time_forms: Default::default(),
            about: live_page::LivePage::new(Default::default()),
            about_name: None,
            region: live_page::LivePage::new(Default::default()),
            users: live_page::LivePage::new(Default::default()),
            users_forms: Default::default(),
            privacy: live_page::LivePage::new(Default::default()),
            printers: live_page::LivePage::new(Default::default()),
            input_level: None,
            alert_sounds: None,
            profile: "none".into(),
            idle_status: fl!("settings-idle-connecting"),
            idle_available: false,
            trial: None,
            remaining: 0,
            #[cfg(debug_assertions)]
            live_example: live_page::LivePage::new(Default::default()),
            activations: None,
            accessibility: None,
        };
        (
            state,
            Task::batch([
                task(|| Client::discover()?.outputs(), Message::Outputs),
                profile_task,
                task(crate::launcher::Model::load, Message::LauncherLoaded),
                task(
                    crate::launcher::ClipboardEditor::load,
                    Message::ClipboardLoaded,
                ),
            ]),
        )
    }
    /// Handle a message, then refresh the accessibility tree if one is read.
    fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::Accessibility(event) = message {
            return match &mut self.accessibility {
                Some(bridge) => bridge.update(event),
                None => Task::none(),
            };
        }
        let task = self.apply(message);
        match &mut self.accessibility {
            Some(bridge) => {
                bridge.set_scale(
                    f64::from(self.personal.value.values.appearance.text_scale()) / 100.0,
                );
                Task::batch([task, bridge.refresh()])
            }
            None => task,
        }
    }
    fn apply(&mut self, message: Message) -> Task<Message> {
        if (self.busy || self.trial.is_some())
            && matches!(
                &message,
                Message::Enabled(..)
                    | Message::Mode(..)
                    | Message::Scale(..)
                    | Message::Rotation(..)
                    | Message::Position(..)
                    | Message::InputText(..)
                    | Message::InputNumber(..)
                    | Message::InputFlag(..)
                    | Message::PersonalText(..)
                    | Message::Light(..)
                    | Message::TextScale(..)
                    | Message::ReducedMotion(..)
                    | Message::HighContrast(..)
                    | Message::CursorSize(..)
                    | Message::Dim(..)
                    | Message::IdleTime(..)
                    | Message::LidClose(..)
                    | Message::Profile(..)
                    | Message::Wallpaper
            )
        {
            return Task::none();
        }
        match message {
            Message::LauncherLoaded(result) => match result {
                Ok(model) => self.launcher = model,
                Err(error) => self.notice = error,
            },
            Message::LauncherStored(result) => {
                self.busy = false;
                match result {
                    Ok(model) => {
                        self.launcher = model;
                        self.notice = fl!("settings-launcher-saved");
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::ClipboardLoaded(result) => match result {
                Ok(editor) => self.clipboard = Some(editor),
                Err(error) => self.clipboard_notice = error,
            },
            Message::ClipboardStored(result) => {
                self.busy = false;
                match result {
                    Ok(editor) => {
                        self.clipboard = Some(editor);
                        self.clipboard_notice = fl!("settings-clipboard-saved");
                    }
                    Err(error) => self.clipboard_notice = error,
                }
            }
            Message::ClipboardRecording(enabled) if !self.busy => {
                if let Some(editor) = &mut self.clipboard {
                    editor.recording = enabled;
                }
            }
            Message::ClipboardImages(enabled) if !self.busy => {
                if let Some(editor) = &mut self.clipboard {
                    editor.images = enabled;
                }
            }
            Message::ClipboardLimit(value) if !self.busy => {
                if let Some(editor) = &mut self.clipboard
                    && value.len() <= 2
                    && value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    editor.limit = value;
                }
            }
            Message::ClipboardSave
                if self.page == Page::Launcher && !self.busy && self.trial.is_none() =>
            {
                if let Some(editor) = self.clipboard.clone() {
                    self.busy = true;
                    self.clipboard_notice.clear();
                    return task(move || editor.save(), Message::ClipboardStored);
                }
            }
            Message::LauncherCatalog(Ok(catalog)) => self.launcher.catalog = catalog,
            Message::LauncherRefresh if self.page == Page::Launcher && !self.busy => {
                return task(
                    || {
                        Ok(canopa_launcher::Catalog::read(
                            &canopa_launcher::catalog_path()?,
                        )?)
                    },
                    Message::LauncherCatalog,
                );
            }
            Message::LauncherEnabled(id, value) if !self.busy => {
                if let Some(pref) = self.launcher.extension(&id) {
                    pref.enabled = value;
                }
            }
            Message::LauncherKeyword(id, value) if !self.busy => {
                if let Some(pref) = self.launcher.extension(&id) {
                    pref.keyword = value;
                }
            }
            Message::LauncherTemplate(value) if !self.busy => {
                self.launcher.draft.web_template = value
            }
            Message::LauncherSearch(index, template, value) if !self.busy => {
                if let Some(search) = self.launcher.draft.searches.get_mut(index) {
                    if template {
                        search.template = value;
                    } else {
                        search.keyword = value;
                    }
                }
            }
            Message::LauncherRemoveSearch(index)
                if !self.busy && index < self.launcher.draft.searches.len() =>
            {
                self.launcher.draft.searches.remove(index);
            }
            Message::LauncherAddSearch if !self.busy && self.launcher.draft.searches.len() < 16 => {
                self.launcher.draft.searches.push(canopa_launcher::Search {
                    keyword: String::new(),
                    template: "https://example.com/?q={query}".into(),
                })
            }
            Message::ReloadSaved if !self.busy && self.trial.is_none() => {
                return self.reload_page();
            }
            Message::BackendChanged if !self.busy && !self.refreshing && self.trial.is_none() => {
                self.refreshing = true;
                return task(|| Client::discover()?.outputs(), Message::Outputs);
            }
            Message::DisplayProfiles(action) => return self.update_profiles(action),
            Message::WidgetLayouts(action) => self.update_widget_layouts(action),
            Message::OpenLockWidgetEditor if !self.busy && self.trial.is_none() => {
                self.widget_layouts.lock = true;
                self.widget_layouts.widget = None;
                return self.update(Message::Page(Page::Widgets));
            }
            Message::Search(query) => self.search = query.chars().take(100).collect(),
            Message::SearchSubmit => {
                if let Some(entry) = registry::search(&self.search).first()
                    && !self.busy
                    && self.trial.is_none()
                {
                    self.search.clear();
                    return self.update(Message::Page(entry.page));
                }
            }
            Message::StepPage(offset) => {
                return self.update(Message::Page(registry::step(self.page, offset)));
            }
            Message::Page(page) if !self.busy && self.trial.is_none() => {
                if page != self.page {
                    // Typed passwords do not outlive the page.
                    self.users_forms.clear_secrets();
                }
                self.page = page;
                self.idle_available = false;
                self.input_level = None;
                if page == Page::Sound {
                    self.alert_sounds = Snapshot::load(self.paths.common())
                        .ok()
                        .and_then(|source| crate::sound::alert_sounds(&source.text).ok());
                }
                if page == Page::Widgets && self.widget_layouts.editor.is_none() {
                    return widget_layouts::View::load(&self.paths);
                }
                if page == Page::DateTime {
                    self.date_time_opened();
                }
                if page == Page::Notifications && self.notification_apps.is_none() {
                    return task(notifications::installed, |apps| {
                        Message::NotificationsForm(notifications::Edit::Installed(apps))
                    });
                }
                if page == Page::Shortcuts && self.shortcuts.is_none() {
                    let paths = self.paths.clone();
                    return task(
                        move || crate::shortcuts::Editor::load(&paths),
                        Message::ShortcutsLoaded,
                    );
                }
            }
            Message::ShortcutsLoaded(result) => self.shortcuts = Some(result),
            Message::ShortcutsStored(result) => {
                self.busy = false;
                match result {
                    Ok(editor) => {
                        self.shortcuts = Some(Ok(editor));
                        self.notice = fl!("settings-shortcuts-saved");
                        self.document_saved(Page::Shortcuts, Document::Niri, None);
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::ShortcutSearch(value) if !self.busy => self.shortcut_query = value,
            Message::ShortcutKey(index, value) if !self.busy => {
                if let Some(Ok(editor)) = &mut self.shortcuts
                    && let Some(shortcut) = editor.rows.get_mut(index)
                {
                    shortcut.key = value.chars().take(100).collect();
                }
            }
            Message::ResetShortcuts if !self.busy => {
                if let Some(Ok(editor)) = &mut self.shortcuts {
                    for row in &mut editor.rows {
                        row.key.clone_from(&row.default_key);
                    }
                }
            }
            Message::Refresh if !self.busy && !self.refreshing && self.trial.is_none() => {
                self.refreshing = true;
                self.display_dirty = false;
                return task(|| Client::discover()?.outputs(), Message::Outputs);
            }
            Message::Outputs(result) => {
                self.refreshing = false;
                match result {
                    Ok(outputs) => {
                        self.outputs = outputs;
                        if !self.display_dirty {
                            let mut names: Vec<_> = self.outputs.keys().cloned().collect();
                            names.sort();
                            self.monitors = names
                                .iter()
                                .map(|name| Monitor::from_output(&self.outputs[name]))
                                .collect();
                            self.positions = self
                                .monitors
                                .iter()
                                .map(|m| (m.x.to_string(), m.y.to_string()))
                                .collect();
                        } else {
                            self.notice = fl!("settings-displays-changed-while-editing");
                        }
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::Enabled(index, value) => {
                self.monitors[index].enabled = value;
                self.display_dirty = true;
            }
            Message::Mode(index, mode) => {
                self.monitors[index].mode =
                    (mode < self.monitors[index].modes.len()).then_some(mode);
                self.monitors[index].custom_mode = self
                    .outputs
                    .get(&self.monitors[index].name)
                    .is_some_and(|o| {
                        o.is_custom_mode && o.current_mode == self.monitors[index].mode
                    });
                self.display_dirty = true;
            }
            Message::Scale(index, scale) => {
                self.monitors[index].scale = scale;
                self.display_dirty = true;
            }
            Message::Rotation(index, transform) => {
                self.monitors[index].transform = transform;
                self.display_dirty = true;
            }
            Message::Position(index, horizontal, value) => {
                if horizontal {
                    self.positions[index].0 = value;
                } else {
                    self.positions[index].1 = value;
                }
                self.display_dirty = true;
            }
            Message::InputText(field, value) => match field {
                0 => self.input.value.layouts = value,
                1 => self.input.value.variants = value,
                _ => self.input.value.options = value,
            },
            Message::InputNumber(field, value) => match field {
                0 => self.input.value.repeat_delay = value as u16,
                1 => self.input.value.repeat_rate = value as u8,
                2 => self.input.value.mouse_speed = value,
                _ => self.input.value.touchpad_speed = value,
            },
            Message::InputFlag(field, value) => match field {
                0 => self.input.value.mouse_left_handed = value,
                1 => self.input.value.mouse_natural_scroll = value,
                2 => self.input.value.touchpad_tap = value,
                3 => self.input.value.touchpad_natural_scroll = value,
                _ => self.input.value.touchpad_disable_while_typing = value,
            },
            Message::PersonalText(field, value) => match field {
                0 => self.personal.value.accent = value,
                1 => self.personal.value.values.lock.clock_format = value,
                2 => self.personal.value.values.lock.date_format = value,
                3 => self.personal.value.values.lock.font = value,
                _ => {
                    self.personal.value.values.lock.avatar =
                        (!value.is_empty()).then(|| value.into())
                }
            },
            Message::Light(value) => {
                self.personal.value.values.appearance.color_scheme = if value {
                    canopa_theme::ColorScheme::Light
                } else {
                    canopa_theme::ColorScheme::Dark
                }
            }
            Message::TextScale(value) => self.personal.value.values.text_scale = value as u16,
            Message::ReducedMotion(value) => self.personal.value.values.reduced_motion = value,
            Message::HighContrast(value) => {
                let values = &mut self.personal.value.values;
                values.high_contrast = value;
                // Preview at once; Save writes it for every Canopa surface.
                Arc::make_mut(&mut values.appearance.customization).high_contrast = value;
            }
            Message::CursorSize(size)
                if self.paths.active() && size != self.input.value.cursor_size =>
            {
                let unsaved = self
                    .input
                    .source()
                    .and_then(|source| <Input as FileModel>::read(source).ok())
                    .is_none_or(|saved| saved != self.input.value);
                if unsaved {
                    self.notice = fl!("settings-cursor-size-unsaved-input");
                    return Task::none();
                }
                self.busy = true;
                self.notice.clear();
                let paths = self.paths.clone();
                return task(
                    move || crate::input::set_cursor_size(&paths, size),
                    Message::CursorSizeStored,
                );
            }
            Message::CursorSizeStored(result) => {
                self.busy = false;
                match result {
                    Ok(_) => {
                        // The Input draft had no edits; read the new include.
                        self.input.reload(&self.paths);
                        self.notice = fl!("settings-cursor-size-applied");
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::CompositorMotion(enabled) if !self.busy && self.trial.is_none() => {
                if matches!(&self.shortcuts, Some(Ok(editor)) if editor.dirty()) {
                    self.notice = fl!("settings-compositor-motion-unsaved-shortcuts");
                    return Task::none();
                }
                self.busy = true;
                self.notice.clear();
                let paths = self.paths.clone();
                return task(
                    move || {
                        crate::niri::set_reduced_motion(&paths, enabled)?;
                        Ok(enabled)
                    },
                    Message::CompositorMotionStored,
                );
            }
            Message::CompositorMotionStored(result) => {
                self.busy = false;
                match result {
                    Ok(enabled) => {
                        self.compositor_motion = Some(enabled);
                        self.notice = fl!("settings-compositor-motion-saved");
                        self.document_saved(Page::Personal, Document::Niri, None);
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::Dim(value) => self.personal.value.values.lock.dim = value as u8,
            Message::IdleTime(battery, index, value) => {
                if battery {
                    self.idle.value.battery[index] = value;
                } else {
                    self.idle.value.ac[index] = value;
                }
            }
            Message::Profile(value) => self.profile = value,
            Message::IdleAction(action)
                if self.page == Page::Power && !self.busy && self.idle_available =>
            {
                self.busy = true;
                return task(
                    move || {
                        crate::preferences::idle_command(&[action.into()])?;
                        Ok(fl!("settings-idle-policy-updated"))
                    },
                    Message::Finished,
                );
            }
            Message::UseProfile
                if self.page == Page::Power && !self.busy && self.idle_available =>
            {
                self.busy = true;
                let profile = self.profile.clone();
                return task(
                    move || {
                        ensure!(
                            profile.len() <= 128 && !profile.contains('\0'),
                            "invalid profile name"
                        );
                        crate::preferences::idle_command(&["profile".into(), profile])?;
                        Ok(fl!("settings-idle-profile-selected"))
                    },
                    Message::Finished,
                );
            }
            Message::IdleStatus(result) => {
                self.idle_status = match result {
                    Ok(value) => serde_json::from_str::<serde_json::Value>(&value)
                        .map(|info| {
                            self.idle_available =
                                info["state"].is_string() && info["manually_paused"].is_boolean();
                            // State and profile are Stasis's own names.
                            fl!(
                                "settings-idle-status",
                                state = info["state"].as_str().unwrap_or("available"),
                                profile = info["profile"].as_str().unwrap_or("default"),
                                paused = if info["manually_paused"].as_bool().unwrap_or(false) {
                                    "yes"
                                } else {
                                    "no"
                                }
                            )
                        })
                        .unwrap_or(value),
                    Err(error) => {
                        self.idle_available = false;
                        fl!("settings-idle-unavailable", error = error)
                    }
                }
            }
            Message::Save if !self.busy && self.trial.is_none() => {
                self.notice.clear();
                self.busy = true;
                let paths = self.paths.clone();
                match self.page {
                    #[cfg(debug_assertions)]
                    Page::LiveExample => self.busy = false,
                    Page::Sound
                    | Page::Network
                    | Page::Bluetooth
                    | Page::Notifications
                    | Page::DefaultApps
                    | Page::DateTime
                    | Page::About
                    | Page::Region
                    | Page::Users
                    | Page::Privacy
                    | Page::Printers => self.busy = false,
                    Page::Launcher => {
                        let model = self.launcher.clone();
                        return task(move || model.save(), Message::LauncherStored);
                    }
                    Page::Widgets => return self.save_widget_layouts(),
                    Page::Shortcuts => {
                        let Some(Ok(editor)) = self.shortcuts.clone() else {
                            self.busy = false;
                            return Task::none();
                        };
                        return task(move || editor.save(&paths), Message::ShortcutsStored);
                    }
                    Page::Displays => {
                        let monitors = match self.display_draft() {
                            Ok(monitors) => monitors,
                            Err(error) => {
                                self.busy = false;
                                self.notice = error.to_string();
                                return Task::none();
                            }
                        };
                        return task(
                            move || TrialHandle::start(&paths, monitors, None, false),
                            Message::TrialStarted,
                        );
                    }
                    Page::Input => return file_save(Page::Input, self.input.save_job(&paths)),
                    // Accessibility shares the Personalization draft of config.kdl.
                    Page::Personal | Page::Accessibility => {
                        return file_save(Page::Personal, self.personal.save_job(&paths));
                    }
                    Page::Power => {
                        let choices = self.power_choices.save_job(&paths);
                        let idle = self.idle_dirty().then(|| self.idle.save_job(&paths));
                        return task(
                            move || {
                                let saved = choices()?;
                                let idle = idle
                                    .map(|job| job())
                                    .transpose()
                                    .map_err(|error| format!("{error:#}"));
                                Ok((saved, idle))
                            },
                            Message::PowerStored,
                        );
                    }
                }
            }
            Message::Stored(result) => {
                self.busy = false;
                match result {
                    Ok((page, saved)) => {
                        self.notice = fl!("settings-saved");
                        self.file_saved(page, saved);
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::TrialStarted(result) => {
                self.busy = false;
                match result {
                    Ok(trial) => {
                        self.trial = Some(trial);
                        self.remaining = crate::display::CONFIRM_SECONDS;
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::Decision(keep) => {
                if let Some(trial) = self.trial.take() {
                    self.busy = true;
                    return task(move || trial.finish(keep), Message::TrialFinished);
                }
            }
            Message::Tick => {
                self.remaining = self.remaining.saturating_sub(1);
                if self.remaining == 0 {
                    return self.update(Message::Decision(false));
                }
            }
            Message::TrialFinished(result) => {
                self.busy = false;
                self.notice = result.unwrap_or_else(|error| error);
                return self.update(Message::Refresh);
            }
            Message::Wallpaper => {
                self.notice.clear();
                let (send, receive) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = crate::storage::application(
                        companion("canopa-wallpaper"),
                        &["pick".into()],
                    )
                    .map_err(|error| format!("{error:#}"));
                    let _ = send.send(result);
                });
                return Task::perform(
                    async move {
                        receive
                            .await
                            .unwrap_or_else(|_| Err(fl!("settings-wallpaper-stopped")))
                    },
                    Message::WallpaperFinished,
                );
            }
            Message::WallpaperFinished(Err(error)) => {
                self.notice = error;
            }
            Message::Finished(result) => {
                self.busy = false;
                self.notice = result.unwrap_or_else(|error| error);
            }
            Message::Activated(request) => return self.activate(request),
            Message::Raised(false) => {
                return iced::window::latest().then(|id| match id {
                    Some(id) => Task::batch([
                        iced::window::gain_focus(id),
                        iced::window::request_user_attention(
                            id,
                            Some(iced::window::UserAttention::Informational),
                        ),
                    ]),
                    None => Task::none(),
                });
            }
            Message::Power(event) => return self.power.update(event, power::message),
            Message::Sound(event) => return self.sound.update(event, sound::message),
            Message::Network(event) => return self.network.update(event, network::message),
            Message::NetworkForm(edit) => return self.network_edit(edit),
            Message::Bluetooth(event) => return self.bluetooth.update(event, bluetooth::message),
            Message::BluetoothForm(edit) => return self.bluetooth_edit(edit),
            Message::Notifications(event) => {
                return self.notifications.update(event, notifications::message);
            }
            Message::NotificationsForm(edit) => return self.notifications_edit(edit),
            Message::DefaultApps(event) => {
                return self.default_apps.update(event, default_apps::message);
            }
            Message::DateTime(event) => return self.date_time.update(event, date_time::message),
            Message::DateTimeForm(edit) => return self.date_time_edit(edit),
            Message::About(event) => return self.about.update(event, about::message),
            Message::AboutForm(edit) => return self.about_edit(edit),
            Message::Region(event) => return self.region.update(event, region::message),
            Message::Users(event) => return self.users.update(event, users::message),
            Message::UsersForm(edit) => return self.users_edit(edit),
            Message::Privacy(event) => return self.privacy.update(event, privacy::message),
            Message::Printers(event) => return self.printers.update(event, printers::message),
            Message::DefaultAppsSearch(query) => {
                self.default_apps_query = query.chars().take(100).collect();
            }
            Message::Level(level) if self.page == Page::Sound => self.input_level = level,
            Message::AlertSounds(enabled) if !self.busy && self.trial.is_none() => {
                self.busy = true;
                self.notice.clear();
                let paths = self.paths.clone();
                return task(
                    move || crate::sound::set_alert_sounds(&paths, enabled),
                    Message::AlertSoundsStored,
                );
            }
            Message::AlertSoundsStored(result) => {
                self.busy = false;
                match result {
                    Ok((previous, saved)) => {
                        self.alert_sounds = crate::sound::alert_sounds(&saved.text).ok();
                        self.notice = fl!("settings-alert-sounds-updated");
                        self.document_saved(
                            Page::Sound,
                            Document::Common,
                            Some((&previous, &saved)),
                        );
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::LidClose(action) => self.power_choices.value.lid_close = action,
            Message::PowerKey(to_logind)
                if !self.busy
                    && self.trial.is_none()
                    && self.paths.active()
                    && to_logind != self.input.value.power_key_to_logind =>
            {
                let unsaved = self
                    .input
                    .source()
                    .and_then(|source| <Input as FileModel>::read(source).ok())
                    .is_none_or(|saved| saved != self.input.value);
                if unsaved {
                    self.notice = fl!("settings-power-key-unsaved-input");
                    return Task::none();
                }
                self.busy = true;
                self.notice.clear();
                let paths = self.paths.clone();
                return task(
                    move || crate::input::set_power_key(&paths, to_logind),
                    Message::PowerKeyStored,
                );
            }
            Message::PowerKeyStored(result) => {
                self.busy = false;
                match result {
                    Ok(_) => {
                        // The Input draft had no edits; read the new include.
                        self.input.reload(&self.paths);
                        self.notice = fl!("settings-power-key-applied");
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::PowerStored(result) => {
                self.busy = false;
                match result {
                    Ok((saved, idle)) => {
                        self.power_choices.saved(saved);
                        self.notice = match idle {
                            Ok(Some(saved)) => {
                                self.idle.saved(saved);
                                if self.idle_available {
                                    fl!("settings-saved")
                                } else {
                                    fl!("settings-saved-idle-not-running")
                                }
                            }
                            Ok(None) => fl!("settings-saved"),
                            Err(error) => fl!("settings-lid-saved-idle-failed", error = error),
                        };
                    }
                    Err(error) => self.notice = error,
                }
            }
            #[cfg(debug_assertions)]
            Message::LiveExample(event) => {
                return self.live_example.update(event, live_example::message);
            }
            #[cfg(debug_assertions)]
            Message::Simulate(simulate) => self.simulate(simulate),
            _ => {}
        }
        Task::none()
    }
    /// Show what a later launch or a D-Bus caller asked for, then raise.
    fn activate(&mut self, request: Request) -> Task<Message> {
        let Some(page) = request.page else {
            return raise();
        };
        if self.busy || self.trial.is_some() {
            self.notice = fl!("settings-activate-busy", page = page.title());
            return raise();
        }
        if let Some(profile) = request.display_profile {
            self.display_profiles.selected = Some(profile);
            self.display_profiles.prominent = true;
        }
        self.search.clear();
        Task::batch([self.update(Message::Page(page)), raise()])
    }
    /// A file-backed page saved: adopt the save, then apply the shared-document rule.
    fn file_saved(&mut self, page: Page, saved: Snapshot) {
        let (document, previous) = match page {
            Page::Input => (Input::DOCUMENT, self.input.saved(saved.clone())),
            Page::Personal => (
                Personalization::DOCUMENT,
                self.personal.saved(saved.clone()),
            ),
            _ => return,
        };
        if let Some(previous) = previous {
            self.document_saved(page, document, Some((&previous, &saved)));
        }
    }
    /// The shared-document rule (see [`crate::file_page`]): `by` saved
    /// `document`, replacing the first snapshot with the second when known.
    /// Every other page that edits it keeps its draft on the new version when
    /// it can. A page without a rule here stays on its version, so its own
    /// next save reports the change instead of overwriting it.
    pub(super) fn document_saved(
        &mut self,
        by: Page,
        document: Document,
        change: Option<(&Snapshot, &Snapshot)>,
    ) {
        let pages = registry::PAGES
            .iter()
            .filter(|entry| entry.page != by && entry.page.edits(document));
        for entry in pages {
            match (entry.page, document, change) {
                (Page::Personal, Document::Common, Some((previous, saved))) => {
                    self.personal.rebase(previous, saved);
                }
                (Page::Widgets, Document::Common, Some((previous, saved))) => {
                    if !self.widget_layouts.dirty() {
                        self.widget_layouts.editor = None;
                    } else if let Some(Ok(editor)) = &mut self.widget_layouts.editor
                        && let Err(error) = editor.rebase(previous, saved)
                    {
                        self.notice = format!("{} {error}", self.notice);
                    }
                }
                // Shortcuts reload from the new file when next opened.
                (Page::Shortcuts, Document::Niri, _) => self.shortcuts = None,
                (Page::Personal, Document::Niri, _) => {
                    self.compositor_motion = Self::load_motion(&self.paths);
                }
                _ => {}
            }
        }
    }
    /// Reload the current page's sources, discarding only its drafts.
    fn reload_page(&mut self) -> Task<Message> {
        self.notice.clear();
        let paths = self.paths.clone();
        match self.page {
            Page::Displays => {
                self.display_dirty = false;
                self.refreshing = true;
                self.display_profiles.discard();
                Task::batch([
                    task(|| Client::discover()?.outputs(), Message::Outputs),
                    profiles::Editor::load(&paths),
                ])
            }
            Page::Input => {
                self.notice = self.input.reload(&paths).unwrap_or_default();
                Task::none()
            }
            Page::Personal | Page::Accessibility => {
                self.notice = self.personal.reload(&paths).unwrap_or_default();
                self.compositor_motion = Self::load_motion(&paths);
                if self.page == Page::Accessibility {
                    let _ = self.input.reload(&paths);
                }
                Task::none()
            }
            Page::Power => {
                self.idle.reload(&paths);
                self.power_choices.reload(&paths);
                self.profile = "none".into();
                self.power
                    .update(crate::live::Event::Reload, power::message)
            }
            Page::Launcher => {
                self.launcher = Default::default();
                self.clipboard = None;
                self.clipboard_notice.clear();
                Task::batch([
                    task(crate::launcher::Model::load, Message::LauncherLoaded),
                    task(
                        crate::launcher::ClipboardEditor::load,
                        Message::ClipboardLoaded,
                    ),
                ])
            }
            Page::Shortcuts => {
                self.shortcuts = None;
                task(
                    move || crate::shortcuts::Editor::load(&paths),
                    Message::ShortcutsLoaded,
                )
            }
            Page::Widgets => {
                self.widget_layouts.editor = None;
                widget_layouts::View::load(&paths)
            }
            #[cfg(debug_assertions)]
            Page::LiveExample => self
                .live_example
                .update(crate::live::Event::Reload, live_example::message),
            Page::Sound => self
                .sound
                .update(crate::live::Event::Reload, sound::message),
            Page::Network => self
                .network
                .update(crate::live::Event::Reload, network::message),
            Page::Bluetooth => self
                .bluetooth
                .update(crate::live::Event::Reload, bluetooth::message),
            Page::Notifications => self
                .notifications
                .update(crate::live::Event::Reload, notifications::message),
            Page::DefaultApps => self
                .default_apps
                .update(crate::live::Event::Reload, default_apps::message),
            Page::Region => self
                .region
                .update(crate::live::Event::Reload, region::message),
            Page::Privacy => self
                .privacy
                .update(crate::live::Event::Reload, privacy::message),
            Page::Printers => self
                .printers
                .update(crate::live::Event::Reload, printers::message),
            Page::Users => {
                self.users_forms.clear_secrets();
                self.users
                    .update(crate::live::Event::Reload, users::message)
            }
            Page::About => {
                self.about_name = None;
                self.about
                    .update(crate::live::Event::Reload, about::message)
            }
            Page::DateTime => {
                self.date_time_opened();
                self.date_time
                    .update(crate::live::Event::Reload, date_time::message)
            }
        }
    }
    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();
        if self.page == Page::Launcher {
            subscriptions
                .push(iced::time::every(Duration::from_secs(2)).map(|_| Message::LauncherRefresh));
        }
        if matches!(self.page, Page::Displays | Page::Widgets) || self.trial.is_some() {
            subscriptions.push(Subscription::run(niri_events));
        }
        if self.page == Page::Power {
            subscriptions.push(Subscription::run(idle_events));
            subscriptions.push(self.power.subscription(Message::Power));
        }
        if self.page == Page::Sound {
            subscriptions.push(self.sound_subscription());
        }
        if self.page == Page::Network {
            subscriptions.push(self.network.subscription(Message::Network));
        }
        if self.page == Page::Bluetooth {
            subscriptions.push(self.bluetooth.subscription(Message::Bluetooth));
        }
        if self.page == Page::Notifications {
            subscriptions.push(self.notifications.subscription(Message::Notifications));
        }
        if self.page == Page::DefaultApps {
            subscriptions.push(self.default_apps.subscription(Message::DefaultApps));
        }
        if self.page == Page::About {
            subscriptions.push(self.about.subscription(Message::About));
        }
        if self.page == Page::Region {
            subscriptions.push(self.region.subscription(Message::Region));
        }
        if self.page == Page::Users {
            subscriptions.push(self.users.subscription(Message::Users));
        }
        if self.page == Page::Privacy {
            subscriptions.push(self.privacy.subscription(Message::Privacy));
        }
        if self.page == Page::Printers {
            subscriptions.push(self.printers.subscription(Message::Printers));
        }
        if self.page == Page::DateTime {
            subscriptions.push(self.date_time.subscription(Message::DateTime));
            subscriptions.push(
                iced::time::every(Duration::from_secs(1))
                    .map(|_| Message::DateTimeForm(date_time::Edit::Tick)),
            );
        }
        if self.trial.is_some() {
            subscriptions.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick));
        }
        if let Some(source) = &self.activations {
            subscriptions.push(Subscription::run_with(source.clone(), activations));
        }
        if let Some(bridge) = &self.accessibility {
            subscriptions.push(bridge.subscription());
        }
        #[cfg(debug_assertions)]
        if self.page == Page::LiveExample {
            subscriptions.push(self.live_example.subscription(live_example::message));
        }
        Subscription::batch(subscriptions)
    }
    fn view(&self) -> Element<'_, Message> {
        let navigation = self.navigation();
        let entry = self.page.entry();
        let page_title = entry.title();
        let title = column![
            semantic(
                Node::heading(page_title.clone(), 1).key("settings-page-title"),
                heading(page_title.clone(), 28)
                    .width(Length::Fill)
                    .align_x(direction::text_start())
            ),
            text(entry.description())
                .size(14)
                .style(canopa_ui::muted)
                .width(Length::Fill)
                .align_x(direction::text_start())
        ]
        .align_x(direction::start())
        .spacing(5);
        // Keep this child present so asynchronous notices do not replace the
        // page's widget tree and discard keyboard focus or typed drafts.
        let mut body = column![semantic(
            Node::status(&self.notice).key("settings-notice"),
            container(text(&self.notice).size(14))
                .padding(if self.notice.is_empty() { 0 } else { 12 })
                .height(if self.notice.is_empty() {
                    // Iced filters zero-height children out of a Column.
                    Length::Fixed(1.)
                } else {
                    Length::Shrink
                })
                .style(canopa_ui::surface)
        )]
        .spacing(16);
        if self.trial.is_some() {
            body = body.push(semantic(
                Node::alert_dialog(fl!("settings-trial-title")).key("settings-trial"),
                container(
                    column![
                        text(fl!("settings-trial-countdown", seconds = self.remaining)).size(20),
                        text(fl!("settings-trial-keys")),
                        row![
                            self.action(
                                "settings-button-revert",
                                fl!("settings-trial-revert"),
                                Some(Message::Decision(false)),
                                false
                            ),
                            self.action(
                                "settings-button-keep-changes",
                                fl!("settings-trial-keep"),
                                Some(Message::Decision(true)),
                                true
                            )
                        ]
                        .spacing(12)
                    ]
                    .spacing(12),
                )
                .padding(16)
                .style(canopa_ui::surface),
            ));
        }
        if matches!(self.page, Page::Displays | Page::Input) && !self.paths.active() {
            body = body.push(settings_group(
                fl!("settings-session-required-title"),
                text(fl!("settings-session-required"))
                    .size(14)
                    .style(canopa_ui::muted),
            ));
        }
        body = body.push(match self.page {
            Page::Displays => self.displays(),
            Page::Input => self.input_view(),
            Page::Sound => self.sound_view(),
            Page::Network => self.network_view(),
            Page::Bluetooth => self.bluetooth_view(),
            Page::Notifications => self.notifications_view(),
            Page::DefaultApps => self.default_apps_view(),
            Page::DateTime => self.date_time_view(),
            Page::About => self.about_view(),
            Page::Region => self.region_view(),
            Page::Users => self.users_view(),
            Page::Privacy => self.privacy_view(),
            Page::Printers => self.printers_view(),
            Page::Personal => self.personal_view(),
            Page::Accessibility => self.accessibility_view(),
            Page::Power => self.power_view(),
            Page::Launcher => self.launcher_view(),
            Page::Shortcuts => self.shortcuts_view(),
            Page::Widgets => self.widget_layouts_view(),
            #[cfg(debug_assertions)]
            Page::LiveExample => self.live_example_view(),
        });
        let content = column![
            container(title).padding([19, 20]).width(Length::Fill),
            rule::horizontal(1).style(settings_rule),
            semantic(
                Node::new(accessibility::Role::Main)
                    .label(page_title)
                    .key("settings-page"),
                canopa_ui::scroll_speed(
                    scrollable(
                        container(body.max_width(800).padding([14, 20])).width(Length::Fill)
                    )
                    .id("settings-page-scroll")
                    .height(Length::Fill)
                )
            ),
        ]
        .width(Length::Fill);
        // Live pages apply each change at once and have no footer.
        let content = match self.file_footer() {
            Some(footer) => content
                .push(rule::horizontal(1).style(settings_rule))
                .push(footer),
            None => content,
        };
        let window = focus::group(
            // The sidebar leads: on the right in right-to-left languages.
            direction::row([
                navigation,
                rule::vertical(1).style(settings_rule).into(),
                content.into(),
            ]),
            self.trial.as_ref().map(|_| Message::Decision(false)),
            format!(
                "{:?}-{}-{:?}",
                self.page,
                self.trial.is_some(),
                if self.page == Page::Displays {
                    self.monitors
                        .iter()
                        .map(|m| (&m.name, &m.make, &m.model, &m.serial))
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                }
            ),
            if self.trial.is_some() {
                "settings-button-revert".into()
            } else {
                navigation::link_id(entry)
            },
            self.busy,
        );
        semantic(
            Node::window(ACCESSIBILITY_ROOT, fl!("settings-window-title")).busy(self.busy),
            window,
        )
    }
    /// Whether the selected file-backed page can save its draft now.
    fn can_save(&self) -> bool {
        (self.page != Page::Launcher || self.launcher.loaded)
            && !self.busy
            && self.trial.is_none()
            && (self.page != Page::Shortcuts
                || (matches!(self.shortcuts, Some(Ok(_))) && self.paths.active()))
            && (self.page != Page::Widgets || self.widget_layouts.dirty())
            && (self.paths.active() || !matches!(self.page, Page::Displays | Page::Input))
    }
    /// The file-backed pattern's footer: reload this page, and save or test.
    fn file_footer(&self) -> Option<Element<'_, Message>> {
        let Kind::File { apply, .. } = self.page.kind() else {
            return None;
        };
        Some(semantic(
            Node::toolbar(fl!("settings-footer-label")).key("settings-footer"),
            // Actions sit at the trailing edge, the primary one last.
            direction::row([
                space().width(Length::Fill).into(),
                // Reloads only the selected page; other drafts remain.
                self.action(
                    "settings-reload-page",
                    fl!("settings-footer-reload"),
                    (!self.busy && self.trial.is_none()).then_some(Message::ReloadSaved),
                    false,
                ),
                button(
                    "settings-apply",
                    if self.busy {
                        fl!("settings-applying")
                    } else if apply == Apply::Trial {
                        fl!("settings-footer-test-displays")
                    } else {
                        fl!("settings-footer-save")
                    },
                    self.can_save().then_some(Message::Save),
                    canopa_ui::Action::Primary,
                    Length::Shrink,
                    self.busy,
                ),
            ])
            .align_y(Alignment::Center)
            .spacing(12)
            .padding([10, 20]),
        ))
    }
    fn displays(&self) -> Element<'_, Message> {
        let mut body = column![
            text(fl!("settings-displays-intro")),
            self.action(
                "settings-button-reload-displays",
                if self.refreshing {
                    fl!("settings-displays-refreshing")
                } else {
                    fl!("settings-displays-reload")
                },
                (!self.busy && !self.refreshing && self.trial.is_none())
                    .then_some(Message::Refresh),
                false
            )
        ]
        .spacing(14);
        if self.display_profiles.prominent {
            body = body.push(self.profiles_view());
        }
        if self.monitors.is_empty() {
            body = body.push(text(fl!("settings-displays-none")));
        }
        for (index, monitor) in self.monitors.iter().enumerate() {
            let modes: Vec<_> = monitor
                .modes
                .iter()
                .enumerate()
                .map(|(index, mode)| (index, mode_label(mode)))
                .collect();
            let card = column![
                text(format!(
                    "{} · {} {}",
                    monitor.name, monitor.make, monitor.model
                ))
                .size(20),
                toggle(
                    format!("display-enabled-{index}"),
                    fl!("settings-display-enabled"),
                    monitor.enabled,
                    Message::Enabled(index, !monitor.enabled)
                ),
                row![
                    column![
                        text(fl!("settings-display-mode")),
                        pick(
                            format!("display-mode-{index}"),
                            modes,
                            monitor.mode,
                            move |v| { Message::Mode(index, v) }
                        )
                    ]
                    .spacing(6),
                    column![
                        text(fl!("settings-display-rotation")),
                        pick(
                            format!("display-rotation-{index}"),
                            transforms()
                                .into_iter()
                                .map(|v| (v, transform_label(v)))
                                .collect(),
                            Some(monitor.transform),
                            move |v| Message::Rotation(index, v)
                        )
                    ]
                    .spacing(6)
                ]
                .spacing(18),
                text(fl!(
                    "settings-display-scale",
                    scale = canopa_i18n::format::percent(monitor.scale * 100.)
                )),
                adjust(
                    format!("display-scale-{index}"),
                    0.5..=4.,
                    monitor.scale,
                    0.25,
                    move |v| Message::Scale(index, v)
                ),
                row![
                    field(
                        format!("display-x-{}", monitor.name),
                        fl!("settings-display-x"),
                        &self.positions[index].0,
                        move |v| Message::Position(index, true, v)
                    ),
                    field(
                        format!("display-y-{}", monitor.name),
                        fl!("settings-display-y"),
                        &self.positions[index].1,
                        move |v| Message::Position(index, false, v)
                    )
                ]
                .spacing(18),
                text(fl!("settings-displays-positions-hint"))
            ]
            .spacing(10);
            body = body.push(
                container(card)
                    .padding(16)
                    .width(Length::Fill)
                    .style(canopa_ui::surface),
            );
        }
        if !self.display_profiles.prominent {
            body = body.push(self.profiles_view());
        }
        body.into()
    }
    fn input_view(&self) -> Element<'_, Message> {
        let keyboard = column![
            field(
                "settings-field-layouts-in-switch-order-for-example-us-fr",
                fl!("settings-input-layouts"),
                &self.input.value.layouts,
                |v| Message::InputText(0, v)
            ),
            field(
                "settings-field-variants-optional-in-the-same-order",
                fl!("settings-input-variants"),
                &self.input.value.variants,
                |v| Message::InputText(1, v)
            ),
            field(
                "settings-field-xkb-options-for-example-grp-alt-shift-toggle",
                fl!("settings-input-options"),
                &self.input.value.options,
                |v| Message::InputText(2, v)
            ),
            text(fl!(
                "settings-input-repeat-delay",
                delay = self.input.value.repeat_delay
            )),
            adjust(
                "repeat-delay",
                100.0..=2000.,
                f64::from(self.input.value.repeat_delay),
                50.,
                |v| Message::InputNumber(0, v)
            ),
            text(fl!(
                "settings-input-repeat-rate",
                rate = self.input.value.repeat_rate
            )),
            adjust(
                "repeat-rate",
                1.0..=100.,
                f64::from(self.input.value.repeat_rate),
                1.,
                |v| Message::InputNumber(1, v)
            ),
        ]
        .spacing(10);
        let mouse = column![
            text(fl!(
                "settings-input-pointer-speed",
                speed = canopa_i18n::format::decimal(self.input.value.mouse_speed, 2, true)
            )),
            adjust(
                "mouse-speed",
                -1.0..=1.,
                self.input.value.mouse_speed,
                0.05,
                |v| Message::InputNumber(2, v)
            ),
            toggle(
                "input-flag-0",
                fl!("settings-input-left-handed"),
                self.input.value.mouse_left_handed,
                Message::InputFlag(0, !self.input.value.mouse_left_handed)
            ),
            toggle(
                "input-flag-1",
                fl!("settings-input-mouse-natural-scroll"),
                self.input.value.mouse_natural_scroll,
                Message::InputFlag(1, !self.input.value.mouse_natural_scroll)
            ),
        ]
        .spacing(10);
        let touchpad = column![
            text(fl!(
                "settings-input-touchpad-speed",
                speed = canopa_i18n::format::decimal(self.input.value.touchpad_speed, 2, true)
            )),
            adjust(
                "touchpad-speed",
                -1.0..=1.,
                self.input.value.touchpad_speed,
                0.05,
                |v| Message::InputNumber(3, v)
            ),
            toggle(
                "input-flag-2",
                fl!("settings-input-tap"),
                self.input.value.touchpad_tap,
                Message::InputFlag(2, !self.input.value.touchpad_tap)
            ),
            toggle(
                "input-flag-3",
                fl!("settings-input-touchpad-natural-scroll"),
                self.input.value.touchpad_natural_scroll,
                Message::InputFlag(3, !self.input.value.touchpad_natural_scroll)
            ),
            toggle(
                "input-flag-4",
                fl!("settings-input-disable-while-typing"),
                self.input.value.touchpad_disable_while_typing,
                Message::InputFlag(4, !self.input.value.touchpad_disable_while_typing)
            )
        ]
        .spacing(10);
        column![
            settings_group(fl!("settings-input-keyboard"), keyboard),
            settings_group(fl!("settings-input-mouse"), mouse),
            settings_group(fl!("settings-input-touchpad"), touchpad)
        ]
        .spacing(14)
        .into()
    }
    /// High contrast, larger text and cursor, and motion, over the
    /// Personalization draft and Niri's cursor settings.
    fn accessibility_view(&self) -> Element<'_, Message> {
        let values = &self.personal.value.values;
        let cursor = self.input.value.cursor_size;
        let cursor_label = |size: Option<u16>| match size {
            None => fl!("settings-cursor-size-default", size = 24),
            Some(size) => fl!("settings-cursor-size-pixels", size = size),
        };
        let cursor_options: Vec<_> = std::iter::once(None)
            .chain(crate::input::CURSOR_SIZES[1..].iter().copied().map(Some))
            .map(|size| (size, cursor_label(size)))
            .collect();
        let cursor_control: Element<'_, Message> = if self.paths.active() {
            pick(
                "settings-cursor-size",
                cursor_options,
                Some(cursor),
                Message::CursorSize,
            )
        } else {
            text(fl!("settings-cursor-size-needs-session"))
                .size(14)
                .style(canopa_ui::muted)
                .into()
        };
        let seeing = column![
            toggle(
                "settings-high-contrast",
                fl!("settings-a11y-high-contrast"),
                values.high_contrast,
                Message::HighContrast(!values.high_contrast)
            ),
            text(fl!("settings-a11y-high-contrast-hint"))
                .size(14)
                .style(canopa_ui::muted),
            text(fl!(
                "settings-a11y-text-scale",
                scale = canopa_i18n::format::percent(f64::from(values.text_scale))
            )),
            adjust(
                "settings-accessibility-text-scale",
                100.0..=150.0,
                f64::from(values.text_scale),
                5.0,
                Message::TextScale
            ),
            text(fl!("settings-a11y-cursor-size")),
            cursor_control,
        ]
        .spacing(12);
        let motion = column![
            toggle(
                "settings-accessibility-reduced-motion",
                fl!("settings-a11y-reduced-motion"),
                values.reduced_motion,
                Message::ReducedMotion(!values.reduced_motion)
            ),
            match self.compositor_motion {
                Some(reduced) => self.action(
                    "settings-accessibility-compositor-motion",
                    if reduced {
                        fl!("settings-motion-use-niri")
                    } else {
                        fl!("settings-motion-reduce")
                    },
                    Some(Message::CompositorMotion(!reduced)),
                    false,
                ),
                None => text(fl!("settings-motion-needs-session"))
                    .size(14)
                    .style(canopa_ui::muted)
                    .into(),
            },
        ]
        .spacing(12);
        let reader = text(fl!("settings-a11y-screen-reader-hint"))
            .size(14)
            .style(canopa_ui::muted);
        column![
            settings_group(fl!("settings-a11y-seeing"), seeing),
            settings_group(fl!("settings-a11y-motion"), motion),
            settings_group(fl!("settings-a11y-screen-reader"), reader),
        ]
        .spacing(16)
        .into()
    }
    fn personal_view(&self) -> Element<'_, Message> {
        let appearance = column![
            toggle(
                "light",
                fl!("settings-personal-light"),
                self.personal.value.values.appearance.color_scheme
                    == canopa_theme::ColorScheme::Light,
                Message::Light(
                    self.personal.value.values.appearance.color_scheme
                        != canopa_theme::ColorScheme::Light
                )
            ),
            container(field(
                "settings-field-accent-color-optional-rrggbb",
                fl!("settings-personal-accent"),
                &self.personal.value.accent,
                |v| { Message::PersonalText(0, v) }
            ))
            .max_width(300),
            text(fl!(
                "settings-personal-text-scale",
                scale =
                    canopa_i18n::format::percent(f64::from(self.personal.value.values.text_scale))
            )),
            adjust(
                "settings-text-scale",
                100.0..=150.0,
                f64::from(self.personal.value.values.text_scale),
                5.0,
                Message::TextScale
            ),
            toggle(
                "settings-reduced-motion",
                fl!("settings-personal-reduced-motion"),
                self.personal.value.values.reduced_motion,
                Message::ReducedMotion(!self.personal.value.values.reduced_motion)
            ),
            self.action(
                "settings-button-choose-wallpaper",
                fl!("settings-personal-wallpaper"),
                Some(Message::Wallpaper),
                true
            ),
            text(fl!(
                "settings-personal-wallpaper-hint",
                folder = "~/Pictures/Wallpapers"
            ))
            .size(14)
            .style(canopa_ui::muted),
        ]
        .spacing(12);
        let lockscreen = column![
            field(
                "settings-field-clock-format",
                fl!("settings-personal-clock-format"),
                &self.personal.value.values.lock.clock_format,
                |v| { Message::PersonalText(1, v) }
            ),
            field(
                "settings-field-date-format",
                fl!("settings-personal-date-format"),
                &self.personal.value.values.lock.date_format,
                |v| { Message::PersonalText(2, v) }
            ),
            field(
                "settings-field-font-family",
                fl!("settings-personal-font"),
                &self.personal.value.values.lock.font,
                |v| { Message::PersonalText(3, v) }
            ),
            field(
                "settings-field-avatar-image-optional-absolute-path",
                fl!("settings-personal-avatar"),
                &self
                    .personal
                    .value
                    .values
                    .lock
                    .avatar
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                |v| Message::PersonalText(4, v)
            ),
            text(fl!(
                "settings-personal-dimming",
                dim = canopa_i18n::format::percent(f64::from(self.personal.value.values.lock.dim))
            )),
            adjust(
                "lock-dim",
                0.0..=100.,
                f64::from(self.personal.value.values.lock.dim),
                1.,
                Message::Dim
            ),
            self.action(
                "settings-button-edit-lock-screen-widgets",
                fl!("settings-personal-edit-lock-widgets"),
                Some(Message::OpenLockWidgetEditor),
                false
            )
        ]
        .spacing(12);
        let motion = if let Some(reduced) = self.compositor_motion {
            self.action(
                "settings-button-compositor-motion",
                if reduced {
                    fl!("settings-motion-use-niri")
                } else {
                    fl!("settings-motion-reduce")
                },
                Some(Message::CompositorMotion(!reduced)),
                false,
            )
        } else {
            text(fl!("settings-motion-needs-session"))
                .size(14)
                .style(canopa_ui::muted)
                .into()
        };
        column![
            settings_group(fl!("settings-personal-appearance"), appearance),
            settings_group(fl!("settings-personal-lock-screen"), lockscreen),
            settings_group(fl!("settings-personal-motion"), motion),
        ]
        .spacing(14)
        .into()
    }
}
/// A labelled text field. `key` is its focus and accessibility identity.
fn field<'a>(
    key: impl Into<String>,
    label: impl Into<String>,
    value: &str,
    change: impl Fn(String) -> Message + Clone + Send + Sync + 'static,
) -> Element<'a, Message> {
    let key = key.into();
    let label = label.into();
    semantic(
        Node::text_input(label.clone(), value)
            .key(format!("{key}-field"))
            .on_text(change.clone()),
        column![
            text(label),
            text_input("", value)
                .id(key)
                .on_input(change)
                .align_x(direction::start())
                .padding(10)
        ]
        .spacing(6)
        .width(Length::Fill),
    )
}
fn heading<'a>(label: impl Into<String>, size: u32) -> iced::widget::Text<'a> {
    text(label.into()).size(size).font(Font {
        weight: Weight::Semibold,
        ..Font::DEFAULT
    })
}

fn settings_group<'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let label = label.into();
    semantic(
        Node::group(label.clone()),
        container(
            column![
                semantic(
                    Node::heading(label.clone(), 2),
                    heading(label, 18)
                        .width(Length::Fill)
                        .align_x(direction::text_start())
                ),
                content.into()
            ]
            .spacing(12),
        )
        .padding(16)
        .width(Length::Fill)
        .style(canopa_ui::surface),
    )
}

fn settings_rule(theme: &Theme) -> rule::Style {
    rule::Style {
        color: canopa_ui::colors(theme).border,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

/// A push button. `key` is a stable focus and accessibility identity that
/// never depends on the (translatable) label.
fn button<'a>(
    key: impl Into<String>,
    label: impl Into<String>,
    message: Option<Message>,
    treatment: canopa_ui::Action,
    width: Length,
    preserve_disabled: bool,
) -> Element<'a, Message> {
    let key = key.into();
    let label = label.into();
    semantic(
        Node::button(label.clone())
            .key(key.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        focus::control(
            key,
            message.clone(),
            None,
            None,
            preserve_disabled,
            canopa_ui::action(text(label), message, treatment, false)
                .width(width)
                .padding([8, 12]),
        ),
    )
}
fn toggle<'a>(
    id: impl Into<String>,
    label: impl Into<String>,
    value: bool,
    message: Message,
) -> Element<'a, Message> {
    let id = id.into();
    let label = label.into();
    semantic(
        Node::check_box(label.clone(), value)
            .key(id.clone())
            .on_click(Some(message.clone())),
        focus::control(
            id,
            Some(message.clone()),
            None,
            None,
            false,
            checkbox(value)
                .label(label)
                .on_toggle(move |_| message.clone()),
        ),
    )
}
/// A slider, named by the text drawn before it. Left and Right step it.
fn adjust<'a>(
    id: impl Into<String>,
    range: std::ops::RangeInclusive<f64>,
    value: f64,
    step: f64,
    change: impl Fn(f64) -> Message + Clone + Send + Sync + 'static,
) -> Element<'a, Message> {
    let id = id.into();
    let left = change((value - step).max(*range.start()));
    let right = change((value + step).min(*range.end()));
    let bounds = range.clone();
    let set = change.clone();
    semantic(
        Node::new(accessibility::Role::Slider)
            .key(id.clone())
            .range(accessibility::Range {
                value,
                min: *range.start(),
                max: *range.end(),
                step: Some(step),
            })
            .on_increment(Some(right.clone()))
            .on_decrement(Some(left.clone()))
            .on_set_value(move |requested| match requested {
                accessibility::Value::Number(number) if bounds.contains(&number) => {
                    Some(set(number))
                }
                accessibility::Value::Text(text) => text
                    .trim()
                    .trim_end_matches('%')
                    .parse::<f64>()
                    .ok()
                    .filter(|number| bounds.contains(number))
                    .map(&set),
                _ => None,
            }),
        focus::control(
            id,
            None,
            Some(left),
            Some(right),
            false,
            slider(range, value, change).step(step),
        ),
    )
}
/// A pick list, named by the text drawn before it. Enter and Right choose
/// the next option, Left the previous one.
fn choice<'a>(
    id: impl Into<String>,
    options: Vec<String>,
    selected: Option<String>,
    change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let id = id.into();
    let index = selected
        .as_ref()
        .and_then(|v| options.iter().position(|o| o == v))
        .unwrap_or(0);
    let left = options
        .get(if index == 0 {
            options.len().saturating_sub(1)
        } else {
            index - 1
        })
        .map(|v| change(v.clone()));
    let right = options
        .get(if options.is_empty() {
            0
        } else {
            (index + 1) % options.len()
        })
        .map(|v| change(v.clone()));
    // An assistive technology may name any listed option.
    let messages: Vec<(String, Message)> = options
        .iter()
        .map(|option| (option.clone(), change(option.clone())))
        .collect();
    semantic(
        Node::new(accessibility::Role::ComboBox)
            .key(id.clone())
            .value(selected.clone().unwrap_or_default())
            .description({
                let (position, count) = (index as u64 + 1, options.len() as u64);
                fl!(
                    "settings-choice-position",
                    position = position,
                    count = count
                )
            })
            .on_click(right.clone())
            .on_increment(right.clone())
            .on_decrement(left.clone())
            .on_set_value(move |requested| match requested {
                accessibility::Value::Text(text) => messages
                    .iter()
                    .find(|(option, _)| *option == text)
                    .map(|(_, message)| message.clone()),
                accessibility::Value::Number(_) => None,
            }),
        focus::control(
            id,
            right.clone(),
            left,
            right,
            false,
            pick_list(options, selected, change),
        ),
    )
}
/// A pick list of values shown by their labels; messages carry the value,
/// never the (translated) label. Labels must be distinct.
fn pick<'a, V: Clone + PartialEq + 'a>(
    id: impl Into<String>,
    options: Vec<(V, String)>,
    selected: Option<V>,
    change: impl Fn(V) -> Message + 'a,
) -> Element<'a, Message> {
    let labels: Vec<String> = options.iter().map(|(_, label)| label.clone()).collect();
    let selected = selected.and_then(|selected| {
        options
            .iter()
            .find(|(value, _)| *value == selected)
            .map(|(_, label)| label.clone())
    });
    choice(id, labels, selected, move |label| {
        let value = options
            .iter()
            .find(|(_, known)| *known == label)
            .or(options.first())
            .map(|(value, _)| value.clone())
            .expect("a pick list chooses one of its options");
        change(value)
    })
}
fn mode_label(mode: &niri_ipc::Mode) -> String {
    // Pixel counts are passed as text: no digit grouping.
    fl!(
        "settings-display-mode-option",
        width = mode.width.to_string(),
        height = mode.height.to_string(),
        refresh = canopa_i18n::format::decimal(f64::from(mode.refresh_rate) / 1000., 2, true)
    )
}
/// A rotation's name in the selected language.
fn transform_label(transform: Transform) -> String {
    match transform {
        Transform::Normal => fl!("settings-rotation-normal"),
        Transform::_90 => fl!("settings-rotation-90"),
        Transform::_180 => fl!("settings-rotation-180"),
        Transform::_270 => fl!("settings-rotation-270"),
        Transform::Flipped => fl!("settings-rotation-flipped"),
        Transform::Flipped90 => fl!("settings-rotation-flipped-90"),
        Transform::Flipped180 => fl!("settings-rotation-flipped-180"),
        Transform::Flipped270 => fl!("settings-rotation-flipped-270"),
    }
}
fn transforms() -> [Transform; 8] {
    [
        Transform::Normal,
        Transform::_90,
        Transform::_180,
        Transform::_270,
        Transform::Flipped,
        Transform::Flipped90,
        Transform::Flipped180,
        Transform::Flipped270,
    ]
}
/// Run a file-backed page's save job and report it as `Message::Stored`.
fn file_save(page: Page, job: impl FnOnce() -> Result<Snapshot> + Send + 'static) -> Task<Message> {
    task(move || Ok((page, job()?)), Message::Stored)
}
fn idle_events() -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};
    iced::stream::channel(
        4,
        |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
            loop {
                let watch = async {
                    let mut child = tokio::process::Command::new(companion("canopa-idle"))
                        .arg("watch")
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .kill_on_drop(true)
                        .spawn()?;
                    let mut reader = tokio::io::BufReader::new(
                        child.stdout.take().context("missing idle status stream")?,
                    );
                    loop {
                        let mut bytes = Vec::new();
                        let size = (&mut reader)
                            .take(16385)
                            .read_until(b'\n', &mut bytes)
                            .await?;
                        ensure!(
                            size > 0 && size <= 16384,
                            "Stasis is not running or its status stream ended"
                        );
                        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
                        ensure!(
                            value["state"].is_string() && value["manually_paused"].is_boolean(),
                            "unsupported Stasis status reply"
                        );
                        if sender
                            .send(Message::IdleStatus(Ok(String::from_utf8(bytes)?)))
                            .await
                            .is_err()
                        {
                            return Ok::<(), anyhow::Error>(());
                        }
                    }
                }
                .await;
                match watch {
                    Ok(()) => break,
                    Err(error) => {
                        if sender
                            .send(Message::IdleStatus(Err(error.to_string())))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        },
    )
}
fn niri_events() -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
    iced::stream::channel(
        4,
        |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
            loop {
                let watch = async {
                    let path = std::env::var_os("NIRI_SOCKET").context("Niri unavailable")?;
                    let mut stream =
                        tokio::net::UnixStream::connect(std::path::PathBuf::from(path)).await?;
                    let mut request = serde_json::to_vec(&niri_ipc::Request::EventStream)?;
                    request.push(b'\n');
                    stream.write_all(&request).await?;
                    let mut reader = tokio::io::BufReader::new(stream);
                    loop {
                        let mut bytes = Vec::new();
                        let size = (&mut reader)
                            .take(1024 * 1024 + 1)
                            .read_until(b'\n', &mut bytes)
                            .await?;
                        ensure!(
                            size > 0 && size <= 1024 * 1024,
                            "compositor event stream ended"
                        );
                        if matches!(
                            serde_json::from_slice::<niri_ipc::Event>(&bytes),
                            Ok(niri_ipc::Event::WorkspacesChanged { .. })
                        ) && sender.send(Message::BackendChanged).await.is_err()
                        {
                            return Ok::<(), anyhow::Error>(());
                        }
                    }
                }
                .await;
                if watch.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        },
    )
}
/// Requests for the running window from later launches and D-Bus callers.
#[derive(Clone)]
struct Activations(Arc<Mutex<Option<UnboundedReceiver<Request>>>>);

impl std::hash::Hash for Activations {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

fn activations(source: &Activations) -> futures_util::stream::BoxStream<'static, Message> {
    use futures_util::StreamExt;
    let receiver = source
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    match receiver {
        Some(receiver) => futures_util::stream::unfold(receiver, |mut receiver| async move {
            let request = receiver.recv().await?;
            Some((Message::Activated(request), receiver))
        })
        .boxed(),
        None => futures_util::stream::pending().boxed(),
    }
}

/// Bring the window forward for a deep link. Niri focuses it through IPC;
/// elsewhere the toolkit asks for focus and attention.
fn raise() -> Task<Message> {
    task(
        || Client::discover()?.focus_own_window(APPLICATION_ID),
        |result| Message::Raised(result.is_ok()),
    )
}

/// Run the window at `request`; with `instance`, also serve later requests.
pub fn run(request: Request, instance: Option<Instance>) -> Result<()> {
    let activations =
        instance.map(|instance| Activations(Arc::new(Mutex::new(Some(instance.requests)))));
    iced::application(
        move || {
            let (mut state, task) = State::boot();
            state.activations = activations.clone();
            let mut bridge = accessibility::Bridge::new("canopa-settings");
            bridge.open(ACCESSIBILITY_ROOT, &fl!("settings-window-title"), None);
            // A new ordinary window normally has focus; compositor events correct it.
            bridge.set_window_focused(ACCESSIBILITY_ROOT, true);
            state.accessibility = Some(bridge);
            let page = request.page.unwrap_or(Page::Displays);
            state
                .display_profiles
                .selected
                .clone_from(&request.display_profile);
            state.display_profiles.prominent = request.display_profile.is_some();
            let page_task = state.update(Message::Page(page));
            (state, Task::batch([task, page_task]))
        },
        State::update,
        State::view,
    )
    .title(|_: &State| fl!("settings-window-title"))
    .theme(|state: &State| canopa_ui::theme(state.personal.value.values.appearance.clone()))
    .scale_factor(|state: &State| {
        f32::from(state.personal.value.values.appearance.text_scale()) / 100.0
    })
    .subscription(State::subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(1000., 780.),
        min_size: Some(iced::Size::new(760., 600.)),
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: APPLICATION_ID.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .run()
    .map_err(anyhow::Error::msg)
}

#[cfg(all(test, debug_assertions))]
mod accessibility_tests;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget_layouts::Editor as Widgets;

    pub(super) fn fresh() -> (tempfile::TempDir, State) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: root.path().join("canopa"),
        };
        let defaults = root.path().join("niri-defaults.kdl");
        std::fs::write(
            &defaults,
            include_str!("../../../packaging/niri/defaults.kdl"),
        )
        .unwrap();
        paths.initialize(Some(&defaults)).unwrap();
        std::fs::write(paths.common(), "schema-version 1\n// keep this note\n").unwrap();
        let (state, _) = State::boot_with(paths);
        (root, state)
    }

    fn open_widgets(state: &mut State) {
        let source = Snapshot::load(state.paths.common()).unwrap();
        state.widget_layouts.editor = Some(Ok(Widgets::load(source).unwrap()));
        let _ = state.update(Message::WidgetLayouts(widget_layouts::Action::Edge(
            "bottom".into(),
        )));
        assert!(state.widget_layouts.dirty());
    }

    /// Save Personalization as Settings does, without the owner's validator.
    fn save_personal(state: &mut State) -> Snapshot {
        let source = state.personal.source().cloned().unwrap();
        state.personal.value.accent = "#123456".into();
        let mut values = state.personal.value.values.clone();
        values.appearance.accent = Some(canopa_theme::Rgb(0x12, 0x34, 0x56));
        let saved = source
            .save(&values.text(&source).unwrap(), |_| Ok(()))
            .unwrap();
        let _ = state.update(Message::Stored(Ok((Page::Personal, saved.clone()))));
        saved
    }

    fn widget_editor(state: &State) -> &Widgets {
        state
            .widget_layouts
            .editor
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
    }

    /// Save Widgets as Settings does, without the owner's validator.
    fn save_widgets(state: &mut State) -> Snapshot {
        let editor = widget_editor(state);
        let saved = editor
            .source
            .save(&editor.candidate().unwrap(), |_| Ok(()))
            .unwrap();
        let reloaded = Widgets::load(saved.clone()).map(Box::new);
        let _ = state.update(Message::WidgetLayouts(widget_layouts::Action::Stored(
            reloaded.map_err(|error| error.to_string()),
        )));
        saved
    }

    #[test]
    fn reload_discards_only_the_current_page() {
        let (_root, mut state) = fresh();
        let _ = state.update(Message::Page(Page::Input));
        let _ = state.update(Message::InputText(0, "fr".into()));
        let _ = state.update(Message::PersonalText(0, "#123456".into()));
        let _ = state.update(Message::ReloadSaved);
        assert_eq!(state.input.value.layouts, Input::default().layouts);
        assert_eq!(state.personal.value.accent, "#123456");

        let _ = state.update(Message::InputText(0, "de".into()));
        let _ = state.update(Message::Page(Page::Personal));
        let _ = state.update(Message::ReloadSaved);
        assert_eq!(state.personal.value.accent, "");
        assert_eq!(state.input.value.layouts, "de");
    }

    #[test]
    fn accessibility_page_edits_the_personalization_draft_and_saves_high_contrast() {
        let (root, mut state) = fresh();
        let _ = state.update(Message::Page(Page::Accessibility));
        assert_eq!(state.page, Page::Accessibility);
        let _ = state.update(Message::HighContrast(true));
        let _ = state.update(Message::TextScale(125.0));
        assert!(state.personal.value.values.appearance.high_contrast());
        // Personalization shows the same draft.
        let _ = state.update(Message::Page(Page::Personal));
        assert!(state.personal.value.values.high_contrast);
        let source = state.personal.source().cloned().unwrap();
        let text = state.personal.value.values.text(&source).unwrap();
        assert!(text.contains("high-contrast #true"), "{text}");
        assert!(text.contains("text-scale 125"), "{text}");
        let written = root.path().join("written.kdl");
        std::fs::write(&written, &text).unwrap();
        let (lock, _) = canopa_session_ui::settings::Settings::load(
            &canopa_config::Request::common(&written),
            canopa_config::Component::Lockscreen,
        )
        .unwrap();
        assert!(lock.appearance.high_contrast());
        // Without a Canopa session the cursor size is not applied.
        let _ = state.update(Message::Page(Page::Accessibility));
        let _ = state.update(Message::CursorSize(Some(48)));
        assert!(!state.busy);
        assert_eq!(state.input.value.cursor_size, None);
    }

    #[test]
    fn saving_personalization_keeps_pending_widget_edits() {
        let (_root, mut state) = fresh();
        open_widgets(&mut state);
        let saved = save_personal(&mut state);
        assert!(state.widget_layouts.dirty());
        let editor = widget_editor(&state);
        assert!(editor.source.same(&saved));
        let candidate = editor.candidate().unwrap();
        assert!(candidate.contains("#123456") && candidate.contains("bottom"));
        assert!(candidate.contains("// keep this note"));
        let after = save_widgets(&mut state);
        // Personalization adopts the Widgets save in turn.
        assert!(state.personal.source().unwrap().same(&after));
    }

    #[test]
    fn saving_widgets_keeps_pending_personalization_edits() {
        let (_root, mut state) = fresh();
        let _ = state.update(Message::PersonalText(0, "#abcdef".into()));
        open_widgets(&mut state);
        let saved = save_widgets(&mut state);
        let common = state.personal.source().cloned().unwrap();
        assert!(common.same(&saved));
        assert_eq!(state.personal.value.accent, "#abcdef");
        assert!(common.unchanged().is_ok());
        assert!(
            state
                .personal
                .value
                .values
                .text(&common)
                .unwrap()
                .contains("bottom")
        );
    }

    #[test]
    fn external_edits_are_still_detected_across_pages() {
        let (_root, mut state) = fresh();
        // Changed by hand after Personalization loaded, before Widgets loaded.
        std::fs::write(
            state.paths.common(),
            "schema-version 1\nappearance { accent \"#ff0000\"; }\n",
        )
        .unwrap();
        open_widgets(&mut state);
        let saved = save_widgets(&mut state);
        let common = state.personal.source().cloned().unwrap();
        assert!(!common.same(&saved));
        assert!(common.unchanged().is_err());

        // Nor does a Personalization save rebase widget edits made on that version.
        let (_root, mut state) = fresh();
        let previous = state.personal.source().cloned().unwrap();
        std::fs::write(state.paths.common(), "schema-version 1\n// external\n").unwrap();
        open_widgets(&mut state);
        let saved = previous
            .save(
                &state.personal.value.values.text(&previous).unwrap(),
                |_| Ok(()),
            )
            .unwrap_err();
        assert!(saved.to_string().contains("outside this editor"));
        let widgets = state
            .widget_layouts
            .editor
            .as_mut()
            .unwrap()
            .as_mut()
            .unwrap();
        let current = widgets.source.clone();
        assert!(widgets.rebase(&previous, &current).is_err());
    }
}
