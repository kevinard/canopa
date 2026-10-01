#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowRole {
    Dismissal,
    BarSurface,
    ClosingPanel,
    Menu,
    Panel,
    Tooltip,
    Unknown,
}

#[derive(Debug, Clone)]
pub(super) enum SurfaceInputEffect {
    ClosePanel,
    BarActionKey {
        window: iced::window::Id,
        modifiers: iced::keyboard::Modifiers,
        key: iced::keyboard::Key,
    },
    BarClick(iced::window::Id),
    BarFocusGained(iced::window::Id),
    BarFocusLost(iced::window::Id),
    BarMoveFocus {
        window: iced::window::Id,
        forward: bool,
    },
    BarFocusZone {
        window: iced::window::Id,
        forward: bool,
    },
    BarHoverEnter(iced::window::Id),
    BarHoverExit(iced::window::Id),
    BarKeyboardActivate {
        window: iced::window::Id,
    },
    BarKeyboardSecondaryActivate {
        window: iced::window::Id,
    },
    BarRightClick(iced::window::Id),
    CloseMenu,
    CloseTooltip(iced::window::Id),
    Ignore,
    ModuleHoverEnter(crate::modules::ModuleId, iced::window::Id),
    ModuleHoverExit(crate::modules::ModuleId, iced::window::Id),
    PanelBack,
    PanelEscape,
    PanelMoveFocus {
        forward: bool,
    },
    PanelKeyboardActivate,
    BarEscape {
        window: iced::window::Id,
    },
    MenuMoveFocus {
        window: iced::window::Id,
        forward: bool,
    },
    MenuKeyboardActivate {
        window: iced::window::Id,
    },
    PanelFocusGained(iced::window::Id),
    Tick,
    TogglePanel,
    TooltipDwellElapsed {
        window: iced::window::Id,
        module: crate::modules::ModuleId,
    },
    TrackCursor {
        window: iced::window::Id,
        position: iced::Point,
    },
    BarResized {
        window: iced::window::Id,
        width: u32,
        height: u32,
    },
    TooltipResized {
        window: iced::window::Id,
        width: u32,
        height: u32,
    },
    BarWindowClosed(iced::window::Id),
    PanelWindowClosed(iced::window::Id),
    RegistryWindowClosed(iced::window::Id),
    MenuWindowClosed(iced::window::Id),
}

pub(super) fn route_surface_input(
    role: WindowRole,
    event: &super::messages::SurfaceEvent,
) -> SurfaceInputEffect {
    if let Some(effect) = route_global_input(event) {
        return effect;
    }

    match role {
        WindowRole::Dismissal => match event {
            super::messages::SurfaceEvent::PointerPressed(_) => SurfaceInputEffect::ClosePanel,
            _ => SurfaceInputEffect::Ignore,
        },
        WindowRole::BarSurface => route_bar_input(event),
        WindowRole::ClosingPanel => route_closing_panel_input(event),
        WindowRole::Menu => route_menu_input(event),
        WindowRole::Panel => route_panel_input(event),
        WindowRole::Tooltip => route_tooltip_input(event),
        WindowRole::Unknown => SurfaceInputEffect::Ignore,
    }
}

fn route_global_input(event: &super::messages::SurfaceEvent) -> Option<SurfaceInputEffect> {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::Tick => Some(SurfaceInputEffect::Tick),
        SurfaceEvent::TogglePanel => Some(SurfaceInputEffect::TogglePanel),
        SurfaceEvent::ModuleHoverEnter(id, window) => {
            Some(SurfaceInputEffect::ModuleHoverEnter(id.clone(), *window))
        }
        SurfaceEvent::ModuleHoverExit(id, window) => {
            Some(SurfaceInputEffect::ModuleHoverExit(id.clone(), *window))
        }
        SurfaceEvent::TooltipDwellElapsed { window, module } => {
            Some(SurfaceInputEffect::TooltipDwellElapsed {
                window: *window,
                module: module.clone(),
            })
        }
        SurfaceEvent::CloseTooltip { window } => Some(SurfaceInputEffect::CloseTooltip(*window)),
        _ => None,
    }
}

fn route_tooltip_input(event: &super::messages::SurfaceEvent) -> SurfaceInputEffect {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::WindowSized(window, width, height) => SurfaceInputEffect::TooltipResized {
            window: *window,
            width: *width,
            height: *height,
        },
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::RegistryWindowClosed(*window),
        _ => SurfaceInputEffect::Ignore,
    }
}

fn route_closing_panel_input(event: &super::messages::SurfaceEvent) -> SurfaceInputEffect {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::PanelWindowClosed(*window),
        _ => SurfaceInputEffect::Ignore,
    }
}

fn route_panel_input(event: &super::messages::SurfaceEvent) -> SurfaceInputEffect {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::WindowSized(window, _, _) | SurfaceEvent::BarFocusGained(window) => {
            SurfaceInputEffect::PanelFocusGained(*window)
        }
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::PanelWindowClosed(*window),
        SurfaceEvent::BarFocusLost(_) => SurfaceInputEffect::Ignore,
        SurfaceEvent::BackPressed(_) => SurfaceInputEffect::PanelBack,
        SurfaceEvent::EscapePressed(_) => SurfaceInputEffect::PanelEscape,
        SurfaceEvent::FocusMoveNext(_) | SurfaceEvent::FocusNextZone(_) => {
            SurfaceInputEffect::PanelMoveFocus { forward: true }
        }
        SurfaceEvent::FocusMovePrev(_) | SurfaceEvent::FocusPrevZone(_) => {
            SurfaceInputEffect::PanelMoveFocus { forward: false }
        }
        SurfaceEvent::KeyboardActivate(_) => SurfaceInputEffect::PanelKeyboardActivate,
        _ => SurfaceInputEffect::Ignore,
    }
}

fn route_menu_input(event: &super::messages::SurfaceEvent) -> SurfaceInputEffect {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::MenuWindowClosed(*window),
        SurfaceEvent::BarFocusLost(_) => SurfaceInputEffect::CloseMenu,
        SurfaceEvent::EscapePressed(_) => SurfaceInputEffect::CloseMenu,
        SurfaceEvent::FocusMoveNext(window) => SurfaceInputEffect::MenuMoveFocus {
            window: *window,
            forward: true,
        },
        SurfaceEvent::FocusMovePrev(window) => SurfaceInputEffect::MenuMoveFocus {
            window: *window,
            forward: false,
        },
        SurfaceEvent::KeyboardActivate(window) => {
            SurfaceInputEffect::MenuKeyboardActivate { window: *window }
        }
        _ => SurfaceInputEffect::Ignore,
    }
}

fn route_bar_input(event: &super::messages::SurfaceEvent) -> SurfaceInputEffect {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::CursorMoved(window, position) => SurfaceInputEffect::TrackCursor {
            window: *window,
            position: *position,
        },
        SurfaceEvent::WindowSized(window, width, height) => SurfaceInputEffect::BarResized {
            window: *window,
            width: *width,
            height: *height,
        },
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::BarWindowClosed(*window),
        SurfaceEvent::BarClick(window) => SurfaceInputEffect::BarClick(*window),
        SurfaceEvent::BarRightClick(window) => SurfaceInputEffect::BarRightClick(*window),
        SurfaceEvent::BarHoverEnter(window) => SurfaceInputEffect::BarHoverEnter(*window),
        SurfaceEvent::BarHoverExit(window) => SurfaceInputEffect::BarHoverExit(*window),
        SurfaceEvent::BarFocusGained(window) => SurfaceInputEffect::BarFocusGained(*window),
        SurfaceEvent::BarFocusLost(window) => SurfaceInputEffect::BarFocusLost(*window),
        SurfaceEvent::EscapePressed(window) => SurfaceInputEffect::BarEscape { window: *window },
        SurfaceEvent::FocusMoveNext(window) => SurfaceInputEffect::BarMoveFocus {
            window: *window,
            forward: true,
        },
        SurfaceEvent::FocusMovePrev(window) => SurfaceInputEffect::BarMoveFocus {
            window: *window,
            forward: false,
        },
        SurfaceEvent::FocusNextZone(window) => SurfaceInputEffect::BarFocusZone {
            window: *window,
            forward: true,
        },
        SurfaceEvent::FocusPrevZone(window) => SurfaceInputEffect::BarFocusZone {
            window: *window,
            forward: false,
        },
        SurfaceEvent::KeyboardActivate(window) => {
            SurfaceInputEffect::BarKeyboardActivate { window: *window }
        }
        SurfaceEvent::KeyboardSecondaryActivate(window) => {
            SurfaceInputEffect::BarKeyboardSecondaryActivate { window: *window }
        }
        SurfaceEvent::ActionKeyPressed {
            window,
            modifiers,
            key,
        } => SurfaceInputEffect::BarActionKey {
            window: *window,
            modifiers: *modifiers,
            key: key.clone(),
        },
        _ => SurfaceInputEffect::Ignore,
    }
}

pub(super) fn route_for_role(
    classify: impl FnOnce(Option<iced::window::Id>) -> WindowRole,
    event: &super::messages::SurfaceEvent,
) -> SurfaceInputEffect {
    route_surface_input(classify(target_window(event)), event)
}

pub(super) fn target_window(event: &super::messages::SurfaceEvent) -> Option<iced::window::Id> {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::PointerPressed(window)
        | SurfaceEvent::BarClick(window)
        | SurfaceEvent::BarRightClick(window)
        | SurfaceEvent::BarHoverEnter(window)
        | SurfaceEvent::BarHoverExit(window)
        | SurfaceEvent::BarFocusGained(window)
        | SurfaceEvent::BarFocusLost(window)
        | SurfaceEvent::EscapePressed(window)
        | SurfaceEvent::FocusMoveNext(window)
        | SurfaceEvent::FocusMovePrev(window)
        | SurfaceEvent::FocusNextZone(window)
        | SurfaceEvent::FocusPrevZone(window)
        | SurfaceEvent::KeyboardActivate(window)
        | SurfaceEvent::KeyboardSecondaryActivate(window)
        | SurfaceEvent::BackPressed(window)
        | SurfaceEvent::CursorMoved(window, _)
        | SurfaceEvent::WindowSized(window, _, _)
        | SurfaceEvent::WindowClosed(window)
        | SurfaceEvent::CloseTooltip { window }
        | SurfaceEvent::TooltipDwellElapsed { window, .. }
        | SurfaceEvent::ModuleHoverEnter(_, window)
        | SurfaceEvent::ModuleHoverExit(_, window)
        | SurfaceEvent::ActionKeyPressed { window, .. } => Some(*window),
        SurfaceEvent::Tick | SurfaceEvent::TogglePanel => None,
    }
}

impl PartialEq for SurfaceInputEffect {
    fn eq(&self, other: &Self) -> bool {
        use SurfaceInputEffect::*;

        match (self, other) {
            (
                BarActionKey {
                    window: lw,
                    modifiers: lm,
                    key: lk,
                },
                BarActionKey {
                    window: rw,
                    modifiers: rm,
                    key: rk,
                },
            ) => lw == rw && lm == rm && format!("{lk:?}") == format!("{rk:?}"),
            (BarClick(left), BarClick(right))
            | (BarFocusGained(left), BarFocusGained(right))
            | (BarFocusLost(left), BarFocusLost(right))
            | (BarHoverEnter(left), BarHoverEnter(right))
            | (BarHoverExit(left), BarHoverExit(right))
            | (BarRightClick(left), BarRightClick(right))
            | (CloseTooltip(left), CloseTooltip(right))
            | (PanelFocusGained(left), PanelFocusGained(right))
            | (BarWindowClosed(left), BarWindowClosed(right))
            | (PanelWindowClosed(left), PanelWindowClosed(right))
            | (RegistryWindowClosed(left), RegistryWindowClosed(right))
            | (MenuWindowClosed(left), MenuWindowClosed(right)) => left == right,
            (BarEscape { window: left }, BarEscape { window: right }) => left == right,
            (BarKeyboardActivate { window: left }, BarKeyboardActivate { window: right })
            | (MenuKeyboardActivate { window: left }, MenuKeyboardActivate { window: right })
            | (
                BarKeyboardSecondaryActivate { window: left },
                BarKeyboardSecondaryActivate { window: right },
            ) => left == right,
            (
                BarMoveFocus {
                    window: lw,
                    forward: lf,
                },
                BarMoveFocus {
                    window: rw,
                    forward: rf,
                },
            )
            | (
                MenuMoveFocus {
                    window: lw,
                    forward: lf,
                },
                MenuMoveFocus {
                    window: rw,
                    forward: rf,
                },
            )
            | (
                BarFocusZone {
                    window: lw,
                    forward: lf,
                },
                BarFocusZone {
                    window: rw,
                    forward: rf,
                },
            ) => lw == rw && lf == rf,
            (ModuleHoverEnter(left_id, left_window), ModuleHoverEnter(right_id, right_window))
            | (ModuleHoverExit(left_id, left_window), ModuleHoverExit(right_id, right_window)) => {
                left_id == right_id && left_window == right_window
            }
            (
                TooltipDwellElapsed {
                    window: left_window,
                    module: left_module,
                },
                TooltipDwellElapsed {
                    window: right_window,
                    module: right_module,
                },
            ) => left_window == right_window && left_module == right_module,
            (
                TrackCursor {
                    window: left_window,
                    position: left_position,
                },
                TrackCursor {
                    window: right_window,
                    position: right_position,
                },
            ) => left_window == right_window && left_position == right_position,
            (
                BarResized {
                    window: lw,
                    width: lwidth,
                    height: lheight,
                },
                BarResized {
                    window: rw,
                    width: rwidth,
                    height: rheight,
                },
            )
            | (
                TooltipResized {
                    window: lw,
                    width: lwidth,
                    height: lheight,
                },
                TooltipResized {
                    window: rw,
                    width: rwidth,
                    height: rheight,
                },
            ) => lw == rw && lwidth == rwidth && lheight == rheight,
            (ClosePanel, ClosePanel)
            | (CloseMenu, CloseMenu)
            | (Ignore, Ignore)
            | (PanelBack, PanelBack)
            | (PanelEscape, PanelEscape)
            | (PanelKeyboardActivate, PanelKeyboardActivate)
            | (PanelMoveFocus { forward: true }, PanelMoveFocus { forward: true })
            | (PanelMoveFocus { forward: false }, PanelMoveFocus { forward: false })
            | (Tick, Tick)
            | (TogglePanel, TogglePanel) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    #[test]
    fn pointer_dismisses_only_from_a_dismissal_surface() {
        let event = super::super::messages::SurfaceEvent::PointerPressed(wid());
        assert_eq!(
            route_surface_input(WindowRole::Dismissal, &event),
            SurfaceInputEffect::ClosePanel
        );
        for role in [
            WindowRole::Panel,
            WindowRole::ClosingPanel,
            WindowRole::BarSurface,
            WindowRole::Menu,
            WindowRole::Unknown,
        ] {
            assert_eq!(
                route_surface_input(role, &event),
                SurfaceInputEffect::Ignore
            );
        }
    }

    #[test]
    fn panel_focus_lost_is_ignored_for_click_outside_dismissal() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Panel,
                &super::super::messages::SurfaceEvent::BarFocusLost(window),
            ),
            SurfaceInputEffect::Ignore
        );
    }

    #[test]
    fn menu_focus_lost_closes_menu() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Menu,
                &super::super::messages::SurfaceEvent::BarFocusLost(window),
            ),
            SurfaceInputEffect::CloseMenu
        );
    }

    #[test]
    fn panel_focus_move_next_moves_focus_forward() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Panel,
                &super::super::messages::SurfaceEvent::FocusMoveNext(window),
            ),
            SurfaceInputEffect::PanelMoveFocus { forward: true }
        );
    }

    #[test]
    fn menu_escape_routes_to_escape_intent() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Menu,
                &super::super::messages::SurfaceEvent::EscapePressed(window),
            ),
            SurfaceInputEffect::CloseMenu
        );
    }

    #[test]
    fn tooltip_focus_movement_is_ignored() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Tooltip,
                &super::super::messages::SurfaceEvent::FocusMoveNext(window),
            ),
            SurfaceInputEffect::Ignore
        );
    }

    #[test]
    fn menu_keyboard_activate_routes_to_activate_intent() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Menu,
                &super::super::messages::SurfaceEvent::KeyboardActivate(window),
            ),
            SurfaceInputEffect::MenuKeyboardActivate { window }
        );
    }

    #[test]
    fn tooltip_window_sized_routes_to_tooltip_resized() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Tooltip,
                &super::super::messages::SurfaceEvent::WindowSized(window, 80, 40),
            ),
            SurfaceInputEffect::TooltipResized {
                window,
                width: 80,
                height: 40,
            }
        );
    }

    #[test]
    fn menu_window_closed_routes_to_menu_window_closed_intent() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Menu,
                &super::super::messages::SurfaceEvent::WindowClosed(window),
            ),
            SurfaceInputEffect::MenuWindowClosed(window)
        );
    }

    #[test]
    fn closing_panel_window_closed_routes_to_panel_window_closed_intent() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::ClosingPanel,
                &super::super::messages::SurfaceEvent::WindowClosed(window),
            ),
            SurfaceInputEffect::PanelWindowClosed(window)
        );
    }

    #[test]
    fn panel_window_closed_routes_to_panel_window_closed_intent() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Panel,
                &super::super::messages::SurfaceEvent::WindowClosed(window),
            ),
            SurfaceInputEffect::PanelWindowClosed(window)
        );
    }

    #[test]
    fn target_window_extracts_module_hover_window() {
        let window = wid();

        assert_eq!(
            target_window(&super::super::messages::SurfaceEvent::ModuleHoverEnter(
                crate::modules::ModuleId::new("clock"),
                window,
            )),
            Some(window)
        );
    }

    #[test]
    fn target_window_is_none_for_global_toggle_panel() {
        assert_eq!(
            target_window(&super::super::messages::SurfaceEvent::TogglePanel),
            None
        );
    }

    #[test]
    fn route_for_role_uses_target_window_and_role() {
        let window = wid();

        assert_eq!(
            route_for_role(
                |_| WindowRole::Menu,
                &super::super::messages::SurfaceEvent::WindowClosed(window),
            ),
            SurfaceInputEffect::MenuWindowClosed(window)
        );
    }
}
