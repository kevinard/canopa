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

#[test]
fn notification_portal_activation_uses_the_existing_notify_service_and_a_dedicated_descriptor() {
    let descriptor =
        std::fs::read_to_string("../../packaging/portal/canopa-notify.portal").unwrap();
    assert!(
        descriptor
            .lines()
            .any(|line| line == "DBusName=org.freedesktop.impl.portal.desktop.canopa.notify")
    );
    assert!(
        descriptor
            .lines()
            .any(|line| line == "Interfaces=org.freedesktop.impl.portal.Notification;")
    );
    let activation = std::fs::read_to_string(
        "../../packaging/portal/org.freedesktop.impl.portal.desktop.canopa.notify.service",
    )
    .unwrap();
    assert!(
        activation
            .lines()
            .any(|line| line == "Exec=/usr/bin/env canopa-notify daemon --service")
    );
    assert!(
        activation
            .lines()
            .any(|line| line == "SystemdService=canopa-notify.service")
    );
}
