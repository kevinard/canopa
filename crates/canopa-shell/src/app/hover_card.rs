//! Hover cards on the bar: the windows of a taskbar application, and the
//! layout of a workspace. They open where tooltips open, after the same dwell,
//! stay while the pointer travels from the bar into them, and close a moment
//! after it leaves both. Thumbnails come from Niri's preview service
//! (`org.canopa.Niri.WindowPreview1`); until they arrive, windows show their
//! application's icon.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced::Task;
use iced_layershell::actions::{LayerShellCustomAction, LayerShellCustomActionWithId};

use super::messages::PointerChange;
use super::{App, Message};
use crate::config::ModuleParams;
use crate::modules::ModuleId;
use crate::surface::hover_card::{
    AppFace, Event, HoverCard, MAX_ROWS, Thumbnail, TileFace, WindowList, WindowRow, WorkspaceCard,
};

/// How long a card waits for the pointer to come back before closing.
const CLOSE_GRACE: Duration = Duration::from_millis(300);
/// After one card, the next opens this soon, so moving along the taskbar
/// switches cards instead of making the person wait again.
const WARM_DWELL_MS: f32 = 90.0;
/// Thumbnails fit this square, in physical pixels: sharp at twice the
/// largest size a card draws them.
const PREVIEW_SIDE: u32 = 320;
/// A card asks for at most this many thumbnails at once.
const MAX_CAPTURES: usize = 16;

#[derive(Default)]
pub(super) struct HoverCardRuntime {
    /// Window thumbnails by window ID, kept while the window is open.
    previews: HashMap<u64, Thumbnail>,
    /// The card popup the pointer is over.
    pointer_in: Option<iced::window::Id>,
    /// When the open cards close, unless the pointer returns first.
    close_at: Option<Instant>,
}

impl App {
    /// Whether hovering `module` shows a card rather than a text tooltip:
    /// a taskbar application with open windows, or a workspace. `tooltip
    /// #false` on the module turns its card off too.
    pub(super) fn module_has_hover_card(&self, module: &ModuleId) -> bool {
        let card_kind = module.workspace_id().is_some()
            || self
                .taskbar_entry_for_module(module)
                .is_some_and(|entry| !entry.layout_order.is_empty());
        if !card_kind {
            return false;
        }
        let tooltip = self
            .config
            .modules
            .iter()
            .find(|config| config.name == module.configured_name())
            .and_then(|config| match &config.params {
                ModuleParams::Taskbar(config) => Some(&config.tooltip),
                ModuleParams::Workspaces(config) => Some(&config.tooltip),
                _ => None,
            });
        !matches!(tooltip, Some(crate::config::TooltipSetting::Disabled))
    }

    /// The card for `module` as Niri reports things now.
    pub(super) fn hover_card_for(&self, module: &ModuleId) -> Option<HoverCard> {
        if !self.module_has_hover_card(module) {
            return None;
        }
        match module.workspace_id() {
            Some(workspace) => self.workspace_card(workspace),
            None => self.taskbar_card(module),
        }
    }

    fn workspace_title(&self, workspace: &niri_ipc::Workspace) -> String {
        if let Some(name) = workspace.name.as_deref().filter(|name| !name.is_empty()) {
            return name.to_owned();
        }
        let label = self
            .config
            .modules
            .iter()
            .find_map(|config| match &config.params {
                ModuleParams::Workspaces(config) => config
                    .workspaces
                    .get(&u32::from(workspace.idx))
                    .and_then(|workspace| workspace.label.clone()),
                _ => None,
            });
        label.unwrap_or_else(|| fl!("shell-tooltip-workspace", index = workspace.idx))
    }

    fn app_face(&self, app_id: Option<&str>) -> AppFace {
        let app_id = app_id.unwrap_or_default();
        let applications = super::taskbar::applications();
        let application = crate::focused_window_icons::match_desktop_application_for_app_id(
            app_id,
            &applications,
        );
        AppFace {
            key: application.map_or_else(|| app_id.to_owned(), |app| app.id.clone()),
            name: application.map_or_else(
                || app_id.rsplit('.').next().unwrap_or(app_id).to_owned(),
                |app| app.name.clone(),
            ),
            icon: if cfg!(test) {
                None
            } else {
                crate::focused_window_icons::resolve_application_icon(app_id)
            },
        }
    }

    fn taskbar_card(&self, module: &ModuleId) -> Option<HoverCard> {
        let entry = self.taskbar_entry_for_module(module)?;
        let state = self.niri.state();
        let outputs = state
            .workspaces
            .iter()
            .filter_map(|workspace| workspace.output.as_deref())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let rows = entry
            .layout_order
            .iter()
            .take(MAX_ROWS)
            .filter_map(|id| state.windows.get(id))
            .map(|window| {
                let workspace = window
                    .workspace_id
                    .and_then(|id| state.workspaces.iter().find(|w| w.id == id));
                // A scratchpad window keeps a workspace while hidden, but
                // the scratchpad is where the person finds it again.
                let detail = if window.layout.in_scratchpad {
                    Some(fl!("shell-taskbar-scratchpad"))
                } else {
                    workspace.map(|workspace| {
                        let title = self.workspace_title(workspace);
                        match workspace.output.as_deref() {
                            Some(output) if outputs > 1 => format!("{title} · {output}"),
                            _ => title,
                        }
                    })
                };
                WindowRow {
                    id: window.id,
                    title: window
                        .title
                        .clone()
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or_else(|| entry.name.clone()),
                    detail,
                    focused: state.focused_window_id == Some(window.id),
                    urgent: window.is_urgent,
                    thumbnail: self.hover_cards.previews.get(&window.id).cloned(),
                }
            })
            .collect();
        Some(HoverCard::Windows(WindowList {
            app: AppFace {
                key: entry.key.clone(),
                name: entry.name.clone(),
                icon: entry.icon.clone(),
            },
            count: entry.layout_order.len(),
            rows,
        }))
    }

    fn workspace_card(&self, workspace_id: u64) -> Option<HoverCard> {
        let state = self.niri.state();
        let workspace = state.workspaces.iter().find(|w| w.id == workspace_id)?;
        let strip = crate::modules::minimap::workspace_strip(state, workspace);
        let faces = strip
            .columns
            .iter()
            .flat_map(|column| &column.tiles)
            .map(|tile| TileFace {
                id: tile.id,
                app: self.app_face(
                    state
                        .windows
                        .get(&tile.id)
                        .and_then(|window| window.app_id.as_deref()),
                ),
                thumbnail: self.hover_cards.previews.get(&tile.id).cloned(),
            })
            .collect();
        Some(HoverCard::Workspace(WorkspaceCard {
            title: self.workspace_title(workspace),
            strip,
            faces,
        }))
    }

    /// Ask Niri for fresh thumbnails of the windows `card` shows.
    pub(super) fn capture_card_previews(&self, card: &HoverCard) -> Task<Message> {
        let windows: Vec<u64> = card.windows().into_iter().take(MAX_CAPTURES).collect();
        match self.shared.dbus.session.clone() {
            Some(connection) if !windows.is_empty() && !cfg!(test) => {
                Task::perform(capture_previews(connection, windows), |previews| {
                    Message::HoverCard(Event::Previews(previews))
                })
            }
            _ => Task::none(),
        }
    }

    pub(super) fn handle_hover_card_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::FocusWindow(id) => {
                run_niri_action(niri_ipc::Action::FocusWindow { id });
                self.close_hover_cards()
            }
            Event::Open {
                window,
                module,
                bounds,
            } => self.open_hover_card(window, module, bounds),
            Event::CloseWindow(id) => {
                run_niri_action(niri_ipc::Action::CloseWindow { id: Some(id) });
                Task::none()
            }
            Event::Previews(previews) => {
                let open = &self.niri.state().windows;
                self.hover_cards
                    .previews
                    .retain(|id, _| open.contains_key(id));
                for (id, width, height, png) in previews {
                    if open.contains_key(&id) {
                        self.hover_cards.previews.insert(
                            id,
                            Thumbnail {
                                handle: iced::widget::image::Handle::from_bytes(png),
                                width,
                                height,
                            },
                        );
                    }
                }
                self.refresh_hover_cards()
            }
        }
    }

    fn close_hover_cards(&mut self) -> Task<Message> {
        self.hover_cards.close_at = None;
        self.hover_cards.pointer_in = None;
        let cards: Vec<_> = self
            .surfaces
            .tooltip_popups
            .values()
            .filter(|popup| popup.tree.card.is_some())
            .map(|popup| (popup.target_window, popup.target_module.clone()))
            .collect();
        let mut tasks = Vec::new();
        for (window, module) in cards {
            tasks.extend(self.close_tooltip_popup_if_open_for(window, &module));
        }
        Task::batch(tasks)
    }

    /// The pointer entered or left a card popup.
    pub(super) fn hover_card_pointer(&mut self, popup: iced::window::Id, change: PointerChange) {
        match change {
            PointerChange::Entered => {
                self.hover_cards.pointer_in = Some(popup);
                self.hover_cards.close_at = None;
            }
            PointerChange::Left => {
                if self.hover_cards.pointer_in == Some(popup) {
                    self.hover_cards.pointer_in = None;
                }
                if self
                    .surfaces
                    .tooltip_popup(popup)
                    .is_some_and(|p| p.tree.card.is_some())
                {
                    self.hover_cards.close_at = Some(Instant::now() + CLOSE_GRACE);
                }
            }
            PointerChange::Released => {}
        }
    }

    /// The pointer left `module` on `window`. A card stays a moment, so the
    /// pointer can travel into it; returns whether one does.
    pub(super) fn defer_hover_card_close(
        &mut self,
        window: iced::window::Id,
        module: &ModuleId,
    ) -> bool {
        let has_card = self.surfaces.tooltip_popups.values().any(|popup| {
            popup.target_window == window
                && &popup.target_module == module
                && popup.tree.card.is_some()
        });
        if has_card && !self.pointer_in_card() {
            self.hover_cards.close_at = Some(Instant::now() + CLOSE_GRACE);
        }
        has_card
    }

    /// The pointer came onto `module`: keep its open card, or tell how far
    /// along the dwell a new tooltip starts when another card was showing.
    pub(super) fn hover_card_enter(
        &mut self,
        window: iced::window::Id,
        module: &ModuleId,
    ) -> HoverCardEnter {
        let mut other_card = false;
        for popup in self.surfaces.tooltip_popups.values() {
            if popup.target_window != window || popup.tree.card.is_none() {
                continue;
            }
            if &popup.target_module == module {
                self.hover_cards.close_at = None;
                return HoverCardEnter::Keep;
            }
            other_card = true;
        }
        if other_card {
            let delay = self.bar_for_window(window).tooltip_delay_ms as f32;
            HoverCardEnter::Warm((delay - WARM_DWELL_MS).max(0.0))
        } else {
            HoverCardEnter::Cold
        }
    }

    /// The pointer is over a card that is still open.
    fn pointer_in_card(&self) -> bool {
        self.hover_cards
            .pointer_in
            .is_some_and(|popup| self.surfaces.tooltip_popups.contains_key(&popup))
    }

    /// A card is waiting to close.
    pub(super) fn hover_card_closing(&self) -> bool {
        self.hover_cards.close_at.is_some()
    }

    /// Close the cards whose grace has run out.
    pub(super) fn hover_card_tick(&mut self) -> Task<Message> {
        let Some(at) = self.hover_cards.close_at else {
            return Task::none();
        };
        if self.pointer_in_card() {
            self.hover_cards.close_at = None;
            return Task::none();
        }
        self.hover_cards.pointer_in = None;
        if Instant::now() < at {
            return Task::none();
        }
        // The pointer may be back on the module the card belongs to.
        let keep: Vec<_> = self
            .surfaces
            .tooltip_popups
            .values()
            .filter(|popup| {
                popup.tree.card.is_some()
                    && self
                        .surface(popup.target_window)
                        .and_then(|surface| surface.hovered_module.as_ref())
                        == Some(&popup.target_module)
            })
            .map(|popup| popup.window)
            .collect();
        self.hover_cards.close_at = None;
        let cards: Vec<_> = self
            .surfaces
            .tooltip_popups
            .values()
            .filter(|popup| popup.tree.card.is_some() && !keep.contains(&popup.window))
            .map(|popup| (popup.target_window, popup.target_module.clone()))
            .collect();
        let mut tasks = Vec::new();
        for (window, module) in cards {
            tasks.extend(self.close_tooltip_popup_if_open_for(window, &module));
        }
        Task::batch(tasks)
    }

    /// Rebuild open cards after windows, titles, focus or thumbnails changed.
    /// A card whose application has no windows left closes.
    pub(super) fn refresh_hover_cards(&mut self) -> Task<Message> {
        let open: Vec<_> = self
            .surfaces
            .tooltip_popups
            .values()
            .filter(|popup| popup.tree.card.is_some())
            .map(|popup| {
                (
                    popup.window,
                    popup.target_window,
                    popup.target_module.clone(),
                )
            })
            .collect();
        let mut tasks = Vec::new();
        for (popup, target_window, module) in open {
            let Some(card) = self.hover_card_for(&module) else {
                tasks.extend(self.close_tooltip_popup_if_open_for(target_window, &module));
                continue;
            };
            let Some(entry) = self.surfaces.tooltip_popup_mut(popup) else {
                continue;
            };
            let before = crate::surface::tooltip_popup::popup_size(&entry.tree);
            entry.tree.card = Some(card);
            let tree = entry.tree.clone();
            if crate::surface::tooltip_popup::popup_size(&tree) != before {
                let settings = self.tooltip_window_settings(target_window, &tree);
                if let Some(size) = settings.size {
                    tasks.push(Task::done(Message::SizeChange { id: popup, size }));
                }
                if let Some(margin) = settings.margin {
                    tasks.push(Task::done(Message::LayerShell(
                        LayerShellCustomActionWithId::new(
                            Some(popup),
                            LayerShellCustomAction::MarginChange(margin),
                        ),
                    )));
                }
            }
        }
        Task::batch(tasks)
    }
}

/// What a pointer arriving on a module does to hover cards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum HoverCardEnter {
    /// Its own card is open: keep it.
    Keep,
    /// Another card was open: start the dwell this far along.
    Warm(f32),
    Cold,
}

/// Ask Niri's preview service for each window's thumbnail. Windows it cannot
/// render, or a compositor without the service, give none.
async fn capture_previews(
    connection: zbus::Connection,
    windows: Vec<u64>,
) -> Vec<(u64, u32, u32, Vec<u8>)> {
    let Some(client) = canopa_services::window_preview::Client::connect(connection, None).await
    else {
        return Vec::new();
    };
    let mut previews = Vec::new();
    // Keep the card responsive even if several previews are slow to render.
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        for id in windows {
            if let Some(preview) = client.capture(id, PREVIEW_SIDE).await {
                previews.push((id, preview.width, preview.height, preview.png));
            }
        }
    })
    .await;
    previews
}

fn run_niri_action(action: niri_ipc::Action) {
    #[cfg(not(test))]
    tokio::spawn(async move {
        if let Err(error) = crate::command::niri_action(action).await {
            tracing::warn!(target: "canopa_shell::modules", %error, "hover card action failed");
        }
    });
    #[cfg(test)]
    let _ = action;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::messages::AppNiriEvent;
    use crate::niri::NiriEvent;

    fn window(id: u64, workspace: u64, column: usize, app: &str) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: Some(format!("Document {id}")),
            app_id: Some(app.into()),
            pid: None,
            parent_id: None,
            workspace_id: Some(workspace),
            is_focused: id == 2,
            is_floating: false,
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: Some((column, 1)),
                tile_size: (960.0, 1000.0),
                window_size: (960, 1000),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
                pinned: None,
                in_scratchpad: false,
            },
            focus_timestamp: None,
        }
    }

    fn workspace(id: u64, idx: u8) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id,
            idx,
            name: None,
            output: Some("eDP-1".into()),
            is_urgent: false,
            is_active: idx == 1,
            is_focused: idx == 1,
            active_window_id: None,
        }
    }

    fn app() -> App {
        let config = crate::config::Config {
            modules: vec![
                crate::config::ModuleConfig {
                    name: "taskbar".into(),
                    module_type: "taskbar".into(),
                    params: ModuleParams::Taskbar(Default::default()),
                    actions: Default::default(),
                },
                crate::config::ModuleConfig {
                    name: "workspaces".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::Workspaces(Default::default()),
                    actions: Default::default(),
                },
            ],
            ..crate::config::Config::default()
        };
        let (mut app, _) = App::boot(config, vec![]);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            niri_ipc::Event::WorkspacesChanged {
                workspaces: vec![workspace(1, 1), workspace(2, 2)],
            },
        ))));
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            niri_ipc::Event::WindowsChanged {
                windows: vec![
                    window(3, 2, 1, "foot"),
                    window(2, 1, 2, "foot"),
                    window(1, 1, 1, "foot"),
                    window(4, 1, 3, "mpv"),
                ],
            },
        ))));
        app
    }

    #[test]
    fn taskbar_cards_list_windows_in_screen_order_with_their_workspace() {
        let mut app = app();
        let module = crate::modules::taskbar::TaskbarItemModule::module_id("taskbar", "foot");
        assert!(app.module_has_hover_card(&module));
        let Some(HoverCard::Windows(list)) = app.hover_card_for(&module) else {
            panic!("a window list");
        };
        assert_eq!(
            list.rows.iter().map(|r| r.id).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(list.count, 3);
        assert_eq!(list.rows[0].title, "Document 1");
        assert_eq!(
            list.rows[2].detail.as_deref(),
            Some(fl!("shell-tooltip-workspace", index = 2).as_str())
        );

        // Thumbnails replace the icon once they arrive.
        let _ = app.update(Message::HoverCard(Event::Previews(vec![(
            2,
            320,
            180,
            vec![0],
        )])));
        let Some(HoverCard::Windows(list)) = app.hover_card_for(&module) else {
            panic!("a window list");
        };
        assert!(list.rows[1].thumbnail.is_some());
        assert!(list.rows[0].thumbnail.is_none());

        // Turning the tooltip off turns the card off.
        if let ModuleParams::Taskbar(config) = &mut app.config.modules[0].params {
            config.tooltip = crate::config::TooltipSetting::Disabled;
        }
        assert!(!app.module_has_hover_card(&module));
    }

    #[test]
    fn taskbar_cards_name_the_scratchpad_instead_of_the_workspace() {
        let mut app = app();
        let mut hidden = window(5, 1, 4, "foot");
        hidden.is_floating = true;
        hidden.layout.pos_in_scrolling_layout = None;
        hidden.layout.in_scratchpad = true;
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            niri_ipc::Event::WindowOpenedOrChanged { window: hidden },
        ))));
        let module = crate::modules::taskbar::TaskbarItemModule::module_id("taskbar", "foot");
        let Some(HoverCard::Windows(list)) = app.hover_card_for(&module) else {
            panic!("a window list");
        };
        let detail = |id| {
            list.rows
                .iter()
                .find(|row| row.id == id)
                .and_then(|row| row.detail.clone())
        };
        assert_eq!(detail(5), Some(fl!("shell-taskbar-scratchpad")));
        assert_eq!(detail(1), Some(fl!("shell-tooltip-workspace", index = 1)));
    }

    #[test]
    fn workspace_cards_draw_the_workspace_strip() {
        let app = app();
        let module = ModuleId::workspace("workspaces", 1);
        let Some(HoverCard::Workspace(card)) = app.hover_card_for(&module) else {
            panic!("a workspace card");
        };
        assert_eq!(card.title, fl!("shell-tooltip-workspace", index = 1));
        assert_eq!(card.strip.columns.len(), 3);
        assert_eq!(
            card.faces.iter().map(|f| f.id).collect::<Vec<_>>(),
            [1, 2, 4]
        );
        assert_eq!(card.faces[2].app.name, "mpv");
    }

    #[test]
    fn cards_wait_for_the_pointer_and_close_after_it_leaves_both() {
        let mut app = app();
        let bar = iced::window::Id::unique();
        app.surface_mut(bar).bar_name = "main".into();
        let module = crate::modules::taskbar::TaskbarItemModule::module_id("taskbar", "foot");
        let popup = iced::window::Id::unique();
        app.surfaces
            .insert_tooltip_popup(super::super::surfaces::TooltipPopup {
                window: popup,
                target_window: bar,
                target_module: module.clone(),
                tree: crate::surface::tooltip_popup::TooltipTree {
                    segments: Vec::new(),
                    card: app.hover_card_for(&module),
                    anchor: (40, 300),
                    bar_size: (76, 800),
                    screen_size: (76, 800),
                    bar_edge: crate::config::Edge::Left,
                },
            });

        // Leaving the icon keeps the card for a moment.
        assert!(app.defer_hover_card_close(bar, &module));
        assert!(app.hover_card_closing());
        // The pointer reaches the card in time.
        app.hover_card_pointer(popup, PointerChange::Entered);
        assert!(!app.hover_card_closing());
        // Back to the same icon: the card stays.
        assert_eq!(app.hover_card_enter(bar, &module), HoverCardEnter::Keep);
        // Another icon opens its card sooner.
        let other = crate::modules::taskbar::TaskbarItemModule::module_id("taskbar", "mpv");
        assert!(matches!(
            app.hover_card_enter(bar, &other),
            HoverCardEnter::Warm(_)
        ));

        app.hover_card_pointer(popup, PointerChange::Left);
        app.hover_cards.close_at = Some(Instant::now());
        let _ = app.hover_card_tick();
        assert!(app.surfaces.tooltip_popup(popup).is_none());
    }

    #[test]
    fn choosing_a_window_closes_the_card_and_closing_one_keeps_it() {
        let mut app = app();
        let bar = iced::window::Id::unique();
        app.surface_mut(bar).bar_name = "main".into();
        let module = ModuleId::workspace("workspaces", 1);
        let popup = iced::window::Id::unique();
        app.surfaces
            .insert_tooltip_popup(super::super::surfaces::TooltipPopup {
                window: popup,
                target_window: bar,
                target_module: module.clone(),
                tree: crate::surface::tooltip_popup::TooltipTree {
                    segments: Vec::new(),
                    card: app.hover_card_for(&module),
                    anchor: (40, 300),
                    bar_size: (76, 800),
                    screen_size: (76, 800),
                    bar_edge: crate::config::Edge::Left,
                },
            });
        let _ = app.update(Message::HoverCard(Event::CloseWindow(1)));
        assert!(app.surfaces.tooltip_popup(popup).is_some());
        let _ = app.update(Message::HoverCard(Event::FocusWindow(4)));
        assert!(app.surfaces.tooltip_popup(popup).is_none());
    }
}
