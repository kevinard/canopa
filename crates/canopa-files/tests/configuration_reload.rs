use canopa_config::Request;
use canopa_files::{
    configuration::{ConfigurationRuntime, ReloadEvent},
    preferences::ViewMode,
};
use std::fs;

#[test]
fn saved_configuration_is_installed_as_one_valid_candidate_and_bad_edits_keep_the_last_good_state()
{
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    fs::write(
        &path,
        "schema-version 1\nappearance { color-scheme \"dark\"; }\n",
    )
    .unwrap();
    let mut runtime = ConfigurationRuntime::new(Request::common(&path)).unwrap();
    fs::write(root.path().join("replacement"), "schema-version 1\nappearance { color-scheme \"light\"; }\nfiles { view mode=\"list\" column-width=320; shortcut \"search\" \"F6\"; }\n").unwrap();
    fs::rename(root.path().join("replacement"), &path).unwrap();
    assert!(matches!(runtime.poll(), ReloadEvent::Unchanged));
    let ReloadEvent::Updated(snapshot) = runtime.poll() else {
        panic!("settled valid candidate")
    };
    assert_eq!(
        snapshot.appearance.color_scheme,
        canopa_theme::ColorScheme::Light
    );
    assert_eq!(snapshot.settings.view.mode, ViewMode::List);
    assert_eq!(snapshot.settings.view.column_width, 320);
    assert_eq!(runtime.generation(), 2);
    fs::write(&path, "schema-version 1\nappearance { color-scheme \"dark\"; }\nfiles { shortcut \"copy\" \"Up\"; }\n").unwrap();
    assert!(matches!(runtime.poll(), ReloadEvent::Unchanged));
    assert!(matches!(runtime.poll(), ReloadEvent::Rejected(_)));
    assert_eq!(
        runtime.current().appearance.color_scheme,
        canopa_theme::ColorScheme::Light
    );
    assert_eq!(runtime.current().settings.view.mode, ViewMode::List);
    assert_eq!(runtime.generation(), 2);
    assert!(runtime.last_error().is_some());
    assert!(
        matches!(runtime.poll(), ReloadEvent::Unchanged),
        "one failed edit produces one rejection"
    );
    fs::write(
        &path,
        "schema-version 1\nappearance { color-scheme \"dark\"; }\n",
    )
    .unwrap();
    assert!(matches!(runtime.poll(), ReloadEvent::Unchanged));
    assert!(matches!(runtime.poll(), ReloadEvent::Updated(_)));
    assert!(runtime.last_error().is_none());
    assert_eq!(
        runtime.current().appearance.color_scheme,
        canopa_theme::ColorScheme::Dark
    );
}

#[test]
fn invalidating_an_inflight_candidate_republishes_the_latest_source_even_if_it_is_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    fs::write(&path, "schema-version 1\n").unwrap();
    let mut runtime = ConfigurationRuntime::new(Request::common(&path)).unwrap();
    fs::write(&path, "schema-version 1\nfiles { view mode=\"icons\"; }\n").unwrap();
    runtime.poll();
    assert!(matches!(runtime.poll(), ReloadEvent::Updated(_)));
    // A graphical save can install its own candidate while this result is in
    // flight. Discarding that result must still deliver the actual latest file.
    runtime.invalidate();
    assert!(matches!(runtime.poll(), ReloadEvent::Unchanged));
    let ReloadEvent::Updated(snapshot) = runtime.poll() else {
        panic!("latest source must be republished")
    };
    assert_eq!(snapshot.settings.view.mode, ViewMode::Icons);
}

#[test]
fn missing_and_modified_theme_dependencies_are_watched_without_changing_the_main_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    let theme = root.path().join("theme.kdl");
    fs::write(&path, "schema-version 1\n").unwrap();
    let mut runtime = ConfigurationRuntime::new(Request::common(&path)).unwrap();
    fs::write(
        &path,
        "schema-version 1\nappearance { theme \"theme.kdl\"; }\nfiles { view mode=\"icons\"; }\n",
    )
    .unwrap();
    runtime.poll();
    assert!(matches!(runtime.poll(), ReloadEvent::Rejected(_)));
    assert_eq!(runtime.current().settings.view.mode, ViewMode::Columns);
    fs::write(&theme, "palette { background \"#123456\"; }\n").unwrap();
    runtime.poll();
    assert!(matches!(runtime.poll(), ReloadEvent::Updated(_)));
    assert_eq!(
        runtime.current().appearance.palette().background,
        canopa_theme::Rgb(18, 52, 86)
    );
    assert_eq!(runtime.current().settings.view.mode, ViewMode::Icons);
    let main_fingerprint = runtime.current().source.fingerprint.clone();
    let combined = runtime.current().source.combined_fingerprint.clone();
    fs::write(
        root.path().join("new-theme"),
        "palette { background \"#304050\"; }\n",
    )
    .unwrap();
    fs::rename(root.path().join("new-theme"), &theme).unwrap();
    runtime.poll();
    assert!(matches!(runtime.poll(), ReloadEvent::Updated(_)));
    assert_eq!(runtime.current().source.fingerprint, main_fingerprint);
    assert_ne!(runtime.current().source.combined_fingerprint, combined);
    assert_eq!(
        runtime.current().appearance.palette().background,
        canopa_theme::Rgb(48, 64, 80)
    );
    fs::write(&theme, "private-theme-value \"do-not-expose-this\"\n").unwrap();
    runtime.poll();
    let ReloadEvent::Rejected(error) = runtime.poll() else {
        panic!("invalid theme")
    };
    assert!(!error.to_string().contains("do-not-expose-this"));
    assert_eq!(
        runtime.current().appearance.palette().background,
        canopa_theme::Rgb(48, 64, 80)
    );
}

#[test]
fn source_replacement_and_same_content_symlink_retargeting_keep_editor_staleness_visible() {
    use canopa_files::configuration::ConfigurationSnapshot;
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first.kdl");
    let second = root.path().join("second.kdl");
    fs::write(&first, "schema-version 1\n").unwrap();
    // Retargeting to a pre-existing hard link changes the editor's write
    // destination without changing the followed inode or any of its metadata.
    fs::hard_link(&first, &second).unwrap();
    let selected = root.path().join("selected.kdl");
    std::os::unix::fs::symlink(&first, &selected).unwrap();
    let draft = ConfigurationSnapshot::load(&Request::common(&selected)).unwrap();
    let mut runtime = ConfigurationRuntime::from_snapshot(draft.clone());
    runtime.poll();
    runtime.poll();
    fs::remove_file(&selected).unwrap();
    std::os::unix::fs::symlink(&second, &selected).unwrap();
    runtime.poll();
    let ReloadEvent::Updated(snapshot) = runtime.poll() else {
        panic!("retargeted source")
    };
    assert_eq!(snapshot.source.fingerprint, draft.source.fingerprint);
    assert!(!snapshot.same_source(&draft));
    let mut edited = draft.settings.clone();
    edited.view.mode = ViewMode::List;
    assert!(draft.save_settings(edited).is_err());
    assert_eq!(fs::read_to_string(&second).unwrap(), "schema-version 1\n");
    let directory = root.path().join("config-dir");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("config.kdl");
    fs::write(&path, "schema-version 1\n").unwrap();
    let mut runtime = ConfigurationRuntime::new(Request::common(&path)).unwrap();
    runtime.poll();
    runtime.poll();
    fs::rename(&directory, root.path().join("old-dir")).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(&path, "schema-version 1\nfiles { view mode=\"list\"; }\n").unwrap();
    runtime.poll();
    assert!(matches!(runtime.poll(), ReloadEvent::Updated(_)));
    assert_eq!(runtime.current().settings.view.mode, ViewMode::List);
    fs::remove_file(&path).unwrap();
    runtime.poll();
    assert!(
        matches!(runtime.poll(), ReloadEvent::Rejected(_)),
        "explicit missing sources never fall back"
    );
    assert_eq!(runtime.current().settings.view.mode, ViewMode::List);
}

#[test]
fn discovery_survives_graphical_saves_and_explicit_defaults_never_read_ambient_files() {
    if let Some(root) = std::env::var_os("CANOPA_CONFIG_RELOAD_CHILD") {
        let root = std::path::PathBuf::from(root);
        assert!(
            root.starts_with(std::env::temp_dir())
                && root
                    .file_name()
                    .unwrap()
                    .as_encoded_bytes()
                    .starts_with(b"canopa-config-reload-")
        );
        let common = root.join("config/canopa/config.kdl");
        let mut runtime = ConfigurationRuntime::new(Request::Discover).unwrap();
        assert!(runtime.current().source.path.is_none());
        fs::create_dir_all(common.parent().unwrap()).unwrap();
        fs::write(&common, "schema-version 1\nfiles { view mode=\"list\"; }\n").unwrap();
        runtime.poll();
        assert!(matches!(runtime.poll(), ReloadEvent::Updated(_)));
        let mut settings = runtime.current().settings.clone();
        settings.view.mode = ViewMode::Icons;
        let saved = runtime.current().save_settings(settings).unwrap();
        let mut runtime = ConfigurationRuntime::from_snapshot(saved);
        fs::remove_file(&common).unwrap();
        runtime.poll();
        assert!(matches!(runtime.poll(), ReloadEvent::Updated(_)));
        assert!(runtime.current().source.path.is_none());
        std::os::unix::fs::symlink("missing.kdl", &common).unwrap();
        runtime.poll();
        assert!(matches!(runtime.poll(), ReloadEvent::Rejected(_)));
        fs::write(
            common.with_file_name("missing.kdl"),
            "schema-version 1\nfiles { view mode=\"list\"; }\n",
        )
        .unwrap();
        runtime.poll();
        assert!(matches!(runtime.poll(), ReloadEvent::Updated(_)));
        assert_eq!(runtime.current().settings.view.mode, ViewMode::List);
        let mut defaults = ConfigurationRuntime::new(Request::Defaults).unwrap();
        assert!(!defaults.is_watching());
        assert!(matches!(defaults.poll(), ReloadEvent::Unchanged));
        assert!(defaults.current().source.path.is_none());
        assert_eq!(defaults.current().settings.view.mode, ViewMode::Columns);
        return;
    }
    let root = tempfile::Builder::new()
        .prefix("canopa-config-reload-")
        .tempdir()
        .unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "discovery_survives_graphical_saves_and_explicit_defaults_never_read_ambient_files",
        ])
        .env("CANOPA_CONFIG_RELOAD_CHILD", root.path())
        .env("XDG_CONFIG_HOME", root.path().join("config"))
        .env("HOME", root.path().join("home"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
