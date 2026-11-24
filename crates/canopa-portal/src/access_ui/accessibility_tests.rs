//! Native AT-SPI acceptance of the consent dialog on a private accessibility
//! bus: its choices and buttons send the messages input sends.
use super::*;
use canopa_ui::accessibility::testing::{self, atspi};
use canopa_ui::accessibility::{Bridge, Event as Accessibility, Harness};
use iced::advanced::renderer::Headless;
use std::time::Duration;

const SIZE: Size = Size::new(680.0, 440.0);

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
async fn native_consent_choices_and_buttons_send_the_input_messages() {
    if !testing::isolated(
        "access_ui::accessibility_tests::native_consent_choices_and_buttons_send_the_input_messages",
    ) {
        return;
    }
    let connection = testing::connect().await;
    let mut request = AccessRequest::parse(
        "org.example.Camera".into(),
        String::new(),
        "Allow camera access?".into(),
        "Camera".into(),
        "The application wants to record video.".into(),
        Default::default(),
    )
    .unwrap();
    request.choices = vec![
        (
            "remember".into(),
            "Remember this choice".into(),
            Vec::new(),
            "false".into(),
        ),
        (
            "quality".into(),
            "Quality".into(),
            vec![("low".into(), "Low".into()), ("high".into(), "High".into())],
            "low".into(),
        ),
    ];
    let mut dialog = Dialog {
        requester: "Camera test".into(),
        request,
        theme: canopa_ui::theme(Default::default()),
        icon: None,
        output: Arc::default(),
        window: window::Id::unique(),
        accessibility: None,
    };
    let title = dialog.window_title();
    let mut bridge = Bridge::new("access-test");
    bridge.open(WINDOW_KEY, &title, None);
    bridge.set_window_focused(WINDOW_KEY, true);
    assert!(matches!(next(&bridge).await, Accessibility::Activated));
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let publish = |dialog: &Dialog, bridge: &mut Bridge<Message>| {
        let mut harness = Harness::new(dialog.view(dialog.window), &renderer, SIZE);
        bridge.publish_snapshot(&harness.collect());
    };
    publish(&dialog, &mut bridge);
    let window = testing::named(&connection, &title).await;
    if std::env::var_os("CANOPA_ACCESSIBILITY_DUMP").is_some() {
        testing::dump(&window).await;
    }
    // The consent question is an alert dialog described by what it asks.
    let question = testing::named_role(&connection, "Allow camera access?", "alert").await;
    assert_eq!(
        question.description().await,
        "Camera The application wants to record video."
    );

    // A check box choice toggles through the message its click sends.
    let remember = testing::named_role(&connection, "Remember this choice", "check box").await;
    assert!(!remember.has_state(atspi::STATE_CHECKED).await);
    assert!(remember.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(&message, Message::Choice(0, value) if value == "true"));
    let _ = dialog.update(message);
    publish(&dialog, &mut bridge);
    testing::named_role(&connection, "Remember this choice", "check box")
        .await
        .wait_for_state(atspi::STATE_CHECKED)
        .await;

    // A pick-list choice reports its value and steps to the next option.
    let quality = testing::named_role(&connection, "Quality", "combo box").await;
    assert!(quality.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(&message, Message::Choice(1, value) if value == "high"));

    // Allow sends Grant; Deny sends Deny and grants nothing.
    let allow = testing::named_role(&connection, "Allow", "push button").await;
    assert!(allow.do_action(0).await);
    assert!(matches!(action(&bridge).await, Message::Grant));
    let deny = testing::named_role(&connection, "Deny", "push button").await;
    assert!(deny.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(message, Message::Deny));
    let _ = dialog.update(message);
    assert!(dialog.output.lock().unwrap().is_none());

    drop(bridge);
    testing::gone(&connection, &title).await;
}
