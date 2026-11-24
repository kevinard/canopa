use super::*;

fn dialog() -> Dialog {
    let mut dialog = Dialog::new(
        AccountRequest::parse("org.canopa.Test".into(), String::new(), Default::default()).unwrap(),
        canopa_theme::Appearance::default(),
        canopa_theme::Rgb(250, 250, 250),
        Arc::default(),
    );
    let _ = dialog.update(Message::Loaded(Ok(Profile {
        id: "fixture-user".into(),
        name: "Fixture Name".into(),
        requester: "Fixture".into(),
        pictures: PathBuf::from("/fixture-only"),
        avatar: None,
    })));
    dialog
}

fn key(key: keyboard::key::Named) -> Message {
    Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(key),
        modified_key: keyboard::Key::Named(key),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::default(),
        text: None,
        repeat: false,
    }))
}

fn information() -> UserInformation {
    UserInformation {
        id: "edited".into(),
        name: "Edited Name".into(),
        image: None,
    }
}

fn picker(dialog: &mut Dialog, folder: PathBuf) -> u64 {
    dialog.generation += 1;
    let generation = dialog.generation;
    dialog.view = View::PreparingPicker(generation);
    let request = crate::file_chooser::parse_request(
        Mode::Open,
        String::new(),
        String::new(),
        "Image".into(),
        Default::default(),
    )
    .unwrap();
    let _ = dialog.update(Message::PickerReady(
        generation,
        Ok(Box::new(PickerStart::fixture(request, folder))),
    ));
    assert!(matches!(dialog.view, View::Picker(_, _)));
    generation
}

#[test]
fn only_explicit_current_share_can_complete_and_cancel_wins_a_late_result() {
    let mut dialog = dialog();
    let _ = dialog.update(key(keyboard::key::Named::Enter));
    let _ = dialog.update(Message::Shared(dialog.generation, Ok(information())));
    assert!(!dialog.busy && !dialog.completed);
    assert!(dialog.output.lock().unwrap().is_none());
    let _ = dialog.update(Message::Share(dialog.generation - 1));
    assert!(!dialog.busy);
    let _ = dialog.update(Message::Share(dialog.generation));
    assert!(dialog.busy);
    let _ = dialog.update(Message::Id("late edit".into()));
    assert_eq!(dialog.id, "fixture-user");
    let _ = dialog.update(Message::Cancel);
    let _ = dialog.update(Message::Shared(dialog.generation, Ok(information())));
    assert!(dialog.completed);
    assert!(dialog.output.lock().unwrap().is_none());
}

#[test]
fn picker_return_keeps_the_draft_and_old_picker_events_cannot_cancel_a_new_one() {
    let root = tempfile::tempdir().unwrap();
    let mut dialog = dialog();
    let _ = dialog.update(Message::Name("Edited draft".into()));
    let _ = dialog.update(Message::ShareId(false));
    let old_share = dialog.generation;
    let first = picker(&mut dialog, root.path().to_owned());
    let _ = dialog.update(Message::Picker(first, chooser_ui::Message::Cancel));
    assert!(matches!(dialog.view, View::Profile));
    assert_eq!(dialog.name, "Edited draft");
    assert!(!dialog.share_id);
    let _ = dialog.update(Message::Share(old_share));
    assert!(!dialog.busy);
    let second = picker(&mut dialog, root.path().to_owned());
    let _ = dialog.update(Message::Picker(first, chooser_ui::Message::Cancel));
    assert!(matches!(dialog.view, View::Picker(current, _) if current == second));
    let _ = dialog.update(Message::Picker(
        second,
        chooser_ui::Message::Event(Event::Window(window::Event::CloseRequested)),
    ));
    assert!(dialog.completed);
    assert!(dialog.output.lock().unwrap().is_none());
}

#[test]
fn back_and_cleared_image_reject_late_preparation_and_loading_blocks_share() {
    let mut dialog = dialog();
    dialog.generation += 1;
    let retired = dialog.generation;
    dialog.view = View::PreparingPicker(retired);
    let _ = dialog.update(Message::Back);
    let _ = dialog.update(Message::PickerReady(retired, Err("old failure".into())));
    assert!(matches!(dialog.view, View::Profile));
    assert!(dialog.error.is_none());
    dialog.image_generation = 3;
    dialog.image_loading = true;
    let _ = dialog.update(Message::Share(dialog.generation));
    assert!(!dialog.busy);
    let _ = dialog.update(Message::ClearImage);
    let _ = dialog.update(Message::Avatar(
        3,
        Ok(Avatar {
            raster: Arc::new(canopa_assets::Raster {
                width: 1,
                height: 1,
                pixels: vec![20, 90, 200, 255],
            }),
            png: Arc::new(Vec::new()),
        }),
    ));
    assert!(!dialog.image_loading && !dialog.share_image);
    assert!(dialog.avatar.is_none() && dialog.error.is_none());
    assert!(dialog.can_share());
}

#[test]
fn invalid_edits_cannot_silently_share_a_hidden_previous_value() {
    let mut dialog = dialog();
    for invalid in ["x".repeat(FIELD_BYTES + 1), "bad\0value".into()] {
        let _ = dialog.update(Message::Name(invalid));
        assert!(dialog.invalid_name && !dialog.can_share());
        assert_eq!(dialog.name, "Fixture Name");
        let _ = dialog.update(Message::Share(dialog.generation));
        assert!(!dialog.busy);
    }
    let _ = dialog.update(Message::ShareName(false));
    assert!(dialog.can_share());
    let _ = dialog.update(Message::ShareName(true));
    assert!(!dialog.invalid_name);
    let _ = dialog.update(Message::Name("Reviewed name".into()));
    assert_eq!(dialog.name, "Reviewed name");
    assert!(dialog.can_share());
}
