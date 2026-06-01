#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowRole {
    BarSurface,
    ClosingPanel,
    Menu,
    Panel,
    Tooltip,
    Unknown,
}

#[derive(Debug, Clone)]
pub(super) enum SurfaceInputEffect {
    BarActionKey {
        window: iced::window::Id,
        modifiers: iced::keyboard::Modifiers,
        key: iced::keyboard::Key,
    },
    BarClick(iced::window::Id),
    BarFocusGained(iced::window::Id),
    BarFocusLost(iced::window::Id),
    MoveFocus {
        window: iced::window::Id,
        forward: bool,
    },
    BarFocusZone {
        window: iced::window::Id,
        forward: bool,
    },
    BarHoverEnter(iced::window::Id),
    BarHoverExit(iced::window::Id),
    KeyboardActivate {
        window: iced::window::Id,
    },
    KeyboardSecondaryActivate {
        window: iced::window::Id,
    },
    BarRightClick(iced::window::Id),
    CloseMenu,
    CloseTooltip(iced::window::Id),
    Ignore,
    ModuleHoverEnter(crate::modules::ModuleId, iced::window::Id),
    ModuleHoverExit(crate::modules::ModuleId, iced::window::Id),
    PanelBack,
    Escape {
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
    WindowClosed(iced::window::Id),
    ClosingPanelWindowClosed(iced::window::Id),
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
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::WindowClosed(*window),
        _ => SurfaceInputEffect::Ignore,
    }
}

fn route_closing_panel_input(event: &super::messages::SurfaceEvent) -> SurfaceInputEffect {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::ClosingPanelWindowClosed(*window),
        _ => SurfaceInputEffect::Ignore,
    }
}

fn route_panel_input(event: &super::messages::SurfaceEvent) -> SurfaceInputEffect {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::WindowSized(window, _, _) | SurfaceEvent::BarFocusGained(window) => {
            SurfaceInputEffect::PanelFocusGained(*window)
        }
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::WindowClosed(*window),
        SurfaceEvent::BarFocusLost(_) => SurfaceInputEffect::Ignore,
        SurfaceEvent::BackPressed(_) => SurfaceInputEffect::PanelBack,
        SurfaceEvent::EscapePressed(window) => SurfaceInputEffect::Escape { window: *window },
        SurfaceEvent::FocusMoveNext(window) => SurfaceInputEffect::MoveFocus {
            window: *window,
            forward: true,
        },
        SurfaceEvent::FocusMovePrev(window) => SurfaceInputEffect::MoveFocus {
            window: *window,
            forward: false,
        },
        SurfaceEvent::KeyboardActivate(window) => {
            SurfaceInputEffect::KeyboardActivate { window: *window }
        }
        _ => SurfaceInputEffect::Ignore,
    }
}

fn route_menu_input(event: &super::messages::SurfaceEvent) -> SurfaceInputEffect {
    use super::messages::SurfaceEvent;

    match event {
        SurfaceEvent::WindowClosed(window) => SurfaceInputEffect::MenuWindowClosed(*window),
        SurfaceEvent::BarFocusLost(_) => SurfaceInputEffect::CloseMenu,
        SurfaceEvent::EscapePressed(window) => SurfaceInputEffect::Escape { window: *window },
        SurfaceEvent::FocusMoveNext(window) => SurfaceInputEffect::MoveFocus {
            window: *window,
            forward: true,
        },
        SurfaceEvent::FocusMovePrev(window) => SurfaceInputEffect::MoveFocus {
            window: *window,
            forward: false,
        },
        SurfaceEvent::KeyboardActivate(window) => {
            SurfaceInputEffect::KeyboardActivate { window: *window }
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
        SurfaceEvent::BarClick(window) => SurfaceInputEffect::BarClick(*window),
        SurfaceEvent::BarRightClick(window) => SurfaceInputEffect::BarRightClick(*window),
        SurfaceEvent::BarHoverEnter(window) => SurfaceInputEffect::BarHoverEnter(*window),
        SurfaceEvent::BarHoverExit(window) => SurfaceInputEffect::BarHoverExit(*window),
        SurfaceEvent::BarFocusGained(window) => SurfaceInputEffect::BarFocusGained(*window),
        SurfaceEvent::BarFocusLost(window) => SurfaceInputEffect::BarFocusLost(*window),
        SurfaceEvent::EscapePressed(window) => SurfaceInputEffect::Escape { window: *window },
        SurfaceEvent::FocusMoveNext(window) => SurfaceInputEffect::MoveFocus {
            window: *window,
            forward: true,
        },
        SurfaceEvent::FocusMovePrev(window) => SurfaceInputEffect::MoveFocus {
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
            SurfaceInputEffect::KeyboardActivate { window: *window }
        }
        SurfaceEvent::KeyboardSecondaryActivate(window) => {
            SurfaceInputEffect::KeyboardSecondaryActivate { window: *window }
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
        SurfaceEvent::BarClick(window)
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
            | (WindowClosed(left), WindowClosed(right))
            | (ClosingPanelWindowClosed(left), ClosingPanelWindowClosed(right))
            | (MenuWindowClosed(left), MenuWindowClosed(right)) => left == right,
            (Escape { window: left }, Escape { window: right }) => left == right,
            (KeyboardActivate { window: left }, KeyboardActivate { window: right })
            | (
                KeyboardSecondaryActivate { window: left },
                KeyboardSecondaryActivate { window: right },
            ) => left == right,
            (
                MoveFocus {
                    window: lw,
                    forward: lf,
                },
                MoveFocus {
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
            (CloseMenu, CloseMenu)
            | (Ignore, Ignore)
            | (PanelBack, PanelBack)
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
            SurfaceInputEffect::MoveFocus {
                window,
                forward: true,
            }
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
            SurfaceInputEffect::Escape { window }
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
            SurfaceInputEffect::KeyboardActivate { window }
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
    fn closing_panel_window_closed_routes_to_closing_panel_window_closed_intent() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::ClosingPanel,
                &super::super::messages::SurfaceEvent::WindowClosed(window),
            ),
            SurfaceInputEffect::ClosingPanelWindowClosed(window)
        );
    }

    #[test]
    fn panel_window_closed_routes_to_plain_window_closed_intent() {
        let window = wid();

        assert_eq!(
            route_surface_input(
                WindowRole::Panel,
                &super::super::messages::SurfaceEvent::WindowClosed(window),
            ),
            SurfaceInputEffect::WindowClosed(window)
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
