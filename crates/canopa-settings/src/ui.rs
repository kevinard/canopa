//! Ordinary settings window with shared controls and bounded background work.
mod bluetooth;
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
mod profiles;
mod shortcuts;
mod sound;
mod widget_layouts;
use crate::{
    display::{Monitor, TrialHandle, transform_name},
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
/// The footer action reloads only the selected page; other drafts remain.
const RELOAD_PAGE: &str = "Reload this page and discard its edits";
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
    Mode(usize, String),
    Scale(usize, f64),
    Rotation(usize, String),
    Position(usize, bool, String),
    InputText(u8, String),
    InputNumber(u8, f64),
    InputFlag(u8, bool),
    PersonalText(u8, String),
    Light(bool),
    TextScale(f64),
    ReducedMotion(bool),
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
}
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
        label: &'a str,
        message: Option<Message>,
        active: bool,
    ) -> Element<'a, Message> {
        button(
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
            input_level: None,
            alert_sounds: None,
            profile: "none".into(),
            idle_status: "Connecting to the idle manager…".into(),
            idle_available: false,
            trial: None,
            remaining: 0,
            #[cfg(debug_assertions)]
            live_example: live_page::LivePage::new(Default::default()),
            activations: None,
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
    fn update(&mut self, message: Message) -> Task<Message> {
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
                        self.notice = "Launcher preferences saved.".into();
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
                        self.clipboard_notice = "Clipboard settings saved.".into();
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
                        self.notice = "Keyboard shortcuts saved.".into();
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
                            self.notice="Displays changed while you were editing. Reload connected displays to update the draft.".into();
                        }
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::Enabled(index, value) => {
                self.monitors[index].enabled = value;
                self.display_dirty = true;
            }
            Message::Mode(index, label) => {
                self.monitors[index].mode = self.monitors[index]
                    .modes
                    .iter()
                    .position(|mode| mode_label(mode) == label);
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
            Message::Rotation(index, name) => {
                self.monitors[index].transform = transforms()
                    .into_iter()
                    .find(|t| transform_name(*t) == name)
                    .unwrap_or(Transform::Normal);
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
            Message::CompositorMotion(enabled) if !self.busy && self.trial.is_none() => {
                if matches!(&self.shortcuts, Some(Ok(editor)) if editor.dirty()) {
                    self.notice =
                        "Save or discard your shortcut edits before changing compositor motion."
                            .into();
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
                        self.notice = "Compositor motion settings saved.".into();
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
                        Ok("Idle policy updated.".into())
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
                        Ok("Idle profile selected.".into())
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
                            format!(
                                "State: {}    Profile: {}    Paused manually: {}",
                                info["state"].as_str().unwrap_or("available"),
                                info["profile"].as_str().unwrap_or("default"),
                                if info["manually_paused"].as_bool().unwrap_or(false) {
                                    "yes"
                                } else {
                                    "no"
                                }
                            )
                        })
                        .unwrap_or(value),
                    Err(error) => {
                        self.idle_available = false;
                        format!("Idle manager unavailable: {error}")
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
                    | Page::DefaultApps => self.busy = false,
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
                    Page::Personal => {
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
                        self.notice = "Settings saved.".into();
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
                        receive.await.unwrap_or_else(|_| {
                            Err("Wallpaper launcher stopped unexpectedly.".into())
                        })
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
                        self.notice = "Notification sounds updated.".into();
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
                    self.notice =
                        "Save or reload your Keyboard & pointer edits before changing the power button."
                            .into();
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
                        self.notice = "Power button setting applied.".into();
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
                                    "Settings saved.".into()
                                } else {
                                    "Settings saved. No idle daemon is running; Stasis uses these timeouts when it starts.".into()
                                }
                            }
                            Ok(None) => "Settings saved.".into(),
                            Err(error) => format!(
                                "The lid choice was saved, but the idle timeouts were not: {error}"
                            ),
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
            self.notice = format!("Finish the current change, then open {}.", page.title());
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
            Page::Personal => {
                self.notice = self.personal.reload(&paths).unwrap_or_default();
                self.compositor_motion = Self::load_motion(&paths);
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
        if self.trial.is_some() {
            subscriptions.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick));
        }
        if let Some(source) = &self.activations {
            subscriptions.push(Subscription::run_with(source.clone(), activations));
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
        let title = column![
            heading(entry.title, 28),
            text(entry.description).size(14).style(canopa_ui::muted)
        ]
        .spacing(5);
        // Keep this child present so asynchronous notices do not replace the
        // page's widget tree and discard keyboard focus or typed drafts.
        let mut body = column![
            container(text(&self.notice).size(14))
                .padding(if self.notice.is_empty() { 0 } else { 12 })
                .height(if self.notice.is_empty() {
                    // Iced filters zero-height children out of a Column.
                    Length::Fixed(1.)
                } else {
                    Length::Shrink
                })
                .style(canopa_ui::surface)
        ]
        .spacing(16);
        if self.trial.is_some() {
            body = body.push(
                container(
                    column![
                        text(format!(
                            "Keep these display settings? Reverting in {} seconds.",
                            self.remaining
                        ))
                        .size(20),
                        text("Ctrl+Enter keeps changes. Escape reverts."),
                        row![
                            self.action("Revert", Some(Message::Decision(false)), false),
                            self.action("Keep changes", Some(Message::Decision(true)), true)
                        ]
                        .spacing(12)
                    ]
                    .spacing(12),
                )
                .padding(16)
                .style(canopa_ui::surface),
            );
        }
        if matches!(self.page, Page::Displays | Page::Input) && !self.paths.active() {
            body = body.push(settings_group(
                "Changes require a Canopa session",
                text("Sign in to the Canopa desktop to save display and input settings.")
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
            Page::Personal => self.personal_view(),
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
            canopa_ui::scroll_speed(
                scrollable(container(body.max_width(800).padding([14, 20])).width(Length::Fill))
                    .id("settings-page-scroll")
                    .height(Length::Fill)
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
        focus::group(
            row![navigation, rule::vertical(1).style(settings_rule), content],
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
                "settings-button-Revert".into()
            } else {
                navigation::link_id(entry)
            },
            self.busy,
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
        Some(
            row![
                space().width(Length::Fill),
                self.action(
                    RELOAD_PAGE,
                    (!self.busy && self.trial.is_none()).then_some(Message::ReloadSaved),
                    false
                ),
                button(
                    if self.busy {
                        "Applying…"
                    } else if apply == Apply::Trial {
                        "Test display changes"
                    } else {
                        "Save changes"
                    },
                    self.can_save().then_some(Message::Save),
                    canopa_ui::Action::Primary,
                    Length::Shrink,
                    self.busy,
                )
            ]
            .align_y(Alignment::Center)
            .spacing(12)
            .padding([10, 20])
            .into(),
        )
    }
    fn displays(&self) -> Element<'_, Message> {
        let mut body = column![
            text("Choose a mode, scale and position for each connected display."),
            self.action(
                if self.refreshing {
                    "Refreshing…"
                } else {
                    "Reload connected displays"
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
            body = body.push(text("No connected displays are available."));
        }
        for (index, monitor) in self.monitors.iter().enumerate() {
            let modes: Vec<_> = monitor.modes.iter().map(mode_label).collect();
            let selected = monitor.mode.and_then(|m| modes.get(m)).cloned();
            let card = column![
                text(format!(
                    "{} · {} {}",
                    monitor.name, monitor.make, monitor.model
                ))
                .size(20),
                toggle(
                    format!("display-enabled-{index}"),
                    "Enabled",
                    monitor.enabled,
                    Message::Enabled(index, !monitor.enabled)
                ),
                row![
                    column![
                        text("Resolution & refresh rate"),
                        choice(format!("display-mode-{index}"), modes, selected, move |v| {
                            Message::Mode(index, v)
                        })
                    ]
                    .spacing(6),
                    column![
                        text("Rotation"),
                        choice(
                            format!("display-rotation-{index}"),
                            transforms()
                                .into_iter()
                                .map(|v| transform_name(v).to_owned())
                                .collect::<Vec<_>>(),
                            Some(transform_name(monitor.transform).to_owned()),
                            move |v| Message::Rotation(index, v)
                        )
                    ]
                    .spacing(6)
                ]
                .spacing(18),
                text(format!("Scale: {:.0}%", monitor.scale * 100.)),
                adjust(
                    format!("display-scale-{index}"),
                    0.5..=4.,
                    monitor.scale,
                    0.25,
                    move |v| Message::Scale(index, v)
                ),
                row![
                    field_id(
                        format!("display-x-{}", monitor.name),
                        "Horizontal position",
                        &self.positions[index].0,
                        move |v| Message::Position(index, true, v)
                    ),
                    field_id(
                        format!("display-y-{}", monitor.name),
                        "Vertical position",
                        &self.positions[index].1,
                        move |v| Message::Position(index, false, v)
                    )
                ]
                .spacing(18),
                text("Positions use logical pixels. Keep at least one display enabled.")
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
                "Layouts, in switch order (for example us,fr)",
                &self.input.value.layouts,
                |v| Message::InputText(0, v)
            ),
            field(
                "Variants (optional, in the same order)",
                &self.input.value.variants,
                |v| Message::InputText(1, v)
            ),
            field(
                "XKB options (for example grp:alt_shift_toggle)",
                &self.input.value.options,
                |v| Message::InputText(2, v)
            ),
            text(format!(
                "Repeat delay: {} ms",
                self.input.value.repeat_delay
            )),
            adjust(
                "repeat-delay",
                100.0..=2000.,
                f64::from(self.input.value.repeat_delay),
                50.,
                |v| Message::InputNumber(0, v)
            ),
            text(format!(
                "Repeat rate: {} keys/second",
                self.input.value.repeat_rate
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
            text(format!(
                "Pointer speed: {:.2}",
                self.input.value.mouse_speed
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
                "Left-handed buttons",
                self.input.value.mouse_left_handed,
                Message::InputFlag(0, !self.input.value.mouse_left_handed)
            ),
            toggle(
                "input-flag-1",
                "Natural scrolling for mouse",
                self.input.value.mouse_natural_scroll,
                Message::InputFlag(1, !self.input.value.mouse_natural_scroll)
            ),
        ]
        .spacing(10);
        let touchpad = column![
            text(format!(
                "Touchpad speed: {:.2}",
                self.input.value.touchpad_speed
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
                "Tap to click",
                self.input.value.touchpad_tap,
                Message::InputFlag(2, !self.input.value.touchpad_tap)
            ),
            toggle(
                "input-flag-3",
                "Natural scrolling for touchpad",
                self.input.value.touchpad_natural_scroll,
                Message::InputFlag(3, !self.input.value.touchpad_natural_scroll)
            ),
            toggle(
                "input-flag-4",
                "Disable touchpad while typing",
                self.input.value.touchpad_disable_while_typing,
                Message::InputFlag(4, !self.input.value.touchpad_disable_while_typing)
            )
        ]
        .spacing(10);
        column![
            settings_group("Keyboard", keyboard),
            settings_group("Mouse", mouse),
            settings_group("Touchpad", touchpad)
        ]
        .spacing(14)
        .into()
    }
    fn personal_view(&self) -> Element<'_, Message> {
        let appearance = column![
            toggle(
                "light",
                "Light appearance",
                self.personal.value.values.appearance.color_scheme == canopa_theme::ColorScheme::Light,
                Message::Light(
                    self.personal.value.values.appearance.color_scheme != canopa_theme::ColorScheme::Light
                )
            ),
            container(field("Accent color (optional, #RRGGBB)", &self.personal.value.accent, |v| {
                Message::PersonalText(0, v)
            })).max_width(300),
            text(format!("Text size · {}%", self.personal.value.values.text_scale)),
            adjust(
                "settings-text-scale",
                100.0..=150.0,
                f64::from(self.personal.value.values.text_scale),
                5.0,
                Message::TextScale
            ),
            toggle(
                "settings-reduced-motion",
                "Reduce Shell panel motion",
                self.personal.value.values.reduced_motion,
                Message::ReducedMotion(!self.personal.value.values.reduced_motion)
            ),
            self.action("Choose wallpaper…", Some(Message::Wallpaper), true),
            text("Preview and save wallpaper for each monitor. The default image folder is ~/Pictures/Wallpapers.")
                .size(14).style(canopa_ui::muted),
        ].spacing(12);
        let lockscreen = column![
            field(
                "Clock format",
                &self.personal.value.values.lock.clock_format,
                |v| { Message::PersonalText(1, v) }
            ),
            field(
                "Date format",
                &self.personal.value.values.lock.date_format,
                |v| { Message::PersonalText(2, v) }
            ),
            field("Font family", &self.personal.value.values.lock.font, |v| {
                Message::PersonalText(3, v)
            }),
            field(
                "Avatar image (optional absolute path)",
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
            text(format!(
                "Background dimming: {}%",
                self.personal.value.values.lock.dim
            )),
            adjust(
                "lock-dim",
                0.0..=100.,
                f64::from(self.personal.value.values.lock.dim),
                1.,
                Message::Dim
            ),
            self.action(
                "Edit lock screen widgets…",
                Some(Message::OpenLockWidgetEditor),
                false
            )
        ]
        .spacing(12);
        let motion = if let Some(reduced) = self.compositor_motion {
            self.action(
                if reduced {
                    "Use Niri animation settings"
                } else {
                    "Reduce window and workspace motion"
                },
                Some(Message::CompositorMotion(!reduced)),
                false,
            )
        } else {
            text("Compositor motion is editable in a Canopa session with a managed Niri configuration.")
                .size(14).style(canopa_ui::muted).into()
        };
        column![
            settings_group("Appearance", appearance),
            settings_group("Lock screen", lockscreen),
            settings_group("Window and workspace motion", motion),
        ]
        .spacing(14)
        .into()
    }
}
fn field<'a>(
    label: &'a str,
    value: &str,
    change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    field_id(format!("settings-field-{label}"), label, value, change)
}
fn field_id<'a>(
    id: impl Into<String>,
    label: &'a str,
    value: &str,
    change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        text(label),
        text_input("", value)
            .id(id.into())
            .on_input(change)
            .padding(10)
    ]
    .spacing(6)
    .width(Length::Fill)
    .into()
}
fn heading(label: &str, size: u32) -> iced::widget::Text<'_> {
    text(label).size(size).font(Font {
        weight: Weight::Semibold,
        ..Font::DEFAULT
    })
}

fn settings_group<'a>(
    label: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(column![heading(label, 18), content.into()].spacing(12))
        .padding(16)
        .width(Length::Fill)
        .style(canopa_ui::surface)
        .into()
}

fn settings_rule(theme: &Theme) -> rule::Style {
    rule::Style {
        color: canopa_ui::colors(theme).border,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

fn button<'a>(
    label: &'a str,
    message: Option<Message>,
    treatment: canopa_ui::Action,
    width: Length,
    preserve_disabled: bool,
) -> Element<'a, Message> {
    focus::control(
        if matches!(label, "Applying…" | "Test display changes" | "Save changes") {
            "settings-apply".to_owned()
        } else {
            format!("settings-button-{label}")
        },
        message.clone(),
        None,
        None,
        preserve_disabled,
        canopa_ui::action(text(label), message, treatment, false)
            .width(width)
            .padding([8, 12]),
    )
}
fn toggle<'a>(
    id: impl Into<String>,
    label: impl Into<String>,
    value: bool,
    message: Message,
) -> Element<'a, Message> {
    focus::control(
        id,
        Some(message.clone()),
        None,
        None,
        false,
        checkbox(value)
            .label(label.into())
            .on_toggle(move |_| message.clone()),
    )
}
fn adjust<'a>(
    id: impl Into<String>,
    range: std::ops::RangeInclusive<f64>,
    value: f64,
    step: f64,
    change: impl Fn(f64) -> Message + 'a,
) -> Element<'a, Message> {
    let left = change((value - step).max(*range.start()));
    let right = change((value + step).min(*range.end()));
    focus::control(
        id,
        None,
        Some(left),
        Some(right),
        false,
        slider(range, value, change).step(step),
    )
}
fn choice<'a>(
    id: impl Into<String>,
    options: Vec<String>,
    selected: Option<String>,
    change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
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
    focus::control(
        id,
        right.clone(),
        left,
        right,
        false,
        pick_list(options, selected, change),
    )
}
fn mode_label(mode: &niri_ipc::Mode) -> String {
    format!(
        "{} × {} · {:.2} Hz",
        mode.width,
        mode.height,
        f64::from(mode.refresh_rate) / 1000.
    )
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
    .title("Canopa Settings")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget_layouts::Editor as Widgets;

    fn fresh() -> (tempfile::TempDir, State) {
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
