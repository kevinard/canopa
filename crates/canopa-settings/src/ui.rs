//! Ordinary settings window with shared controls and bounded background work.
mod focus;
mod launcher;
mod profiles;
mod shortcuts;
mod widget_layouts;
use crate::{
    display::{Monitor, TrialHandle, transform_name},
    input::Input,
    niri::Client,
    preferences::{IdleTimes, Personal},
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
use std::{collections::HashMap, process::Stdio, time::Duration};

use crate::Page;
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
    notice: String,
    busy: bool,
    refreshing: bool,
    outputs: HashMap<String, Output>,
    monitors: Vec<Monitor>,
    positions: Vec<(String, String)>,
    display_dirty: bool,
    display_profiles: profiles::Editor,
    widget_layouts: widget_layouts::View,
    input: Input,
    input_source: Option<Snapshot>,
    personal: Personal,
    common_source: Option<Snapshot>,
    accent: String,
    idle_source: Option<Snapshot>,
    idle_ac: [String; 3],
    idle_battery: [String; 3],
    profile: String,
    idle_status: String,
    idle_available: bool,
    trial: Option<TrialHandle>,
    remaining: u64,
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

    fn load_input(paths: &Paths) -> (Option<Snapshot>, Input, Option<String>) {
        let source = Snapshot::load(paths.input()).ok();
        let (input, error) = match source.as_ref().map(|s| Input::parse(&s.text)).transpose() {
            Ok(input) => (input.unwrap_or_default(), None),
            Err(error) => (Input::default(), Some(error.to_string())),
        };
        (source, input, error)
    }

    fn load_personal(paths: &Paths) -> (Option<Snapshot>, Personal, String, Option<String>) {
        let source = Snapshot::load(paths.common()).ok();
        let (personal, error) = match source.as_ref().map(Personal::load).transpose() {
            Ok(Some(personal)) => (personal, None),
            result => {
                let (lock, _) = canopa_session_ui::settings::Settings::load(
                    &canopa_config::Request::Defaults,
                    canopa_config::Component::Lockscreen,
                )
                .expect("embedded lock settings are valid");
                (
                    Personal::from_lock(lock),
                    result.err().map(|error| error.to_string()),
                )
            }
        };
        let accent = personal
            .appearance
            .accent
            .map(|canopa_theme::Rgb(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}"))
            .unwrap_or_default();
        (source, personal, accent, error)
    }

    fn load_idle(paths: &Paths) -> (Option<Snapshot>, IdleTimes) {
        let source = Snapshot::load(paths.idle()).ok();
        let idle = source
            .as_ref()
            .and_then(|s| IdleTimes::parse(&s.text).ok())
            .unwrap_or_default();
        (source, idle)
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
        let (input_source, input, input_error) = Self::load_input(&paths);
        let (common_source, personal, accent, personal_error) = Self::load_personal(&paths);
        let (idle_source, idle) = Self::load_idle(&paths);
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
            input_source,
            personal,
            common_source,
            accent,
            idle_source,
            idle_ac: idle.ac.map(|v| v.to_string()),
            idle_battery: idle.battery.map(|v| v.to_string()),
            profile: "none".into(),
            idle_status: "Connecting to the idle manager…".into(),
            idle_available: false,
            trial: None,
            remaining: 0,
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
            Message::Page(page) if !self.busy && self.trial.is_none() => {
                self.page = page;
                self.idle_available = false;
                if page == Page::Widgets && self.widget_layouts.editor.is_none() {
                    return widget_layouts::View::load(&self.paths);
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
                0 => self.input.layouts = value,
                1 => self.input.variants = value,
                _ => self.input.options = value,
            },
            Message::InputNumber(field, value) => match field {
                0 => self.input.repeat_delay = value as u16,
                1 => self.input.repeat_rate = value as u8,
                2 => self.input.mouse_speed = value,
                _ => self.input.touchpad_speed = value,
            },
            Message::InputFlag(field, value) => match field {
                0 => self.input.mouse_left_handed = value,
                1 => self.input.mouse_natural_scroll = value,
                2 => self.input.touchpad_tap = value,
                3 => self.input.touchpad_natural_scroll = value,
                _ => self.input.touchpad_disable_while_typing = value,
            },
            Message::PersonalText(field, value) => match field {
                0 => self.accent = value,
                1 => self.personal.lock.clock_format = value,
                2 => self.personal.lock.date_format = value,
                3 => self.personal.lock.font = value,
                _ => self.personal.lock.avatar = (!value.is_empty()).then(|| value.into()),
            },
            Message::Light(value) => {
                self.personal.appearance.color_scheme = if value {
                    canopa_theme::ColorScheme::Light
                } else {
                    canopa_theme::ColorScheme::Dark
                }
            }
            Message::TextScale(value) => self.personal.text_scale = value as u16,
            Message::ReducedMotion(value) => self.personal.reduced_motion = value,
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
                        self.shortcuts = None;
                        self.notice = "Compositor motion settings saved.".into();
                    }
                    Err(error) => self.notice = error,
                }
            }
            Message::Dim(value) => self.personal.lock.dim = value as u8,
            Message::IdleTime(battery, index, value) => {
                if battery {
                    self.idle_battery[index] = value;
                } else {
                    self.idle_ac[index] = value;
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
            Message::Save
                if !self.busy
                    && self.trial.is_none()
                    && (self.page != Page::Power || self.idle_available) =>
            {
                self.notice.clear();
                self.busy = true;
                let paths = self.paths.clone();
                match self.page {
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
                    Page::Input => {
                        let value = self.input.clone();
                        let source = self.input_source.clone();
                        return task(
                            move || {
                                let source = source.context("input configuration is read-only")?;
                                let saved = crate::input::apply(&paths, &source, &value)?;
                                Ok((Page::Input, saved))
                            },
                            Message::Stored,
                        );
                    }
                    Page::Personal => {
                        let mut value = self.personal.clone();
                        let source = self.common_source.clone();
                        let accent = self.accent.clone();
                        return task(
                            move || {
                                value.appearance.accent = parse_accent(&accent)?;
                                let saved =
                                    value.save(&source.context("configuration is read-only")?)?;
                                // The Settings portal owns its own explicit reload contract.
                                let _ = crate::storage::command(
                                    "systemctl",
                                    &[
                                        "--user".into(),
                                        "reload".into(),
                                        "xdg-desktop-portal-canopa.service".into(),
                                    ],
                                );
                                let _ = crate::storage::command(
                                    companion("canopa-notify"),
                                    &["ctl".into(), "reload".into()],
                                );
                                Ok((Page::Personal, saved))
                            },
                            Message::Stored,
                        );
                    }
                    Page::Power => {
                        let source = self.idle_source.clone();
                        let ac = self.idle_ac.clone();
                        let battery = self.idle_battery.clone();
                        return task(
                            move || {
                                let parse = |values: [String; 3]| -> Result<[u32; 3]> {
                                    Ok([values[0].parse()?, values[1].parse()?, values[2].parse()?])
                                };
                                let times = IdleTimes {
                                    ac: parse(ac)?,
                                    battery: parse(battery)?,
                                };
                                Ok((
                                    Page::Power,
                                    times.save(&source.context(
                                        "initialize Canopa idle policy before editing timeouts",
                                    )?)?,
                                ))
                            },
                            Message::Stored,
                        );
                    }
                }
            }
            Message::Stored(result) => {
                self.busy = false;
                match result {
                    Ok((page, snapshot)) => {
                        match page {
                            Page::Input => self.input_source = Some(snapshot),
                            Page::Personal => {
                                if let Ok(personal) = Personal::load(&snapshot) {
                                    self.personal = personal;
                                }
                                let previous = self.common_source.replace(snapshot.clone());
                                self.notice = "Settings saved.".into();
                                if !self.widget_layouts.dirty() {
                                    self.widget_layouts.editor = None;
                                } else if let (Some(previous), Some(Ok(editor))) =
                                    (previous, &mut self.widget_layouts.editor)
                                    && let Err(error) = editor.rebase(&previous, &snapshot)
                                {
                                    self.notice = format!("Settings saved. {error}");
                                }
                                return Task::none();
                            }
                            Page::Power => self.idle_source = Some(snapshot),
                            _ => {}
                        }
                        self.notice = "Settings saved.".into();
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
            _ => {}
        }
        Task::none()
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
                let (source, input, error) = Self::load_input(&paths);
                self.input_source = source;
                self.input = input;
                self.notice = error.unwrap_or_default();
                Task::none()
            }
            Page::Personal => {
                let (source, personal, accent, error) = Self::load_personal(&paths);
                self.common_source = source;
                self.personal = personal;
                self.accent = accent;
                self.compositor_motion = Self::load_motion(&paths);
                self.notice = error.unwrap_or_default();
                Task::none()
            }
            Page::Power => {
                let (source, idle) = Self::load_idle(&paths);
                self.idle_source = source;
                self.idle_ac = idle.ac.map(|v| v.to_string());
                self.idle_battery = idle.battery.map(|v| v.to_string());
                self.profile = "none".into();
                Task::none()
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
        }
        if self.trial.is_some() {
            subscriptions.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick));
        }
        Subscription::batch(subscriptions)
    }
    fn view(&self) -> Element<'_, Message> {
        let mut links = column![].spacing(5);
        for page in Page::ALL {
            links = links.push(button(
                page.label(),
                (!self.busy && self.trial.is_none()).then_some(Message::Page(page)),
                if page == self.page {
                    canopa_ui::Action::Normal
                } else {
                    canopa_ui::Action::Quiet
                },
                Length::Fill,
                self.busy,
            ));
        }
        let navigation = container(
            column![
                column![
                    text("Canopa").size(13).style(canopa_ui::muted),
                    heading("Settings", 24)
                ]
                .spacing(3)
                .padding([8, 8]),
                links
            ]
            .spacing(18),
        )
        .width(200)
        .height(Length::Fill)
        .padding(14)
        .style(|theme| container::Style::default().background(canopa_ui::colors(theme).surface));

        let description = match self.page {
            Page::Displays => "Manage connected monitors and save display layouts.",
            Page::Input => "Adjust keyboard, mouse, and touchpad behavior.",
            Page::Personal => "Choose how your desktop and lock screen look and feel.",
            Page::Power => "Set how your desktop responds when you step away.",
            Page::Launcher => "Choose search providers and manage clipboard history.",
            Page::Shortcuts => "Find and edit keyboard shortcuts for your desktop.",
            Page::Widgets => "Arrange widgets on your desktop and lock screen.",
        };
        let title = column![
            heading(self.page.label(), 28),
            text(description).size(14).style(canopa_ui::muted)
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
            Page::Personal => self.personal_view(),
            Page::Power => self.power_view(),
            Page::Launcher => self.launcher_view(),
            Page::Shortcuts => self.shortcuts_view(),
            Page::Widgets => self.widget_layouts_view(),
        });
        let available = (self.page != Page::Launcher || self.launcher.loaded)
            && !self.busy
            && self.trial.is_none()
            && (self.page != Page::Power || self.idle_available)
            && (self.page != Page::Shortcuts
                || (matches!(self.shortcuts, Some(Ok(_))) && self.paths.active()))
            && (self.page != Page::Widgets || self.widget_layouts.dirty())
            && (self.paths.active() || !matches!(self.page, Page::Displays | Page::Input));
        let footer = row![
            space().width(Length::Fill),
            self.action(
                RELOAD_PAGE,
                (!self.busy && self.trial.is_none()).then_some(Message::ReloadSaved),
                false
            ),
            button(
                if self.busy {
                    "Applying…"
                } else if self.page == Page::Displays {
                    "Test display changes"
                } else {
                    "Save changes"
                },
                available.then_some(Message::Save),
                canopa_ui::Action::Primary,
                Length::Shrink,
                self.busy,
            )
        ]
        .align_y(Alignment::Center)
        .spacing(12)
        .padding([10, 20]);
        let content = column![
            container(title).padding([19, 20]).width(Length::Fill),
            rule::horizontal(1).style(settings_rule),
            canopa_ui::scroll_speed(
                scrollable(container(body.max_width(800).padding([14, 20])).width(Length::Fill))
                    .id("settings-page-scroll")
                    .height(Length::Fill)
            ),
            rule::horizontal(1).style(settings_rule),
            footer
        ]
        .width(Length::Fill);
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
                format!("settings-button-{}", self.page.label())
            },
            self.busy,
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
                &self.input.layouts,
                |v| Message::InputText(0, v)
            ),
            field(
                "Variants (optional, in the same order)",
                &self.input.variants,
                |v| Message::InputText(1, v)
            ),
            field(
                "XKB options (for example grp:alt_shift_toggle)",
                &self.input.options,
                |v| Message::InputText(2, v)
            ),
            text(format!("Repeat delay: {} ms", self.input.repeat_delay)),
            adjust(
                "repeat-delay",
                100.0..=2000.,
                f64::from(self.input.repeat_delay),
                50.,
                |v| Message::InputNumber(0, v)
            ),
            text(format!(
                "Repeat rate: {} keys/second",
                self.input.repeat_rate
            )),
            adjust(
                "repeat-rate",
                1.0..=100.,
                f64::from(self.input.repeat_rate),
                1.,
                |v| Message::InputNumber(1, v)
            ),
        ]
        .spacing(10);
        let mouse = column![
            text(format!("Pointer speed: {:.2}", self.input.mouse_speed)),
            adjust(
                "mouse-speed",
                -1.0..=1.,
                self.input.mouse_speed,
                0.05,
                |v| Message::InputNumber(2, v)
            ),
            toggle(
                "input-flag-0",
                "Left-handed buttons",
                self.input.mouse_left_handed,
                Message::InputFlag(0, !self.input.mouse_left_handed)
            ),
            toggle(
                "input-flag-1",
                "Natural scrolling for mouse",
                self.input.mouse_natural_scroll,
                Message::InputFlag(1, !self.input.mouse_natural_scroll)
            ),
        ]
        .spacing(10);
        let touchpad = column![
            text(format!("Touchpad speed: {:.2}", self.input.touchpad_speed)),
            adjust(
                "touchpad-speed",
                -1.0..=1.,
                self.input.touchpad_speed,
                0.05,
                |v| Message::InputNumber(3, v)
            ),
            toggle(
                "input-flag-2",
                "Tap to click",
                self.input.touchpad_tap,
                Message::InputFlag(2, !self.input.touchpad_tap)
            ),
            toggle(
                "input-flag-3",
                "Natural scrolling for touchpad",
                self.input.touchpad_natural_scroll,
                Message::InputFlag(3, !self.input.touchpad_natural_scroll)
            ),
            toggle(
                "input-flag-4",
                "Disable touchpad while typing",
                self.input.touchpad_disable_while_typing,
                Message::InputFlag(4, !self.input.touchpad_disable_while_typing)
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
                self.personal.appearance.color_scheme == canopa_theme::ColorScheme::Light,
                Message::Light(
                    self.personal.appearance.color_scheme != canopa_theme::ColorScheme::Light
                )
            ),
            container(field("Accent color (optional, #RRGGBB)", &self.accent, |v| {
                Message::PersonalText(0, v)
            })).max_width(300),
            text(format!("Text size · {}%", self.personal.text_scale)),
            adjust(
                "settings-text-scale",
                100.0..=150.0,
                f64::from(self.personal.text_scale),
                5.0,
                Message::TextScale
            ),
            toggle(
                "settings-reduced-motion",
                "Reduce Shell panel motion",
                self.personal.reduced_motion,
                Message::ReducedMotion(!self.personal.reduced_motion)
            ),
            self.action("Choose wallpaper…", Some(Message::Wallpaper), true),
            text("Preview and save wallpaper for each monitor. The default image folder is ~/Pictures/Wallpapers.")
                .size(14).style(canopa_ui::muted),
        ].spacing(12);
        let lockscreen = column![
            field("Clock format", &self.personal.lock.clock_format, |v| {
                Message::PersonalText(1, v)
            }),
            field("Date format", &self.personal.lock.date_format, |v| {
                Message::PersonalText(2, v)
            }),
            field("Font family", &self.personal.lock.font, |v| {
                Message::PersonalText(3, v)
            }),
            field(
                "Avatar image (optional absolute path)",
                &self
                    .personal
                    .lock
                    .avatar
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                |v| Message::PersonalText(4, v)
            ),
            text(format!("Background dimming: {}%", self.personal.lock.dim)),
            adjust(
                "lock-dim",
                0.0..=100.,
                f64::from(self.personal.lock.dim),
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
    fn power_view(&self) -> Element<'_, Message> {
        let mut ac = column![].spacing(10);
        let mut battery = column![].spacing(10);
        for (index, label) in [
            "Lock after (seconds)",
            "After lock: displays off (seconds)",
            "After displays off: suspend (seconds)",
        ]
        .into_iter()
        .enumerate()
        {
            ac = ac.push(field_id(
                format!("idle-ac-{index}"),
                label,
                &self.idle_ac[index],
                move |v| Message::IdleTime(false, index, v),
            ));
            battery = battery.push(field_id(
                format!("idle-battery-{index}"),
                label,
                &self.idle_battery[index],
                move |v| Message::IdleTime(true, index, v),
            ));
        }
        let activity = column![
            text(&self.idle_status).style(canopa_ui::muted),
            row![
                self.action(
                    "Pause idle actions",
                    (!self.busy && self.idle_available).then_some(Message::IdleAction("pause")),
                    false
                ),
                self.action(
                    "Resume",
                    (!self.busy && self.idle_available).then_some(Message::IdleAction("resume")),
                    false
                )
            ]
            .spacing(12),
            row![
                field("Stasis profile", &self.profile, Message::Profile),
                self.action(
                    "Use profile",
                    (!self.busy && self.idle_available && !self.profile.trim().is_empty())
                        .then_some(Message::UseProfile),
                    false
                )
            ]
            .spacing(12)
            .align_y(Alignment::End),
        ]
        .spacing(12);
        column![
            settings_group("Idle controls", activity),
            text("Timeouts run in order: lock, turn off displays, then suspend.")
                .size(14)
                .style(canopa_ui::muted),
            row![
                settings_group("On AC power", ac),
                settings_group("On battery", battery)
            ]
            .spacing(12),
            text(
                "Custom commands, profiles and inhibition rules stay in your existing idle policy."
            )
            .size(14)
            .style(canopa_ui::muted),
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
fn parse_accent(value: &str) -> Result<Option<canopa_theme::Rgb>> {
    if value.is_empty() {
        return Ok(None);
    }
    ensure!(
        value.len() == 7
            && value.starts_with('#')
            && value[1..].bytes().all(|v| v.is_ascii_hexdigit()),
        "accent must be #RRGGBB"
    );
    Ok(Some(canopa_theme::Rgb(
        u8::from_str_radix(&value[1..3], 16)?,
        u8::from_str_radix(&value[3..5], 16)?,
        u8::from_str_radix(&value[5..7], 16)?,
    )))
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
pub fn run() -> Result<()> {
    run_page(Page::Displays)
}
pub fn run_page(page: Page) -> Result<()> {
    run_with_profile(page, None)
}
pub fn run_with_profile(page: Page, profile: Option<String>) -> Result<()> {
    iced::application(
        move || {
            let (mut state, task) = State::boot();
            state.display_profiles.selected.clone_from(&profile);
            state.display_profiles.prominent = profile.is_some();
            let page_task = state.update(Message::Page(page));
            (state, Task::batch([task, page_task]))
        },
        State::update,
        State::view,
    )
    .title("Canopa Settings")
    .theme(|state: &State| canopa_ui::theme(state.personal.appearance.clone()))
    .scale_factor(|state: &State| f32::from(state.personal.appearance.text_scale()) / 100.0)
    .subscription(State::subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(1000., 780.),
        min_size: Some(iced::Size::new(760., 600.)),
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "org.canopa.Settings".into(),
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
        let source = state.common_source.clone().unwrap();
        state.personal.appearance.accent = Some(canopa_theme::Rgb(0x12, 0x34, 0x56));
        let saved = source
            .save(&state.personal.text(&source).unwrap(), |_| Ok(()))
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
        assert_eq!(state.input.layouts, Input::default().layouts);
        assert_eq!(state.accent, "#123456");

        let _ = state.update(Message::InputText(0, "de".into()));
        let _ = state.update(Message::Page(Page::Personal));
        let _ = state.update(Message::ReloadSaved);
        assert_eq!(state.accent, "");
        assert_eq!(state.input.layouts, "de");
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
        assert!(state.common_source.as_ref().unwrap().same(&after));
    }

    #[test]
    fn saving_widgets_keeps_pending_personalization_edits() {
        let (_root, mut state) = fresh();
        let _ = state.update(Message::PersonalText(0, "#abcdef".into()));
        open_widgets(&mut state);
        let saved = save_widgets(&mut state);
        let common = state.common_source.clone().unwrap();
        assert!(common.same(&saved));
        assert_eq!(state.accent, "#abcdef");
        assert!(common.unchanged().is_ok());
        assert!(state.personal.text(&common).unwrap().contains("bottom"));
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
        let common = state.common_source.clone().unwrap();
        assert!(!common.same(&saved));
        assert!(common.unchanged().is_err());

        // Nor does a Personalization save rebase widget edits made on that version.
        let (_root, mut state) = fresh();
        let previous = state.common_source.clone().unwrap();
        std::fs::write(state.paths.common(), "schema-version 1\n// external\n").unwrap();
        open_widgets(&mut state);
        let saved = previous
            .save(&state.personal.text(&previous).unwrap(), |_| Ok(()))
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
