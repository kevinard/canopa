use super::*;

fn values(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    pairs
        .iter()
        .map(|(key, value)| (*key, (*value).to_owned()))
        .collect()
}

#[test]
fn window_keys_name_the_surface_kind_and_window() {
    let window = iced::window::Id::unique();
    let bar = window_key(Surface::Bar, window);
    let panel = window_key(Surface::Panel, window);
    let menu = window_key(Surface::Menu, window);
    assert!(bar.starts_with("shell-bar-"));
    assert!(panel.starts_with("shell-panel-"));
    assert!(menu.starts_with("shell-menu-"));
    assert_ne!(window_key(Surface::Bar, iced::window::Id::unique()), bar);
    assert_eq!(
        window_node(Surface::Panel, window, "Launcher").key_str(),
        Some(panel.as_str())
    );
}

#[test]
fn bar_modules_read_their_name_then_their_value() {
    let clock = module_summary(
        ModuleKind::Clock,
        "clock",
        &values(&[("icon", "⌚"), ("time", "12:00"), ("date", "Wed 23 Sep")]),
    );
    assert_eq!(clock.label, "Clock, 12:00, Wed 23 Sep");
    let battery = module_summary(
        ModuleKind::Battery,
        "battery",
        &values(&[("percent", "80"), ("state", "charging")]),
    );
    assert_eq!(battery.label, "Battery, 80%, charging");
    let normal = module_summary(
        ModuleKind::Battery,
        "battery",
        &values(&[("percent", "55"), ("state", "normal")]),
    );
    assert_eq!(normal.label, "Battery, 55%");
    let full = module_summary(
        ModuleKind::Battery,
        "battery",
        &values(&[("percent", "100"), ("state", "full")]),
    );
    assert_eq!(full.label, "Battery, 100%, fully charged");
    let muted = module_summary(
        ModuleKind::Volume,
        "volume",
        &values(&[("percent", "40"), ("state", "muted")]),
    );
    assert_eq!(muted.label, "Volume, muted");
    let loud = module_summary(
        ModuleKind::Volume,
        "volume",
        &values(&[("percent", "40"), ("state", "unmuted")]),
    );
    assert_eq!(loud.label, "Volume, 40%");
    let network = module_summary(
        ModuleKind::Network,
        "network",
        &values(&[
            ("icon", "x"),
            ("label", "Home Wi-Fi"),
            ("state", "connected"),
        ]),
    );
    assert_eq!(network.label, "Network, Home Wi-Fi");
    // No data yet: the module is still named.
    assert_eq!(
        module_summary(ModuleKind::Network, "n", &[]).label,
        "Network"
    );
}

#[test]
fn notifications_and_keep_awake_state_their_state() {
    let notifications = module_summary(
        ModuleKind::Notifications,
        "notifications",
        &values(&[
            ("value", "3"),
            ("state", "dnd"),
            ("label", "3 require attention"),
        ]),
    );
    assert_eq!(
        notifications.label,
        "Notifications, 3 need attention, Do Not Disturb on"
    );
    assert_eq!(
        notifications.description.as_deref(),
        Some("3 require attention")
    );
    let quiet = module_summary(
        ModuleKind::Notifications,
        "notifications",
        &values(&[("value", "0"), ("state", "available")]),
    );
    assert_eq!(quiet.label, "Notifications, none need attention");
    let unavailable = module_summary(
        ModuleKind::Notifications,
        "notifications",
        &values(&[("value", "?"), ("state", "unavailable")]),
    );
    assert_eq!(unavailable.label, "Notifications, unavailable");

    let awake = module_summary(
        ModuleKind::IdleInhibitor,
        "idle",
        &values(&[("state", "active"), ("label", "Click to allow idle")]),
    );
    assert_eq!(awake.label, "Keep awake");
    assert_eq!(awake.toggled, Some(true));
    let off = module_summary(
        ModuleKind::IdleInhibitor,
        "idle",
        &values(&[("state", "inactive")]),
    );
    assert_eq!(off.toggled, Some(false));
    let pending = module_summary(
        ModuleKind::IdleInhibitor,
        "idle",
        &values(&[("state", "pending")]),
    );
    assert_eq!(
        (pending.label.as_str(), pending.toggled),
        ("Keep awake, pending", None)
    );
}

#[test]
fn workspaces_and_tray_items_have_names_without_tooltips() {
    let focused = module_summary(
        ModuleKind::WorkspaceItem,
        "workspaces",
        &values(&[
            ("index", "2"),
            ("label", "2"),
            ("name", "web"),
            ("state", "focused"),
        ]),
    );
    assert_eq!(focused.label, "Workspace 2, web, focused");
    assert_eq!(focused.selected, Some(true));
    let other = module_summary(
        ModuleKind::WorkspaceItem,
        "workspaces",
        &values(&[
            ("index", "3"),
            ("label", "3"),
            ("name", ""),
            ("state", "inactive"),
            ("urgent", "true"),
        ]),
    );
    assert_eq!(other.label, "Workspace 3, urgent");
    assert_eq!(other.selected, Some(false));

    let titled = module_summary(
        ModuleKind::TrayIcon,
        "tray",
        &values(&[
            ("title", "Konsole"),
            ("description", "2 sessions"),
            ("service", ":1.4"),
        ]),
    );
    assert_eq!(titled.label, "Konsole");
    assert_eq!(titled.description.as_deref(), Some("2 sessions"));
    let untitled = module_summary(
        ModuleKind::TrayIcon,
        "tray",
        &values(&[("icon-name", "nm-applet"), ("service", ":1.4")]),
    );
    assert_eq!(untitled.label, "nm-applet");
    assert_eq!(
        module_summary(ModuleKind::TrayIcon, "tray", &[]).label,
        "Tray item"
    );
    let script = module_summary(ModuleKind::Script, "updates", &values(&[("output", "4")]));
    assert_eq!(script.label, "updates, 4");
}

#[test]
fn bar_module_nodes_are_buttons_keyed_by_module_identity() {
    let id = ModuleId::workspace("workspaces", 7);
    let node = bar_module_node(
        &id,
        ModuleSummary {
            label: "Workspace 1".into(),
            ..Default::default()
        },
        true,
        false,
        Message::PerfReport,
    );
    assert_eq!(node.role(), accessibility::Role::Button);
    assert_eq!(node.key_str(), Some("module:workspaces:workspace:7"));
    assert_eq!(
        ModuleId::tray("tray", "org.kde.konsole").accessibility_key(),
        "module:tray:tray:org.kde.konsole"
    );
    assert_eq!(ModuleId::new("clock").accessibility_key(), "module:clock");
}

#[test]
fn menu_rows_are_keyed_by_their_target_not_their_label() {
    let entry = |target| PanelMenuEntry {
        label: "Translated label".into(),
        icon: None,
        path: vec![],
        target,
        is_submenu: false,
    };
    assert_eq!(
        menu_entry_key(&entry(PanelMenuEntryTarget::StaticPath(vec![1, 2])), 5),
        "menu-item-1.2"
    );
    assert_eq!(
        menu_entry_key(
            &entry(PanelMenuEntryTarget::InstalledApp("firefox.desktop".into())),
            0
        ),
        "menu-app-firefox.desktop"
    );
    assert_eq!(
        menu_entry_key(&entry(PanelMenuEntryTarget::SniMenuItem("12".into())), 0),
        "menu-sni-12"
    );
    assert_eq!(
        menu_entry_key(&entry(PanelMenuEntryTarget::DmenuValue("a".into())), 3),
        "menu-choice-3"
    );
}

#[test]
fn launcher_failures_are_assertive_and_never_announced_twice() {
    assert_eq!(
        launcher_announcements("", None, Some("4 matches")),
        ("4 matches".into(), String::new())
    );
    assert_eq!(
        launcher_announcements("Searching files…", None, Some("4 matches")),
        ("Searching files…".into(), String::new())
    );
    assert_eq!(
        launcher_announcements("No matches", None, None),
        ("No matches".into(), String::new())
    );
    assert_eq!(
        launcher_announcements(
            "Searching files… · History is unreadable",
            Some("History is unreadable"),
            None
        ),
        ("Searching files…".into(), "History is unreadable".into())
    );
    assert_eq!(
        launcher_announcements(
            "Could not launch",
            Some("Could not launch"),
            Some("1 match")
        ),
        ("1 match".into(), "Could not launch".into())
    );
}

#[test]
fn published_surfaces_follow_what_the_shell_shows() {
    use crate::app::messages::PanelEvent;
    use crate::config::Config;
    let (mut app, _) = App::boot(Config::default(), vec![]);
    let bar = iced::window::Id::unique();
    app.surface_mut(bar);
    app.set_accessibility_for_test(ShellAccessibility::with_bridge(Bridge::new(
        "shell-surfaces-test",
    )));
    let _ = app.update(Message::PerfReport);
    let bar_key = window_key(Surface::Bar, bar);
    assert!(app.accessibility_for_test().unwrap().is_published(&bar_key));

    // Quick settings is the Shell's own panel; it is published while shown.
    let _ = app.update(Message::Panel(PanelEvent::Open {
        origin_window: bar,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content: crate::surface::panel::PanelContent::BarPanel,
        anchor: None,
    }));
    let panel = app.foreground.active_panel_window().expect("panel window");
    let panel_key = window_key(Surface::Panel, panel);
    assert!(
        app.accessibility_for_test()
            .unwrap()
            .is_published(&panel_key)
    );
    let _ = app.update(Message::Panel(PanelEvent::Close));
    assert!(
        !app.accessibility_for_test()
            .unwrap()
            .is_published(&panel_key)
    );
    assert!(app.accessibility_for_test().unwrap().is_published(&bar_key));
}
