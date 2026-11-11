//! Native AT-SPI acceptance on a private accessibility bus: page navigation
//! from the sidebar, keyboard focus, and one live control.
use super::{ACCESSIBILITY_ROOT, Message, tests::fresh};
use crate::Page;
use crate::live::{Event as LiveEvent, fake};
use canopa_ui::accessibility::testing::{self, atspi};
use canopa_ui::accessibility::{Bridge, Event, Harness};
use iced::Size;
use iced::advanced::renderer::Headless;
use std::time::Duration;

const SIZE: Size = Size::new(1000.0, 780.0);

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

#[tokio::test]
async fn native_settings_tree_follows_pages_focus_and_live_controls() {
    if !testing::isolated(
        "ui::accessibility_tests::native_settings_tree_follows_pages_focus_and_live_controls",
    ) {
        return;
    }
    let connection = testing::connect().await;
    let (_root, mut state) = fresh();
    let mut bridge = Bridge::new("settings-test");
    bridge.open(ACCESSIBILITY_ROOT, "Canopa Settings", None);
    bridge.set_window_focused(ACCESSIBILITY_ROOT, true);
    // Nothing is published before an assistive technology asks.
    assert!(matches!(next(&bridge).await, Event::Activated));
    assert!(bridge.is_active());
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let publish = |state: &super::State, bridge: &mut Bridge<Message>| {
        let mut harness = Harness::new(state.view(), &renderer, SIZE);
        bridge.publish_snapshot(&harness.collect());
    };
    publish(&state, &mut bridge);
    let window = testing::named(&connection, "Canopa Settings").await;
    if std::env::var_os("CANOPA_ACCESSIBILITY_DUMP").is_some() {
        testing::dump(&window).await;
    }

    // The sidebar is a list of pages; the current one is selected.
    let displays = testing::named_role(&connection, "Displays", "list item").await;
    assert!(displays.has_state(atspi::STATE_SELECTED).await);
    let sound = testing::named_role(&connection, "Sound", "list item").await;
    assert!(!sound.has_state(atspi::STATE_SELECTED).await);
    let (_, _, width, height) = sound.extents().await;
    assert!(width > 0 && height > 0, "bounds for magnifiers");
    // The default action sends the message a click sends.
    assert!(sound.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(message, Message::Page(Page::Sound)));
    let _ = state.update(message);
    publish(&state, &mut bridge);
    testing::named_role(&connection, "Sound", "heading").await;
    let sound = testing::named_role(&connection, "Sound", "list item").await;
    sound.wait_for_state(atspi::STATE_SELECTED).await;
    // A live page starts with an announced loading status.
    testing::named(&connection, "Connecting to The audio server…")
        .await
        .role_name()
        .await;

    // AT focus requests move keyboard focus; collected focus follows it.
    let search = testing::named_role(&connection, "Search settings", "entry").await;
    assert!(search.grab_focus().await);
    let Event::Focus(id) = next(&bridge).await else {
        panic!("expected a focus request");
    };
    assert_eq!(id, iced::widget::Id::from(super::navigation::SEARCH));
    {
        let mut harness = Harness::new(state.view(), &renderer, SIZE);
        harness.focus(id);
        bridge.publish_snapshot(&harness.collect());
    }
    search.wait_for_state(atspi::STATE_FOCUSED).await;
    assert_eq!(
        bridge.focused(ACCESSIBILITY_ROOT),
        Some("settings-search-field")
    );
    let controlled = search.relation(atspi::RELATION_CONTROLLER_FOR).await;
    assert_eq!(controlled.len(), 1);
    assert_eq!(controlled[0].name().await, "Pages");

    // A live control: its action is the same message, and the pending change
    // is announced through the control's status.
    let _ = state.update(Message::Page(Page::LiveExample));
    let _ = state.update(Message::LiveExample(LiveEvent::Loaded(Ok(fake::State {
        enabled: false,
        level: 40,
    }))));
    publish(&state, &mut bridge);
    let switch = testing::named_role(&connection, "Example switch", "check box").await;
    assert!(!switch.has_state(atspi::STATE_CHECKED).await);
    assert!(switch.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(
        message,
        Message::LiveExample(LiveEvent::Apply(fake::Action::Enabled(true)))
    ));
    let _ = state.update(message);
    publish(&state, &mut bridge);
    testing::named_role(&connection, "Applying…", "status bar").await;
    let level = testing::named_role(&connection, "Example level: 40%", "slider").await;
    assert_eq!(level.value().await, 40.0);
    level.set_value(65.0).await;
    let message = action(&bridge).await;
    assert!(matches!(
        message,
        Message::LiveExample(LiveEvent::Apply(fake::Action::Level(65)))
    ));
    let _ = state.update(Message::LiveExample(LiveEvent::Applied(
        fake::Action::Enabled(true),
        Err(crate::live::ServiceError::Failed(
            "Refused by the test".into(),
        )),
    )));
    publish(&state, &mut bridge);
    testing::named_role(&connection, "Refused by the test", "notification").await;
    testing::named_role(&connection, "Retry", "push button").await;

    drop(bridge);
    testing::gone(&connection, "Canopa Settings").await;
}
