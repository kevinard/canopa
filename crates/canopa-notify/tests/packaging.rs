#[test]
fn service_unit_starts_service_mode_without_autostart_desktop_file() {
    let unit = std::fs::read_to_string("../../packaging/systemd/canopa-notify.service").unwrap();
    assert!(unit.contains("ExecStart=%h/.local/bin/canopa-notify daemon --service"));
    assert!(unit.contains("--socket %t/canopa-notify/control.sock"));

    let autostart = std::path::Path::new("../../packaging/canopa-notify.desktop");
    assert!(!autostart.exists());
}

#[test]
fn graphical_stack_does_not_enable_linux_theme_detection() {
    let workspace_manifest = std::fs::read_to_string("../../Cargo.toml").unwrap();
    let crate_manifest = std::fs::read_to_string("Cargo.toml").unwrap();

    assert!(workspace_manifest.contains("iced = { version = \"0.14\", default-features = false"));
    assert!(
        workspace_manifest
            .contains("iced_layershell = { version = \"0.19.1\", default-features = false")
    );
    assert!(!workspace_manifest.contains("linux-theme-detection"));
    assert!(!crate_manifest.contains("mundy"));
}
