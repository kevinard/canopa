use super::*;

fn dialog(editable: bool) -> Dialog {
    Dialog {
        draft: LauncherDraft {
            app_id: "org.canopa.Fixture".into(),
            parent_window: "".into(),
            name: "Original".into(),
            launcher_type: LauncherType::Application,
            target: "".into(),
            modal: true,
            editable_name: editable,
        },
        name: "Original".into(),
        preview: image::Handle::from_rgba(1, 1, vec![0; 4]),
        theme: Theme::Dark,
        output: Arc::default(),
        invalid: false,
        completed: false,
        window: window::Id::unique(),
        accessibility: None,
    }
}

#[test]
fn edits_require_valid_explicit_acceptance_and_cannot_survive_cancel() {
    let mut app = dialog(true);
    let _ = app.update(Message::Opened);
    let _ = app.update(Message::Name("".into()));
    let _ = app.update(Message::Install);
    assert!(app.output.lock().unwrap().is_none());
    let _ = app.update(Message::Name("Edited".into()));
    let _ = app.update(Message::Name("x".repeat(NAME_BYTES + 1)));
    let _ = app.update(Message::Install);
    assert!(app.output.lock().unwrap().is_none());
    let _ = app.update(Message::Name("Edited".into()));
    let _ = app.update(Message::Install);
    let _ = app.update(Message::Name("Late edit".into()));
    let _ = app.update(Message::Install);
    assert_eq!(app.output.lock().unwrap().as_deref(), Some("Edited"));
    let mut app = dialog(true);
    let _ = app.update(Message::Cancel);
    let _ = app.update(Message::Install);
    assert!(app.output.lock().unwrap().is_none());
}

#[test]
fn noneditable_name_and_initial_enter_do_not_allow_implicit_edits_or_install() {
    let mut app = dialog(false);
    let _ = app.update(Message::Name("Ignored".into()));
    let _ = app.update(Message::Event(Event::Keyboard(
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            modified_key: keyboard::Key::Named(keyboard::key::Named::Enter),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::default(),
            text: None,
            repeat: false,
        },
    )));
    assert!(app.output.lock().unwrap().is_none());
    let _ = app.update(Message::Install);
    assert_eq!(app.output.lock().unwrap().as_deref(), Some("Original"));
}
