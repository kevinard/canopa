//! Taskbar: pinned applications first, then the other running ones, as app
//! icons. A click focuses an application's window, cycles through its windows
//! when it is already focused, or launches it when nothing is open. Niri owns
//! the windows; the Shell only groups them by desktop application.
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
    pub focused: bool,
    pub urgent: bool,
}

#[derive(Debug, Clone)]
pub enum TaskbarEvent {
    /// Focus the application, cycle its windows, or launch it.
    Activate { key: String },
    /// Start another instance.
    Launch { key: String },
}

/// Groups `windows` by application. Pinned applications keep their configured
/// order; others follow in the order they were first opened.
pub(crate) fn entries(
    pinned: &[String],
    windows: &[&niri_ipc::Window],
    focused_window: Option<u64>,
    applications: &[DesktopApplication],
) -> Vec<TaskbarEntry> {
    let mut entries: Vec<TaskbarEntry> = pinned
        .iter()
        .filter_map(|id| applications.iter().find(|app| &app.id == id))
        .map(|app| entry_for(app.id.clone(), Some(app), true))
        .collect();

    let mut ordered: Vec<_> = windows.to_vec();
    ordered.sort_by_key(|window| window.id);
    let mut recency: Vec<(u64, (u64, u32))> = Vec::new();
    for window in ordered {
        let Some(app_id) = window.app_id.as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        let application =
            crate::focused_window_icons::match_desktop_application_for_app_id(app_id, applications);
        let key = application.map_or_else(|| app_id.to_owned(), |app| app.id.clone());
        let index = match entries.iter().position(|entry| entry.key == key) {
            Some(index) => index,
            None => {
                entries.push(entry_for(key, application, false));
                entries.len() - 1
            }
        };
        let entry = &mut entries[index];
        entry.windows.push(window.id);
        entry.focused |= focused_window == Some(window.id);
        entry.urgent |= window.is_urgent;
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
        focused: false,
        urgent: false,
    }
}

/// The window a click focuses: the next of the application's windows when
/// one of them is focused, otherwise its most recently focused one.
pub(crate) fn window_to_focus(entry: &TaskbarEntry, focused_window: Option<u64>) -> Option<u64> {
    if entry.windows.len() > 1
        && let Some(focused) = focused_window
        && entry.windows.contains(&focused)
    {
        // Cycle in a stable order so repeated clicks visit every window.
        let mut stable = entry.windows.clone();
        stable.sort_unstable();
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
        let path = self
            .entry
            .icon
            .as_ref()
            .and_then(|icon| icon.path.clone())
            .filter(|path| path.exists())
            .or_else(|| crate::icon_theme::resolve_icon_path("application-x-executable"));
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
                a: 0.55,
                ..colors.text
            }
        };
        let (long, short) = if self.entry.focused {
            (16.0, 4.0)
        } else if count > 1 {
            (9.0, 4.0)
        } else {
            (4.0, 4.0)
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
                    radius: 2.0.into(),
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
        let icon = iced::widget::container(self.icon(APP_ICON_SIZE))
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
            InteractionEvent::RightClicked(_) => {
                Some(crate::app::Message::Taskbar(TaskbarEvent::Launch {
                    key: self.entry.key.clone(),
                }))
            }
            _ => None,
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
            },
            focus_timestamp: Some(niri_ipc::Timestamp {
                secs: focus_secs,
                nanos: 0,
            }),
        }
    }

    #[test]
    fn pinned_apps_lead_then_running_apps_in_opening_order() {
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
        let entries = entries(&[], &refs, None, &[]);
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
            focused: true,
            urgent: false,
        };
        assert_eq!(window_to_focus(&entry, Some(4)), Some(6));
        assert_eq!(window_to_focus(&entry, Some(9)), Some(4));
        assert_eq!(window_to_focus(&entry, None), Some(9), "most recent first");
    }
}
