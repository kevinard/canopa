use super::*;
use crate::app::messages::{IpcEvent, ipc_responder};
use crate::ipc::protocol::IpcResponse;
use crate::surface::panel::PanelContent;

fn open_menu(app: &mut App, name: &str) -> IpcResponse {
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    let _ = app.update(Message::Ipc(IpcEvent::OpenMenu {
        name: name.into(),
        responder: ipc_responder(tx),
    }));
    rx.try_recv()
        .expect("open-menu responds without waiting for menu selection")
}

fn toggle_menu(app: &mut App, name: &str) -> IpcResponse {
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    let _ = app.update(Message::Ipc(IpcEvent::ToggleMenu {
        name: name.into(),
        responder: ipc_responder(tx),
    }));
    rx.try_recv().expect("toggle-menu responds immediately")
}

#[test]
fn toggle_menu_opens_closes_and_switches_named_menus() {
    let mut config = config_with_menu(menu_with_two_entries());
    let mut custom = menu_with_apps_browser();
    custom.name = "Tools ☰".into();
    config.menus.push(custom);
    let (mut app, _) = App::boot(config, vec![]);
    app.surface_mut(iced::window::Id::unique());

    assert_eq!(toggle_menu(&mut app, "main"), IpcResponse::Accepted);
    assert_eq!(active_menu_entries(&app)[0].label, "Terminal");
    assert_eq!(toggle_menu(&mut app, "main"), IpcResponse::Accepted);
    assert!(!app.foreground.has_active_panel());
    assert_eq!(toggle_menu(&mut app, "main"), IpcResponse::Accepted);
    assert_eq!(toggle_menu(&mut app, "Tools ☰"), IpcResponse::Accepted);
    assert_eq!(
        app.foreground.active_panel_tree().unwrap().content,
        PanelContent::Menu("Tools ☰".into())
    );
    let panel = app.foreground.active_panel_window();
    assert!(matches!(
        toggle_menu(&mut app, "missing"),
        IpcResponse::Rejected { .. }
    ));
    assert_eq!(app.foreground.active_panel_window(), panel);
}

#[test]
fn toggle_menu_closes_a_menu_opened_from_a_module_on_another_output() {
    let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
    let first = iced::window::Id::unique();
    app.surface_mut(first).output_name = "DP-1".into();
    let other = iced::window::Id::unique();
    app.surface_mut(other).output_name = "DP-2".into();
    let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
        origin_window: other,
        origin: crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
            "launcher",
        )),
        content: PanelContent::Menu("apps".into()),
        anchor: None,
    }));
    assert_eq!(
        app.foreground.active_panel_tree().unwrap().origin_window,
        other
    );

    assert_eq!(toggle_menu(&mut app, "apps"), IpcResponse::Accepted);
    assert!(!app.foreground.has_active_panel());
    assert_eq!(toggle_menu(&mut app, "apps"), IpcResponse::Accepted);
    assert_eq!(
        app.foreground.active_panel_tree().unwrap().origin_window,
        first
    );
    assert_eq!(active_menu_entries(&app)[0].label, "Alpha");
}

#[test]
fn open_menu_opens_named_menus_and_repeated_requests_keep_a_panel_open() {
    let mut custom = menu_with_two_entries();
    custom.name = "My tools ☰".into();
    let mut config = config_with_menu(custom);
    config.menus.push(menu_with_apps_browser());
    let (mut app, _) = App::boot(config, vec![]);
    app.surface_mut(iced::window::Id::unique());

    for name in ["My tools ☰", "My tools ☰", "main"] {
        assert_eq!(open_menu(&mut app, name), IpcResponse::Accepted);
        assert_eq!(
            app.foreground.active_panel_tree().unwrap().content,
            PanelContent::Menu(name.into())
        );
        assert!(!active_menu_entries(&app).is_empty());
    }
    let panel = app.foreground.active_panel_window();
    assert!(matches!(
        open_menu(&mut app, "Main"),
        IpcResponse::Rejected { reason } if reason.contains("unknown menu")
    ));
    assert_eq!(app.foreground.active_panel_window(), panel);
    assert_eq!(
        app.foreground.active_panel_tree().unwrap().content,
        PanelContent::Menu("main".into())
    );
}

#[test]
fn open_menu_loads_installed_applications() {
    let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
    app.surface_mut(iced::window::Id::unique());

    assert_eq!(open_menu(&mut app, "apps"), IpcResponse::Accepted);
    assert_eq!(active_menu_entries(&app)[0].label, "Alpha");
}

#[test]
fn open_menu_rejects_without_a_bar_surface() {
    let (mut app, _) = App::boot(config_with_menu(menu_with_two_entries()), vec![]);
    assert!(matches!(
        open_menu(&mut app, "main"),
        IpcResponse::Rejected { reason } if reason.contains("no bar surface")
    ));
    assert!(!app.foreground.has_active_panel());
    assert!(matches!(
        toggle_menu(&mut app, "main"),
        IpcResponse::Rejected { .. }
    ));
}

#[test]
fn open_menu_uses_focused_output_and_falls_back_when_it_has_no_bar() {
    let (mut app, _) = App::boot(config_with_menu(menu_with_two_entries()), vec![]);
    let fallback = iced::window::Id::unique();
    let focused = iced::window::Id::unique();
    app.surface_mut(fallback).output_name = "DP-1".into();
    app.surface_mut(focused).output_name = "eDP-1".into();

    for (output, expected) in [("eDP-1", focused), ("missing", fallback)] {
        let _ = app.update(Message::Niri(crate::app::messages::AppNiriEvent::Ipc(
            crate::niri::NiriEvent::Event(niri_ipc::Event::WorkspacesChanged {
                workspaces: vec![niri_ipc::Workspace {
                    id: 1,
                    idx: 1,
                    name: None,
                    output: Some(output.into()),
                    is_urgent: false,
                    is_active: true,
                    is_focused: true,
                    active_window_id: None,
                }],
            }),
        )));
        assert_eq!(open_menu(&mut app, "main"), IpcResponse::Accepted);
        assert_eq!(
            app.foreground.active_panel_tree().unwrap().origin_window,
            expected
        );
    }
}

#[test]
fn open_menu_replacement_completes_a_pending_dmenu_request() {
    let (mut app, _) = App::boot(config_with_menu(menu_with_two_entries()), vec![]);
    app.surface_mut(iced::window::Id::unique());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    let _ = app.update(Message::Ipc(IpcEvent::DmenuSelection {
        request: crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("one")],
            allow_custom: false,
        },
        responder: ipc_responder(tx),
    }));

    assert_eq!(open_menu(&mut app, "main"), IpcResponse::Accepted);
    assert_eq!(rx.try_recv().unwrap(), IpcResponse::Cancelled);
    assert_eq!(active_menu_entries(&app)[0].label, "Terminal");
}
