use canopa_config::Request;
#[test]
fn configuration_preview_and_validation_do_not_connect_or_execute_commands() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    let output = root.path().join("preview.png");
    std::fs::write(&path, "schema-version 1\npolkit {auth-timeout 10;}\n").unwrap();
    for action in ["check-config", "preview"] {
        let mut app = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-polkit"));
        app.env("CANOPA_LANGUAGE", "en");
        app.arg("--canopa-config").arg(&path).arg(action);
        if action == "preview" {
            app.arg(&output);
        }
        app.env_remove("WAYLAND_DISPLAY")
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                "unix:path=/unavailable-fixture-bus",
            )
            .assert()
            .success();
    }
    assert_eq!(
        image::open(output).unwrap().to_rgb8().dimensions(),
        canopa_session_ui::view::AUTHORIZATION_DIALOG_SIZE
    );
    for setting in [
        "auth-timeout 1;",
        "widgets \"media\";",
        "widget \"x\" {command \"touch no\";}",
        "image \"private.png\";",
        "auth-timeout 10;auth-timeout 20;",
    ] {
        std::fs::write(&path, format!("schema-version 1\npolkit {{ {setting} }}\n")).unwrap();
        assert!(canopa_polkit::config::load(&Request::common(&path)).is_err());
    }
}
