//! The Shell's own surfaces on the accessibility bus through the shared
//! `canopa-ui` layer: bars, Quick settings, menus, confirmations, the calendar,
//! the launcher and tray menus.
//!
//! Canopa provider panels keep their own adapter (`canopa_accessibility`). A
//! panel surface is published here only while it shows something else, so no
//! surface ever has two adapters. Nothing is collected or built until an
//! assistive technology activates a window.
use super::panel_menu_session::{PanelMenuEntry, PanelMenuEntryTarget};
use super::{App, Message};
use crate::modules::{ModuleId, ModuleKind};
use canopa_ui::accessibility::{self, Bridge, Node};
use iced::Task;
use std::collections::BTreeMap;

impl accessibility::Message for Message {
    fn accessibility(event: accessibility::Event<Self>) -> Self {
        Self::Accessibility(event)
    }
}

/// Which kind of Shell surface a published window is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Surface {
    Bar,
    Panel,
    Menu,
}

/// The key of a surface's root node; it selects the window among published ones.
pub(super) fn window_key(surface: Surface, window: iced::window::Id) -> String {
    let kind = match surface {
        Surface::Bar => "bar",
        Surface::Panel => "panel",
        Surface::Menu => "menu",
    };
    format!("shell-{kind}-{window}")
}

/// The root node of a surface's view.
pub(super) fn window_node(
    surface: Surface,
    window: iced::window::Id,
    title: impl Into<String>,
) -> Node<Message> {
    Node::window(window_key(surface, window), title)
}

struct Published {
    window: iced::window::Id,
    focused: bool,
}

/// The process's bridge and which surfaces it currently publishes.
pub(super) struct ShellAccessibility {
    bridge: Bridge<Message>,
    windows: BTreeMap<String, Published>,
}

impl ShellAccessibility {
    pub(super) fn new() -> Self {
        Self::with_bridge(Bridge::new("canopa-shell"))
    }

    pub(super) fn with_bridge(bridge: Bridge<Message>) -> Self {
        Self {
            bridge,
            windows: BTreeMap::new(),
        }
    }

    pub(super) fn subscription(&self) -> iced::Subscription<Message> {
        self.bridge.subscription()
    }

    #[cfg(test)]
    pub(super) fn is_published(&self, key: &str) -> bool {
        self.windows.contains_key(key) && self.bridge.is_open(key)
    }

    /// Open the surfaces in `desired` and close every other one.
    fn sync(
        &mut self,
        desired: &[(String, iced::window::Id)],
        keyboard_window: Option<iced::window::Id>,
    ) {
        let bridge = &mut self.bridge;
        self.windows.retain(|key, published| {
            let keep = desired
                .iter()
                .any(|(wanted, window)| wanted == key && *window == published.window);
            if !keep {
                bridge.close(key);
            }
            keep
        });
        for (key, window) in desired {
            let published = self.windows.entry(key.clone()).or_insert_with(|| {
                bridge.open(key.clone(), "Canopa Shell", Some(*window));
                Published {
                    window: *window,
                    focused: false,
                }
            });
            let focused = keyboard_window == Some(*window);
            if published.focused != focused {
                published.focused = focused;
                bridge.set_window_focused(key, focused);
            }
        }
    }
}

impl App {
    /// The surfaces the shared bridge publishes now.
    fn accessible_surfaces(&self) -> Vec<(String, iced::window::Id)> {
        let panel = self.foreground.active_panel_window();
        let menu = self.foreground.active_menu_window();
        let mut surfaces: Vec<_> = self
            .surfaces
            .keys()
            .copied()
            // Input bookkeeping can create state for other windows; keep bars only.
            .filter(|window| {
                Some(*window) != panel
                    && Some(*window) != menu
                    && !self.foreground.is_dismissal_window(*window)
                    && !self.wallpaper.contains(*window)
                    && !self.osd.contains(*window)
                    && self.surfaces.tooltip_popup(*window).is_none()
            })
            .map(|window| (window_key(Surface::Bar, window), window))
            .collect();
        if let Some(window) = panel
            && self
                .foreground
                .active_panel_tree()
                .is_some_and(|tree| !shows_canopa_provider(&tree.content))
        {
            surfaces.push((window_key(Surface::Panel, window), window));
        }
        if let Some(window) = menu {
            surfaces.push((window_key(Surface::Menu, window), window));
        }
        surfaces
    }

    /// Publish the current surfaces and refresh their trees. Free while no
    /// assistive technology listens.
    pub(super) fn sync_accessibility(&mut self) -> Task<Message> {
        if self.accessibility.is_none() {
            return Task::none();
        }
        let desired = self.accessible_surfaces();
        let keyboard_window = self.accessibility_focused_window;
        let shell = self.accessibility.as_mut().expect("checked above");
        shell.sync(&desired, keyboard_window);
        shell.bridge.refresh()
    }

    pub(super) fn handle_accessibility_event(
        &mut self,
        event: accessibility::Event<Message>,
    ) -> Task<Message> {
        match &mut self.accessibility {
            Some(shell) => shell.bridge.update(event),
            None => Task::none(),
        }
    }

    #[cfg(test)]
    pub(super) fn set_accessibility_for_test(&mut self, shell: ShellAccessibility) {
        self.accessibility = Some(shell);
    }

    #[cfg(test)]
    pub(super) fn accessibility_for_test(&mut self) -> Option<&mut ShellAccessibility> {
        self.accessibility.as_mut()
    }
}

/// Whether the provider bridge publishes this panel content itself.
pub(super) fn shows_canopa_provider(content: &crate::surface::panel::PanelContent) -> bool {
    matches!(
        content,
        crate::surface::panel::PanelContent::ExternalProvider(content)
            if content.document.canopa().is_some()
    )
}

/// What a bar module says to an assistive technology.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ModuleSummary {
    pub(super) label: String,
    pub(super) description: Option<String>,
    /// Toggle modules (Keep awake): whether they are on.
    pub(super) toggled: Option<bool>,
    /// Workspaces: whether this one is focused.
    pub(super) selected: Option<bool>,
}

fn value<'a>(values: &'a [(&'static str, String)], key: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
}

/// A module's `{percent}` value read as a percentage.
fn percent(value: &str) -> String {
    value
        .parse::<f64>()
        .map(canopa_i18n::format::percent)
        .unwrap_or_else(|_| format!("{value}%"))
}

/// Name a bar module and state its value from the values its format uses.
pub(super) fn module_summary(
    kind: ModuleKind,
    configured_name: &str,
    values: &[(&'static str, String)],
) -> ModuleSummary {
    let get = |key| value(values, key);
    let mut summary = ModuleSummary::default();
    let (name, parts): (String, Vec<String>) = match kind {
        ModuleKind::Clock => (
            fl!("shell-a11y-clock"),
            [get("time"), get("date")]
                .into_iter()
                .flatten()
                .map(str::to_owned)
                .collect(),
        ),
        ModuleKind::Battery => {
            let mut parts: Vec<String> = get("percent").map(percent).into_iter().collect();
            match get("state") {
                Some("normal" | "unknown") | None => {}
                Some("full") => parts.push(fl!("shell-a11y-battery-full")),
                Some("pending-charge") => parts.push(fl!("shell-a11y-battery-not-charging")),
                Some("charging") => parts.push(fl!("shell-a11y-battery-charging")),
                Some("pending-discharge") => parts.push(fl!("shell-a11y-battery-discharging")),
                Some("critical") => parts.push(fl!("shell-a11y-battery-critical")),
                Some("low") => parts.push(fl!("shell-a11y-battery-low")),
                // A state the person named in their configuration.
                Some(state) => parts.push(state.replace('-', " ")),
            }
            (fl!("shell-a11y-battery"), parts)
        }
        ModuleKind::Network => (
            fl!("shell-a11y-network"),
            get("label").map(str::to_owned).into_iter().collect(),
        ),
        ModuleKind::Volume => (
            fl!("shell-a11y-volume"),
            if get("state") == Some("muted") {
                vec![fl!("shell-a11y-volume-muted")]
            } else {
                get("percent").map(percent).into_iter().collect()
            },
        ),
        ModuleKind::Notifications => {
            summary.description = get("label").map(str::to_owned);
            let parts = match get("state") {
                Some("unavailable") | None => vec![fl!("shell-a11y-unavailable")],
                Some(state) => {
                    // The value is the formatted count; its digits give the plural.
                    let count: u64 = get("value")
                        .map(|value| {
                            value
                                .chars()
                                .filter(char::is_ascii_digit)
                                .collect::<String>()
                        })
                        .and_then(|digits| digits.parse().ok())
                        .unwrap_or(0);
                    let mut parts = vec![fl!("shell-a11y-attention", count = count)];
                    if state == "dnd" {
                        parts.push(fl!("shell-a11y-dnd-on"));
                    }
                    parts
                }
            };
            (fl!("shell-a11y-notifications"), parts)
        }
        ModuleKind::IdleInhibitor => {
            summary.description = get("label").map(str::to_owned);
            let parts = match get("state") {
                Some("active") => {
                    summary.toggled = Some(true);
                    Vec::new()
                }
                Some("inactive") => {
                    summary.toggled = Some(false);
                    Vec::new()
                }
                Some(state) => vec![state.to_owned()],
                None => Vec::new(),
            };
            (fl!("shell-idle-keep-awake"), parts)
        }
        ModuleKind::Media => (
            fl!("shell-media-title"),
            [get("label"), get("state")]
                .into_iter()
                .flatten()
                .map(str::to_owned)
                .collect(),
        ),
        ModuleKind::FocusedWindow => (
            fl!("shell-panel-focused-window"),
            get("title")
                .or(get("app-id"))
                .map(str::to_owned)
                .into_iter()
                .collect(),
        ),
        ModuleKind::Button => (
            get("label").unwrap_or(configured_name).to_owned(),
            Vec::new(),
        ),
        ModuleKind::Script => (
            configured_name.to_owned(),
            get("output").map(str::to_owned).into_iter().collect(),
        ),
        ModuleKind::WorkspaceItem => {
            let label = get("label").or(get("index")).unwrap_or("?");
            let mut parts: Vec<String> = get("name")
                .filter(|name| *name != label)
                .map(str::to_owned)
                .into_iter()
                .collect();
            let focused = get("state") == Some("focused");
            if focused {
                parts.push(fl!("shell-a11y-workspace-focused"));
            } else if get("state") == Some("active") {
                parts.push(fl!("shell-a11y-workspace-active"));
            }
            if get("urgent").is_some() {
                parts.push(fl!("shell-a11y-workspace-urgent"));
            }
            summary.selected = Some(focused);
            (fl!("shell-a11y-workspace", label = label), parts)
        }
        ModuleKind::TrayIcon => {
            let title = get("title");
            summary.description = title.and(get("description")).map(str::to_owned);
            (
                title
                    .or(get("description"))
                    .or(get("icon-name"))
                    .or(get("service"))
                    .map(str::to_owned)
                    .unwrap_or_else(|| fl!("shell-a11y-tray-item")),
                Vec::new(),
            )
        }
        ModuleKind::Workspaces => (fl!("shell-panel-workspaces"), Vec::new()),
        ModuleKind::Tray => (fl!("shell-a11y-tray"), Vec::new()),
        ModuleKind::Taskbar => (fl!("shell-a11y-taskbar"), Vec::new()),
        ModuleKind::Minimap => {
            let number = |key| get(key).and_then(|value| value.parse::<usize>().ok());
            let columns = number("columns").unwrap_or(0);
            let mut parts = Vec::new();
            if columns == 0 {
                parts.push(fl!("shell-a11y-minimap-empty"));
            } else if let Some(column) = number("column") {
                parts.push(fl!(
                    "shell-a11y-minimap-column",
                    column = column,
                    columns = columns
                ));
            }
            parts.extend(get("title").map(str::to_owned));
            match get("pinned") {
                Some("left") => parts.push(fl!("shell-a11y-minimap-pinned-left")),
                Some("right") => parts.push(fl!("shell-a11y-minimap-pinned-right")),
                _ => {}
            }
            if let Some(count) = number("floating") {
                parts.push(fl!("shell-a11y-minimap-floating", count = count));
            }
            (fl!("shell-a11y-minimap"), parts)
        }
        ModuleKind::TaskbarItem => {
            let mut parts = Vec::new();
            let count = get("count")
                .and_then(|count| count.parse::<usize>().ok())
                .unwrap_or(0);
            if count > 0 {
                parts.push(fl!("shell-taskbar-windows", count = count));
            } else if get("pinned").is_some() {
                parts.push(fl!("shell-a11y-taskbar-pinned"));
            }
            if get("state") == Some("focused") {
                parts.push(fl!("shell-a11y-taskbar-focused"));
            }
            if get("urgent").is_some() {
                parts.push(fl!("shell-a11y-workspace-urgent"));
            }
            (
                get("name")
                    .map(str::to_owned)
                    .unwrap_or_else(|| fl!("shell-a11y-taskbar")),
                parts,
            )
        }
    };
    summary.label = std::iter::once(name)
        .chain(parts)
        .collect::<Vec<_>>()
        .join(", ");
    summary
}

/// A bar module as a button whose default action is the click's message.
pub(super) fn bar_module_node(
    id: &ModuleId,
    summary: ModuleSummary,
    focused: bool,
    expanded: bool,
    click: Message,
) -> Node<Message> {
    let mut node = Node::button(summary.label)
        .key(id.accessibility_key())
        .focused(focused)
        .on_click(Some(click));
    if let Some(description) = summary.description {
        node = node.description(description);
    }
    if let Some(on) = summary.toggled {
        node = node.toggled(on);
    }
    if let Some(selected) = summary.selected {
        node = node.selected(selected);
    }
    if expanded {
        node = node.expanded(true);
    }
    node
}

/// A label-independent key for one row of a Shell menu.
pub(super) fn menu_entry_key(entry: &PanelMenuEntry, index: usize) -> String {
    match &entry.target {
        PanelMenuEntryTarget::StaticPath(path) => format!(
            "menu-item-{}",
            path.iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(".")
        ),
        PanelMenuEntryTarget::InstalledApp(id) => format!("menu-app-{id}"),
        PanelMenuEntryTarget::SniMenuItem(id) => format!("menu-sni-{id}"),
        PanelMenuEntryTarget::Launcher(target) => format!("launcher-result-{}", target.result.id),
        PanelMenuEntryTarget::DmenuValue(_) | PanelMenuEntryTarget::DmenuCustomValue(_) => {
            format!("menu-choice-{index}")
        }
    }
}

/// The launcher's live regions: a polite status (progress, match count) and
/// an assertive alert for failures. A failure is never announced twice.
pub(super) fn launcher_announcements(
    status: &str,
    error: Option<&str>,
    matches: Option<&str>,
) -> (String, String) {
    let error = error.unwrap_or_default();
    let polite: Vec<&str> = status
        .split(" · ")
        .map(str::trim)
        .filter(|part| !part.is_empty() && *part != error)
        .collect();
    let polite = if polite.is_empty() {
        matches.unwrap_or_default().to_owned()
    } else {
        polite.join(" · ")
    };
    (polite, error.to_owned())
}

#[cfg(all(test, debug_assertions, feature = "accessibility-testing"))]
mod native_tests;
#[cfg(test)]
mod tests;
