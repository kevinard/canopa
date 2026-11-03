//! Ordinary settings window with shared controls and bounded background work.
mod focus;
mod launcher;
mod shortcuts;
use crate::{
    display::{Monitor, TrialRequest, transform_name},
    input::Input,
    niri::Client,
    preferences::{IdleTimes, Personal},
    session::Paths,
    storage::{Snapshot, companion},
};
use anyhow::{Context, Result, ensure};
use iced::{
    Element, Length, Subscription, Task,
    widget::{checkbox, column, container, pick_list, row, scrollable, slider, text, text_input},
};
use niri_ipc::{Output, Transform};
use std::os::unix::process::CommandExt;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::Page;
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
    Widget(String, bool),
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
#[derive(Debug)]
struct Trial {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Drop for Trial {
    fn drop(&mut self) {
        let _ = self.input.write_all(b"revert\n");
        let _ = self.input.flush();
        let _ = self.child.wait();
    }
}
#[derive(Clone, Debug)]
struct TrialHandle(Arc<Mutex<Trial>>);
impl TrialHandle {
    fn start(paths: &Paths, monitors: Vec<Monitor>) -> Result<Self> {
        let mut child = Command::new(std::env::current_exe()?)
            .arg("display-guard")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .process_group(0)
            .spawn()?;
        let input = child.stdin.take().context("missing display helper input")?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .context("missing display helper output")?,
        );
        let mut trial = Trial {
            child,
            input,
            output,
        };
        serde_json::to_writer(
            &mut trial.input,
            &TrialRequest {
                directory: paths.directory.clone(),
                monitors,
            },
        )?;
        trial.input.write_all(b"\n")?;
        trial.input.flush()?;
        let reply = frame(&mut trial.output)?;
        ensure!(
            reply["state"] == "ready",
            "{}",
            reply["message"]
                .as_str()
                .unwrap_or("display helper did not become ready")
        );
        Ok(Self(Arc::new(Mutex::new(trial))))
    }
    fn finish(self, keep: bool) -> Result<String> {
        let mut trial = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("display helper lock failed"))?;
        // The independent helper may already have reverted on its own deadline.
        let _ = trial
            .input
            .write_all(if keep { b"keep\n" } else { b"revert\n" });
        let reply = frame(&mut trial.output)?;
        let _ = trial.child.wait();
        match reply["state"].as_str() {
            Some("kept") => Ok("Display settings saved.".into()),
            Some("reverted") => Ok("Previous display settings restored.".into()),
            _ => anyhow::bail!(
                "{}",
                reply["message"].as_str().unwrap_or("display change failed")
            ),
        }
    }
}
fn frame(reader: &mut BufReader<ChildStdout>) -> Result<serde_json::Value> {
    let mut line = String::new();
    reader.take(16385).read_line(&mut line)?;
    ensure!(
        !line.is_empty() && line.len() <= 16384,
        "display helper stopped unexpectedly"
    );
    Ok(serde_json::from_str(&line)?)
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
        button(label, message, active, self.busy)
    }

    fn boot() -> (Self, Task<Message>) {
        let paths = Paths::discover().expect("settings require a configuration home");
        let input_source = Snapshot::load(paths.input()).ok();
        let common_source = Snapshot::load(paths.common()).ok();
        let idle_source = Snapshot::load(paths.idle()).ok();
        let compositor_motion = paths
            .active()
            .then(|| {
                Snapshot::load(paths.niri())
                    .and_then(|source| crate::niri::reduced_motion(&source))
                    .ok()
            })
            .flatten();
        let (lock, _) = canopa_session_ui::settings::Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .expect("embedded lock settings are valid");
        let mut notice = String::new();
        let input = input_source
            .as_ref()
            .map(|s| Input::parse(&s.text))
            .transpose()
            .unwrap_or_else(|error| {
                notice = error.to_string();
                None
            })
            .unwrap_or_default();
        let personal = common_source
            .as_ref()
            .map(Personal::load)
            .transpose()
            .unwrap_or_else(|error| {
                notice = error.to_string();
                None
            })
            .unwrap_or_else(|| Personal::from_lock(lock));
        let idle = idle_source
            .as_ref()
            .and_then(|s| IdleTimes::parse(&s.text).ok())
            .unwrap_or_default();
        let accent = personal
            .appearance
            .accent
            .map(|canopa_theme::Rgb(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}"))
            .unwrap_or_default();
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
                    | Message::Widget(..)
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
                let page = self.page;
                let (mut reloaded, work) = Self::boot();
                let page_work = reloaded.update(Message::Page(page));
                *self = reloaded;
                return Task::batch([work, page_work]);
            }
            Message::BackendChanged if !self.busy && !self.refreshing && self.trial.is_none() => {
                self.refreshing = true;
                return task(|| Client::discover()?.outputs(), Message::Outputs);
            }
            Message::Page(page) if !self.busy && self.trial.is_none() => {
                self.page = page;
                self.idle_available = false;
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
            Message::Widget(name, value) => {
                self.personal.lock.widgets.retain(|v| v != &name);
                if value {
                    self.personal.lock.widgets.push(name);
                }
            }
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
                    Page::Shortcuts => {
                        let Some(Ok(editor)) = self.shortcuts.clone() else {
                            self.busy = false;
                            return Task::none();
                        };
                        return task(move || editor.save(&paths), Message::ShortcutsStored);
                    }
                    Page::Displays => {
                        let mut monitors = self.monitors.clone();
                        for (monitor, position) in monitors.iter_mut().zip(&self.positions) {
                            let parsed = position
                                .0
                                .parse::<i32>()
                                .and_then(|x| position.1.parse::<i32>().map(|y| (x, y)));
                            match parsed {
                                Ok((x, y)) => {
                                    monitor.x = x;
                                    monitor.y = y;
                                }
                                Err(_) => {
                                    self.busy = false;
                                    self.notice = "Display positions must be whole numbers.".into();
                                    return Task::none();
                                }
                            }
                        }
                        return task(
                            move || TrialHandle::start(&paths, monitors),
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
                                self.common_source = Some(snapshot);
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
    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();
        if self.page == Page::Launcher {
            subscriptions
                .push(iced::time::every(Duration::from_secs(2)).map(|_| Message::LauncherRefresh));
        }
        if self.page == Page::Displays || self.trial.is_some() {
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
        let mut navigation = column![text("CANOPA").size(16), text("Settings").size(28)]
            .spacing(12)
            .width(210);
        for page in Page::ALL {
            navigation = navigation.push(self.action(
                page.label(),
                (!self.busy && self.trial.is_none()).then_some(Message::Page(page)),
                page == self.page,
            ));
        }
        // Keep this child present so asynchronous notices do not replace the
        // page's widget tree and discard keyboard focus or typed drafts.
        let mut body = column![
            text(self.page.label()).size(28),
            container(text(&self.notice))
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
            body = body.push(text(
                "Log into the Canopa desktop session to save display and input settings.",
            ));
        }
        body = body.push(match self.page {
            Page::Displays => self.displays(),
            Page::Input => self.input_view(),
            Page::Personal => self.personal_view(),
            Page::Power => self.power_view(),
            Page::Launcher => self.launcher_view(),
            Page::Shortcuts => self.shortcuts_view(),
        });
        let available = (self.page != Page::Launcher || self.launcher.loaded)
            && !self.busy
            && self.trial.is_none()
            && (self.page != Page::Power || self.idle_available)
            && (self.page != Page::Shortcuts
                || (matches!(self.shortcuts, Some(Ok(_))) && self.paths.active()))
            && (self.paths.active() || !matches!(self.page, Page::Displays | Page::Input));
        let footer = row![
            self.action(
                if self.busy {
                    "Applying…"
                } else if self.page == Page::Displays {
                    "Test display changes"
                } else {
                    "Save changes"
                },
                available.then_some(Message::Save),
                true
            ),
            self.action(
                "Reload and discard edits",
                (!self.busy && self.trial.is_none()).then_some(Message::ReloadSaved),
                false
            )
        ]
        .spacing(16);
        let content = column![
            scrollable(body.padding(4))
                .id("settings-page-scroll")
                .height(Length::Fill),
            footer
        ]
        .spacing(18)
        .width(Length::Fill);
        focus::group(
            container(row![navigation, content].spacing(28)).padding(24),
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
        body.into()
    }
    fn input_view(&self) -> Element<'_, Message> {
        column![
            text("Keyboard").size(22),
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
            text("Mouse").size(22),
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
            text("Touchpad").size(22),
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
        .spacing(12)
        .into()
    }
    fn personal_view(&self) -> Element<'_, Message> {
        let mut widgets = row![].spacing(16);
        for name in ["keyboard", "battery", "media", "username"] {
            widgets = widgets.push(toggle(
                format!("lock-widget-{name}"),
                name,
                self.personal.lock.widgets.iter().any(|w| w == name),
                Message::Widget(
                    name.into(),
                    !self.personal.lock.widgets.iter().any(|w| w == name),
                ),
            ));
        }
        let content = column![
            text("Appearance").size(22),
            toggle(
                "light",
                "Light appearance",
                self.personal.appearance.color_scheme == canopa_theme::ColorScheme::Light,
                Message::Light(
                    self.personal.appearance.color_scheme != canopa_theme::ColorScheme::Light
                )
            ),
            field("Accent color (optional, #RRGGBB)", &self.accent, |v| {
                Message::PersonalText(0, v)
            }),
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
            self.action("Choose wallpaper…", Some(Message::Wallpaper), false),
            text("The wallpaper picker previews and saves per-monitor assignments."),
            text("Add images to ~/Pictures/Wallpapers when no wallpaper folder is configured."),
            text("Lockscreen").size(22),
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
            text("Lockscreen widgets"),
            widgets
        ];
        let mut content = content.push(text("Window and workspace motion").size(22));
        if let Some(reduced) = self.compositor_motion {
            content = content.push(self.action(
                if reduced {
                    "Use Niri animation settings"
                } else {
                    "Reduce window and workspace motion"
                },
                Some(Message::CompositorMotion(!reduced)),
                false,
            ));
        } else {
            content = content.push(text("Compositor motion is editable in a Canopa session with a managed Niri configuration."));
        }
        content.spacing(12).into()
    }
    fn power_view(&self) -> Element<'_, Message> {
        let mut ac = column![text("On AC power").size(20)].spacing(12);
        let mut battery = column![text("On battery").size(20)].spacing(12);
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
        column![
            text(&self.idle_status),
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
            .spacing(12),
            text(
                "Timeouts are sequential: lock, wait before displays off, then wait before suspend."
            ),
            row![ac, battery].spacing(24),
            text(
                "Custom commands, profiles and inhibition rules stay in your existing idle policy."
            )
        ]
        .spacing(16)
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
fn button<'a>(
    label: &'a str,
    message: Option<Message>,
    active: bool,
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
        canopa_ui::action(
            text(label),
            message,
            if active {
                canopa_ui::Action::Normal
            } else {
                canopa_ui::Action::Quiet
            },
            false,
        ),
    )
}
fn toggle<'a>(
    id: impl Into<String>,
    label: &'a str,
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
            .label(label)
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
    iced::application(
        move || {
            let (mut state, task) = State::boot();
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
