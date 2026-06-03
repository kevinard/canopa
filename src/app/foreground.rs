use super::panel_runtime::{PanelAttachmentId, PanelWindowOpenRequest};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ForegroundOverlay {
    Panel {
        attachment_id: PanelAttachmentId,
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
    #[cfg(test)]
    OpenPanel {
        tree: crate::surface::panel::PanelTree,
    },
    OpenPanelWindow(PanelWindowOpenRequest),
    ClosePanel,
    PanelWindowOpened {
        attachment_id: PanelAttachmentId,
        window: iced::window::Id,
    },
    PanelWindowClosed {
        attachment_id: PanelAttachmentId,
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
    OpenPanel(PanelWindowOpenRequest),
    ClosePanel {
        window: iced::window::Id,
        attachment_id: PanelAttachmentId,
    },
    CloseMenu {
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    },
}

#[derive(Debug, Default)]
pub(super) struct Foreground {
    active: Option<ForegroundOverlay>,
    #[cfg(test)]
    next_panel_attachment_id: u64,
}

impl Foreground {
    pub(super) fn handle(&mut self, intent: ForegroundIntent) -> Vec<ForegroundEffect> {
        match intent {
            #[cfg(test)]
            ForegroundIntent::OpenPanel { tree } => self.open_panel(tree),
            ForegroundIntent::OpenPanelWindow(request) => self.open_panel_window(request),
            ForegroundIntent::ClosePanel => self.close_panel(),
            ForegroundIntent::PanelWindowOpened {
                attachment_id,
                window,
            } => {
                self.panel_window_opened(attachment_id, window);
                Vec::new()
            }
            ForegroundIntent::PanelWindowClosed {
                attachment_id,
                window,
            } => {
                self.panel_window_closed(attachment_id, window);
                Vec::new()
            }
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

    #[cfg(test)]
    pub(super) fn current_panel_attachment(&self) -> Option<PanelAttachmentId> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel { attachment_id, .. } => Some(*attachment_id),
            ForegroundOverlay::Menu { .. } => None,
        }
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
        if self.is_panel_window(window) {
            return Some(super::surface_input_router::WindowRole::Panel);
        }
        if self.is_menu_window(window) {
            return Some(super::surface_input_router::WindowRole::Menu);
        }
        None
    }

    #[cfg(test)]
    fn open_panel(&mut self, tree: crate::surface::panel::PanelTree) -> Vec<ForegroundEffect> {
        let attachment_id = self.allocate_panel_attachment_id();
        self.open_panel_window(PanelWindowOpenRequest {
            attachment_id,
            tree,
        })
    }

    fn open_panel_window(&mut self, request: PanelWindowOpenRequest) -> Vec<ForegroundEffect> {
        let mut effects = self.close();
        self.active = Some(ForegroundOverlay::Panel {
            attachment_id: request.attachment_id,
            window: None,
            tree: request.tree.clone(),
        });
        effects.push(ForegroundEffect::OpenPanel(request));
        effects
    }

    fn close_panel(&mut self) -> Vec<ForegroundEffect> {
        if matches!(self.active, Some(ForegroundOverlay::Panel { .. })) {
            self.close()
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
        let effects = self.close();
        self.active = Some(ForegroundOverlay::Menu {
            window,
            meta: meta.clone(),
        });
        effects
    }

    fn close(&mut self) -> Vec<ForegroundEffect> {
        let active = self.active.take();
        match active {
            Some(ForegroundOverlay::Panel {
                window: Some(window),
                attachment_id,
                ..
            }) => vec![ForegroundEffect::ClosePanel {
                window,
                attachment_id,
            }],
            Some(ForegroundOverlay::Panel { window: None, .. }) | None => Vec::new(),
            Some(ForegroundOverlay::Menu { window, meta }) => {
                vec![ForegroundEffect::CloseMenu { window, meta }]
            }
        }
    }

    fn panel_window_opened(&mut self, attachment_id: PanelAttachmentId, window: iced::window::Id) {
        if let Some(ForegroundOverlay::Panel {
            attachment_id: current,
            window: current_window,
            ..
        }) = self.active.as_mut()
            && *current == attachment_id
        {
            *current_window = Some(window);
        }
    }

    fn panel_window_closed(&mut self, attachment_id: PanelAttachmentId, window: iced::window::Id) {
        let is_current = matches!(
            self.active.as_ref(),
            Some(ForegroundOverlay::Panel {
                attachment_id: current,
                window: Some(current_window),
                ..
            }) if *current == attachment_id && *current_window == window
        );
        if is_current {
            self.active = None;
        }
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

    #[cfg(test)]
    fn allocate_panel_attachment_id(&mut self) -> PanelAttachmentId {
        self.next_panel_attachment_id = self.next_panel_attachment_id.saturating_add(1);
        PanelAttachmentId::new(self.next_panel_attachment_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::surfaces;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn panel_tree(
        origin_window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) -> crate::surface::panel::PanelTree {
        crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content,
            origin_window,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 1080),
        }
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

    fn open_panel_window_request(
        tree: crate::surface::panel::PanelTree,
        attachment_id: u64,
    ) -> ForegroundEffect {
        ForegroundEffect::OpenPanel(PanelWindowOpenRequest {
            attachment_id: PanelAttachmentId::for_test(attachment_id),
            tree,
        })
    }

    #[test]
    fn panel_open_replace_current_close_and_stale_close_are_owned_by_foreground() {
        let origin = wid();
        let old_window = wid();
        let current_window = wid();
        let old_tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);
        let current_tree = panel_tree(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );
        let mut foreground = Foreground::default();

        assert!(foreground.active_overlay().is_none());

        let first = foreground.handle(ForegroundIntent::OpenPanel {
            tree: old_tree.clone(),
        });
        assert_eq!(first, vec![open_panel_window_request(old_tree, 1)]);
        assert_eq!(
            foreground.current_panel_attachment(),
            Some(PanelAttachmentId::for_test(1))
        );
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            attachment_id: PanelAttachmentId::for_test(1),
            window: old_window,
        });

        let replacement = foreground.handle(ForegroundIntent::OpenPanel {
            tree: current_tree.clone(),
        });
        assert_eq!(
            replacement,
            vec![
                ForegroundEffect::ClosePanel {
                    window: old_window,
                    attachment_id: PanelAttachmentId::for_test(1),
                },
                open_panel_window_request(current_tree.clone(), 2),
            ]
        );
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            attachment_id: PanelAttachmentId::for_test(2),
            window: current_window,
        });

        foreground.handle(ForegroundIntent::PanelWindowClosed {
            attachment_id: PanelAttachmentId::for_test(1),
            window: old_window,
        });
        assert_eq!(foreground.active_panel_window(), Some(current_window));
        assert_eq!(foreground.active_panel_tree(), Some(&current_tree));

        foreground.handle(ForegroundIntent::PanelWindowClosed {
            attachment_id: PanelAttachmentId::for_test(2),
            window: current_window,
        });
        assert!(foreground.active_overlay().is_none());
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
    fn opening_menu_replaces_panel_and_opening_panel_replaces_menu() {
        let origin = wid();
        let panel_window = wid();
        let menu_window = wid();
        let tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);
        let meta = menu_meta("org.menu");
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::OpenPanel { tree: tree.clone() });
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            attachment_id: PanelAttachmentId::for_test(1),
            window: panel_window,
        });

        let menu_effects = foreground.handle(ForegroundIntent::OpenMenu {
            window: menu_window,
            meta: meta.clone(),
        });
        assert_eq!(
            menu_effects,
            vec![ForegroundEffect::ClosePanel {
                window: panel_window,
                attachment_id: PanelAttachmentId::for_test(1),
            }]
        );
        assert_eq!(foreground.active_menu_window(), Some(menu_window));
        assert_eq!(foreground.active_menu_meta(), Some(&meta));

        let panel_effects = foreground.handle(ForegroundIntent::OpenPanel { tree: tree.clone() });
        assert_eq!(
            panel_effects,
            vec![
                ForegroundEffect::CloseMenu {
                    window: menu_window,
                    meta,
                },
                open_panel_window_request(tree, 2),
            ]
        );
    }
}
