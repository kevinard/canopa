//! Taskbar: pinned applications first, then the other running ones, as app
//! icons. A click focuses an application's window, cycles through its windows
//! when it is already focused, or launches it when nothing is open. A right
//! click opens its menu: a new window, the desktop entry's actions, and Pin or
//! Unpin. Niri owns the windows; the Shell only groups them by desktop
//! application.
use std::collections::BTreeMap;

use crate::desktop_apps::DesktopApplication;
use crate::focused_window_icons::ApplicationIcon;
use crate::modules::{InteractionEvent, ModuleId, ModuleKind, ModuleState};

/// One application on the taskbar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TaskbarEntry {
    /// The desktop ID, or the window's app ID when no application matches.
    pub key: String,
    pub name: String,
    pub icon: Option<ApplicationIcon>,
    /// Set when the entry can be launched from its desktop file.
    pub desktop_id: Option<String>,
    pub pinned: bool,
    /// Open windows, most recently focused first.
    pub windows: Vec<u64>,
    /// The same windows in screen order: by output, workspace, column and
    /// tile, floating windows after their workspace's columns. Clicks cycle
    /// through them in this order.
    pub layout_order: Vec<u64>,
    pub focused: bool,
    pub urgent: bool,
}

#[derive(Debug, Clone)]
pub enum TaskbarEvent {
    /// Focus the application, cycle its windows, or launch it.
    Activate { key: String },
    /// Start another instance.
    Launch { key: String },
    /// Open the application's menu beside its icon on `window`.
    Menu {
        key: String,
        module: ModuleId,
        window: iced::window::Id,
    },
    /// The icon's bounds were measured; show the menu there.
    MenuAt {
        key: String,
        window: iced::window::Id,
        bounds: Option<iced::Rectangle>,
    },
    /// Window thumbnails arrived from Niri: window ID, width, height, PNG.
    Previews(Vec<(u64, u32, u32, Vec<u8>)>),
}

/// The menu service of taskbar menus. It cannot be a D-Bus bus name, so a
/// taskbar menu is never confused with a tray item's DBusMenu.
pub(crate) const MENU_SERVICE: &str = "canopa-shell/taskbar";
/// The object path the menu popup records for a taskbar menu.
pub(crate) const MENU_PATH: &str = "/taskbar";

/// One command in an application's taskbar menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MenuCommand {
    /// Start the application: "Open" when nothing runs, otherwise "New window".
    Launch,
    /// Run one of the desktop entry's `[Desktop Action]` groups.
    DesktopAction(String),
    Pin,
    Unpin,
    /// Layout commands for the application's current window.
    Layout(LayoutCommand),
}

/// Niri layout commands offered for an application's current window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutCommand {
    /// Set its column's width to this percentage of the working area.
    Width(u8),
    ToggleTabs,
    Center,
    ToggleFloating,
    MoveToWorkspace(u64),
    /// Keep the column at an edge of the view while the strip scrolls.
    Pin(niri_ipc::PinSide),
    Unpin,
}

/// What the layout commands need to know about the window they act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayoutTarget {
    pub floating: bool,
    /// The edge the window's column is pinned to.
    pub pinned: Option<niri_ipc::PinSide>,
    /// The workspaces above and below the window's, on its output.
    pub above: Option<u64>,
    pub below: Option<u64>,
}

impl LayoutTarget {
    pub(crate) fn of(window: &niri_ipc::Window, workspaces: &[niri_ipc::Workspace]) -> Self {
        let current = window
            .workspace_id
            .and_then(|id| workspaces.iter().find(|workspace| workspace.id == id));
        let neighbour = |offset: i16| {
            let current = current?;
            let index = i16::from(current.idx) + offset;
            workspaces
                .iter()
                .find(|workspace| {
                    workspace.output == current.output && i16::from(workspace.idx) == index
                })
                .map(|workspace| workspace.id)
        };
        Self {
            floating: window.is_floating,
            pinned: window.layout.pinned,
            above: neighbour(-1),
            below: neighbour(1),
        }
    }
}

/// The Niri actions that run `command` on `window`. Column commands act on
/// the focused column, so they focus the window first.
pub(crate) fn layout_actions(command: LayoutCommand, window: u64) -> Vec<niri_ipc::Action> {
    use niri_ipc::Action;
    let focus = Action::FocusWindow { id: window };
    match command {
        LayoutCommand::Width(percent) => vec![
            focus,
            Action::SetColumnWidth {
                change: niri_ipc::SizeChange::SetProportion(f64::from(percent)),
            },
        ],
        LayoutCommand::ToggleTabs => vec![focus, Action::ToggleColumnTabbedDisplay {}],
        LayoutCommand::Center => vec![focus, Action::CenterColumn {}],
        LayoutCommand::ToggleFloating => vec![Action::ToggleWindowFloating { id: Some(window) }],
        LayoutCommand::Pin(side) => vec![focus, Action::PinColumn { side }],
        LayoutCommand::Unpin => vec![focus, Action::UnpinColumn {}],
        LayoutCommand::MoveToWorkspace(workspace) => vec![Action::MoveWindowToWorkspace {
            window_id: Some(window),
            reference: niri_ipc::WorkspaceReferenceArg::Id(workspace),
            focus: false,
        }],
    }
}

/// A row of the menu; `None` is a separator.
pub(crate) type MenuRow = Option<(String, MenuCommand)>;

/// The menu of `entry`: Open or New window, the desktop entry's actions, the
/// layout commands for its current window when it runs, then Pin or Unpin. A
/// running application that declares a single main window gets no New
/// window. A desktop `new-window` action stands in for a plain launch, which
/// a D-Bus-activated application would answer by raising its existing window.
pub(crate) fn menu(
    entry: &TaskbarEntry,
    application: Option<&DesktopApplication>,
    target: Option<LayoutTarget>,
) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    let Some(app) = application else {
        return rows;
    };
    let running = !entry.windows.is_empty();
    let mut actions: Vec<_> = app.actions.iter().collect();
    if !running {
        rows.push(Some((fl!("shell-taskbar-open"), MenuCommand::Launch)));
    } else if !app.single_main_window {
        let command = match actions.iter().position(|action| action.id == "new-window") {
            Some(position) => MenuCommand::DesktopAction(actions.remove(position).id.clone()),
            None => MenuCommand::Launch,
        };
        rows.push(Some((fl!("shell-taskbar-new-window"), command)));
    }
    if !actions.is_empty() {
        if !rows.is_empty() {
            rows.push(None);
        }
        rows.extend(actions.into_iter().map(|action| {
            Some((
                action.name.clone(),
                MenuCommand::DesktopAction(action.id.clone()),
            ))
        }));
    }
    if let Some(target) = target {
        if !rows.is_empty() {
            rows.push(None);
        }
        rows.extend(layout_rows(target));
    }
    if !rows.is_empty() {
        rows.push(None);
    }
    rows.push(Some(if entry.pinned {
        (fl!("shell-taskbar-unpin"), MenuCommand::Unpin)
    } else {
        (fl!("shell-taskbar-pin"), MenuCommand::Pin)
    }));
    rows
}

fn layout_rows(target: LayoutTarget) -> Vec<MenuRow> {
    let row = |label: String, command| Some((label, MenuCommand::Layout(command)));
    let mut rows = Vec::new();
    if !target.floating {
        rows.extend([
            row(fl!("shell-taskbar-width-third"), LayoutCommand::Width(33)),
            row(fl!("shell-taskbar-width-half"), LayoutCommand::Width(50)),
            row(
                fl!("shell-taskbar-width-two-thirds"),
                LayoutCommand::Width(67),
            ),
            row(fl!("shell-taskbar-toggle-tabs"), LayoutCommand::ToggleTabs),
            row(fl!("shell-taskbar-center"), LayoutCommand::Center),
        ]);
        match target.pinned {
            Some(_) => rows.push(row(fl!("shell-taskbar-unpin-column"), LayoutCommand::Unpin)),
            None => rows.extend([
                row(
                    fl!("shell-taskbar-pin-left"),
                    LayoutCommand::Pin(niri_ipc::PinSide::Left),
                ),
                row(
                    fl!("shell-taskbar-pin-right"),
                    LayoutCommand::Pin(niri_ipc::PinSide::Right),
                ),
            ]),
        }
    }
    rows.push(row(
        if target.floating {
            fl!("shell-taskbar-tile")
        } else {
            fl!("shell-taskbar-float")
        },
        LayoutCommand::ToggleFloating,
    ));
    if let Some(workspace) = target.above {
        rows.push(row(
            fl!("shell-taskbar-move-up"),
            LayoutCommand::MoveToWorkspace(workspace),
        ));
    }
    if let Some(workspace) = target.below {
        rows.push(row(
            fl!("shell-taskbar-move-down"),
            LayoutCommand::MoveToWorkspace(workspace),
        ));
    }
    rows
}

/// The menu popup's tree for `rows`. Item ids are the row positions, so a
/// selection maps back to its command.
pub(crate) fn menu_tree(
    rows: &[MenuRow],
    anchor: (i32, i32),
    screen_size: (u32, u32),
) -> crate::surface::menu_popup::MenuTree {
    use crate::surface::menu_popup::{MenuItem, MenuTree};
    MenuTree {
        items: rows
            .iter()
            .enumerate()
            .map(|(position, row)| MenuItem {
                id: position.to_string(),
                label: row
                    .as_ref()
                    .map(|(label, _)| label.clone())
                    .unwrap_or_default(),
                enabled: row.is_some(),
                visible: true,
                icon_name: None,
                children: Vec::new(),
                is_separator: row.is_none(),
                toggle: None,
            })
            .collect(),
        anchor,
        screen_size,
    }
}

/// Pins changed from the bar, layered over a taskbar's configured `pinned`
/// list: configured applications the person unpinned, and others they pinned.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinEdits {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pinned: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unpinned: Vec<String>,
}

impl PinEdits {
    /// The pinned applications: the configured ones still pinned, in their
    /// order, then the ones pinned from the bar, in pinning order.
    pub(crate) fn apply(&self, configured: &[String]) -> Vec<String> {
        configured
            .iter()
            .filter(|id| !self.unpinned.contains(id))
            .chain(self.pinned.iter().filter(|id| !configured.contains(id)))
            .cloned()
            .collect()
    }

    /// Record pinning or unpinning `id`, keeping only what differs from the
    /// configuration.
    pub(crate) fn set(&mut self, configured: &[String], id: &str, pinned: bool) {
        self.pinned.retain(|other| other != id);
        self.unpinned.retain(|other| other != id);
        match (pinned, configured.iter().any(|other| other == id)) {
            (true, false) => self.pinned.push(id.to_owned()),
            (false, true) => self.unpinned.push(id.to_owned()),
            _ => {}
        }
    }

    fn is_empty(&self) -> bool {
        self.pinned.is_empty() && self.unpinned.is_empty()
    }
}

/// The saved pin edits of every taskbar, by its configured module name.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinStore {
    pub version: u32,
    #[serde(default)]
    pub taskbars: BTreeMap<String, PinEdits>,
}

/// Pin files are a few desktop IDs; anything larger is not ours.
pub(crate) const PIN_STORE_MAX_BYTES: usize = 64 * 1024;

impl PinStore {
    pub(crate) fn edits(&self, taskbar: &str) -> Option<&PinEdits> {
        self.taskbars.get(taskbar)
    }

    pub(crate) fn set(&mut self, taskbar: &str, configured: &[String], id: &str, pinned: bool) {
        let edits = self.taskbars.entry(taskbar.to_owned()).or_default();
        edits.set(configured, id, pinned);
        if edits.is_empty() {
            self.taskbars.remove(taskbar);
        }
        self.version = 1;
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, String> {
        let store: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if store.version != 1 {
            return Err(format!("unsupported taskbar pin version {}", store.version));
        }
        Ok(store)
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(&Self {
            version: 1,
            taskbars: self.taskbars.clone(),
        })
        .expect("taskbar pins serialize");
        bytes.push(b'\n');
        bytes
    }
}

/// Where `window` sits on screen, for ordering: windows on a workspace come
/// first, by output name, workspace index, column and tile; floating windows
/// follow the columns of their workspace.
fn layout_key<'a>(
    window: &'a niri_ipc::Window,
    workspaces: &'a [niri_ipc::Workspace],
) -> (bool, &'a str, u8, usize, usize, u64) {
    let workspace = window
        .workspace_id
        .and_then(|id| workspaces.iter().find(|workspace| workspace.id == id));
    let (column, tile) = window
        .layout
        .pos_in_scrolling_layout
        .unwrap_or((usize::MAX, usize::MAX));
    (
        workspace.is_none(),
        workspace
            .and_then(|workspace| workspace.output.as_deref())
            .unwrap_or(""),
        workspace.map_or(u8::MAX, |workspace| workspace.idx),
        column,
        tile,
        window.id,
    )
}

/// Groups `windows` by application. Pinned applications keep their configured
/// order; others follow in screen order of their leftmost window, so the
/// taskbar reads like the strip.
pub(crate) fn entries(
    pinned: &[String],
    windows: &[&niri_ipc::Window],
    workspaces: &[niri_ipc::Workspace],
    focused_window: Option<u64>,
    applications: &[DesktopApplication],
) -> Vec<TaskbarEntry> {
    let mut entries: Vec<TaskbarEntry> = pinned
        .iter()
        .filter_map(|id| applications.iter().find(|app| &app.id == id))
        .map(|app| entry_for(app.id.clone(), Some(app), true))
        .collect();

    let mut ordered: Vec<_> = windows.to_vec();
    ordered.sort_by_key(|window| layout_key(window, workspaces));
    let app_key = |window: &niri_ipc::Window| {
        window
            .app_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(|app_id| {
                crate::focused_window_icons::match_desktop_application_for_app_id(
                    app_id,
                    applications,
                )
                .map_or_else(|| app_id.to_owned(), |app| app.id.clone())
            })
    };
    let mut recency: Vec<(u64, (u64, u32))> = Vec::new();
    for window in ordered {
        let Some(app_id) = window.app_id.as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        let application =
            crate::focused_window_icons::match_desktop_application_for_app_id(app_id, applications);
        let key = application.map_or_else(|| app_id.to_owned(), |app| app.id.clone());
        // A dialog of another of the application's windows is not a window
        // of its own: it neither counts nor takes a turn when clicks cycle.
        let dialog = window.parent_id.is_some_and(|parent| {
            windows
                .iter()
                .find(|other| other.id == parent)
                .is_some_and(|parent| app_key(parent).as_ref() == Some(&key))
        });
        let index = match entries.iter().position(|entry| entry.key == key) {
            Some(index) => index,
            None => {
                entries.push(entry_for(key, application, false));
                entries.len() - 1
            }
        };
        let entry = &mut entries[index];
        entry.focused |= focused_window == Some(window.id);
        entry.urgent |= window.is_urgent;
        if dialog {
            continue;
        }
        entry.windows.push(window.id);
        entry.layout_order.push(window.id);
        recency.push((
            window.id,
            window
                .focus_timestamp
                .map_or((0, 0), |stamp| (stamp.secs, stamp.nanos)),
        ));
    }
    for entry in &mut entries {
        entry.windows.sort_by_key(|id| {
            let stamp = recency
                .iter()
                .find(|(window, _)| window == id)
                .map_or((0, 0), |(_, stamp)| *stamp);
            (std::cmp::Reverse(stamp), *id)
        });
    }
    entries
}

fn entry_for(key: String, application: Option<&DesktopApplication>, pinned: bool) -> TaskbarEntry {
    TaskbarEntry {
        // "org.example.Notes" reads as "Notes" when no desktop file names it.
        name: application.map_or_else(
            || key.rsplit('.').next().unwrap_or(&key).to_owned(),
            |app| app.name.clone(),
        ),
        desktop_id: application.map(|app| app.id.clone()),
        key,
        // Resolved by the caller, which caches icon theme lookups.
        icon: None,
        pinned,
        windows: Vec::new(),
        layout_order: Vec::new(),
        focused: false,
        urgent: false,
    }
}

/// The window a click focuses: the next of the application's windows when
/// one of them is focused, otherwise its most recently focused one. Clicking
/// the bar moves keyboard focus to it, so callers pass the window that is in
/// front, not only Niri's keyboard-focused one.
pub(crate) fn window_to_focus(entry: &TaskbarEntry, focused_window: Option<u64>) -> Option<u64> {
    if entry.windows.len() > 1
        && let Some(focused) = focused_window
        && entry.windows.contains(&focused)
    {
        // Cycle in screen order so repeated clicks visit every window, left
        // to right.
        let stable = if entry.layout_order.len() == entry.windows.len() {
            entry.layout_order.clone()
        } else {
            let mut stable = entry.windows.clone();
            stable.sort_unstable();
            stable
        };
        let next = stable
            .iter()
            .position(|id| *id == focused)
            .map(|i| (i + 1) % stable.len());
        return next.map(|i| stable[i]);
    }
    entry.windows.first().copied()
}

/// Config-level placeholder, replaced at runtime by one `TaskbarItemModule`
/// per application.
pub(crate) struct TaskbarModule {
    id: ModuleId,
}

impl TaskbarModule {
    pub(crate) fn new(id: ModuleId) -> Self {
        Self { id }
    }
}

impl crate::modules::Module for TaskbarModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Taskbar
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }
}

impl crate::modules::render::ModuleRender for TaskbarModule {
    fn has_content(&self, _state: &ModuleState, _mode: crate::config::FormatMode) -> bool {
        false
    }

    fn render_collapsed_with_style(
        &self,
        _state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn render_panel_with_style(
        &self,
        _state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }
}

/// One application on the taskbar, rendered as an independent bar module.
pub(crate) struct TaskbarItemModule {
    id: ModuleId,
    entry: TaskbarEntry,
}

/// Application icons are a little larger than status glyphs.
const APP_ICON_SIZE: f32 = 26.0;
const CELL: f32 = 40.0;

impl TaskbarItemModule {
    pub(crate) fn module_id(config_name: &str, key: &str) -> ModuleId {
        ModuleId::taskbar(config_name, key)
    }

    pub(crate) fn new(entry: TaskbarEntry, config_name: &str) -> Self {
        Self {
            id: Self::module_id(config_name, &entry.key),
            entry,
        }
    }

    fn icon(&self, size: f32) -> iced::Element<'static, crate::app::Message> {
        let installed = self
            .entry
            .icon
            .as_ref()
            .and_then(|icon| icon.path.clone())
            .filter(|path| path.is_file());
        if installed.is_none() {
            let glyph: Option<&[u8]> = match self.entry.key.as_str() {
                "org.canopa.Files" | "org.canopa.Files.desktop" => {
                    Some(include_bytes!("../../assets/folder.svg"))
                }
                "org.canopa.Settings" | "org.canopa.Settings.desktop" => {
                    Some(include_bytes!("../../assets/controls.svg"))
                }
                _ => None,
            };
            if let Some(glyph) = glyph {
                return crate::theme::icon(glyph, size, crate::theme::palette().text);
            }
        }
        let path =
            installed.or_else(|| crate::icon_theme::resolve_icon_path("application-x-executable"));
        match path {
            Some(path)
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg")) =>
            {
                iced::widget::svg(iced::widget::svg::Handle::from_path(path))
                    .width(size)
                    .height(size)
                    .into()
            }
            Some(path) => iced::widget::image(iced::widget::image::Handle::from_path(path))
                .width(size)
                .height(size)
                .into(),
            None => letter_icon(&self.entry.name, size),
        }
    }

    /// Running windows show as dots on the bar's screen edge; the focused
    /// application's indicator is a longer accent pill.
    fn indicator(&self, horizontal: bool) -> iced::Element<'static, crate::app::Message> {
        let colors = crate::theme::palette();
        let count = self.entry.windows.len();
        if count == 0 {
            return iced::widget::Space::new().into();
        }
        let color = if self.entry.urgent {
            colors.warning
        } else if self.entry.focused {
            colors.accent
        } else {
            iced::Color {
                a: 0.68,
                ..colors.text
            }
        };
        let (long, short) = if self.entry.focused {
            (18.0, 4.0)
        } else if count > 1 {
            (10.0, 5.0)
        } else {
            (6.0, 6.0)
        };
        let (width, height) = if horizontal {
            (long, short)
        } else {
            (short, long)
        };
        iced::widget::container(iced::widget::Space::new().width(width).height(height))
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(color)),
                border: iced::Border {
                    radius: (short / 2.0).into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }

    fn build_view(
        &self,
        edge: &crate::config::Edge,
    ) -> iced::Element<'static, crate::app::Message> {
        use crate::config::Edge;
        use iced::Alignment;
        let horizontal = matches!(edge, Edge::Top | Edge::Bottom);
        let focused = self.entry.focused;
        let accent = crate::theme::palette().accent;
        let icon = iced::widget::container(self.icon(APP_ICON_SIZE))
            .center_x(iced::Length::Fixed(CELL - 6.0))
            .center_y(iced::Length::Fixed(CELL - 6.0))
            .style(move |_| iced::widget::container::Style {
                background: focused.then_some(accent.scale_alpha(0.14).into()),
                border: iced::border::rounded(11),
                ..Default::default()
            });
        let icon = iced::widget::container(icon)
            .center_x(iced::Length::Fill)
            .center_y(CELL);
        let (align_x, align_y) = match edge {
            Edge::Left => (Alignment::Start, Alignment::Center),
            Edge::Right => (Alignment::End, Alignment::Center),
            Edge::Top => (Alignment::Center, Alignment::Start),
            Edge::Bottom => (Alignment::Center, Alignment::End),
        };
        let indicator = iced::widget::container(self.indicator(horizontal))
            .width(iced::Length::Fill)
            .height(CELL)
            .align_x(align_x)
            .align_y(align_y);
        let key = self.entry.key.clone();
        let cell = iced::widget::stack![icon, indicator];
        let cell: iced::Element<'static, crate::app::Message> = if horizontal {
            iced::widget::container(cell).width(CELL).into()
        } else {
            cell.into()
        };
        iced::widget::mouse_area(cell)
            .on_middle_press(crate::app::Message::Taskbar(TaskbarEvent::Launch { key }))
            .into()
    }
}

/// The first letter on a soft disc, for applications without an icon.
fn letter_icon(name: &str, size: f32) -> iced::Element<'static, crate::app::Message> {
    let colors = crate::theme::palette();
    let letter: String = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into());
    iced::widget::container(
        iced::widget::text(letter)
            .size(size * 0.5)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::DEFAULT
            })
            .color(colors.on_accent),
    )
    .center_x(size)
    .center_y(size)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(colors.accent)),
        border: iced::Border {
            radius: (size * 0.3).into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

impl crate::modules::Module for TaskbarItemModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::TaskbarItem
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn interaction_handler(
        &self,
    ) -> Option<&dyn crate::modules::interaction::ModuleInteractionHandler> {
        Some(self)
    }

    fn dynamic_instance(&self) -> Option<&dyn crate::modules::dynamic::DynamicModuleInstance> {
        Some(self)
    }
}

impl crate::modules::interaction::ModuleInteractionHandler for TaskbarItemModule {
    fn on_interaction(&self, event: InteractionEvent) -> Option<crate::app::Message> {
        match event {
            InteractionEvent::Clicked | InteractionEvent::Activated(_) => {
                Some(crate::app::Message::Taskbar(TaskbarEvent::Activate {
                    key: self.entry.key.clone(),
                }))
            }
            _ => None,
        }
    }

    fn on_interaction_in_window(
        &self,
        event: InteractionEvent,
        window: iced::window::Id,
    ) -> Option<crate::app::Message> {
        match event {
            InteractionEvent::RightClicked(_) => {
                Some(crate::app::Message::Taskbar(TaskbarEvent::Menu {
                    key: self.entry.key.clone(),
                    module: self.id.clone(),
                    window,
                }))
            }
            event => self.on_interaction(event),
        }
    }
}

impl crate::modules::dynamic::DynamicModuleInstance for TaskbarItemModule {
    fn config_name(&self) -> &str {
        self.id.configured_name()
    }
}

impl crate::modules::render::ModuleRender for TaskbarItemModule {
    fn has_content(&self, _state: &ModuleState, mode: crate::config::FormatMode) -> bool {
        mode != crate::config::FormatMode::Expanded
    }

    fn accessible_values(&self, state: &ModuleState) -> Vec<(&'static str, String)> {
        let mut values = self.tooltip_values(state);
        if self.entry.focused {
            values.push(("state", "focused".into()));
        }
        if self.entry.pinned {
            values.push(("pinned", "true".into()));
        }
        if self.entry.urgent {
            values.push(("urgent", "true".into()));
        }
        values.push(("count", self.entry.windows.len().to_string()));
        values
    }

    fn tooltip_values(&self, _state: &ModuleState) -> Vec<(&'static str, String)> {
        let windows = match self.entry.windows.len() {
            0 => String::new(),
            count => fl!("shell-taskbar-windows", count = count),
        };
        vec![("name", self.entry.name.clone()), ("windows", windows)]
    }

    fn render_collapsed_with_style(
        &self,
        _state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn render_panel_with_style(
        &self,
        _state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn render_bar_collapsed_with_style(
        &self,
        _state: &ModuleState,
        context: &crate::modules::render::BarRenderContext<'_>,
        _bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        Some(self.build_view(context.edge))
    }

    fn render_bar_horizontal_with_style(
        &self,
        _state: &ModuleState,
        context: &crate::modules::render::BarRenderContext<'_>,
        _bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        Some(self.build_view(context.edge))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str, wm_class: Option<&str>) -> DesktopApplication {
        DesktopApplication {
            id: id.into(),
            name: name.into(),
            startup_wm_class: wm_class.map(Into::into),
            ..DesktopApplication::default()
        }
    }

    fn window(id: u64, app_id: &str, focus_secs: u64) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: None,
            app_id: Some(app_id.into()),
            pid: None,
            parent_id: None,
            workspace_id: None,
            is_focused: false,
            is_floating: false,
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
                pinned: None,
            },
            focus_timestamp: Some(niri_ipc::Timestamp {
                secs: focus_secs,
                nanos: 0,
            }),
        }
    }

    #[test]
    fn pinned_apps_lead_then_running_apps_in_window_order_without_layouts() {
        let apps = [
            app("org.gnome.Nautilus.desktop", "Files", None),
            app("firefox.desktop", "Firefox", None),
            app("foot.desktop", "Foot", None),
        ];
        let windows = [
            window(4, "foot", 1),
            window(2, "firefox", 2),
            window(9, "foot", 5),
        ];
        let refs: Vec<_> = windows.iter().collect();
        let entries = entries(
            &[
                "org.gnome.Nautilus.desktop".into(),
                "missing.desktop".into(),
            ],
            &refs,
            &[],
            Some(9),
            &apps,
        );
        let keys: Vec<_> = entries.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "org.gnome.Nautilus.desktop",
                "firefox.desktop",
                "foot.desktop"
            ]
        );
        assert!(entries[0].pinned && entries[0].windows.is_empty());
        assert_eq!(entries[2].windows, [9, 4], "most recently focused first");
        assert!(entries[2].focused && !entries[1].focused);
    }

    #[test]
    fn unknown_app_ids_still_get_an_entry_without_a_launcher() {
        let windows = [window(1, "org.example.Unknown", 0)];
        let refs: Vec<_> = windows.iter().collect();
        let entries = entries(&[], &refs, &[], None, &[]);
        assert_eq!(entries[0].key, "org.example.Unknown");
        assert_eq!(entries[0].desktop_id, None);
    }

    #[test]
    fn clicking_the_focused_app_cycles_its_windows() {
        let entry = TaskbarEntry {
            key: "foot.desktop".into(),
            name: "Foot".into(),
            icon: None,
            desktop_id: Some("foot.desktop".into()),
            pinned: false,
            windows: vec![9, 4, 6],
            layout_order: Vec::new(),
            focused: true,
            urgent: false,
        };
        assert_eq!(window_to_focus(&entry, Some(4)), Some(6));
        assert_eq!(window_to_focus(&entry, Some(9)), Some(4));
        assert_eq!(window_to_focus(&entry, None), Some(9), "most recent first");
    }

    fn placed(
        id: u64,
        app_id: &str,
        workspace: u64,
        column: usize,
        tile: usize,
    ) -> niri_ipc::Window {
        let mut placed = window(id, app_id, 0);
        placed.workspace_id = Some(workspace);
        placed.layout.pos_in_scrolling_layout = Some((column, tile));
        placed
    }

    fn workspace(id: u64, output: &str, idx: u8) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id,
            idx,
            name: None,
            output: Some(output.into()),
            is_urgent: false,
            is_active: true,
            is_focused: false,
            active_window_id: None,
        }
    }

    #[test]
    fn running_apps_follow_the_strip_and_cycle_left_to_right() {
        let apps = [
            app("firefox.desktop", "Firefox", None),
            app("foot.desktop", "Foot", None),
            app("org.gnome.Nautilus.desktop", "Files", None),
        ];
        let workspaces = [workspace(10, "eDP-1", 2), workspace(11, "eDP-1", 1)];
        let mut floating = placed(1, "foot", 11, 0, 0);
        floating.layout.pos_in_scrolling_layout = None;
        let windows = [
            // Workspace 2: Firefox in column 1.
            placed(2, "firefox", 10, 1, 1),
            // Workspace 1: Files in column 2, Foot in column 3 and stacked
            // under Files, and a floating Foot.
            placed(3, "foot", 11, 3, 1),
            placed(4, "org.gnome.Nautilus", 11, 2, 1),
            placed(5, "foot", 11, 2, 2),
            floating,
        ];
        let refs: Vec<_> = windows.iter().collect();

        let entries = entries(&[], &refs, &workspaces, Some(5), &apps);

        let keys: Vec<_> = entries.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "org.gnome.Nautilus.desktop",
                "foot.desktop",
                "firefox.desktop"
            ]
        );
        let foot = &entries[1];
        assert_eq!(foot.layout_order, [5, 3, 1]);
        assert_eq!(window_to_focus(foot, Some(5)), Some(3));
        assert_eq!(window_to_focus(foot, Some(3)), Some(1));
        assert_eq!(window_to_focus(foot, Some(1)), Some(5));
    }

    fn entry(windows: Vec<u64>, pinned: bool) -> TaskbarEntry {
        TaskbarEntry {
            key: "org.example.Editor.desktop".into(),
            name: "Editor".into(),
            icon: None,
            desktop_id: Some("org.example.Editor.desktop".into()),
            pinned,
            layout_order: windows.clone(),
            windows,
            focused: false,
            urgent: false,
        }
    }

    fn editor(actions: &[&str], single_main_window: bool) -> DesktopApplication {
        DesktopApplication {
            id: "org.example.Editor.desktop".into(),
            name: "Editor".into(),
            actions: actions
                .iter()
                .map(|id| crate::desktop_apps::DesktopAction {
                    id: (*id).into(),
                    name: format!("Action {id}"),
                    icon: None,
                    exec: None,
                })
                .collect(),
            single_main_window,
            ..DesktopApplication::default()
        }
    }

    fn commands(rows: &[MenuRow]) -> Vec<Option<MenuCommand>> {
        rows.iter()
            .map(|row| row.as_ref().map(|(_, command)| command.clone()))
            .collect()
    }

    #[test]
    fn menus_offer_a_new_window_the_desktop_actions_and_pinning() {
        let app = editor(&["new-window", "new-private"], false);
        let rows = menu(&entry(vec![3], false), Some(&app), None);
        assert_eq!(
            commands(&rows),
            [
                Some(MenuCommand::DesktopAction("new-window".into())),
                None,
                Some(MenuCommand::DesktopAction("new-private".into())),
                None,
                Some(MenuCommand::Pin),
            ],
            "the entry's own new-window action stands in for a launch"
        );
        assert_eq!(rows[0].as_ref().unwrap().0, fl!("shell-taskbar-new-window"));

        let rows = menu(&entry(vec![], true), Some(&editor(&[], false)), None);
        assert_eq!(
            commands(&rows),
            [Some(MenuCommand::Launch), None, Some(MenuCommand::Unpin)],
            "nothing runs, so the first row opens the application"
        );

        let rows = menu(
            &entry(vec![3], true),
            Some(&editor(&["new-window"], true)),
            None,
        );
        assert_eq!(
            commands(&rows),
            [
                Some(MenuCommand::DesktopAction("new-window".into())),
                None,
                Some(MenuCommand::Unpin),
            ],
            "a single-main-window application keeps its actions but no generic New window"
        );

        assert!(
            menu(&entry(vec![3], false), None, None).is_empty(),
            "windows without a desktop entry have nothing to launch or pin"
        );
    }

    #[test]
    fn dialogs_belong_to_their_parent_window() {
        let apps = [app("org.canopa.Files.desktop", "Files", None)];
        let mut quick_look = placed(3, "org.canopa.Files", 11, 0, 0);
        quick_look.layout.pos_in_scrolling_layout = None;
        quick_look.parent_id = Some(1);
        // Another application's dialog parented to Files keeps its own entry.
        let mut portal = window(4, "org.canopa.Portal", 0);
        portal.parent_id = Some(1);
        let windows = [
            placed(1, "org.canopa.Files", 11, 1, 1),
            placed(2, "org.canopa.Files", 11, 2, 1),
            quick_look,
            portal,
        ];
        let refs: Vec<_> = windows.iter().collect();

        let entries = entries(&[], &refs, &[workspace(11, "eDP-1", 1)], Some(3), &apps);

        let files = &entries[0];
        assert_eq!(files.layout_order, [1, 2]);
        assert!(files.focused, "a focused dialog marks its application");
        assert_eq!(entries[1].key, "org.canopa.Portal");
        assert_eq!(entries[1].windows, [4]);
    }

    #[test]
    fn running_windows_get_layout_commands_for_their_place() {
        let tiled = LayoutTarget {
            floating: false,
            pinned: None,
            above: None,
            below: Some(12),
        };
        let rows = menu(
            &entry(vec![3], false),
            Some(&editor(&[], false)),
            Some(tiled),
        );
        let layout: Vec<_> = commands(&rows)
            .into_iter()
            .flatten()
            .filter_map(|command| match command {
                MenuCommand::Layout(command) => Some(command),
                _ => None,
            })
            .collect();
        assert_eq!(
            layout,
            [
                LayoutCommand::Width(33),
                LayoutCommand::Width(50),
                LayoutCommand::Width(67),
                LayoutCommand::ToggleTabs,
                LayoutCommand::Center,
                LayoutCommand::Pin(niri_ipc::PinSide::Left),
                LayoutCommand::Pin(niri_ipc::PinSide::Right),
                LayoutCommand::ToggleFloating,
                LayoutCommand::MoveToWorkspace(12),
            ]
        );
        let pinned = LayoutTarget {
            pinned: Some(niri_ipc::PinSide::Left),
            ..tiled
        };
        let rows = menu(
            &entry(vec![3], false),
            Some(&editor(&[], false)),
            Some(pinned),
        );
        assert!(commands(&rows).contains(&Some(MenuCommand::Layout(LayoutCommand::Unpin))));
        assert!(
            !commands(&rows).contains(&Some(MenuCommand::Layout(LayoutCommand::Pin(
                niri_ipc::PinSide::Left
            ))))
        );

        let floating = LayoutTarget {
            floating: true,
            pinned: None,
            above: Some(10),
            below: Some(12),
        };
        let rows = menu(
            &entry(vec![3], false),
            Some(&editor(&[], false)),
            Some(floating),
        );
        assert!(!commands(&rows).contains(&Some(MenuCommand::Layout(LayoutCommand::Center))));
        assert!(commands(&rows).contains(&Some(MenuCommand::Layout(
            LayoutCommand::MoveToWorkspace(10)
        ))));
    }

    #[test]
    fn layout_targets_find_the_neighbouring_workspaces_on_the_same_output() {
        let workspaces = [
            workspace(10, "eDP-1", 1),
            workspace(11, "eDP-1", 2),
            workspace(12, "eDP-1", 3),
            workspace(20, "DP-1", 3),
        ];
        let window = placed(1, "foot", 11, 1, 1);
        let target = LayoutTarget::of(&window, &workspaces);
        assert_eq!((target.above, target.below), (Some(10), Some(12)));
        let first = LayoutTarget::of(&placed(2, "foot", 10, 1, 1), &workspaces);
        assert_eq!((first.above, first.below), (None, Some(11)));
    }

    #[test]
    fn column_commands_focus_the_window_first() {
        use niri_ipc::Action;
        let actions = layout_actions(LayoutCommand::Width(33), 7);
        assert!(matches!(actions[0], Action::FocusWindow { id: 7 }));
        assert!(matches!(
            actions[1],
            Action::SetColumnWidth {
                change: niri_ipc::SizeChange::SetProportion(value)
            } if value == 33.0
        ));
        assert!(matches!(
            layout_actions(LayoutCommand::MoveToWorkspace(12), 7).as_slice(),
            [Action::MoveWindowToWorkspace {
                window_id: Some(7),
                focus: false,
                ..
            }]
        ));
    }

    #[test]
    fn menu_trees_map_item_ids_to_rows_and_separators() {
        let rows = menu(&entry(vec![], false), Some(&editor(&["a"], false)), None);
        let tree = menu_tree(&rows, (5, 6), (800, 600));
        assert_eq!(tree.items.len(), rows.len());
        assert_eq!(tree.items[0].id, "0");
        assert!(tree.items[1].is_separator && !tree.items[1].enabled);
        assert_eq!(tree.items[2].label, "Action a");
        assert_eq!((tree.anchor, tree.screen_size), ((5, 6), (800, 600)));
    }

    #[test]
    fn pin_edits_layer_over_the_configured_pins() {
        let configured: Vec<String> = vec!["a.desktop".into(), "b.desktop".into()];
        let mut edits = PinEdits::default();
        edits.set(&configured, "c.desktop", true);
        edits.set(&configured, "a.desktop", false);
        assert_eq!(edits.apply(&configured), ["b.desktop", "c.desktop"]);
        assert_eq!(edits.pinned, ["c.desktop"]);
        assert_eq!(edits.unpinned, ["a.desktop"]);

        edits.set(&configured, "a.desktop", true);
        edits.set(&configured, "c.desktop", false);
        assert!(
            edits.is_empty(),
            "edits that restore the configuration vanish"
        );
        assert_eq!(edits.apply(&configured), configured);
        // A pin added later to the configuration keeps its configured place.
        edits.set(&configured, "d.desktop", true);
        let configured: Vec<String> = vec!["d.desktop".into(), "a.desktop".into()];
        assert_eq!(edits.apply(&configured), ["d.desktop", "a.desktop"]);
    }

    #[test]
    fn pin_stores_round_trip_and_reject_foreign_files() {
        let mut store = PinStore::default();
        store.set("apps", &[], "c.desktop", true);
        store.set("dock", &["a.desktop".into()], "a.desktop", false);
        let bytes = store.to_bytes();
        assert!(bytes.len() < PIN_STORE_MAX_BYTES);
        let parsed = PinStore::parse(&bytes).expect("own file");
        assert_eq!(parsed, store);
        assert_eq!(parsed.edits("apps").unwrap().pinned, ["c.desktop"]);

        store.set("apps", &[], "c.desktop", false);
        assert!(store.edits("apps").is_none());

        assert!(PinStore::parse(br#"{"version":2,"taskbars":{}}"#).is_err());
        assert!(PinStore::parse(br#"{"version":1,"other":1}"#).is_err());
    }
}
