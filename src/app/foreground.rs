use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PanelForegroundRequest {
    pub(super) origin_window: iced::window::Id,
    pub(super) origin: crate::surface::panel::PanelOrigin,
    pub(super) content: crate::surface::panel::PanelContent,
    pub(super) anchor: Option<iced::Rectangle>,
    pub(super) origin_bar_name: String,
    pub(super) origin_output_name: String,
    pub(super) screen_size: (u32, u32),
}

impl PanelForegroundRequest {
    pub(super) fn tree(&self) -> crate::surface::panel::PanelTree {
        crate::surface::panel::PanelTree {
            origin: self.origin.clone(),
            content: self.content.clone(),
            origin_window: self.origin_window,
            origin_bar_name: self.origin_bar_name.clone(),
            origin_output_name: self.origin_output_name.clone(),
            anchor: self.anchor,
            screen_size: self.screen_size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelOpenMode {
    Open,
    Toggle,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ForegroundOverlay {
    Panel {
        window: Option<iced::window::Id>,
        tree: crate::surface::panel::PanelTree,
    },
    Menu {
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    },
}

#[derive(Debug, Clone)]
pub(super) enum ForegroundIntent {
    OpenPanel {
        request: PanelForegroundRequest,
    },
    TogglePanel {
        request: PanelForegroundRequest,
    },
    ClosePanel,
    PanelRuntimePrepared {
        tree: crate::surface::panel::PanelTree,
    },
    PanelWindowOpened {
        window: iced::window::Id,
    },
    PanelWindowClosed {
        window: iced::window::Id,
    },
    #[cfg(test)]
    OpenMenu {
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    },
    MenuWindowOpened {
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    },
    MenuWindowClosed {
        window: iced::window::Id,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ForegroundEffect {
    PreparePanelRuntime {
        request: PanelForegroundRequest,
        mode: PanelOpenMode,
    },
    ClearPanelRuntime,
    OpenPanelWindow {
        tree: crate::surface::panel::PanelTree,
    },
    RemoveWindow(iced::window::Id),
    SuspendBarKeyboardInteractivity,
    RestoreBarKeyboardInteractivity,
    CloseTooltipsForSurface(iced::window::Id),
    CloseMenu {
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    },
}

#[derive(Debug, Default)]
pub(super) struct Foreground {
    active: Option<ForegroundOverlay>,
    closing_panel_windows: HashSet<iced::window::Id>,
}

impl Foreground {
    pub(super) fn handle(&mut self, intent: ForegroundIntent) -> Vec<ForegroundEffect> {
        match intent {
            ForegroundIntent::OpenPanel { request } => {
                self.prepare_panel(request, PanelOpenMode::Open)
            }
            ForegroundIntent::TogglePanel { request } => self.toggle_panel(request),
            ForegroundIntent::ClosePanel => self.close_panel(),
            ForegroundIntent::PanelRuntimePrepared { tree } => self.panel_runtime_prepared(tree),
            ForegroundIntent::PanelWindowOpened { window } => self.panel_window_opened(window),
            ForegroundIntent::PanelWindowClosed { window } => self.panel_window_closed(window),
            #[cfg(test)]
            ForegroundIntent::OpenMenu { window, meta } => self.open_menu(window, meta),
            ForegroundIntent::MenuWindowOpened { window, meta } => {
                self.track_menu_window_opened(window, meta);
                Vec::new()
            }
            ForegroundIntent::MenuWindowClosed { window } => {
                self.menu_window_closed(window);
                Vec::new()
            }
        }
    }

    #[cfg(test)]
    pub(super) fn active_overlay(&self) -> Option<&ForegroundOverlay> {
        self.active.as_ref()
    }

    pub(super) fn has_active_panel(&self) -> bool {
        matches!(self.active, Some(ForegroundOverlay::Panel { .. }))
    }

    pub(super) fn is_closing_panel_window(&self, window: iced::window::Id) -> bool {
        self.closing_panel_windows.contains(&window)
    }

    pub(super) fn active_panel_window(&self) -> Option<iced::window::Id> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel { window, .. } => *window,
            ForegroundOverlay::Menu { .. } => None,
        }
    }

    #[cfg(test)]
    pub(super) fn active_panel_tree(&self) -> Option<&crate::surface::panel::PanelTree> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel { tree, .. } => Some(tree),
            ForegroundOverlay::Menu { .. } => None,
        }
    }

    pub(super) fn active_panel_origin_for(
        &self,
        origin_window: iced::window::Id,
    ) -> Option<&crate::surface::panel::PanelOrigin> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel { tree, .. } if tree.origin_window == origin_window => {
                Some(&tree.origin)
            }
            ForegroundOverlay::Panel { .. } | ForegroundOverlay::Menu { .. } => None,
        }
    }

    pub(super) fn panel_tree_for_window(
        &self,
        window: iced::window::Id,
    ) -> Option<&crate::surface::panel::PanelTree> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel {
                window: Some(current),
                tree,
            } if *current == window => Some(tree),
            ForegroundOverlay::Panel { .. } | ForegroundOverlay::Menu { .. } => None,
        }
    }

    pub(super) fn is_menu_panel_window(&self, window: iced::window::Id) -> bool {
        self.panel_tree_for_window(window).is_some_and(|tree| {
            matches!(
                tree.content,
                crate::surface::panel::PanelContent::Menu(_)
                    | crate::surface::panel::PanelContent::DmenuRequest
            )
        })
    }

    pub(super) fn active_menu_window(&self) -> Option<iced::window::Id> {
        match self.active.as_ref()? {
            ForegroundOverlay::Menu { window, .. } => Some(*window),
            ForegroundOverlay::Panel { .. } => None,
        }
    }

    #[cfg(test)]
    pub(super) fn active_menu_meta(&self) -> Option<&super::surfaces::PopupMeta> {
        match self.active.as_ref()? {
            ForegroundOverlay::Menu { meta, .. } => Some(meta),
            ForegroundOverlay::Panel { .. } => None,
        }
    }

    pub(super) fn is_panel_window(&self, window: iced::window::Id) -> bool {
        self.active_panel_window() == Some(window)
    }

    pub(super) fn is_menu_window(&self, window: iced::window::Id) -> bool {
        self.active_menu_window() == Some(window)
    }

    pub(super) fn window_role(
        &self,
        window: Option<iced::window::Id>,
    ) -> Option<super::surface_input_router::WindowRole> {
        let window = window?;
        if self.is_closing_panel_window(window) {
            return Some(super::surface_input_router::WindowRole::ClosingPanel);
        }
        if self.is_panel_window(window) {
            return Some(super::surface_input_router::WindowRole::Panel);
        }
        if self.is_menu_window(window) {
            return Some(super::surface_input_router::WindowRole::Menu);
        }
        None
    }

    fn prepare_panel(
        &mut self,
        request: PanelForegroundRequest,
        mode: PanelOpenMode,
    ) -> Vec<ForegroundEffect> {
        vec![ForegroundEffect::PreparePanelRuntime { request, mode }]
    }

    fn toggle_panel(&mut self, request: PanelForegroundRequest) -> Vec<ForegroundEffect> {
        if self.active_panel_matches(&request) {
            return self.close_panel();
        }
        self.prepare_panel(request, PanelOpenMode::Toggle)
    }

    fn panel_runtime_prepared(
        &mut self,
        tree: crate::surface::panel::PanelTree,
    ) -> Vec<ForegroundEffect> {
        let mut effects = self.close_for_replacement();
        effects.push(ForegroundEffect::CloseTooltipsForSurface(
            tree.origin_window,
        ));
        effects.push(ForegroundEffect::SuspendBarKeyboardInteractivity);
        self.active = Some(ForegroundOverlay::Panel {
            window: None,
            tree: tree.clone(),
        });
        effects.push(ForegroundEffect::OpenPanelWindow { tree });
        effects
    }

    fn close_panel(&mut self) -> Vec<ForegroundEffect> {
        if matches!(self.active, Some(ForegroundOverlay::Panel { .. })) {
            self.close_active_panel()
        } else {
            Vec::new()
        }
    }

    #[cfg(test)]
    fn open_menu(
        &mut self,
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    ) -> Vec<ForegroundEffect> {
        let effects = self.close_for_replacement();
        self.active = Some(ForegroundOverlay::Menu { window, meta });
        effects
    }

    fn close_for_replacement(&mut self) -> Vec<ForegroundEffect> {
        let active = self.active.take();
        match active {
            Some(ForegroundOverlay::Panel {
                window: Some(window),
                ..
            }) => {
                self.closing_panel_windows.insert(window);
                vec![ForegroundEffect::RemoveWindow(window)]
            }
            Some(ForegroundOverlay::Panel { window: None, .. }) | None => Vec::new(),
            Some(ForegroundOverlay::Menu { window, meta }) => {
                vec![ForegroundEffect::CloseMenu { window, meta }]
            }
        }
    }

    fn close_active_panel(&mut self) -> Vec<ForegroundEffect> {
        let active = self.active.take();
        let mut effects = match active {
            Some(ForegroundOverlay::Panel {
                window: Some(window),
                ..
            }) => {
                self.closing_panel_windows.insert(window);
                vec![ForegroundEffect::RemoveWindow(window)]
            }
            Some(ForegroundOverlay::Panel { window: None, .. }) => Vec::new(),
            Some(ForegroundOverlay::Menu { window, meta }) => {
                self.active = Some(ForegroundOverlay::Menu { window, meta });
                return Vec::new();
            }
            None => return Vec::new(),
        };
        effects.push(ForegroundEffect::ClearPanelRuntime);
        effects.push(ForegroundEffect::RestoreBarKeyboardInteractivity);
        effects
    }

    fn panel_window_opened(&mut self, window: iced::window::Id) -> Vec<ForegroundEffect> {
        if let Some(ForegroundOverlay::Panel {
            window: current_window,
            ..
        }) = self.active.as_mut()
            && current_window.is_none()
        {
            *current_window = Some(window);
        }
        Vec::new()
    }

    fn panel_window_closed(&mut self, window: iced::window::Id) -> Vec<ForegroundEffect> {
        if self.closing_panel_windows.remove(&window) {
            return Vec::new();
        }
        let is_current = matches!(
            self.active.as_ref(),
            Some(ForegroundOverlay::Panel {
                window: Some(current_window),
                ..
            }) if *current_window == window
        );
        if is_current {
            self.active = None;
            return vec![
                ForegroundEffect::ClearPanelRuntime,
                ForegroundEffect::RestoreBarKeyboardInteractivity,
            ];
        }
        Vec::new()
    }

    fn menu_window_closed(&mut self, window: iced::window::Id) {
        if self.is_menu_window(window) {
            self.active = None;
        }
    }

    fn track_menu_window_opened(
        &mut self,
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    ) {
        self.active = Some(ForegroundOverlay::Menu { window, meta });
    }

    fn active_panel_matches(&self, request: &PanelForegroundRequest) -> bool {
        matches!(
            self.active.as_ref(),
            Some(ForegroundOverlay::Panel { tree, .. })
                if tree.origin_window == request.origin_window
                    && tree.origin == request.origin
                    && tree.content == request.content
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::surfaces;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn panel_request(
        origin_window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) -> super::PanelForegroundRequest {
        super::PanelForegroundRequest {
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content,
            anchor: None,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            screen_size: (1920, 1080),
        }
    }

    fn panel_tree(
        origin_window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) -> crate::surface::panel::PanelTree {
        panel_request(origin_window, content).tree()
    }

    fn menu_meta(service: &str) -> surfaces::PopupMeta {
        surfaces::PopupMeta {
            service: service.into(),
            menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
            tree: crate::surface::menu_popup::MenuTree {
                items: Vec::new(),
                anchor: (0, 0),
                screen_size: (1920, 1080),
            },
            origin: surfaces::BarSurfaceIdentity::all_screens(),
        }
    }

    fn panel_prepare_effect(
        request: super::PanelForegroundRequest,
        mode: PanelOpenMode,
    ) -> ForegroundEffect {
        ForegroundEffect::PreparePanelRuntime { request, mode }
    }

    fn panel_open_effects(
        origin: iced::window::Id,
        tree: crate::surface::panel::PanelTree,
    ) -> Vec<ForegroundEffect> {
        vec![
            ForegroundEffect::CloseTooltipsForSurface(origin),
            ForegroundEffect::SuspendBarKeyboardInteractivity,
            ForegroundEffect::OpenPanelWindow { tree },
        ]
    }

    #[test]
    fn opening_panel_prepares_panel_runtime_before_opening_window() {
        let origin = wid();
        let mut foreground = Foreground::default();
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);

        let effects = foreground.handle(ForegroundIntent::OpenPanel {
            request: request.clone(),
        });

        assert_eq!(
            effects,
            vec![panel_prepare_effect(request, PanelOpenMode::Open)]
        );
        assert!(foreground.active_overlay().is_none());
    }

    #[test]
    fn prepared_panel_is_foreground_before_window_opens() {
        let origin = wid();
        let mut foreground = Foreground::default();
        let tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);

        let effects =
            foreground.handle(ForegroundIntent::PanelRuntimePrepared { tree: tree.clone() });

        assert_eq!(effects, panel_open_effects(origin, tree.clone()));
        assert_eq!(foreground.active_panel_tree(), Some(&tree));
        assert_eq!(foreground.active_panel_window(), None);
    }

    #[test]
    fn replacing_panel_closes_old_window_and_opens_new_panel() {
        let origin = wid();
        let old_window = wid();
        let old_tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);
        let new_tree = panel_tree(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::PanelRuntimePrepared {
            tree: old_tree.clone(),
        });
        foreground.handle(ForegroundIntent::PanelWindowOpened { window: old_window });

        let effects = foreground.handle(ForegroundIntent::PanelRuntimePrepared {
            tree: new_tree.clone(),
        });

        let mut expected = vec![ForegroundEffect::RemoveWindow(old_window)];
        expected.extend(panel_open_effects(origin, new_tree.clone()));
        assert_eq!(effects, expected);
        assert_eq!(foreground.active_panel_tree(), Some(&new_tree));
        assert_eq!(foreground.active_panel_window(), None);
        assert!(foreground.is_closing_panel_window(old_window));
    }

    #[test]
    fn stale_panel_window_close_does_not_clear_current_panel() {
        let origin = wid();
        let old_window = wid();
        let current_window = wid();
        let old_tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);
        let current_tree = panel_tree(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::PanelRuntimePrepared { tree: old_tree });
        foreground.handle(ForegroundIntent::PanelWindowOpened { window: old_window });
        foreground.handle(ForegroundIntent::PanelRuntimePrepared {
            tree: current_tree.clone(),
        });
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            window: current_window,
        });

        let effects = foreground.handle(ForegroundIntent::PanelWindowClosed { window: old_window });

        assert!(effects.is_empty());
        assert_eq!(foreground.active_panel_window(), Some(current_window));
        assert_eq!(foreground.active_panel_tree(), Some(&current_tree));
        assert!(!foreground.is_closing_panel_window(old_window));
    }

    #[test]
    fn current_panel_window_close_clears_foreground_and_panel_runtime() {
        let origin = wid();
        let panel_window = wid();
        let tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::PanelRuntimePrepared { tree });
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            window: panel_window,
        });

        let effects = foreground.handle(ForegroundIntent::PanelWindowClosed {
            window: panel_window,
        });

        assert_eq!(
            effects,
            vec![
                ForegroundEffect::ClearPanelRuntime,
                ForegroundEffect::RestoreBarKeyboardInteractivity,
            ]
        );
        assert!(foreground.active_overlay().is_none());
    }

    #[test]
    fn toggle_same_panel_closes_through_foreground() {
        let origin = wid();
        let window = wid();
        let mut foreground = Foreground::default();
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);

        foreground.handle(ForegroundIntent::PanelRuntimePrepared {
            tree: request.tree(),
        });
        foreground.handle(ForegroundIntent::PanelWindowOpened { window });

        let effects = foreground.handle(ForegroundIntent::TogglePanel { request });

        assert_eq!(
            effects,
            vec![
                ForegroundEffect::RemoveWindow(window),
                ForegroundEffect::ClearPanelRuntime,
                ForegroundEffect::RestoreBarKeyboardInteractivity,
            ]
        );
        assert!(foreground.active_overlay().is_none());
    }

    #[test]
    fn toggle_different_panel_prepares_replacement() {
        let origin = wid();
        let mut foreground = Foreground::default();
        let active = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let replacement = panel_request(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );

        foreground.handle(ForegroundIntent::PanelRuntimePrepared {
            tree: active.tree(),
        });

        let effects = foreground.handle(ForegroundIntent::TogglePanel {
            request: replacement.clone(),
        });

        assert_eq!(
            effects,
            vec![panel_prepare_effect(replacement, PanelOpenMode::Toggle)]
        );
    }

    #[test]
    fn panel_opening_while_menu_is_active_prepares_before_closing_menu() {
        let origin = wid();
        let menu_window = wid();
        let meta = menu_meta("org.menu");
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let tree = request.tree();
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::OpenMenu {
            window: menu_window,
            meta: meta.clone(),
        });

        let prepare = foreground.handle(ForegroundIntent::OpenPanel {
            request: request.clone(),
        });

        assert_eq!(
            prepare,
            vec![panel_prepare_effect(request, PanelOpenMode::Open)]
        );
        assert_eq!(foreground.active_menu_window(), Some(menu_window));

        let open = foreground.handle(ForegroundIntent::PanelRuntimePrepared { tree: tree.clone() });
        let mut expected = vec![ForegroundEffect::CloseMenu {
            window: menu_window,
            meta,
        }];
        expected.extend(panel_open_effects(origin, tree));
        assert_eq!(open, expected);
    }

    #[test]
    fn menu_open_replace_current_close_and_stale_close_are_owned_by_foreground() {
        let old_window = wid();
        let current_window = wid();
        let old_meta = menu_meta("org.old");
        let current_meta = menu_meta("org.current");
        let mut foreground = Foreground::default();

        let first = foreground.handle(ForegroundIntent::OpenMenu {
            window: old_window,
            meta: old_meta.clone(),
        });
        assert!(first.is_empty());
        assert_eq!(foreground.active_menu_window(), Some(old_window));
        assert_eq!(foreground.active_menu_meta(), Some(&old_meta));

        let replacement = foreground.handle(ForegroundIntent::OpenMenu {
            window: current_window,
            meta: current_meta.clone(),
        });
        assert_eq!(
            replacement,
            vec![ForegroundEffect::CloseMenu {
                window: old_window,
                meta: old_meta,
            }]
        );
        assert_eq!(foreground.active_menu_window(), Some(current_window));
        assert_eq!(foreground.active_menu_meta(), Some(&current_meta));

        foreground.handle(ForegroundIntent::MenuWindowClosed { window: old_window });
        assert_eq!(foreground.active_menu_window(), Some(current_window));
        assert_eq!(foreground.active_menu_meta(), Some(&current_meta));

        foreground.handle(ForegroundIntent::MenuWindowClosed {
            window: current_window,
        });
        assert!(foreground.active_overlay().is_none());
    }

    #[test]
    fn opening_menu_replaces_panel() {
        let origin = wid();
        let panel_window = wid();
        let menu_window = wid();
        let tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);
        let meta = menu_meta("org.menu");
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::PanelRuntimePrepared { tree });
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            window: panel_window,
        });

        let menu_effects = foreground.handle(ForegroundIntent::OpenMenu {
            window: menu_window,
            meta: meta.clone(),
        });
        assert_eq!(
            menu_effects,
            vec![ForegroundEffect::RemoveWindow(panel_window)]
        );
        assert_eq!(foreground.active_menu_window(), Some(menu_window));
        assert_eq!(foreground.active_menu_meta(), Some(&meta));
    }
}
