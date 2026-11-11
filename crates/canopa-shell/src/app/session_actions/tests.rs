use super::*;
use crate::app::messages::PanelEvent;
use crate::modules::{ModuleId, ModuleState};
use crate::session::Availability;
use crate::surface::panel::{PanelContent, PanelOrigin};

fn default_app() -> (App, iced::window::Id) {
    let (config, _) = crate::config::load(&canopa_config::Request::Defaults, false).unwrap();
    let (mut app, _) = App::boot(config, vec!["DP-1".into()]);
    for module in app.config.modules.clone() {
        if module.module_type == "button" {
            app.modules.set_state(
                ModuleId::new(&module.name),
                ModuleState::Button(crate::modules::button::ButtonState),
            );
        }
    }
    let window = *app.surfaces.keys().next().unwrap();
    (app, window)
}

fn quick_settings(app: &App, window: iced::window::Id) -> Vec<String> {
    app.panel_module_layout_for_content(window, &PanelOrigin::Bar, &PanelContent::BarPanel)
        .into_iter()
        .map(|entry| entry.id.configured_name().to_string())
        .filter(|name| name.starts_with("session-"))
        .collect()
}

#[test]
fn quick_settings_follow_logind_answers() {
    let (mut app, window) = default_app();
    // Before logind answers, everything but hibernate is offered.
    assert_eq!(
        quick_settings(&app, window),
        [
            "session-lock",
            "session-suspend",
            "session-logout",
            "session-reboot",
            "session-poweroff"
        ]
    );
    let _ = app.update(Message::Session(Event::Capabilities(Capabilities {
        suspend: Availability::NotApplicable,
        hibernate: Availability::Yes,
        reboot: Availability::No,
        power_off: Availability::Challenge,
    })));
    assert_eq!(
        quick_settings(&app, window),
        [
            "session-lock",
            "session-hibernate",
            "session-logout",
            "session-poweroff"
        ]
    );
}

#[test]
fn refused_items_are_left_out_of_menus() {
    let mut menus = vec![MenuConfig {
        name: "Power".into(),
        title: None,
        show_icons: false,
        prompt: None,
        source: None,
        items: [
            ("Suspend", SessionAction::Suspend),
            ("Hibernate", SessionAction::Hibernate),
            ("Restart", SessionAction::Reboot),
        ]
        .into_iter()
        .map(|(label, action)| MenuItemConfig {
            label: label.into(),
            icon: None,
            keywords: Vec::new(),
            action: Some(PanelAction::Session(action)),
            children: Vec::new(),
        })
        .collect(),
    }];
    let nested = menus[0].items[1].clone();
    menus[0].items.push(MenuItemConfig {
        label: "More".into(),
        icon: None,
        keywords: Vec::new(),
        action: None,
        children: vec![nested],
    });
    let all = Capabilities {
        suspend: Availability::Yes,
        hibernate: Availability::Yes,
        reboot: Availability::Yes,
        power_off: Availability::Yes,
    };
    assert!(matches!(offered_menus(&menus, &all), Cow::Borrowed(_)));
    let shown = offered_menus(
        &menus,
        &Capabilities {
            hibernate: Availability::NotApplicable,
            ..all
        },
    );
    let labels: Vec<_> = shown[0]
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(labels, ["Suspend", "Restart", "More"]);
    assert!(shown[0].items[2].children.is_empty());
}

#[test]
fn confirmed_reboot_asks_logind_once_and_failures_are_reported_with_blockers() {
    let (mut app, window) = default_app();
    let _ = app.update(Message::Panel(PanelEvent::Open {
        origin_window: window,
        origin: PanelOrigin::Bar,
        content: PanelContent::Menu("Reboot?".into()),
        anchor: None,
    }));
    let _ = app.update(Message::Panel(PanelEvent::ActivateIndex(1)));
    assert_eq!(app.session.pending, Some(SessionAction::Reboot));
    // A second request while polkit is asking is ignored.
    let _ = app.request_session_action(SessionAction::PowerOff);
    assert_eq!(app.session.pending, Some(SessionAction::Reboot));

    let firefox = Inhibitor {
        who: "Firefox".into(),
        why: "Downloading files".into(),
    };
    let _ = app.update(Message::Session(Event::Finished(
        SessionAction::Reboot,
        Err(Failure::Blocked(vec![firefox.clone()])),
    )));
    assert_eq!(app.session.pending, None);
    let Some(crate::notice::Command::Show(NoticeSlot::Session, notice)) = app.notices.sent.last()
    else {
        panic!("a failed request must be reported: {:?}", app.notices.sent)
    };
    assert_eq!(notice.summary, "Couldn't restart");
    assert!(notice.body.contains("Firefox: Downloading files"));

    // The next confirmation names the blocker under its prompt.
    let content = PanelContent::Menu("Reboot?".into());
    assert_eq!(
        app.session_prompt(&content, Some("Unsaved work may be lost.".into())),
        Some("Unsaved work may be lost.\nBlocked by Firefox: Downloading files.".into())
    );
    let _ = app.update(Message::Session(Event::Blockers(
        SessionAction::Reboot,
        Vec::new(),
    )));
    assert_eq!(
        app.session_prompt(&content, Some("Unsaved work may be lost.".into())),
        Some("Unsaved work may be lost.".into())
    );
    assert_eq!(
        app.session_prompt(&PanelContent::BarPanel, Some("x".into())),
        Some("x".into())
    );

    let _ = app.request_session_action(SessionAction::Reboot);
    let _ = app.update(Message::Session(Event::Finished(
        SessionAction::Reboot,
        Ok(()),
    )));
    assert_eq!(
        app.notices.sent.last(),
        Some(&crate::notice::Command::Clear(NoticeSlot::Session))
    );
}

#[test]
fn suspend_runs_without_confirmation() {
    let (mut app, window) = default_app();
    let _ = app.update(Message::Panel(PanelEvent::Open {
        origin_window: window,
        origin: PanelOrigin::Bar,
        content: PanelContent::BarPanel,
        anchor: None,
    }));
    let effects = app.dispatch_panel_action(
        window,
        PanelOrigin::Module(ModuleId::new("session-suspend")),
        PanelAction::Session(SessionAction::Suspend),
    );
    drop(effects);
    assert_eq!(app.session.pending, Some(SessionAction::Suspend));
}

#[test]
fn shells_without_session_actions_never_ask_logind() {
    let (app, _) = App::boot(crate::config::Config::default(), vec![]);
    assert!(!app.uses_session_actions());
    let (app, _) = default_app();
    assert!(app.uses_session_actions());
}
