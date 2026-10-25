use std::{
    ffi::OsString,
    fs,
    os::unix::{ffi::OsStringExt, fs::PermissionsExt},
};

use canopa_files::preferences::{PreferenceStore, SortField, ViewMode, ViewSettings};

#[test]
fn search_control_bindings_are_configurable_without_claiming_existing_shortcuts() {
    use canopa_files::{
        configuration::ConfigurationSnapshot,
        interface::{Command, KeyChord},
    };
    for command in [
        Command::SearchSubfolders,
        Command::StopSearch,
        Command::MoreSearchResults,
        Command::RevealSearchResult,
        Command::SearchProblems,
    ] {
        assert!(command.defaults().is_empty());
        assert_eq!(Command::from_id(command.id()), Some(command));
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    fs::write(&path, "schema-version 1\nfiles { action \"legacy\" name=\"Legacy\" command=\"/bin/true\" shortcut=\"<Alt>s\"; }\n").unwrap();
    let snapshot = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let mut settings = snapshot.settings.clone();
    settings
        .shortcuts
        .insert(Command::MoreSearchResults, vec!["<Alt>m".into()]);
    let saved = snapshot.save_settings(settings).unwrap().reload().unwrap();
    assert_eq!(
        saved
            .settings
            .command_for(&KeyChord::parse("<Alt>m").unwrap()),
        Some(Command::MoreSearchResults)
    );
    assert_eq!(saved.actions[0].shortcut.as_deref(), Some("<Alt>s"));
}

#[test]
fn folder_choices_override_only_the_fields_deliberately_changed() {
    let root = tempfile::tempdir().unwrap();
    let pictures = root.path().join("Pictures");
    fs::create_dir(&pictures).unwrap();
    let path = root.path().join("views.json");
    let mut preferences = PreferenceStore::new(ViewSettings::default());
    let icons = ViewSettings {
        mode: ViewMode::Icons,
        ..ViewSettings::default()
    };
    preferences.remember_changes(&pictures, icons).unwrap();
    let changed_defaults = ViewSettings {
        mode: ViewMode::List,
        sort: SortField::Modified,
        column_width: 320,
        ..ViewSettings::default()
    };
    preferences.set_defaults(changed_defaults.clone()).unwrap();
    assert_eq!(preferences.for_folder(&pictures).mode, ViewMode::Icons);
    assert_eq!(preferences.for_folder(&pictures).sort, SortField::Modified);
    assert_eq!(preferences.for_folder(&pictures).column_width, 320);
    preferences.save(&path).unwrap();
    let mut restored = PreferenceStore::load(&path, changed_defaults).unwrap();
    assert_eq!(restored.for_folder(&pictures).mode, ViewMode::Icons);
    let new_defaults = ViewSettings {
        column_width: 400,
        ..ViewSettings::default()
    };
    restored.set_defaults(new_defaults.clone()).unwrap();
    assert_eq!(restored.for_folder(&pictures).column_width, 400);
    restored.use_defaults(&pictures);
    assert_eq!(restored.for_folder(&pictures), &new_defaults);
}

#[test]
fn shared_interface_edits_preserve_actions_and_validate_shortcuts_as_one_catalog() {
    use canopa_files::{
        configuration::ConfigurationSnapshot,
        interface::{Command, Place},
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    fs::write(
        &path,
        "schema-version 1\nfiles { action \"inspect\" name=\"Inspect\" command=\"/bin/true\"; }\n",
    )
    .unwrap();
    let initial = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let mut settings = initial.settings.clone();
    settings.view.mode = ViewMode::List;
    settings.view.column_width = 320;
    settings.toolbar = vec!["parent".into(), "location".into(), "copy".into()];
    let raw = root.path().join(OsString::from_vec(b"work-\xff".to_vec()));
    settings.places = vec![Place::Bookmark {
        name: "Work".into(),
        path: raw.clone(),
    }];
    settings
        .shortcuts
        .insert(Command::Search, vec!["F6".into()]);
    let saved = initial.save_settings(settings).unwrap();
    let restored = saved.reload().unwrap();
    assert_eq!(restored.settings.view.mode, ViewMode::List);
    assert_eq!(restored.settings.view.column_width, 320);
    assert_eq!(restored.settings.toolbar, ["parent", "location", "copy"]);
    assert_eq!(
        restored.settings.places,
        [Place::Bookmark {
            name: "Work".into(),
            path: raw
        }]
    );
    assert_eq!(restored.actions[0].id, "inspect");
    let mut actions = restored.actions.clone();
    actions[0].shortcut = Some("F6".into());
    assert!(
        restored.save_actions(actions).is_err(),
        "custom actions cannot silently shadow a navigation shortcut"
    );
    let before = fs::read(&path).unwrap();
    let mut invalid = restored.settings.clone();
    invalid
        .shortcuts
        .insert(Command::Copy, vec!["<Ctrl>f".into()]);
    invalid.shortcuts.remove(&Command::Search);
    assert!(
        restored.save_settings(invalid).is_err(),
        "aliases are compared against all effective defaults"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn hand_authored_shortcuts_reject_conflicts_and_keep_selection_and_text_input_available() {
    use canopa_files::configuration::ConfigurationSnapshot;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    for body in [
        "shortcut \"copy\" \"<Primary>f\"",
        "shortcut \"copy\" \"Up\"",
        "shortcut \"copy\" \"<Shift>Home\"",
        "shortcut \"copy\" \"a\"",
        "shortcut \"copy\" \"F01\"",
        "shortcut \"copy\" \"<Ctrl>KP_1\"",
        "action \"one\" name=\"One\" command=\"/bin/true\" shortcut=\"<Control><Alt>i\"; action \"two\" name=\"Two\" command=\"/bin/true\" shortcut=\"<Alt><Ctrl>I\"",
    ] {
        let text = format!("schema-version 1\nfiles {{ {body}; }}\n");
        fs::write(&path, &text).unwrap();
        assert!(
            ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).is_err(),
            "{body}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn action_toolbar_toggle_and_explicit_toolbar_positions_stay_in_sync() {
    use canopa_files::configuration::ConfigurationSnapshot;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    fs::write(
        &path,
        "schema-version 1\nfiles { action \"inspect\" name=\"Inspect\" command=\"/bin/true\"; }\n",
    )
    .unwrap();
    let initial = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let mut settings = initial.settings.clone();
    settings.toolbar.push("action:inspect".into());
    let positioned = initial.save_settings(settings).unwrap();
    assert!(
        positioned.actions[0].toolbar,
        "an explicitly positioned action is enabled in the action editor too"
    );
    let mut actions = positioned.actions.clone();
    actions[0].toolbar = false;
    let hidden = positioned.save_actions(actions).unwrap();
    assert!(
        !hidden
            .settings
            .toolbar
            .iter()
            .any(|id| id == "action:inspect")
    );
    let mut settings = hidden.settings.clone();
    settings.toolbar.push("action:inspect".into());
    let positioned = hidden.save_settings(settings).unwrap();
    let mut settings = positioned.settings.clone();
    settings.toolbar.retain(|id| id != "action:inspect");
    let removed = positioned.save_settings(settings).unwrap();
    assert!(
        !removed.actions[0].toolbar,
        "removing a position must not move the same button into the fallback row"
    );
}

#[test]
fn legacy_folder_snapshots_keep_their_old_defaults_when_global_defaults_change() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Pictures");
    fs::create_dir(&folder).unwrap();
    let path = root.path().join("views.json");
    fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "version": 1, "folders": [{ "path": folder, "view": {"mode": "icons"} }]
        }))
        .unwrap(),
    )
    .unwrap();
    let defaults = ViewSettings {
        mode: ViewMode::List,
        column_width: 320,
        ..ViewSettings::default()
    };
    let restored = PreferenceStore::load(&path, defaults.clone()).unwrap();
    assert_eq!(restored.for_folder(&folder).mode, ViewMode::Icons);
    assert_eq!(restored.for_folder(&folder).column_width, 240);
    restored.save(&path).unwrap();
    let mut migrated = PreferenceStore::load(&path, defaults).unwrap();
    migrated
        .set_defaults(ViewSettings {
            column_width: 400,
            ..ViewSettings::default()
        })
        .unwrap();
    assert_eq!(migrated.for_folder(&folder).column_width, 240);
    migrated.use_defaults(&folder);
    assert_eq!(migrated.for_folder(&folder).column_width, 400);
}

#[test]
fn folder_overrides_survive_restart_without_losing_unix_filename_bytes() {
    let root = tempfile::tempdir().unwrap();
    let pictures = root
        .path()
        .join(OsString::from_vec(b"Pictures-\xff".to_vec()));
    fs::create_dir(&pictures).unwrap();
    let state_path = root.path().join("state/views.json");
    let mut preferences = PreferenceStore::new(ViewSettings::default());
    let icons = ViewSettings {
        mode: ViewMode::Icons,
        sort: SortField::Modified,
        descending: true,
        ..ViewSettings::default()
    };
    preferences.set_folder(&pictures, icons).unwrap();
    preferences.save(&state_path).unwrap();
    assert_eq!(
        fs::metadata(&state_path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let defaults = ViewSettings {
        mode: ViewMode::List,
        ..ViewSettings::default()
    };
    let mut restored = PreferenceStore::load(&state_path, defaults).unwrap();
    assert_eq!(restored.for_folder(&pictures).mode, ViewMode::Icons);
    assert_eq!(restored.for_folder(&pictures).sort, SortField::Modified);
    assert!(restored.for_folder(&pictures).descending);
    assert_eq!(restored.for_folder(root.path()).mode, ViewMode::List);
    restored.use_defaults(&pictures);
    assert_eq!(restored.for_folder(&pictures).mode, ViewMode::List);
    restored.save(&state_path).unwrap();
    let restored = PreferenceStore::load(&state_path, ViewSettings::default()).unwrap();
    assert_eq!(restored.for_folder(&pictures).mode, ViewMode::Columns);
}
