//! Ordinary settings window with shared controls and bounded background work.
mod about;
mod arrangement;
mod bluetooth;
mod date_time;
mod default_apps;
mod focus;
mod inhibit;
mod kit;
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
use arrangement::Screen;
use canopa_ui::{
    accessibility::{self, Node, semantic},
    direction,
};
use iced::{
    Alignment, Element, Length, Subscription, Task, Theme,
    font::Weight,
    widget::{
        column, container, pick_list, row, rule, scrollable, slider, space, text, text_input,
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
    /// A listed search engine by index into [`launcher::ENGINES`], or none
    /// to type an address.
    LauncherEngine(Option<usize>),
    LauncherSearch(usize, bool, String),
    LauncherRemoveSearch(usize),
    LauncherAddSearch,
    ShortcutsLoaded(Result<crate::shortcuts::Editor, String>),
    ShortcutsStored(Result<crate::shortcuts::Editor, String>),
    ShortcutSearch(String),
    ShortcutKey(usize, String),
    /// Record one shortcut's keys, or go back to showing them.
    EditShortcut(Option<usize>),
    /// Type the shortcut being edited instead of pressing it.
    TypeShortcut,
    /// A key pressed while a shortcut is recorded.
    ShortcutPressed(iced::keyboard::Key, iced::keyboard::Modifiers),
    /// The desktop's own shortcuts are held off while recording.
    ShortcutsInhibited(Option<inhibit::Held>),
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
    /// Copy the applied display layout to the login screen.
    LoginDisplays,
    DisplayProfiles(profiles::Action),
    WidgetLayouts(widget_layouts::Action),
    OpenLockWidgetEditor,
    /// Emitted by controls with nothing to choose; changes nothing.
    Noop,
    Enabled(MonitorSlot, bool),
    /// A display's mode, by its index in the display's mode list.
    Mode(MonitorSlot, usize),
    Scale(MonitorSlot, f64),
    Rotation(MonitorSlot, Transform),
    Position(MonitorSlot, bool, String),
    InputText(u8, String),
    InputNumber(u8, f64),
    InputFlag(u8, bool),
    PersonalText(u8, String),
    Light(bool),
    /// Show or hide a collapsible section, by its key.
    Expand(&'static str),
    /// Start the Orca screen reader, detached from Settings.
    StartScreenReader,
    /// The pointer moved over the display arrangement, in its own coordinates.
    ArrangementPointer(iced::Point),
    /// A display in the arrangement was grabbed.
    ArrangementGrab(MonitorSlot),
    /// The grabbed display was let go.
    ArrangementRelease,
    TextScale(f64),
    ReducedMotion(bool),
    HighContrast(bool),
    /// Apply a Niri cursor size at once (`None`: Niri's default).
    CursorSize(Option<u16>),
    CursorSizeStored(Result<Snapshot, String>),
    CompositorMotion(bool),
    CompositorMotionStored(Result<bool, String>),
    /// Apply a window layout over the shipped defaults at once (`None`:
    /// the shipped layout).
    WindowLayout(Option<crate::niri::WindowLayout>),
    WindowLayoutStored(Result<Option<crate::niri::WindowLayout>, String>),
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
    /// Choose a picture file for the wallpaper through the desktop portal.
    WallpaperFile,
    /// The picture chosen for the wallpaper, if any.
    WallpaperChosen(std::result::Result<Option<std::path::PathBuf>, String>),
    /// A later launch or a D-Bus caller asks for this window.
    Activated(Request),
    /// Whether the compositor focused the window.
    Raised(bool),
    /// The window gained or lost keyboard focus.
    WindowFocus(bool),
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
    /// Whether the displays are labelled while Displays arranges them; off
    /// where no window is shown, as in tests.
    identify: bool,
    /// The process labelling the displays, while it runs.
    identifier: Option<Identifier>,
    /// Whether the window has keyboard focus.
    window_focused: bool,
    paths: Paths,
    launcher: crate::launcher::Model,
    /// The default search is typed as an address rather than chosen.
    custom_search: bool,
    clipboard: Option<crate::launcher::ClipboardEditor>,
    clipboard_notice: String,
    shortcuts: Option<Result<crate::shortcuts::Editor, String>>,
    shortcut_query: String,
    /// The shortcut being recorded, or typed when `shortcut_typing`.
    shortcut_editing: Option<usize>,
    shortcut_typing: bool,
    /// The last key pressed while recording could not be a shortcut.
    shortcut_unsupported: bool,
    /// The desktop's shortcuts, held off while one is recorded.
    shortcut_inhibition: Option<inhibit::Held>,
    /// Collapsible sections the person opened, by key.
    expanded: std::collections::HashSet<&'static str>,
    compositor_motion: Option<bool>,
    /// `None` outside the Canopa session; an error when the block was edited.
    window_layout: Option<Result<Option<crate::niri::WindowLayout>, String>>,
    page: Page,
    search: String,
    notice: String,
    busy: bool,
    refreshing: bool,
    outputs: HashMap<String, Output>,
    /// What monitors the compositor calls "Unknown" report themselves, by
    /// connector.
    reported: HashMap<String, (String, String)>,
    monitors: Vec<Monitor>,
    positions: Vec<(String, String)>,
    /// The pointer over the display arrangement, and the display being dragged.
    arrangement_pointer: iced::Point,
    display_drag: Option<DisplayDrag>,
    /// Advanced whenever `monitors` is replaced, so an edit queued for the
    /// previous list (a click during hotplug) is dropped instead of applied
    /// to another display or out of range.
    display_generation: u64,
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
    /// The monitor connected at `connector`, named by [`monitor_title`] from
    /// what the compositor or, failing that, the monitor itself reports.
    fn monitor_name(&self, connector: &str) -> String {
        if let Some((make, model)) = self.reported.get(connector) {
            return monitor_title(make, model, connector);
        }
        let monitor = self
            .monitors
            .iter()
            .find(|monitor| monitor.name == connector);
        match (self.outputs.get(connector), monitor) {
            (Some(output), _) => monitor_title(&output.make, &output.model, connector),
            (None, Some(monitor)) => monitor_title(&monitor.make, &monitor.model, connector),
            (None, None) => connector.to_owned(),
        }
    }
    fn action<'a>(
        &self,
        key: impl Into<String>,
        label: impl Into<String>,
        message: Option<Message>,
        active: bool,
    ) -> Element<'a, Message> {
        // An active action without a message is the chosen one of a set.
        let treatment = if active && message.is_none() {
            kit::Treatment::Selected
        } else {
            kit::Treatment::Secondary
        };
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
                self.busy,
                styled_button(text(label), message, treatment).padding([7, 14]),
            ),
        )
    }

    /// A collapsible part of a card: a row that shows or hides `content`.
    fn disclosure<'a>(
        &self,
        key: &'static str,
        label: String,
        content: impl FnOnce() -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let open = self.expanded.contains(key);
        let toggle = Message::Expand(key);
        let header = semantic(
            Node::button(label.clone())
                .key(format!("settings-disclosure-{key}"))
                .expanded(open)
                .on_click(Some(toggle.clone())),
            focus::control(
                format!("settings-disclosure-{key}"),
                Some(toggle.clone()),
                None,
                None,
                false,
                styled_button(
                    direction::row([
                        text(label).width(Length::Fill).into(),
                        kit::glyph(
                            if open {
                                "chevron-down"
                            } else {
                                "chevron-right"
                            },
                            16.0,
                            kit::Tint::Muted,
                        ),
                    ])
                    .align_y(Alignment::Center),
                    Some(toggle),
                    kit::Treatment::Flat,
                )
                .width(Length::Fill)
                .padding([4, 0]),
            ),
        );
        if open {
            column![header, content()].spacing(12).into()
        } else {
            header
        }
    }

    /// A button in an explicit treatment: the one action a card leads with
    /// ([`kit::Treatment::Primary`]) or one that removes something.
    fn treated_action<'a>(
        &self,
        key: impl Into<String>,
        label: impl Into<String>,
        message: Option<Message>,
        treatment: kit::Treatment,
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
                self.busy,
                styled_button(text(label), message, treatment).padding([7, 14]),
            ),
        )
    }

    /// One segment of a [`kit::segmented`] control: chosen, or a quiet
    /// button that chooses it.
    fn segment<'a>(
        &self,
        key: impl Into<String>,
        label: impl Into<String>,
        message: Option<Message>,
        selected: bool,
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
                self.busy,
                styled_button(
                    text(label).size(kit::size::BODY),
                    message,
                    if selected {
                        kit::Treatment::Selected
                    } else {
                        kit::Treatment::Flat
                    },
                )
                .padding([6, 14]),
            ),
        )
    }

    /// The monitor `slot` names, unless the list changed since it was drawn.
    fn monitor_index(&self, slot: MonitorSlot) -> Option<usize> {
        (slot.generation == self.display_generation && slot.index < self.monitors.len())
            .then_some(slot.index)
    }

    fn monitor_mut(&mut self, slot: MonitorSlot) -> Option<&mut Monitor> {
        self.monitor_index(slot)
            .map(|index| &mut self.monitors[index])
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

    fn load_window_layout(
        paths: &Paths,
    ) -> Option<Result<Option<crate::niri::WindowLayout>, String>> {
        paths.active().then(|| {
            Snapshot::load(paths.niri())
                .and_then(|source| crate::niri::window_layout(&source))
                .map_err(|error| error.to_string())
        })
    }

    fn boot_with(paths: Paths) -> (Self, Task<Message>) {
        let (input, input_error) = FileDraft::load(&paths);
        let (personal, personal_error) = FileDraft::load(&paths);
        // Unreadable idle policy shows the defaults without a notice, as before.
        let (idle, _) = FileDraft::load(&paths);
        let (power_choices, _) = FileDraft::load(&paths);
        let compositor_motion = Self::load_motion(&paths);
        let window_layout = Self::load_window_layout(&paths);
        let notice = personal_error.or(input_error).unwrap_or_default();
        let profile_task = profiles::Editor::load(&paths);
        let state = Self {
            paths,
            launcher: Default::default(),
            custom_search: false,
            clipboard: None,
            clipboard_notice: String::new(),
            shortcuts: None,
            shortcut_query: String::new(),
            shortcut_editing: None,
            shortcut_typing: false,
            shortcut_unsupported: false,
            shortcut_inhibition: None,
            expanded: Default::default(),
            compositor_motion,
            window_layout,
            page: Page::Displays,
            search: String::new(),
            notice,
            busy: false,
            refreshing: true,
            identify: false,
            identifier: None,
            window_focused: true,
            outputs: HashMap::new(),
            reported: HashMap::new(),
            monitors: vec![],
            positions: vec![],
            arrangement_pointer: iced::Point::ORIGIN,
            display_drag: None,
            display_generation: 0,
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
        self.label_displays();
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
                    | Message::WallpaperFile
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
                        if self.clipboard.as_ref().is_some_and(|editor| editor.dirty()) {
                            return self.update(Message::ClipboardSave);
                        }
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
            Message::LauncherEngine(engine) if !self.busy => {
                self.custom_search = engine.is_none();
                if let Some((_, address)) = engine.and_then(|index| launcher::ENGINES.get(index)) {
                    self.launcher.draft.web_template = (*address).to_owned();
                }
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
                let changed = page != self.page;
                let opened = self.open_page(page);
                return if changed {
                    Task::batch([opened, navigation::reveal(page)])
                } else {
                    opened
                };
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
            Message::EditShortcut(editing) => {
                self.shortcut_editing = editing;
                self.shortcut_typing = false;
                self.shortcut_unsupported = false;
                // Dropping the held inhibition gives the desktop its keys back.
                self.shortcut_inhibition = None;
                if editing.is_some() {
                    return iced::window::latest()
                        .and_then(|id| iced::window::run(id, inhibit::inhibit))
                        .map(Message::ShortcutsInhibited);
                }
            }
            // Recording may have ended before the compositor answered.
            Message::ShortcutsInhibited(held)
                if self.shortcut_editing.is_some() && !self.shortcut_typing =>
            {
                self.shortcut_inhibition = held;
            }
            Message::ShortcutsInhibited(_) => {}
            Message::TypeShortcut => {
                self.shortcut_typing = true;
                self.shortcut_inhibition = None;
                if let Some(index) = self.shortcut_editing {
                    return iced::widget::operation::focus(format!("settings-shortcut-{index}"));
                }
            }
            Message::ShortcutPressed(key, modifiers) if !self.busy => {
                let Some(index) = self.shortcut_editing.filter(|_| !self.shortcut_typing) else {
                    return Task::none();
                };
                match shortcuts::press(&key, modifiers) {
                    shortcuts::Press::Modifier => {}
                    shortcuts::Press::Unsupported => self.shortcut_unsupported = true,
                    shortcuts::Press::Cancel => return self.update(Message::EditShortcut(None)),
                    shortcuts::Press::Clear => {
                        let _ = self.update(Message::ShortcutKey(index, String::new()));
                        return self.update(Message::EditShortcut(None));
                    }
                    shortcuts::Press::Combination(combination) => {
                        let _ = self.update(Message::ShortcutKey(index, combination));
                        return self.update(Message::EditShortcut(None));
                    }
                }
            }
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
                // Reload rereads the monitors and the saved profiles.
                let profiles = self.update_profiles(profiles::Action::Load);
                return Task::batch([
                    task(|| Client::discover()?.outputs(), Message::Outputs),
                    profiles,
                ]);
            }
            Message::LoginDisplays
                if !self.busy
                    && !self.refreshing
                    && !self.display_dirty
                    && self.trial.is_none() =>
            {
                self.busy = true;
                self.notice.clear();
                let outputs = self.outputs.clone();
                return task(
                    move || crate::display::use_for_login_screen(&outputs),
                    Message::Finished,
                );
            }
            Message::Outputs(result) => {
                self.refreshing = false;
                match result {
                    Ok(outputs) => {
                        self.outputs = outputs;
                        let unknown = |value: &str| {
                            value.trim().is_empty() || value.eq_ignore_ascii_case("unknown")
                        };
                        self.reported = self
                            .outputs
                            .values()
                            .filter(|output| unknown(&output.make) && unknown(&output.model))
                            .filter_map(|output| {
                                crate::display::reported_identity(&output.name)
                                    .map(|identity| (output.name.clone(), identity))
                            })
                            .collect();
                        if !self.display_dirty {
                            let mut names: Vec<_> = self.outputs.keys().cloned().collect();
                            names.sort();
                            self.display_generation += 1;
                            self.monitors = names
                                .iter()
                                .map(|name| Monitor::from_output(&self.outputs[name]))
                                .collect();
                            // Positions read from 0, 0, wherever the
                            // compositor put the arrangement's corner.
                            let enabled = || self.monitors.iter().filter(|m| m.enabled);
                            let left = enabled().map(|m| m.x).min().unwrap_or(0);
                            let top = enabled().map(|m| m.y).min().unwrap_or(0);
                            self.positions = self
                                .monitors
                                .iter()
                                .map(|m| ((m.x - left).to_string(), (m.y - top).to_string()))
                                .collect();
                        } else {
                            self.notice = fl!("settings-displays-changed-while-editing");
                        }
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::Enabled(slot, value) => {
                if let Some(monitor) = self.monitor_mut(slot) {
                    monitor.enabled = value;
                    self.display_dirty = true;
                }
            }
            Message::Mode(slot, mode) => {
                let outputs = &self.outputs;
                if let Some(index) = self.monitor_index(slot) {
                    let monitor = &mut self.monitors[index];
                    monitor.mode = (mode < monitor.modes.len()).then_some(mode);
                    monitor.custom_mode = outputs
                        .get(&monitor.name)
                        .is_some_and(|o| o.is_custom_mode && o.current_mode == monitor.mode);
                    self.display_dirty = true;
                }
            }
            Message::Scale(slot, scale) => {
                if let Some(monitor) = self.monitor_mut(slot) {
                    monitor.scale = scale;
                    self.display_dirty = true;
                }
            }
            Message::Rotation(slot, transform) => {
                if let Some(monitor) = self.monitor_mut(slot) {
                    monitor.transform = transform;
                    self.display_dirty = true;
                }
            }
            Message::Position(slot, horizontal, value) => {
                let Some(position) = self
                    .monitor_index(slot)
                    .and_then(|index| self.positions.get_mut(index))
                else {
                    return Task::none();
                };
                if horizontal {
                    position.0 = value;
                } else {
                    position.1 = value;
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
            Message::WindowLayout(layout) if !self.busy && self.trial.is_none() => {
                if matches!(&self.shortcuts, Some(Ok(editor)) if editor.dirty()) {
                    self.notice = fl!("settings-compositor-motion-unsaved-shortcuts");
                    return Task::none();
                }
                self.busy = true;
                self.notice.clear();
                let paths = self.paths.clone();
                return task(
                    move || {
                        crate::niri::set_window_layout(&paths, layout)?;
                        Ok(layout)
                    },
                    Message::WindowLayoutStored,
                );
            }
            Message::WindowLayoutStored(result) => {
                self.busy = false;
                match result {
                    Ok(layout) => {
                        self.window_layout = Some(Ok(layout));
                        self.notice = fl!("settings-window-layout-saved");
                        self.document_saved(Page::Personal, Document::Niri, None);
                    }
                    Err(error) => self.notice = error,
                }
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
                    // Launcher preferences first, then the Shell's clipboard
                    // options when they changed too (see LauncherStored).
                    Page::Launcher if self.launcher.draft != self.launcher.saved => {
                        let model = self.launcher.clone();
                        return task(move || model.save(), Message::LauncherStored);
                    }
                    Page::Launcher => {
                        self.busy = false;
                        return self.update(Message::ClipboardSave);
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
                        self.notice = saved
                            .reload_problem
                            .clone()
                            .unwrap_or_else(|| fl!("settings-saved"));
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
            Message::ArrangementPointer(point) => {
                self.arrangement_pointer = point;
                if let Some(drag) = self.display_drag.as_mut()
                    && !self.busy
                    && self.trial.is_none()
                {
                    drag.moved = true;
                    let drag = *drag;
                    // Where the grabbed display is, its edges pulled onto
                    // its neighbours' when they are close in the drawing.
                    let others: Vec<_> = self
                        .arranged()
                        .into_iter()
                        .filter(|(index, _)| *index != drag.slot.index)
                        .map(|(_, screen)| screen)
                        .collect();
                    let placed = arrangement::snap(
                        Screen {
                            x: (point.x - drag.grab.x - drag.x) / drag.fit,
                            y: (point.y - drag.grab.y - drag.y) / drag.fit,
                            width: drag.width,
                            height: drag.height,
                        },
                        &others,
                        arrangement::REACH / drag.fit,
                    );
                    if let Some(position) = self
                        .monitor_index(drag.slot)
                        .and_then(|index| self.positions.get_mut(index))
                    {
                        *position = (
                            (placed.x.round() as i32).to_string(),
                            (placed.y.round() as i32).to_string(),
                        );
                        self.display_dirty = true;
                    }
                }
            }
            Message::ArrangementGrab(slot) if !self.busy && self.trial.is_none() => {
                if let Some(geometry) = self.arrangement_geometry()
                    && let Some((_, screen)) = self
                        .arranged()
                        .into_iter()
                        .find(|(index, _)| *index == slot.index)
                {
                    self.display_drag = Some(DisplayDrag {
                        slot,
                        grab: iced::Point::new(
                            self.arrangement_pointer.x - (geometry.x + screen.x * geometry.fit),
                            self.arrangement_pointer.y - (geometry.y + screen.y * geometry.fit),
                        ),
                        x: geometry.x,
                        y: geometry.y,
                        fit: geometry.fit,
                        width: screen.width,
                        height: screen.height,
                        moved: false,
                    });
                }
            }
            Message::ArrangementRelease => {
                // A dropped display settles touching another, overlapping
                // none, and the arrangement starts at 0, 0.
                if let Some(drag) = self.display_drag.take()
                    && drag.moved
                    && !self.busy
                    && self.trial.is_none()
                    && self.monitor_index(drag.slot).is_some()
                {
                    let placed = self.arranged();
                    if let Some(dropped) = placed
                        .iter()
                        .position(|(index, _)| *index == drag.slot.index)
                    {
                        let mut screens: Vec<_> =
                            placed.iter().map(|(_, screen)| *screen).collect();
                        arrangement::tidy(&mut screens, dropped);
                        for ((index, _), screen) in placed.iter().zip(screens) {
                            if let Some(position) = self.positions.get_mut(*index) {
                                *position =
                                    ((screen.x as i32).to_string(), (screen.y as i32).to_string());
                            }
                        }
                        self.display_dirty = true;
                    }
                }
            }
            Message::StartScreenReader => {
                self.notice = match orca().map(|path| {
                    std::process::Command::new(path)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                }) {
                    Some(Ok(_)) => fl!("settings-a11y-orca-started"),
                    Some(Err(error)) => error.to_string(),
                    None => fl!("settings-a11y-orca-missing"),
                };
            }
            Message::Expand(key) if !self.expanded.remove(key) => {
                self.expanded.insert(key);
            }
            Message::Expand(_) => {}
            Message::Wallpaper => return wallpaper_picker(Vec::new()),
            Message::WallpaperFile => {
                self.notice.clear();
                let title = fl!("settings-personal-wallpaper-file-title");
                let filter = fl!("settings-personal-wallpaper-file-filter");
                return Task::perform(
                    async move {
                        crate::network::choose_picture(&title, &filter)
                            .await
                            .map_err(|error| format!("{error:#}"))
                    },
                    Message::WallpaperChosen,
                );
            }
            Message::WallpaperChosen(Ok(Some(path))) => {
                // The picture first, then its folder to browse around it.
                let mut images = vec![path.to_string_lossy().into_owned()];
                images.extend(
                    path.parent()
                        .map(|folder| folder.to_string_lossy().into_owned()),
                );
                return wallpaper_picker(images);
            }
            Message::WallpaperChosen(Ok(None)) => {}
            Message::WallpaperChosen(Err(error)) => self.notice = error,
            Message::WallpaperFinished(Err(error)) => {
                self.notice = error;
            }
            Message::Finished(result) => {
                self.busy = false;
                self.notice = result.unwrap_or_else(|error| error);
            }
            Message::Activated(request) => return self.activate(request),
            Message::WindowFocus(focused) => self.window_focused = focused,
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
            Message::Region(event) => return self.region_event(event),
            Message::Users(event) => {
                let task = self.users.update(event, users::message);
                return Task::batch([task, self.load_user_pictures()]);
            }
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
                    self.window_layout = Self::load_window_layout(&self.paths);
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
                self.window_layout = Self::load_window_layout(&paths);
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
        if self.page == Page::Displays {
            subscriptions.push(iced::event::listen_with(|event, _, _| match event {
                iced::Event::Window(iced::window::Event::Focused) => {
                    Some(Message::WindowFocus(true))
                }
                iced::Event::Window(iced::window::Event::Unfocused) => {
                    Some(Message::WindowFocus(false))
                }
                _ => None,
            }));
        }
        if self.page == Page::Shortcuts && self.shortcut_editing.is_some() && !self.shortcut_typing
        {
            subscriptions.push(iced::event::listen_with(|event, _, _| match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    repeat: false,
                    ..
                }) => Some(Message::ShortcutPressed(key, modifiers)),
                _ => None,
            }));
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
    /// Read the selected page's documents again when it holds no edits, so
    /// changes made outside Settings show without a reload button.
    fn reread_clean_page(&mut self) {
        let paths = self.paths.clone();
        match self.page {
            Page::Input => {
                let _ = self.input.reload(&paths);
            }
            Page::Personal | Page::Accessibility => {
                let _ = self.personal.reload(&paths);
                if self.page == Page::Accessibility {
                    let _ = self.input.reload(&paths);
                }
                self.compositor_motion = Self::load_motion(&paths);
                self.window_layout = Self::load_window_layout(&paths);
            }
            Page::Power => {
                let _ = self.idle.reload(&paths);
                let _ = self.power_choices.reload(&paths);
            }
            _ => {}
        }
    }
    /// Show `page`, loading what it needs the first time.
    fn open_page(&mut self, page: Page) -> Task<Message> {
        if page != self.page {
            // Typed passwords do not outlive the page, and one page's
            // result or error is not shown on another.
            self.users_forms.clear_secrets();
            self.notice.clear();
        }
        let changed = page != self.page;
        if changed {
            self.shortcut_editing = None;
            self.shortcut_inhibition = None;
        }
        self.page = page;
        if changed && !self.page_dirty() {
            self.reread_clean_page();
        }
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
        Task::none()
    }
    fn view(&self) -> Element<'_, Message> {
        let navigation = self.navigation();
        let entry = self.page.entry();
        let page_title = entry.title();
        let title = column![
            semantic(
                Node::heading(page_title.clone(), 1).key("settings-page-title"),
                heading(page_title.clone(), kit::size::PAGE_TITLE)
                    .width(Length::Fill)
                    .align_x(direction::text_start())
            ),
            text(entry.description())
                .size(kit::size::BODY)
                .style(canopa_ui::muted)
                .width(Length::Fill)
                .align_x(direction::text_start())
        ]
        .align_x(direction::start())
        .spacing(6);
        // Keep this child present so asynchronous notices do not replace the
        // page's widget tree and discard keyboard focus or typed drafts.
        let title = column![
            title,
            semantic(
                Node::status(&self.notice).key("settings-notice"),
                container(text(&self.notice).size(kit::size::BODY))
                    .width(Length::Fill)
                    .padding(if self.notice.is_empty() { 0 } else { 12 })
                    .height(if self.notice.is_empty() {
                        // Iced filters zero-height children out of a Column.
                        Length::Fixed(1.)
                    } else {
                        Length::Shrink
                    })
                    .style(if self.notice.is_empty() {
                        |_: &Theme| container::Style::default()
                    } else {
                        kit::note
                    })
            )
        ]
        .spacing(if self.notice.is_empty() { 0 } else { 16 });
        let mut body = column![].spacing(24);
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
                .width(Length::Fill)
                .style(kit::note),
            ));
        }
        if matches!(self.page, Page::Displays | Page::Input) && !self.paths.active() {
            body = body.push(settings_group(
                fl!("settings-session-required-title"),
                text(fl!("settings-session-required"))
                    .size(13)
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
        let content = column![semantic(
            Node::new(accessibility::Role::Main)
                .label(page_title)
                .key("settings-page"),
            canopa_ui::scroll_speed(
                scrollable(
                    container(
                        column![title, body]
                            .spacing(24)
                            .max_width(kit::CONTENT_WIDTH)
                    )
                    .center_x(Length::Fill)
                    .padding(iced::Padding {
                        top: 30.0,
                        right: 36.0,
                        bottom: 36.0,
                        left: 36.0,
                    })
                )
                .id("settings-page-scroll")
                .style(canopa_ui::scrollbar)
                .height(Length::Fill)
            )
        ),]
        .width(Length::Fill);
        // Live pages apply each change at once and have no footer; file
        // pages show theirs only while they hold edits to save or discard.
        let content = match self.file_footer() {
            Some(footer) if self.page_dirty() || self.busy || self.trial.is_some() => {
                content.push(footer)
            }
            _ => content,
        };
        let window = focus::group(
            // The sidebar leads: on the right in right-to-left languages.
            direction::row([
                navigation,
                rule::vertical(1).style(kit::divider).into(),
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
            // Include the background in the layer diff for reused software buffers.
            container(window)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme| container::background(canopa_ui::colors(theme).background)),
        )
    }
    /// Whether the selected file-backed page can save its draft now.
    /// Whether the selected file-backed page holds edits it has not saved.
    fn page_dirty(&self) -> bool {
        match self.page {
            Page::Displays => self.display_dirty,
            Page::Input => self.input.dirty(),
            Page::Personal => self.personal.dirty(),
            Page::Accessibility => self.personal.dirty() || self.input.dirty(),
            Page::Power => self.power_choices.dirty() || self.idle_dirty(),
            Page::Launcher => {
                self.launcher.draft != self.launcher.saved
                    || self.clipboard.as_ref().is_some_and(|editor| editor.dirty())
            }
            Page::Shortcuts => matches!(&self.shortcuts, Some(Ok(editor)) if editor.dirty()),
            Page::Widgets => self.widget_layouts.dirty(),
            _ => false,
        }
    }
    fn can_save(&self) -> bool {
        self.page_dirty()
            && (self.page != Page::Launcher || self.launcher.loaded)
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
            container(
                // Actions sit at the trailing edge, the primary one last.
                direction::row([
                    {
                        let dirty = self.page_dirty();
                        let status = fl!("settings-footer-unsaved");
                        semantic(
                            Node::status(status.clone()).key("settings-footer-status"),
                            direction::row([
                                container(space().width(8).height(8))
                                    .style(move |theme: &Theme| container::Style {
                                        background: Some(
                                            if dirty {
                                                canopa_ui::colors(theme).accent
                                            } else {
                                                kit::hairline(theme)
                                            }
                                            .into(),
                                        ),
                                        border: iced::border::rounded(u32::MAX),
                                        ..Default::default()
                                    })
                                    .into(),
                                text(status)
                                    .size(kit::size::CAPTION)
                                    .style(if dirty {
                                        |theme: &Theme| text::Style {
                                            color: Some(canopa_ui::colors(theme).text),
                                        }
                                    } else {
                                        canopa_ui::muted
                                    })
                                    .into(),
                            ])
                            .spacing(8)
                            .align_y(Alignment::Center)
                            .width(Length::Fill),
                        )
                    },
                    // Reloads only the selected page; other drafts remain.
                    button(
                        "settings-reload-page",
                        fl!("settings-footer-reload"),
                        (!self.busy && self.trial.is_none()).then_some(Message::ReloadSaved),
                        canopa_ui::Action::Normal,
                        Length::Shrink,
                        self.busy,
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
                .spacing(10)
                .padding([12, 24]),
            )
            .width(Length::Fill)
            .style(|theme| container::Style {
                background: Some(kit::sidebar_background(theme).into()),
                border: iced::Border {
                    color: kit::hairline(theme),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            }),
        ))
    }
    /// The enabled displays in logical pixels, where the position fields put
    /// them, by monitor index.
    fn arranged(&self) -> Vec<(usize, Screen)> {
        self.monitors
            .iter()
            .enumerate()
            .filter(|(_, monitor)| monitor.enabled)
            .filter_map(|(index, monitor)| {
                let mode = monitor.mode.and_then(|mode| monitor.modes.get(mode))?;
                let scale = monitor.scale.max(0.1) as f32;
                let (mut width, mut height) = (
                    f32::from(mode.width) / scale,
                    f32::from(mode.height) / scale,
                );
                if matches!(
                    monitor.transform,
                    Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
                ) {
                    std::mem::swap(&mut width, &mut height);
                }
                let typed = self.positions.get(index);
                let x = typed
                    .and_then(|(x, _)| x.trim().parse::<f32>().ok())
                    .unwrap_or(monitor.x as f32);
                let y = typed
                    .and_then(|(_, y)| y.trim().parse::<f32>().ok())
                    .unwrap_or(monitor.y as f32);
                // The compositor rounds sizes to whole logical pixels.
                Some((
                    index,
                    Screen {
                        x,
                        y,
                        width: width.round(),
                        height: height.round(),
                    },
                ))
            })
            .collect()
    }

    /// The labels the displays should show: while Displays is in front and
    /// arranges several displays, each one's number and name.
    fn display_labels(&self) -> Option<crate::identify::Labels> {
        if !self.identify || self.page != Page::Displays || !self.window_focused {
            return None;
        }
        let screens = self.arranged();
        (screens.len() > 1).then(|| crate::identify::Labels {
            displays: screens
                .into_iter()
                .map(|(index, _)| crate::identify::Label {
                    output: self.monitors[index].name.clone(),
                    number: index + 1,
                    name: self.monitor_name(&self.monitors[index].name),
                    lit: self
                        .display_drag
                        .is_some_and(|drag| drag.slot.index == index),
                })
                .collect(),
        })
    }

    /// Start, update or stop the labels on the displays.
    fn label_displays(&mut self) {
        let Some(labels) = self.display_labels() else {
            self.identifier = None;
            return;
        };
        let Ok(line) = serde_json::to_string(&labels) else {
            return;
        };
        if self.identifier.is_none() {
            self.identifier = Identifier::start();
        }
        if let Some(identifier) = &mut self.identifier
            && identifier.sent != line
        {
            use std::io::Write;
            if writeln!(identifier.input, "{line}").is_ok() {
                identifier.sent = line;
            } else {
                // It stopped; the next change starts another.
                self.identifier = None;
            }
        }
    }

    /// How the arrangement maps logical pixels to the drawing; frozen while a
    /// display is dragged so it stays under the pointer.
    fn arrangement_geometry(&self) -> Option<ArrangementGeometry> {
        if let Some(drag) = self.display_drag {
            return Some(ArrangementGeometry {
                x: drag.x,
                y: drag.y,
                fit: drag.fit,
            });
        }
        let screens: Vec<_> = self.arranged().into_iter().map(|(_, s)| s).collect();
        if screens.is_empty() {
            return None;
        }
        let fit = arrangement::fit(&screens);
        let (x, y) = arrangement::origin(&screens, fit);
        Some(ArrangementGeometry { x, y, fit })
    }

    /// The enabled displays drawn where they are placed, numbered as on
    /// their cards and on the screens themselves. Dragging one moves it,
    /// through the same position fields as typing.
    fn arrangement(&self) -> Option<Element<'_, Message>> {
        let geometry = self.arrangement_geometry()?;
        let mut canvas = iced::widget::stack![
            space()
                .width(arrangement::WIDTH)
                .height(arrangement::HEIGHT)
        ];
        for (index, screen) in self.arranged() {
            let monitor = &self.monitors[index];
            let mode = monitor.mode.and_then(|mode| monitor.modes.get(mode));
            let product = self.monitor_name(&monitor.name);
            let dragged = self
                .display_drag
                .is_some_and(|drag| drag.slot.index == index);
            let slot = MonitorSlot {
                generation: self.display_generation,
                index,
            };
            let picture: iced::widget::Container<'_, Message> = container(
                column![
                    display_number(index + 1, dragged),
                    text(product)
                        .size(kit::size::CAPTION)
                        .font(kit::strong_font(Weight::Semibold))
                        .wrapping(iced::widget::text::Wrapping::None),
                    text(
                        mode.map(|m| format!("{} × {}", m.width, m.height))
                            .unwrap_or_default()
                    )
                    .size(kit::size::SMALL)
                    .style(canopa_ui::muted)
                    .wrapping(iced::widget::text::Wrapping::None),
                    // A rotated display says so, on a line of its own.
                    text(if monitor.transform == Transform::Normal {
                        String::new()
                    } else {
                        transform_label(monitor.transform)
                    })
                    .size(kit::size::SMALL)
                    .style(canopa_ui::muted)
                    .wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(3)
                .align_x(Alignment::Center),
            )
            // A hairline apart, so displays that touch read as two.
            .center_x((screen.width * geometry.fit - 2.0).max(1.0))
            .center_y((screen.height * geometry.fit - 2.0).max(1.0))
            .clip(true)
            .style(move |theme: &Theme| {
                let p = canopa_ui::colors(theme);
                container::Style {
                    background: Some(
                        kit::blend(p.raised, p.accent, if dragged { 0.3 } else { 0.12 }).into(),
                    ),
                    border: iced::Border {
                        color: kit::blend(p.raised, p.accent, if dragged { 0.95 } else { 0.55 }),
                        width: if dragged { 2.0 } else { 1.0 },
                        radius: 6.0.into(),
                    },
                    shadow: iced::Shadow {
                        color: iced::Color {
                            a: if dragged { 0.25 } else { 0.0 },
                            ..iced::Color::BLACK
                        },
                        offset: iced::Vector::new(0.0, 4.0),
                        blur_radius: 12.0,
                    },
                    ..Default::default()
                }
            });
            let picture = iced::widget::mouse_area(picture)
                .on_press(Message::ArrangementGrab(slot))
                .interaction(if dragged {
                    iced::mouse::Interaction::Grabbing
                } else {
                    iced::mouse::Interaction::Grab
                });
            canvas = canvas.push(column![
                space().height((geometry.y + screen.y * geometry.fit + 1.0).max(0.0)),
                row![
                    space().width((geometry.x + screen.x * geometry.fit + 1.0).max(0.0)),
                    picture
                ]
            ]);
        }
        let canvas = iced::widget::mouse_area(
            container(canvas)
                .width(arrangement::WIDTH)
                .height(arrangement::HEIGHT)
                .clip(true)
                .style(|theme: &Theme| {
                    let p = canopa_ui::colors(theme);
                    container::Style {
                        background: Some(kit::blend(p.background, p.surface, 0.4).into()),
                        border: iced::Border {
                            radius: kit::CONTROL_RADIUS.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
        )
        .on_move(Message::ArrangementPointer)
        .on_release(Message::ArrangementRelease)
        .on_exit(Message::ArrangementRelease);
        Some(container(canvas).center_x(Length::Fill).into())
    }
    fn displays(&self) -> Element<'_, Message> {
        let reload = self.action(
            "settings-button-reload-displays",
            if self.refreshing {
                fl!("settings-displays-refreshing")
            } else {
                fl!("settings-displays-reload-short")
            },
            (!self.busy && !self.refreshing && self.trial.is_none()).then_some(Message::Refresh),
            false,
        );
        let several = self.arranged().len() > 1;
        // One display has nothing to arrange: a quiet line instead of a card.
        let mut body = if several {
            let mut arrangement = column![kit::row(
                fl!("settings-displays-arrangement-hint"),
                None,
                reload
            )]
            .spacing(14);
            if let Some(picture) = self.arrangement() {
                arrangement = arrangement.push(picture);
            }
            column![settings_group(
                fl!("settings-displays-arrangement"),
                arrangement
            )]
        } else {
            column![kit::status(
                kit::Notice::Info,
                fl!("settings-displays-one-connected"),
                Some(fl!("settings-displays-arrangement-single")),
                Some(reload),
            )]
        }
        .spacing(28);
        if self.display_profiles.prominent {
            body = body.push(self.profiles_view());
        }
        if self.monitors.is_empty() {
            body = body.push(
                container(text(fl!("settings-displays-none")))
                    .padding(kit::ROW_PADDING)
                    .width(Length::Fill)
                    .style(kit::card),
            );
        }
        for (index, monitor) in self.monitors.iter().enumerate() {
            let slot = MonitorSlot {
                generation: self.display_generation,
                index,
            };
            let modes: Vec<_> = monitor
                .modes
                .iter()
                .enumerate()
                .map(|(index, mode)| (index, mode_label(mode)))
                .collect();
            // Two monitors of the same kind are told apart by connector.
            let product = self.monitor_name(&monitor.name);
            let title = if self
                .monitors
                .iter()
                .filter(|other| self.monitor_name(&other.name) == product)
                .count()
                > 1
            {
                format!("{product} ({})", monitor.name)
            } else {
                product
            };
            // Numbered as in the arrangement and on the screens themselves.
            let title = if self.monitors.len() > 1 {
                fl!(
                    "settings-display-numbered",
                    number = (index + 1).to_string(),
                    name = title
                )
            } else {
                title
            };
            let mut rows: Vec<Element<'_, Message>> = vec![
                toggle(
                    format!("display-enabled-{index}"),
                    fl!("settings-display-enabled"),
                    monitor.enabled,
                    Message::Enabled(slot, !monitor.enabled),
                ),
                kit::row(
                    fl!("settings-display-mode"),
                    None,
                    pick(
                        format!("display-mode-{index}"),
                        modes,
                        monitor.mode,
                        move |v| Message::Mode(slot, v),
                    ),
                ),
                kit::row(
                    fl!("settings-display-rotation"),
                    None,
                    pick(
                        format!("display-rotation-{index}"),
                        transforms()
                            .into_iter()
                            .map(|v| (v, transform_label(v)))
                            .collect(),
                        Some(monitor.transform),
                        move |v| Message::Rotation(slot, v),
                    ),
                ),
                kit::row(
                    fl!("settings-display-scale-label"),
                    None,
                    pick(
                        format!("display-scale-{index}"),
                        {
                            // Quarter steps, and the current scale if it is between them.
                            let mut scales: Vec<f64> =
                                (2..=16).map(|quarter| f64::from(quarter) * 0.25).collect();
                            if !scales
                                .iter()
                                .any(|scale| (scale - monitor.scale).abs() < 1e-6)
                            {
                                scales.push(monitor.scale);
                                scales.sort_by(f64::total_cmp);
                            }
                            scales
                                .into_iter()
                                .map(|scale| (scale, canopa_i18n::format::percent(scale * 100.)))
                                .collect()
                        },
                        Some(monitor.scale),
                        move |v| Message::Scale(slot, v),
                    ),
                ),
            ];
            // Typed positions for exact placement; dragging does the rest.
            // A lone display has nowhere else to be.
            if several {
                rows.push(self.disclosure(
                    "display-positions",
                    fl!("settings-display-exact-position"),
                    || {
                        column![
                            row![
                                field(
                                    format!("display-x-{}", monitor.name),
                                    fl!("settings-display-x"),
                                    &self.positions[index].0,
                                    move |v| Message::Position(slot, true, v)
                                ),
                                field(
                                    format!("display-y-{}", monitor.name),
                                    fl!("settings-display-y"),
                                    &self.positions[index].1,
                                    move |v| Message::Position(slot, false, v)
                                )
                            ]
                            .spacing(16),
                            kit::caption(fl!("settings-displays-positions-hint")),
                        ]
                        .spacing(8)
                        .into()
                    },
                ));
            }
            let card = kit::rows(rows);
            // How changes apply, under the last display's card.
            body = body.push(if index + 1 == self.monitors.len() {
                noted_group(title, card, [fl!("settings-displays-apply-note")])
            } else {
                settings_group(title, card)
            });
        }
        if !self.display_profiles.prominent {
            body = body.push(self.profiles_view());
        }
        if crate::display::login_screen_available() {
            // Copies what is applied, so an unsaved draft must be saved first.
            body = body.push(settings_group(
                fl!("settings-displays-login-title"),
                kit::row(
                    fl!("settings-displays-login-label"),
                    Some(fl!("settings-displays-login-hint")),
                    self.action(
                        "settings-button-login-screen-displays",
                        fl!("settings-displays-login-button"),
                        (!self.busy
                            && !self.refreshing
                            && !self.display_dirty
                            && self.trial.is_none()
                            && !self.outputs.is_empty())
                        .then_some(Message::LoginDisplays),
                        false,
                    ),
                ),
            ));
        }
        body.into()
    }
    fn input_view(&self) -> Element<'_, Message> {
        let value = &self.input.value;
        // A speed of zero is libinput's default; others are shown as an offset.
        let speed = |speed: f64| {
            if speed.abs() < 0.005 {
                fl!("settings-input-speed-default")
            } else {
                format!(
                    "{}{}",
                    if speed > 0.0 { "+" } else { "\u{2212}" },
                    canopa_i18n::format::percent(speed.abs() * 100.0)
                )
            }
        };
        // One row per layout, by name; the codes stay editable below.
        let layouts = value.layouts.clone();
        let codes = crate::keyboard_layouts::codes(&value.layouts);
        let single = codes.len() <= 1;
        let mut keyboard_rows: Vec<Element<'_, Message>> = codes
            .iter()
            .enumerate()
            .map(|(index, code)| {
                let remaining = crate::keyboard_layouts::without(&layouts, code);
                let mut lead = vec![text(crate::keyboard_layouts::name(code)).into()];
                // The first layout is the one in use at login.
                if index == 0 {
                    lead.push(kit::badge(fl!("settings-input-layout-first")));
                }
                // The only layout cannot be removed.
                direction::row([
                    direction::row(lead)
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .width(Length::Fill)
                        .into(),
                    if single {
                        space().into()
                    } else {
                        self.treated_action(
                            format!("settings-input-layout-remove-{code}"),
                            fl!("settings-input-layout-remove"),
                            Some(Message::InputText(0, remaining)),
                            kit::Treatment::Destructive,
                        )
                    },
                ])
                .spacing(16)
                .align_y(Alignment::Center)
                .into()
            })
            .collect();
        keyboard_rows.push(kit::row(
            fl!("settings-input-layouts-add-label"),
            Some(if single {
                fl!("settings-input-layout-only")
            } else {
                fl!("settings-input-layouts-switch")
            }),
            hinted_choice(
                "settings-input-layout-add",
                crate::keyboard_layouts::all()
                    .iter()
                    .map(|layout| layout.name.clone())
                    .collect(),
                None,
                fl!("settings-input-layouts-add"),
                move |name| {
                    let code = crate::keyboard_layouts::all()
                        .iter()
                        .find(|layout| layout.name == name)
                        .map_or(name.as_str(), |layout| layout.code.as_str());
                    Message::InputText(0, crate::keyboard_layouts::with(&layouts, code))
                },
            ),
        ));
        keyboard_rows.push(self.disclosure(
            "input-advanced",
            fl!("settings-input-advanced"),
            || {
                column![
                    field(
                        "settings-field-layouts-in-switch-order-for-example-us-fr",
                        fl!("settings-input-layouts-codes"),
                        &value.layouts,
                        |v| Message::InputText(0, v)
                    ),
                    row![
                        field(
                            "settings-field-variants-optional-in-the-same-order",
                            fl!("settings-input-variants"),
                            &value.variants,
                            |v| Message::InputText(1, v)
                        ),
                        field(
                            "settings-field-xkb-options-for-example-grp-alt-shift-toggle",
                            fl!("settings-input-options"),
                            &value.options,
                            |v| Message::InputText(2, v)
                        ),
                    ]
                    .spacing(16),
                ]
                .spacing(12)
                .into()
            },
        ));
        let keyboard = kit::rows(keyboard_rows.into_iter().chain([
            kit::slider_row(
                fl!("settings-input-repeat-delay-label"),
                fl!(
                    "settings-input-repeat-delay-value",
                    delay = value.repeat_delay
                ),
                adjust(
                    "repeat-delay",
                    100.0..=2000.,
                    f64::from(value.repeat_delay),
                    50.,
                    |v| Message::InputNumber(0, v),
                ),
            ),
            kit::slider_row(
                fl!("settings-input-repeat-rate-label"),
                fl!("settings-input-repeat-rate-value", rate = value.repeat_rate),
                adjust(
                    "repeat-rate",
                    1.0..=100.,
                    f64::from(value.repeat_rate),
                    1.,
                    |v| Message::InputNumber(1, v),
                ),
            ),
        ]));
        let mouse = kit::rows([
            kit::slider_row(
                fl!("settings-input-pointer-speed-label"),
                speed(value.mouse_speed),
                adjust("mouse-speed", -1.0..=1., value.mouse_speed, 0.05, |v| {
                    Message::InputNumber(2, v)
                }),
            ),
            toggle(
                "input-flag-0",
                fl!("settings-input-left-handed"),
                value.mouse_left_handed,
                Message::InputFlag(0, !value.mouse_left_handed),
            ),
            toggle(
                "input-flag-1",
                fl!("settings-input-mouse-natural-scroll"),
                value.mouse_natural_scroll,
                Message::InputFlag(1, !value.mouse_natural_scroll),
            ),
        ]);
        let touchpad = kit::rows([
            kit::slider_row(
                fl!("settings-input-touchpad-speed-label"),
                speed(value.touchpad_speed),
                adjust(
                    "touchpad-speed",
                    -1.0..=1.,
                    value.touchpad_speed,
                    0.05,
                    |v| Message::InputNumber(3, v),
                ),
            ),
            toggle(
                "input-flag-2",
                fl!("settings-input-tap"),
                value.touchpad_tap,
                Message::InputFlag(2, !value.touchpad_tap),
            ),
            toggle(
                "input-flag-3",
                fl!("settings-input-touchpad-natural-scroll"),
                value.touchpad_natural_scroll,
                Message::InputFlag(3, !value.touchpad_natural_scroll),
            ),
            toggle(
                "input-flag-4",
                fl!("settings-input-disable-while-typing"),
                value.touchpad_disable_while_typing,
                Message::InputFlag(4, !value.touchpad_disable_while_typing),
            ),
        ]);
        column![
            settings_group(fl!("settings-input-keyboard"), keyboard),
            settings_group(fl!("settings-input-mouse"), mouse),
            settings_group(fl!("settings-input-touchpad"), touchpad)
        ]
        .spacing(28)
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
        let cursor_row = if self.paths.active() {
            kit::row(
                fl!("settings-a11y-cursor-size-label"),
                None,
                pick(
                    "settings-cursor-size",
                    cursor_options,
                    Some(cursor),
                    Message::CursorSize,
                ),
            )
        } else {
            kit::row(
                fl!("settings-a11y-cursor-size-label"),
                Some(fl!("settings-cursor-size-needs-session")),
                space(),
            )
        };
        let seeing = kit::rows([
            described_toggle(
                "settings-high-contrast",
                fl!("settings-a11y-high-contrast"),
                Some(fl!("settings-a11y-high-contrast-hint")),
                values.high_contrast,
                Message::HighContrast(!values.high_contrast),
            ),
            // Text size in the steps people choose between; a size set in
            // the file stays listed.
            kit::row(
                fl!("settings-a11y-text-size-label"),
                None,
                pick(
                    "settings-accessibility-text-scale",
                    {
                        let mut sizes = vec![100_u16, 110, 125, 150];
                        if !sizes.contains(&values.text_scale) {
                            sizes.push(values.text_scale);
                            sizes.sort_unstable();
                        }
                        sizes
                            .into_iter()
                            .map(|size| {
                                let percent = canopa_i18n::format::percent(f64::from(size));
                                (
                                    size,
                                    if size == 100 {
                                        fl!("settings-a11y-text-size-default", size = percent)
                                    } else {
                                        percent
                                    },
                                )
                            })
                            .collect()
                    },
                    Some(values.text_scale),
                    |size| Message::TextScale(f64::from(size)),
                ),
            ),
            cursor_row,
        ]);
        let motion = kit::rows([
            described_toggle(
                "settings-accessibility-reduced-motion",
                fl!("settings-a11y-reduced-motion"),
                Some(fl!("settings-a11y-reduced-motion-hint")),
                values.reduced_motion,
                Message::ReducedMotion(!values.reduced_motion),
            ),
            match self.compositor_motion {
                Some(reduced) => described_toggle(
                    "settings-accessibility-compositor-motion",
                    fl!("settings-motion-windows-label"),
                    Some(fl!("settings-motion-windows-hint")),
                    reduced,
                    Message::CompositorMotion(!reduced),
                ),
                None => kit::caption(fl!("settings-motion-needs-session")),
            },
        ]);
        column![
            settings_group(fl!("settings-a11y-seeing"), seeing),
            settings_group(fl!("settings-a11y-motion"), motion),
            settings_group(
                fl!("settings-a11y-screen-reader"),
                match orca() {
                    Some(_) => kit::row(
                        fl!("settings-a11y-orca-label"),
                        Some(fl!("settings-a11y-orca-hint")),
                        self.action(
                            "settings-button-start-orca",
                            fl!("settings-a11y-orca-start"),
                            Some(Message::StartScreenReader),
                            false,
                        ),
                    ),
                    None => kit::row(
                        fl!("settings-a11y-orca-label"),
                        Some(fl!("settings-a11y-orca-missing")),
                        space(),
                    ),
                }
            ),
        ]
        .spacing(28)
        .into()
    }
    fn personal_view(&self) -> Element<'_, Message> {
        let values = &self.personal.value.values;
        let light = values.appearance.color_scheme == canopa_theme::ColorScheme::Light;
        let typed = self.personal.value.accent.trim().to_lowercase();
        // Accent presets, each tuned for the dark and the light appearance.
        let presets: [(
            &'static str,
            String,
            Option<&'static str>,
            Option<&'static str>,
        ); 8] = [
            ("default", fl!("settings-accent-default"), None, None),
            (
                "blue",
                fl!("settings-accent-blue"),
                Some("#8ab4f8"),
                Some("#1c5fb0"),
            ),
            (
                "purple",
                fl!("settings-accent-purple"),
                Some("#c6a0f6"),
                Some("#6a3fa0"),
            ),
            (
                "pink",
                fl!("settings-accent-pink"),
                Some("#f5a3c7"),
                Some("#a3306b"),
            ),
            (
                "red",
                fl!("settings-accent-red"),
                Some("#f38ba8"),
                Some("#b0203f"),
            ),
            (
                "orange",
                fl!("settings-accent-orange"),
                Some("#f5a97f"),
                Some("#a8480f"),
            ),
            (
                "yellow",
                fl!("settings-accent-yellow"),
                Some("#eed49f"),
                Some("#7d5d00"),
            ),
            (
                "teal",
                fl!("settings-accent-teal"),
                Some("#8bd5ca"),
                Some("#126b63"),
            ),
        ];
        let swatches =
            direction::row(presets.into_iter().map(|(key, name, dark, bright)| {
                let hex = if light { bright } else { dark };
                let selected = match hex {
                    None => typed.is_empty(),
                    Some(hex) => typed == hex,
                };
                let color = hex.and_then(canopa_theme::Rgb::parse_hex);
                let choose = Message::PersonalText(0, hex.unwrap_or_default().to_owned());
                semantic(
                    Node::button(name.clone())
                        .key(format!("settings-accent-{key}"))
                        .toggled(selected)
                        .on_click(Some(choose.clone())),
                    focus::control(
                        format!("settings-accent-{key}"),
                        Some(choose.clone()),
                        None,
                        None,
                        false,
                        iced::widget::button(container(space().width(20).height(20)).style(
                            move |theme: &Theme| {
                                container::Style {
                                    background: Some(
                                        color
                                            .map(canopa_ui::color)
                                            .unwrap_or(canopa_ui::color(
                                                canopa_theme::Appearance {
                                                    color_scheme: if light {
                                                        canopa_theme::ColorScheme::Light
                                                    } else {
                                                        canopa_theme::ColorScheme::Dark
                                                    },
                                                    ..Default::default()
                                                }
                                                .palette()
                                                .accent,
                                            ))
                                            .into(),
                                    ),
                                    border: iced::Border {
                                        color: kit::hairline(theme),
                                        width: 1.0,
                                        radius: u32::MAX.into(),
                                    },
                                    ..Default::default()
                                }
                            },
                        ))
                        .on_press(choose)
                        .padding(3)
                        .style(move |theme: &Theme, status| iced::widget::button::Style {
                            background: None,
                            border: iced::Border {
                                color: if selected {
                                    canopa_ui::colors(theme).text
                                } else if status == iced::widget::button::Status::Hovered {
                                    kit::hairline(theme)
                                } else {
                                    iced::Color::TRANSPARENT
                                },
                                width: 2.0,
                                radius: u32::MAX.into(),
                            },
                            ..Default::default()
                        }),
                    ),
                )
            }))
            .spacing(6)
            .align_y(Alignment::Center);
        let appearance = kit::rows([
            toggle(
                "light",
                fl!("settings-personal-light"),
                light,
                Message::Light(!light),
            ),
            column![
                kit::row(fl!("settings-personal-accent-label"), None, swatches),
                kit::row(
                    fl!("settings-personal-accent-custom"),
                    Some(fl!("settings-personal-accent-custom-hint")),
                    bare_field(
                        "settings-field-accent-color-optional-rrggbb",
                        fl!("settings-personal-accent"),
                        "#RRGGBB",
                        &self.personal.value.accent,
                        |v| Message::PersonalText(0, v),
                        Length::Fixed(kit::PICK_WIDTH),
                    ),
                ),
            ]
            .spacing(14)
            .into(),
            kit::row(
                fl!("settings-personal-wallpaper-label"),
                Some(fl!("settings-personal-wallpaper-hint")),
                row![
                    self.action(
                        "settings-button-wallpaper-file",
                        fl!("settings-personal-wallpaper-file"),
                        Some(Message::WallpaperFile),
                        false,
                    ),
                    self.action(
                        "settings-button-choose-wallpaper",
                        fl!("settings-personal-wallpaper"),
                        Some(Message::Wallpaper),
                        true,
                    ),
                ]
                .spacing(8),
            ),
        ]);
        let lockscreen = kit::rows([
            kit::slider_row(
                fl!("settings-personal-dimming-label"),
                canopa_i18n::format::percent(f64::from(values.lock.dim)),
                adjust(
                    "lock-dim",
                    0.0..=100.,
                    f64::from(values.lock.dim),
                    1.,
                    Message::Dim,
                ),
            ),
            kit::row(
                fl!("settings-personal-lock-widgets-label"),
                Some(fl!("settings-personal-lock-widgets-hint")),
                self.action(
                    "settings-button-edit-lock-screen-widgets",
                    fl!("settings-personal-edit-lock-widgets"),
                    Some(Message::OpenLockWidgetEditor),
                    false,
                ),
            ),
            self.disclosure(
                "personal-lock",
                fl!("settings-personal-lock-advanced"),
                || {
                    column![
                        row![
                            hinted_field(
                                "settings-field-clock-format",
                                fl!("settings-personal-clock-format"),
                                &fl!("settings-personal-format-default"),
                                &values.lock.clock_format,
                                |v| { Message::PersonalText(1, v) }
                            ),
                            hinted_field(
                                "settings-field-date-format",
                                fl!("settings-personal-date-format"),
                                &fl!("settings-personal-format-default"),
                                &values.lock.date_format,
                                |v| { Message::PersonalText(2, v) }
                            ),
                        ]
                        .spacing(16),
                        kit::caption(fl!("settings-personal-formats-hint")),
                        field(
                            "settings-field-font-family",
                            fl!("settings-personal-font"),
                            &values.lock.font,
                            |v| Message::PersonalText(3, v),
                        ),
                        field(
                            "settings-field-avatar-image-optional-absolute-path",
                            fl!("settings-personal-avatar"),
                            &values
                                .lock
                                .avatar
                                .as_ref()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            |v| Message::PersonalText(4, v),
                        ),
                    ]
                    .spacing(12)
                    .into()
                },
            ),
        ]);
        column![
            settings_group(fl!("settings-personal-appearance"), appearance),
            settings_group(fl!("settings-personal-lock-screen"), lockscreen),
            settings_group(fl!("settings-window-layout"), self.window_layout_view()),
        ]
        .spacing(28)
        .into()
    }
}
impl State {
    /// Gaps, centering and the width of new columns, applied to Niri at once.
    fn window_layout_view(&self) -> Element<'_, Message> {
        use crate::niri::{CenterColumn, ColumnWidth, WindowLayout};
        let layout = match &self.window_layout {
            None => {
                return text(fl!("settings-motion-needs-session"))
                    .size(13)
                    .style(canopa_ui::muted)
                    .into();
            }
            Some(Err(error)) => {
                return text(error.clone()).size(13).style(canopa_ui::muted).into();
            }
            Some(Ok(layout)) => *layout,
        };
        let current = layout.unwrap_or(WindowLayout::SHIPPED);
        let gaps = WindowLayout::GAPS
            .into_iter()
            .map(|gaps| {
                let label = match gaps {
                    0 => fl!("settings-window-gaps-none"),
                    6 => fl!("settings-window-gaps-small"),
                    12 => fl!("settings-window-gaps-medium"),
                    16 => fl!("settings-window-gaps-comfortable"),
                    _ => fl!("settings-window-gaps-large"),
                };
                (gaps, label)
            })
            .collect();
        let center = CenterColumn::ALL
            .into_iter()
            .map(|center| {
                let label = match center {
                    CenterColumn::Never => fl!("settings-window-center-never"),
                    CenterColumn::OnOverflow => fl!("settings-window-center-on-overflow"),
                    CenterColumn::Always => fl!("settings-window-center-always"),
                };
                (center, label)
            })
            .collect();
        let width = ColumnWidth::ALL
            .into_iter()
            .map(|width| {
                let label = match width {
                    ColumnWidth::Third => fl!("settings-window-width-third"),
                    ColumnWidth::Half => fl!("settings-window-width-half"),
                    ColumnWidth::TwoThirds => fl!("settings-window-width-two-thirds"),
                    ColumnWidth::Full => fl!("settings-window-width-full"),
                };
                (width, label)
            })
            .collect();
        // Niri takes these at once, unlike the rest of the page.
        let labelled =
            |label: String, control: Element<'static, Message>| kit::row(label, None, control);
        let mut content = vec![
            labelled(
                fl!("settings-window-gaps"),
                pick(
                    "settings-window-gaps",
                    gaps,
                    Some(current.gaps),
                    move |gaps| Message::WindowLayout(Some(WindowLayout { gaps, ..current })),
                ),
            ),
            labelled(
                fl!("settings-window-center"),
                pick(
                    "settings-window-center",
                    center,
                    Some(current.center),
                    move |center| Message::WindowLayout(Some(WindowLayout { center, ..current })),
                ),
            ),
            labelled(
                fl!("settings-window-width"),
                pick(
                    "settings-window-width",
                    width,
                    Some(current.width),
                    move |width| Message::WindowLayout(Some(WindowLayout { width, ..current })),
                ),
            ),
        ];
        if layout.is_some() {
            content.push(self.action(
                "settings-button-window-layout-reset",
                fl!("settings-window-layout-reset"),
                Some(Message::WindowLayout(None)),
                false,
            ));
        }
        kit::rows(content)
    }
}

/// A labelled text field. `key` is its focus and accessibility identity.
fn field<'a>(
    key: impl Into<String>,
    label: impl Into<String>,
    value: &str,
    change: impl Fn(String) -> Message + Clone + Send + Sync + 'static,
) -> Element<'a, Message> {
    hinted_field(key, label, "", value, change)
}
/// A labelled text field showing `placeholder` while empty.
fn hinted_field<'a>(
    key: impl Into<String>,
    label: impl Into<String>,
    placeholder: &str,
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
            text(label).size(kit::size::BODY),
            text_input(placeholder, value)
                .id(key)
                .on_input(change)
                .align_x(direction::start())
                .padding([6, 12])
                .style(kit::text_input_style)
        ]
        .spacing(7)
        .width(Length::Fill),
    )
}
/// A [`bare_field`] for a key combination: compact and monospaced.
fn key_field<'a>(
    key: impl Into<String>,
    label: impl Into<String>,
    placeholder: &str,
    value: &str,
    change: impl Fn(String) -> Message + Clone + Send + Sync + 'static,
) -> Element<'a, Message> {
    let key = key.into();
    semantic(
        Node::text_input(label.into(), value)
            .key(format!("{key}-field"))
            .on_text(change.clone()),
        text_input(placeholder, value)
            .id(key)
            .on_input(change)
            .align_x(direction::start())
            .padding([4, 10])
            .size(kit::size::CAPTION)
            .font(iced::Font::MONOSPACE)
            .width(Length::Fixed(200.))
            .style(kit::text_input_style),
    )
}
/// A text field named for assistive technologies only, for rows whose
/// visible label leads the row. `key` is its focus and accessibility identity.
fn bare_field<'a>(
    key: impl Into<String>,
    label: impl Into<String>,
    placeholder: &str,
    value: &str,
    change: impl Fn(String) -> Message + Clone + Send + Sync + 'static,
    width: Length,
) -> Element<'a, Message> {
    let key = key.into();
    semantic(
        Node::text_input(label.into(), value)
            .key(format!("{key}-field"))
            .on_text(change.clone()),
        text_input(placeholder, value)
            .id(key)
            .on_input(change)
            .align_x(direction::start())
            .padding([6, 12])
            .width(width)
            .style(kit::text_input_style),
    )
}
/// Open the wallpaper picker on `images`, or on the pictures it finds.
fn wallpaper_picker(images: Vec<String>) -> Task<Message> {
    let (send, receive) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let mut args = vec!["pick".to_owned()];
        args.extend(images);
        let result = crate::storage::application(companion("canopa-wallpaper"), &args)
            .map_err(|error| format!("{error:#}"));
        let _ = send.send(result);
    });
    Task::perform(
        async move {
            receive
                .await
                .unwrap_or_else(|_| Err(fl!("settings-wallpaper-stopped")))
        },
        Message::WallpaperFinished,
    )
}
/// A monitor as people know it: its product name without the company's
/// legal suffix or a repeated brand, the laptop's own screen, or the kind of
/// connector when the monitor does not say what it is.
fn monitor_title(make: &str, model: &str, connector: &str) -> String {
    let kind = connector.split('-').next().unwrap_or_default();
    if matches!(kind, "eDP" | "LVDS" | "DSI") {
        return fl!("settings-display-built-in");
    }
    // Nested and headless compositors draw into a window or memory.
    if make == "Smithay" || matches!(kind, "winit" | "headless" | "WL" | "X11") {
        return fl!("settings-display-virtual");
    }
    let make = make
        .trim()
        .trim_end_matches('.')
        .trim_end_matches(" Inc")
        .trim_end_matches(',')
        .trim_end_matches(" Corporation")
        .trim_end_matches(" Co., Ltd")
        .trim();
    let model = model.trim();
    let unknown = |value: &str| value.is_empty() || value.eq_ignore_ascii_case("unknown");
    let product = match (unknown(make), unknown(model)) {
        (true, true) => String::new(),
        (true, false) => model.to_owned(),
        (false, true) => make.to_owned(),
        // "Dell Inc. DELL U2720Q" reads "Dell U2720Q".
        (false, false) => {
            let model = model
                .strip_prefix(&make.to_uppercase())
                .or_else(|| model.strip_prefix(make))
                .map_or(model, str::trim_start);
            format!("{make} {model}")
        }
    };
    if !product.is_empty() {
        return product;
    }
    match kind {
        "HDMI" => fl!("settings-display-hdmi"),
        "DP" => fl!("settings-display-displayport"),
        _ => connector.to_owned(),
    }
}

/// The Orca executable on `PATH`, looked up once.
fn orca() -> Option<&'static std::path::Path> {
    static ORCA: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    ORCA.get_or_init(|| {
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|directory| directory.join("orca"))
                .find(|candidate| candidate.is_file())
        })
    })
    .as_deref()
}
fn heading<'a>(label: impl Into<String>, size: u32) -> iced::widget::Text<'a> {
    text(label.into())
        .size(size)
        .font(kit::strong_font(Weight::Semibold))
}

fn settings_group<'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let label = label.into();
    semantic(
        Node::group(label.clone()),
        column![
            semantic(
                Node::heading(label.clone(), 2),
                container(
                    heading(label, kit::size::SECTION)
                        .width(Length::Fill)
                        .align_x(direction::text_start())
                )
            ),
            container(content)
                .padding(kit::ROW_PADDING)
                .width(Length::Fill)
                .style(kit::card),
        ]
        .spacing(10),
    )
}

/// A [`settings_group`] with notes about the whole section under its card.
fn noted_group<'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, Message>>,
    notes: impl IntoIterator<Item = String>,
) -> Element<'a, Message> {
    let mut section = column![settings_group(label, content)].spacing(8);
    for note in notes {
        section = section.push(kit::footnote(note));
    }
    section.into()
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
            styled_button(text(label), message, treatment.into())
                .width(width)
                .padding([7, 14]),
        ),
    )
}
/// A button in one of the Settings treatments.
fn styled_button<'a>(
    content: impl Into<Element<'a, Message>>,
    message: Option<Message>,
    treatment: kit::Treatment,
) -> iced::widget::Button<'a, Message> {
    iced::widget::button(content)
        .on_press_maybe(message)
        .style(move |theme, status| kit::button_style(theme, status, treatment))
}
fn toggle<'a>(
    id: impl Into<String>,
    label: impl Into<String>,
    value: bool,
    message: Message,
) -> Element<'a, Message> {
    described_toggle(id, label, None, value, message)
}
/// A [`toggle`] with an explanation under its name.
fn described_toggle<'a>(
    id: impl Into<String>,
    label: impl Into<String>,
    description: Option<String>,
    value: bool,
    message: Message,
) -> Element<'a, Message> {
    let id = id.into();
    let label = label.into();
    let press = message.clone();
    let mut lead = column![
        text(label.clone())
            .size(kit::size::BODY)
            .width(Length::Fill)
            .align_x(direction::text_start())
    ]
    .spacing(2);
    if let Some(description) = description {
        lead = lead.push(kit::caption(description));
    }
    semantic(
        Node::switch(label.clone(), value)
            .key(id.clone())
            .on_click(Some(message.clone())),
        focus::control(
            id,
            Some(message.clone()),
            None,
            None,
            false,
            // The label toggles too, like the switch it names.
            direction::row([
                iced::widget::mouse_area(lead.width(Length::Fill))
                    .on_press(press)
                    .interaction(iced::mouse::Interaction::Pointer)
                    .into(),
                iced::widget::toggler(value)
                    .size(20)
                    .on_toggle(move |_| message.clone())
                    .style(kit::switch_style)
                    .into(),
            ])
            .spacing(16)
            .align_y(Alignment::Center)
            .width(Length::Fill),
        ),
    )
}
/// An on/off choice drawn as a pill, for compact sets such as weekdays.
fn chip<'a>(
    id: impl Into<String>,
    label: impl Into<String>,
    value: bool,
    message: Message,
) -> Element<'a, Message> {
    let id = id.into();
    let label = label.into();
    semantic(
        Node::switch(label.clone(), value)
            .key(id.clone())
            .on_click(Some(message.clone())),
        focus::control(
            id,
            Some(message.clone()),
            None,
            None,
            false,
            iced::widget::button(text(label).size(kit::size::CAPTION))
                .on_press(message)
                .padding([5, 12])
                .style(move |theme, status| {
                    let mut style = kit::button_style(
                        theme,
                        status,
                        if value {
                            kit::Treatment::Selected
                        } else {
                            kit::Treatment::Secondary
                        },
                    );
                    style.border.radius = u32::MAX.into();
                    style
                }),
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
    styled_adjust(id, range, value, step, change, kit::slider_style)
}
/// An [`adjust`] for a value centred on zero, such as balance.
fn centered_adjust<'a>(
    id: impl Into<String>,
    range: std::ops::RangeInclusive<f64>,
    value: f64,
    step: f64,
    change: impl Fn(f64) -> Message + Clone + Send + Sync + 'static,
) -> Element<'a, Message> {
    styled_adjust(id, range, value, step, change, kit::centered_slider_style)
}
fn styled_adjust<'a>(
    id: impl Into<String>,
    range: std::ops::RangeInclusive<f64>,
    value: f64,
    step: f64,
    change: impl Fn(f64) -> Message + Clone + Send + Sync + 'static,
    style: fn(&Theme, iced::widget::slider::Status) -> iced::widget::slider::Style,
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
            slider(range, value, change)
                .step(step)
                .height(22)
                .style(style),
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
    hinted_choice(
        id,
        options,
        selected,
        fl!("settings-choice-placeholder"),
        change,
    )
}
/// A [`choice`] that shows `placeholder` while nothing is chosen.
fn hinted_choice<'a>(
    id: impl Into<String>,
    options: Vec<String>,
    selected: Option<String>,
    placeholder: String,
    change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    sized_choice(id, options, selected, placeholder, kit::PICK_WIDTH, change)
}
/// A [`hinted_choice`] of a given width, for dense rows.
fn sized_choice<'a>(
    id: impl Into<String>,
    options: Vec<String>,
    selected: Option<String>,
    placeholder: String,
    width: f32,
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
            container(
                pick_list(options, selected, change)
                    .placeholder(placeholder)
                    .width(Length::Fill)
                    .padding([5, 12])
                    .text_size(kit::size::BODY)
                    .style(kit::pick_list_style)
                    .menu_style(kit::menu_style),
            )
            .width(Length::Fixed(width)),
        ),
    )
}
/// The process labelling the displays; it ends with this value.
struct Identifier {
    child: std::process::Child,
    input: std::process::ChildStdin,
    /// The labels last written.
    sent: String,
}
impl Identifier {
    fn start() -> Option<Self> {
        let mut child = std::process::Command::new(std::env::current_exe().ok()?)
            .arg("identify-displays")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let input = child.stdin.take()?;
        Some(Self {
            child,
            input,
            sent: String::new(),
        })
    }
}
impl Drop for Identifier {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
/// The arrangement drawing's origin, where logical 0, 0 lands, and scale.
#[derive(Clone, Copy, Debug)]
struct ArrangementGeometry {
    x: f32,
    y: f32,
    fit: f32,
}
/// A display being dragged in the arrangement: where it was grabbed, the
/// drawing's geometry when the drag began, and whether it has moved.
#[derive(Clone, Copy, Debug)]
struct DisplayDrag {
    slot: MonitorSlot,
    grab: iced::Point,
    x: f32,
    y: f32,
    fit: f32,
    width: f32,
    height: f32,
    moved: bool,
}
/// A display's number, as on its card and on the screen itself.
fn display_number<'a>(number: usize, lit: bool) -> Element<'a, Message> {
    container(
        text(number.to_string())
            .size(kit::size::CAPTION)
            .font(kit::strong_font(Weight::Bold))
            .style(move |theme: &Theme| {
                let p = canopa_ui::colors(theme);
                iced::widget::text::Style {
                    color: Some(if lit { p.on_accent } else { p.text }),
                }
            }),
    )
    .center_x(22)
    .center_y(22)
    .style(move |theme: &Theme| {
        let p = canopa_ui::colors(theme);
        container::Style {
            background: Some(
                if lit {
                    p.accent
                } else {
                    kit::blend(p.raised, p.text, 0.1)
                }
                .into(),
            ),
            border: iced::Border {
                radius: 11.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}
/// A display in the list drawn at `generation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorSlot {
    generation: u64,
    index: usize,
}

/// Make equal labels distinct ("60 Hz", "60 Hz (2)") so each option stays
/// selectable: a pick list reports the chosen label, not its position.
fn distinct_labels<V>(options: Vec<(V, String)>) -> Vec<(V, String)> {
    let mut seen = HashMap::<String, usize>::new();
    options
        .into_iter()
        .map(|(value, label)| {
            let count = seen.entry(label.clone()).or_default();
            *count += 1;
            let label = if *count == 1 {
                label
            } else {
                format!("{label} ({count})")
            };
            (value, label)
        })
        .collect()
}

/// A pick list of values shown by their labels; messages carry the value,
/// never the (translated) label. Equal labels are made distinct.
fn pick<'a, V: Clone + PartialEq + 'a>(
    id: impl Into<String>,
    options: Vec<(V, String)>,
    selected: Option<V>,
    change: impl Fn(V) -> Message + 'a,
) -> Element<'a, Message> {
    sized_pick(id, options, selected, kit::PICK_WIDTH, change)
}
/// A [`pick`] of a given width, for dense rows.
fn sized_pick<'a, V: Clone + PartialEq + 'a>(
    id: impl Into<String>,
    options: Vec<(V, String)>,
    selected: Option<V>,
    width: f32,
    change: impl Fn(V) -> Message + 'a,
) -> Element<'a, Message> {
    let options = distinct_labels(options);
    let Some(fallback) = options.first().map(|(value, _)| value.clone()) else {
        return choice(id, Vec::new(), None, |_| Message::Noop);
    };
    let labels: Vec<String> = options.iter().map(|(_, label)| label.clone()).collect();
    let selected = selected.and_then(|selected| {
        options
            .iter()
            .find(|(value, _)| *value == selected)
            .map(|(_, label)| label.clone())
    });
    sized_choice(
        id,
        labels,
        selected,
        fl!("settings-choice-placeholder"),
        width,
        move |label| {
            let value = options
                .iter()
                .find(|(_, known)| *known == label)
                .map_or_else(|| fallback.clone(), |(value, _)| value.clone());
            change(value)
        },
    )
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
    // Without a configuration home there is nothing to show or save: report it
    // before opening a window instead of panicking inside it.
    let paths = Paths::discover()?;
    let activations =
        instance.map(|instance| Activations(Arc::new(Mutex::new(Some(instance.requests)))));
    // The shared typography decides the family; headings reuse it at a heavier weight.
    let font = canopa_ui::font(
        &FileDraft::<Personalization>::load(&paths)
            .0
            .value
            .values
            .appearance,
    );
    kit::set_font(font);
    iced::application(
        move || {
            let (mut state, task) = State::boot_with(paths.clone());
            state.activations = activations.clone();
            state.identify = true;
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
    .settings(iced::Settings {
        default_font: font,
        default_text_size: iced::Pixels(kit::size::BODY as f32),
        ..Default::default()
    })
    .window(iced::window::Settings {
        size: iced::Size::new(1000., 780.),
        // The navigation and page body scroll independently above the actions.
        min_size: Some(iced::Size::new(760., 480.)),
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

    #[test]
    fn a_page_change_clears_the_previous_page_notice() {
        let (_root, mut state) = fresh();
        state.notice = "Display test failed".into();
        let _ = state.update(Message::Page(Page::Input));
        assert!(state.notice.is_empty());
    }

    #[test]
    fn display_edits_queued_before_a_refresh_are_dropped() {
        let (_root, mut state) = fresh();
        let monitor = crate::display::Monitor {
            name: "DP-1".into(),
            make: String::new(),
            model: String::new(),
            serial: None,
            enabled: true,
            mode: None,
            modes: Vec::new(),
            custom_mode: false,
            scale: 1.,
            transform: Transform::Normal,
            x: 0,
            y: 0,
        };
        state.monitors = vec![monitor];
        state.positions = vec![("0".into(), "0".into())];
        let current = MonitorSlot {
            generation: state.display_generation,
            index: 0,
        };
        let _ = state.update(Message::Scale(current, 2.));
        assert_eq!(state.monitors[0].scale, 2.);
        // Hotplug replaced the list: an older click changes nothing and never
        // indexes past the end.
        state.display_generation += 1;
        let _ = state.update(Message::Scale(current, 3.));
        let _ = state.update(Message::Enabled(current, false));
        let beyond = MonitorSlot {
            generation: state.display_generation,
            index: 7,
        };
        let _ = state.update(Message::Position(beyond, true, "10".into()));
        assert_eq!(state.monitors[0].scale, 2.);
        assert!(state.monitors[0].enabled);
    }

    #[test]
    fn dragging_a_display_snaps_it_and_dropping_it_apart_settles_it_beside_its_neighbour() {
        let (_root, mut state) = fresh();
        let display = |name: &str, x: i32| crate::display::Monitor {
            name: name.into(),
            make: String::new(),
            model: String::new(),
            serial: None,
            enabled: true,
            mode: Some(0),
            modes: vec![niri_ipc::Mode {
                width: 1000,
                height: 500,
                refresh_rate: 60_000,
                is_preferred: true,
            }],
            custom_mode: false,
            scale: 1.,
            transform: Transform::Normal,
            x,
            y: 0,
        };
        state.monitors = vec![display("DP-1", 0), display("DP-2", 1000)];
        state.positions = vec![("0".into(), "0".into()), ("1000".into(), "0".into())];
        let second = MonitorSlot {
            generation: state.display_generation,
            index: 1,
        };
        let geometry = state.arrangement_geometry().unwrap();
        // Grab the second display at its centre, then move it 300 logical
        // pixels right and 10 down: the small vertical offset snaps back.
        let grab = iced::Point::new(
            geometry.x + 1500. * geometry.fit,
            geometry.y + 250. * geometry.fit,
        );
        let _ = state.update(Message::ArrangementPointer(grab));
        let _ = state.update(Message::ArrangementGrab(second));
        let _ = state.update(Message::ArrangementPointer(iced::Point::new(
            grab.x + 300. * geometry.fit,
            grab.y + 10. * geometry.fit,
        )));
        assert_eq!(state.positions[1], ("1300".to_owned(), "0".to_owned()));
        assert!(state.display_dirty);
        // Dropped apart, it comes back to touch its neighbour.
        let _ = state.update(Message::ArrangementRelease);
        let _ = state.update(Message::ArrangementPointer(iced::Point::ORIGIN));
        assert_eq!(state.positions[1], ("1000".to_owned(), "0".to_owned()));
    }

    #[test]
    fn equal_option_labels_stay_distinct() {
        let options = distinct_labels(vec![
            (0, "1920 × 1080 @ 60 Hz".to_owned()),
            (1, "1920 × 1080 @ 60 Hz".to_owned()),
            (2, "1280 × 720 @ 60 Hz".to_owned()),
        ]);
        let labels: Vec<_> = options.iter().map(|(_, label)| label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "1920 × 1080 @ 60 Hz",
                "1920 × 1080 @ 60 Hz (2)",
                "1280 × 720 @ 60 Hz"
            ]
        );
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
    fn every_page_renders_right_to_left_in_the_pseudo_locale() {
        crate::i18n::isolated(
            concat!(
                module_path!(),
                "::every_page_renders_right_to_left_in_the_pseudo_locale"
            ),
            || {
                let _language = crate::i18n::Language::english();
                canopa_i18n::select(canopa_i18n::Selection {
                    pseudo: true,
                    ..Default::default()
                });
                assert!(direction::is_rtl());
                let (_root, mut state) = fresh();
                for entry in registry::PAGES {
                    let _ = state.update(Message::Page(entry.page));
                    drop(state.view());
                }
                let title = Page::Displays.title();
                assert_ne!(title, "Displays");
                assert_eq!(canopa_i18n::pseudo::strip(&title), "Displays");
                // English words still find pages.
                assert_eq!(registry::search("display")[0].page, Page::Displays);
            },
        );
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
