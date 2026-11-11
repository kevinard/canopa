//! Native AT-SPI acceptance of the application chooser on a private
//! accessibility bus: rows and buttons send the messages input sends.
use super::*;
use canopa_ui::accessibility::testing::{self, atspi};
use canopa_ui::accessibility::{Bridge, Event as Accessibility, Harness};
use iced::advanced::renderer::Headless;
use std::time::Duration;

const SIZE: Size = Size::new(620.0, 480.0);

async fn next(bridge: &Bridge<Message>) -> Accessibility<Message> {
    tokio::time::timeout(Duration::from_secs(5), bridge.next_event())
        .await
        .expect("no accessibility event")
        .expect("bridge closed")
}

async fn action(bridge: &Bridge<Message>) -> Message {
    loop {
        if let Accessibility::Action(message) = next(bridge).await {
            return *message;
        }
    }
}

#[tokio::test]
async fn native_application_rows_and_buttons_send_the_input_messages() {
    if !testing::isolated(
        "app_chooser_ui::accessibility_tests::native_application_rows_and_buttons_send_the_input_messages",
    ) {
        return;
    }
    let connection = testing::connect().await;
    let mut chooser = super::tests::chooser();
    chooser.reconcile();
    let mut bridge = Bridge::new("app-chooser-test");
    bridge.open(WINDOW_KEY, WINDOW_TITLE, None);
    bridge.set_window_focused(WINDOW_KEY, true);
    assert!(matches!(next(&bridge).await, Accessibility::Activated));
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let publish = |chooser: &Chooser, bridge: &mut Bridge<Message>| {
        let mut harness = Harness::new(chooser.view(chooser.window), &renderer, SIZE);
        bridge.publish_snapshot(&harness.collect());
    };
    publish(&chooser, &mut bridge);
    let window = testing::named(&connection, WINDOW_TITLE).await;
    if std::env::var_os("CANOPA_ACCESSIBILITY_DUMP").is_some() {
        testing::dump(&window).await;
    }
    testing::named_role(&connection, "Open with", "dialog").await;
    testing::named_role(&connection, "2 applications", "status bar").await;

    // The selected application holds focus while arrows drive the list.
    let alpha = testing::named_role(&connection, "Alpha", "list item").await;
    alpha.wait_for_state(atspi::STATE_SELECTED).await;
    alpha.wait_for_state(atspi::STATE_FOCUSED).await;
    assert_eq!(alpha.description().await, "a.desktop");

    // A row's default action is the click's Select message.
    let beta = testing::named_role(&connection, "Beta", "list item").await;
    assert!(!beta.has_state(atspi::STATE_SELECTED).await);
    assert!(beta.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(&message, Message::Select(id) if id == "b.desktop"));
    let _ = chooser.update(message);
    publish(&chooser, &mut bridge);
    let beta = testing::named_role(&connection, "Beta", "list item").await;
    beta.wait_for_state(atspi::STATE_SELECTED).await;
    beta.wait_for_state(atspi::STATE_FOCUSED).await;

    let search = testing::named_role(&connection, "Search applications", "entry").await;
    let controlled = search.relation(atspi::RELATION_CONTROLLER_FOR).await;
    assert_eq!(controlled.len(), 1);
    assert_eq!(controlled[0].name().await, "Applications");

    let open = testing::named_role(&connection, "Open", "push button").await;
    assert!(open.do_action(0).await);
    assert!(matches!(action(&bridge).await, Message::Accept));
    let cancel = testing::named_role(&connection, "Cancel", "push button").await;
    assert!(cancel.do_action(0).await);
    assert!(matches!(action(&bridge).await, Message::Cancel));

    drop(bridge);
    testing::gone(&connection, WINDOW_TITLE).await;
}
