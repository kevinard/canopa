use canopa_config::{Component, Request};
use canopa_session::{background::Fit, settings::Settings};
use std::path::Path;
#[test]
fn output_inheritance_is_order_independent_and_paths_are_config_relative() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.kdl");
    std::fs::write(&file,"schema-version 1\nwallpaper {\noutput \"DP-1\" { fit \"fit\"; }\nimage \"image.png\"\nfit \"center\"\n}\nlockscreen { widgets \"battery\" \"weather\"; }\n").unwrap();
    let (settings, _) = Settings::load(&Request::common(&file), Component::Wallpaper).unwrap();
    assert_eq!(
        settings.for_output("DP-1").image,
        Some(dir.path().join("image.png"))
    );
    assert_eq!(settings.for_output("DP-1").fit, Fit::Fit);
    assert_eq!(settings.for_output("other").fit, Fit::Center);
    let (lock, _) = Settings::load(&Request::common(&file), Component::Lockscreen).unwrap();
    assert_eq!(lock.widgets, ["battery", "weather"]);
}
#[test]
fn malformed_owner_settings_do_not_poison_other_namespaces() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.kdl");
    for invalid in [
        "clock-format \"%Q\"",
        "dim 101",
        "widgets \"shell-action\"",
        "text-size 999",
        "clock-size a=42",
        "output \"\" {}",
        "color \"bad\"",
        "image \"a\"\nimage \"b\"",
    ] {
        std::fs::write(
            &file,
            format!("schema-version 1\nlockscreen {{\n{invalid}\n}}\n"),
        )
        .unwrap();
        assert!(
            Settings::load(&Request::common(&file), Component::Lockscreen).is_err(),
            "{invalid}"
        );
        assert!(Settings::load(&Request::common(&file), Component::Wallpaper).is_ok());
    }
}
#[test]
fn check_and_preview_never_require_a_compositor_or_authentication() {
    let output = tempfile::tempdir().unwrap();
    let mut check = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-lock"));
    check.env("CANOPA_LANGUAGE", "en");
    check
        .args(["--defaults", "check-config"])
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .assert()
        .success();
    let mut preview = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-lock"));
    preview.env("CANOPA_LANGUAGE", "en");
    preview
        .args(["--defaults", "preview"])
        .arg(output.path().join("preview.png"))
        .args(["--width", "640", "--height", "480"])
        .env_remove("WAYLAND_DISPLAY")
        .assert()
        .success();
    assert!(Path::new(&output.path().join("preview.png")).is_file());
}

#[test]
fn custom_commands_and_layout_are_validated_without_executing_them() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.kdl");
    let marker = dir.path().join("executed");
    let source = format!(
        r##"schema-version 1
lockscreen {{
    widgets "status" "second"
    auth-timeout 10
    layout {{
        margin 24
        gap 8
        top-left "clock" "date"
        center-right "authentication" x=-16
        bottom-right "second" "status" flow="row"
    }}
    widget "status" {{
        command {{ argv "touch" "{}"; }}
        interval 2
        timeout 1
        format "Status: {{output}}"
        preview "Example"
        color "#abcdef"
        size 24
    }}
    widget "second" {{ command "printf 'two words'"; }}
}}
"##,
        marker.display()
    );
    std::fs::write(&file, &source).unwrap();
    let (settings, _) = Settings::load(&Request::common(&file), Component::Lockscreen).unwrap();
    assert_eq!(settings.custom["second"].argv, ["printf", "two words"]);
    assert_eq!(
        settings.custom["status"].directory.as_deref(),
        Some(dir.path())
    );
    assert_eq!(settings.layout.regions[2].widgets, ["second", "status"]);
    assert!(settings.layout.regions[2].row);
    assert_eq!(settings.auth_timeout, 10);
    let preview = dir.path().join("preview.png");
    for command in ["check-config", "preview"] {
        let mut executable = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-lock"));
        executable.env("CANOPA_LANGUAGE", "en");
        executable.arg("--canopa-config").arg(&file).arg(command);
        if command == "preview" {
            executable.arg(&preview);
        }
        executable.env_remove("WAYLAND_DISPLAY").assert().success();
        assert!(!marker.exists());
    }
    for (before, after) in [
        (
            "center-right \"authentication\" x=-16",
            "center-right \"clock\"",
        ),
        (
            "top-left \"clock\" \"date\"",
            "top-left \"clock\" \"authentication\"",
        ),
        (
            "\"second\" \"status\" flow=\"row\"",
            "\"status\" flow=\"row\"",
        ),
        ("flow=\"row\"", "flow=\"diagonal\""),
        ("interval 2", "interval 0"),
        ("timeout 1", "timeout 61"),
        ("auth-timeout 10", "auth-timeout 0"),
        ("color \"#abcdef\"", "color \"unknown\""),
        ("widget \"second\"", "widget \"clock\""),
        ("widgets \"status\" \"second\"", "widgets \"missing\""),
        (
            "command \"printf 'two words'\"",
            "command \"printf 'unterminated\"",
        ),
    ] {
        std::fs::write(&file, source.replace(before, after)).unwrap();
        assert!(
            Settings::load(&Request::common(&file), Component::Lockscreen).is_err(),
            "{after}"
        );
    }
}

#[test]
fn custom_widgets_join_the_default_layout_only_when_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.kdl");
    std::fs::write(&file, "schema-version 1\nlockscreen {\nwidgets \"status\"\nwidget \"status\" { command \"true\"; }\nwidget \"disabled\" { command \"false\"; }\n}\n").unwrap();
    let (settings, _) = Settings::load(&Request::common(&file), Component::Lockscreen).unwrap();
    let placed: Vec<_> = settings
        .layout
        .regions
        .iter()
        .flat_map(|r| &r.widgets)
        .collect();
    assert!(placed.iter().any(|n| n.as_str() == "status"));
    assert!(!placed.iter().any(|n| n.as_str() == "disabled"));
}

#[test]
fn an_explicit_layout_keeps_the_component_spacing_it_does_not_set() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("canopa.kdl");
    for component in [Component::Lockscreen, Component::Greeter] {
        let namespace = component.namespace();
        std::fs::write(
            &file,
            format!("schema-version 1\n{namespace} {{ widgets; layout {{ center \"clock\" \"authentication\"; }}; }}\n"),
        )
        .unwrap();
        let (explicit, _) = Settings::load(&Request::common(&file), component).unwrap();
        let (default, _) = Settings::load(&Request::Defaults, component).unwrap();
        assert_eq!(
            (explicit.layout.margin, explicit.layout.gap),
            (default.layout.margin, default.layout.gap),
            "{namespace}"
        );
        assert_eq!((default.layout.margin, default.layout.gap), (42., 18.));
        std::fs::write(
            &file,
            format!(
                "schema-version 1\n{namespace} {{ widgets; layout {{ margin 10; center \"clock\" \"authentication\"; }}; }}\n"
            ),
        )
        .unwrap();
        let (margin, _) = Settings::load(&Request::common(&file), component).unwrap();
        assert_eq!((margin.layout.margin, margin.layout.gap), (10., 18.));
    }
}
