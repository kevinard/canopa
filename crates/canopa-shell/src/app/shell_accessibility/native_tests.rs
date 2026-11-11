//! Native AT-SPI acceptance on a private accessibility bus: launcher search
//! and selection, and a bar module following the bar's keyboard focus.
use super::{Surface, window_key};
use crate::app::messages::{ModuleRuntimeEvent, PanelEvent, SurfaceEvent};
use crate::app::{App, Message};
use crate::config::{Config, MenuConfig, MenuSourceConfig, MenuSourceKind};
use crate::modules::ModuleId;
use canopa_ui::accessibility::testing::{self, atspi};
use canopa_ui::accessibility::{Bridge, Event, Harness};
use iced::Size;
use iced::advanced::renderer::Headless;
use std::time::Duration;

const SIZE: Size = Size::new(640.0, 560.0);

async fn next(bridge: &Bridge<Message>) -> Event<Message> {
    tokio::time::timeout(Duration::from_secs(5), bridge.next_event())
        .await
        .expect("no accessibility event")
        .expect("bridge closed")
}

async fn action(bridge: &Bridge<Message>) -> Message {
    loop {
        if let Event::Action(message) = next(bridge).await {
            return *message;
        }
    }
}

async fn renderer() -> iced::Renderer {
    iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer")
}

fn publish(
    app: &App,
    window: iced::window::Id,
    bridge: &mut Bridge<Message>,
    renderer: &iced::Renderer,
) {
    let mut harness = Harness::new(app.view(window), renderer, SIZE);
    bridge.publish_snapshot(&harness.collect());
}

fn launcher_results(app: &App, ids: &[&str]) -> Message {
    use crate::launcher::{Action, Batch, NamedAction, ResultItem};
    let session = app.foreground.launcher().expect("launcher session");
    Message::Panel(PanelEvent::Launcher(crate::app::launcher::Event::Results {
        session: session.id,
        generation: session.generation,
        provider: 1,
        index_revision: session.index_revision(),
        batch: Batch {
            results: ids
                .iter()
                .map(|id| ResultItem {
                    id: format!("fixture:{id}"),
                    label: format!("Fixture {id}"),
                    description: format!("The {id} fixture"),
                    icon: None,
                    actions: vec![NamedAction {
                        label: "Copy".into(),
                        action: Action::Copy { text: (*id).into() },
                    }],
                })
                .collect(),
            status: String::new(),
        },
    }))
}

#[tokio::test]
async fn native_launcher_search_announces_the_selected_result_and_launches_it() {
    if !testing::isolated(
        "app::shell_accessibility::native_tests::native_launcher_search_announces_the_selected_result_and_launches_it",
    ) {
        return;
    }
    let connection = testing::connect().await;
    let config = Config {
        menus: vec![MenuConfig {
            name: "Applications".into(),
            show_icons: false,
            prompt: Some("Search apps".into()),
            source: Some(MenuSourceConfig {
                kind: MenuSourceKind::Launcher(crate::launcher::Config::default()),
            }),
            items: vec![],
        }],
        ..Config::default()
    };
    let (mut app, _) = App::boot(config, vec![]);
    let bar = iced::window::Id::unique();
    app.surface_mut(bar).screen_size = (1280, 800);
    let _ = app.update(Message::Panel(PanelEvent::Open {
        origin_window: bar,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content: crate::surface::panel::PanelContent::Menu("Applications".into()),
        anchor: None,
    }));
    let panel = app
        .foreground
        .active_panel_window()
        .expect("launcher window");
    let session = app.foreground.launcher().expect("launcher session").id;
    let _ = app.update(Message::Panel(PanelEvent::LauncherSearchChanged {
        session,
        query: "fixture".into(),
    }));
    let results = launcher_results(&app, &["first", "second"]);
    let _ = app.update(results);

    let key = window_key(Surface::Panel, panel);
    let mut bridge = Bridge::new("shell-launcher-test");
    bridge.open(key.clone(), "Launcher", Some(panel));
    bridge.set_window_focused(&key, true);
    assert!(matches!(next(&bridge).await, Event::Activated));
    let renderer = renderer().await;
    publish(&app, panel, &mut bridge, &renderer);
    let window = testing::named(&connection, "Launcher").await;
    if std::env::var_os("CANOPA_ACCESSIBILITY_DUMP").is_some() {
        testing::dump(&window).await;
    }

    // The search field controls the results list.
    let search = testing::named_role(&connection, "Search apps", "entry").await;
    let controlled = search.relation(atspi::RELATION_CONTROLLER_FOR).await;
    assert_eq!(controlled.len(), 1);
    assert_eq!(controlled[0].name().await, "Results");
    let first = testing::named_role(&connection, "Fixture first", "list item").await;
    assert!(first.has_state(atspi::STATE_SELECTED).await);
    assert_eq!(first.description().await, "The first fixture");
    let second = testing::named_role(&connection, "Fixture second", "list item").await;
    assert!(!second.has_state(atspi::STATE_SELECTED).await);
    let (_, _, width, height) = second.extents().await;
    assert!(width > 0 && height > 0, "bounds for magnifiers");

    // Focusing the field focuses its active descendant: the selected result.
    assert!(search.grab_focus().await);
    let Event::Focus(id) = next(&bridge).await else {
        panic!("expected a focus request");
    };
    {
        let mut harness = Harness::new(app.view(panel), &renderer, SIZE);
        harness.focus(id);
        bridge.publish_snapshot(&harness.collect());
    }
    first.wait_for_state(atspi::STATE_FOCUSED).await;
    assert_eq!(bridge.focused(&key), Some("launcher-search"));

    // Moving the selection moves the active descendant, which announces the row.
    let _ = app.update(Message::Panel(PanelEvent::FocusIndex(1)));
    {
        let mut harness = Harness::new(app.view(panel), &renderer, SIZE);
        harness.focus(crate::app::panel::panel_search_input_id(panel));
        bridge.publish_snapshot(&harness.collect());
    }
    let second = testing::named_role(&connection, "Fixture second", "list item").await;
    second.wait_for_state(atspi::STATE_SELECTED).await;
    second.wait_for_state(atspi::STATE_FOCUSED).await;
    assert!(!first.has_state(atspi::STATE_SELECTED).await);

    // The default action is the message a click or Enter sends.
    assert!(second.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(
        &message,
        Message::Panel(PanelEvent::ActivateLauncher(target, 0))
            if target.result.id == "fixture:second"
    ));

    // A failure is an assertive live region.
    let session = app.foreground.launcher().expect("launcher session");
    let failed = Message::Panel(PanelEvent::Launcher(
        crate::app::launcher::Event::ActionFinished {
            session: session.id,
            generation: session.generation,
            result: Err("Could not copy".into()),
        },
    ));
    let _ = app.update(failed);
    publish(&app, panel, &mut bridge, &renderer);
    testing::named_role(&connection, "Could not copy", "notification").await;

    drop(bridge);
    testing::gone(&connection, "Launcher").await;
}

#[tokio::test]
async fn native_bar_module_follows_bar_focus_and_activates_like_a_click() {
    if !testing::isolated(
        "app::shell_accessibility::native_tests::native_bar_module_follows_bar_focus_and_activates_like_a_click",
    ) {
        return;
    }
    use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
    let connection = testing::connect().await;
    let mut config = Config {
        modules: vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
            actions: crate::config::ModuleActionsConfig::default(),
        }],
        ..Default::default()
    };
    config.bars[0].modules.start = vec!["clock".into()];
    let (mut app, _) = App::boot(config, vec![]);
    let set_time = |app: &mut App, time: &str| {
        app.modules.set_state(
            ModuleId::new("clock"),
            crate::modules::ModuleState::Clock(crate::modules::clock::ClockState {
                time: time.into(),
                date: "Wed 23 Sep".into(),
            }),
        );
    };
    set_time(&mut app, "12:00");
    let bar = iced::window::Id::unique();
    app.surface_mut(bar).screen_size = (1280, 800);

    let key = window_key(Surface::Bar, bar);
    let mut bridge = Bridge::new("shell-bar-test");
    bridge.open(key.clone(), "Canopa bar", Some(bar));
    bridge.set_window_focused(&key, true);
    assert!(matches!(next(&bridge).await, Event::Activated));
    let renderer = renderer().await;
    publish(&app, bar, &mut bridge, &renderer);
    let window = testing::named(&connection, "Canopa bar").await;
    if std::env::var_os("CANOPA_ACCESSIBILITY_DUMP").is_some() {
        testing::dump(&window).await;
    }

    let clock = testing::named_role(&connection, "Clock, 12:00, Wed 23 Sep", "push button").await;
    assert!(!clock.has_state(atspi::STATE_FOCUSED).await);
    let (_, _, width, height) = clock.extents().await;
    assert!(width > 0 && height > 0, "bounds for magnifiers");

    // Keyboard focus held by the bar's own selection reaches the module.
    let _ = app.update(Message::Surface(SurfaceEvent::BarFocusGained(bar)));
    publish(&app, bar, &mut bridge, &renderer);
    clock.wait_for_state(atspi::STATE_FOCUSED).await;
    assert_eq!(bridge.focused(&key), Some("module:clock"));

    // The default action is the module's click.
    assert!(clock.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(
        &message,
        Message::Modules(ModuleRuntimeEvent::ModuleClick(id, window))
            if *id == ModuleId::new("clock") && *window == bar
    ));

    // The name follows the value.
    set_time(&mut app, "12:01");
    publish(&app, bar, &mut bridge, &renderer);
    testing::named_role(&connection, "Clock, 12:01, Wed 23 Sep", "push button").await;

    drop(bridge);
    testing::gone(&connection, "Canopa bar").await;
}
