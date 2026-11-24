//! Native AT-SPI acceptance of the FileChooser on a private accessibility bus:
//! the file list, keyboard focus and the dialog buttons send the messages
//! pointer and keyboard input send.
use super::*;
use canopa_ui::accessibility::testing::{self, atspi};
use canopa_ui::accessibility::{Bridge, Event as Accessibility, Harness};
use iced::{Size, advanced::renderer::Headless};
const WINDOW_KEY: &str = "embedded-chooser";

impl canopa_ui::accessibility::Message for Message {
    fn accessibility(event: Accessibility<Self>) -> Self {
        Self::Accessibility(event)
    }
}

use std::time::Duration;

const SIZE: Size = Size::new(820.0, 560.0);

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

fn key(named: keyboard::key::Named) -> Message {
    Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(named),
        modified_key: keyboard::Key::Named(named),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
    }))
}

#[tokio::test]
async fn native_file_list_focus_and_dialog_buttons_send_the_input_messages() {
    if !testing::isolated(
        "chooser_ui::accessibility_tests::native_file_list_focus_and_dialog_buttons_send_the_input_messages",
    ) {
        return;
    }
    let connection = testing::connect().await;
    let root = tempfile::tempdir().unwrap();
    let alpha = root.path().join("alpha.txt");
    std::fs::write(&alpha, b"alpha").unwrap();
    std::fs::write(root.path().join("beta.pdf"), b"beta").unwrap();
    std::fs::create_dir(root.path().join("folder")).unwrap();
    let request = crate::file_chooser::parse_request(
        Mode::Open,
        String::new(),
        String::new(),
        "Open a document".into(),
        Default::default(),
    )
    .unwrap();
    let (mut picker, _) = Picker::new(
        PickerStart::fixture(request, root.path().into()),
        PickerOptions::default(),
    );
    let _ = picker.update(Message::Loaded(
        picker.revision,
        false,
        Ok(Directory::read(root.path()).unwrap()),
    ));
    let title = picker.window_title();
    let mut bridge = Bridge::new("file-chooser-test");
    bridge.open(WINDOW_KEY, &title, None);
    bridge.set_window_focused(WINDOW_KEY, true);
    // Nothing is published before an assistive technology asks.
    assert!(matches!(next(&bridge).await, Accessibility::Activated));
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let publish = |picker: &Picker, bridge: &mut Bridge<Message>| {
        let mut harness = Harness::new(
            semantic(Node::window(WINDOW_KEY, title.clone()), picker.view()),
            &renderer,
            SIZE,
        );
        bridge.publish_snapshot(&harness.collect());
    };
    publish(&picker, &mut bridge);
    let window = testing::named(&connection, &title).await;
    if std::env::var_os("CANOPA_ACCESSIBILITY_DUMP").is_some() {
        testing::dump(&window).await;
    }
    testing::named_role(&connection, "Open a document", "heading").await;
    testing::named_role(&connection, "3 items", "status bar").await;

    // Rows are options named by file name and described by their type; the
    // default action sends the message a click sends.
    let row = testing::named_role(&connection, "alpha.txt", "list item").await;
    assert_eq!(row.description().await, "TXT file");
    assert!(!row.has_state(atspi::STATE_SELECTED).await);
    let (_, _, width, height) = row.extents().await;
    assert!(width > 0 && height > 0, "bounds for magnifiers");
    assert!(row.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(&message, Message::Select(path) if *path == alpha));
    let _ = picker.update(message);
    publish(&picker, &mut bridge);
    let row = testing::named_role(&connection, "alpha.txt", "list item").await;
    row.wait_for_state(atspi::STATE_SELECTED).await;
    row.wait_for_state(atspi::STATE_FOCUSED).await;

    // Arrow keys move the cursor row, and accessibility focus follows it.
    let order: Vec<String> = picker
        .entries()
        .into_iter()
        .map(|entry| entry.name().into_owned())
        .collect();
    let below = &order[order.iter().position(|name| name == "alpha.txt").unwrap() + 1];
    let _ = picker.update(key(keyboard::key::Named::ArrowDown));
    publish(&picker, &mut bridge);
    let next_row = testing::named_role(&connection, below, "list item").await;
    next_row.wait_for_state(atspi::STATE_FOCUSED).await;
    next_row.wait_for_state(atspi::STATE_SELECTED).await;
    assert_eq!(
        bridge.focused(WINDOW_KEY),
        Some(entry_key(&root.path().join(below)).as_str())
    );

    // The search field filters the list it controls; focusing it from an
    // assistive technology moves keyboard focus there.
    let _ = picker.update(Message::ShowNameFilter);
    publish(&picker, &mut bridge);
    let search = testing::named_role(&connection, "Filter names in this folder", "entry").await;
    let controlled = search.relation(atspi::RELATION_CONTROLLER_FOR).await;
    assert_eq!(controlled.len(), 1);
    assert_eq!(controlled[0].name().await, "Files");
    assert!(search.grab_focus().await);
    let event = next(&bridge).await;
    let Accessibility::Focus(id) = &event else {
        panic!("expected a focus request");
    };
    assert_eq!(*id, iced::widget::Id::from("search"));
    let id = id.clone();
    let _ = picker.update(Message::Accessibility(event));
    {
        let mut harness = Harness::new(
            semantic(Node::window(WINDOW_KEY, title.clone()), picker.view()),
            &renderer,
            SIZE,
        );
        harness.focus(id);
        bridge.publish_snapshot(&harness.collect());
    }
    search.wait_for_state(atspi::STATE_FOCUSED).await;
    assert_eq!(bridge.focused(WINDOW_KEY), Some("chooser-search"));

    // The accept button sends Accept, as pressing it does.
    let open = testing::named_role(&connection, "Open", "push button").await;
    assert!(open.do_action(0).await);
    assert!(matches!(action(&bridge).await, Message::Accept));

    // A refused acceptance is announced assertively.
    let _ = picker.update(Message::Checked(Err("Select a file".into())));
    publish(&picker, &mut bridge);
    testing::named_role(&connection, "Select a file", "notification").await;

    // Cancel sends Cancel and the chooser finishes without a selection.
    let cancel = testing::named_role(&connection, "Cancel", "push button").await;
    assert!(cancel.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(message, Message::Cancel));
    let _ = picker.update(message);
    assert!(picker.finished);
    assert!(matches!(picker.take_outcome(), Some(Outcome::Cancelled)));

    drop(bridge);
    testing::gone(&connection, &title).await;
}
