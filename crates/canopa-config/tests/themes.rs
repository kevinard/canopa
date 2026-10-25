use canopa_config::{Component, Document, Request};
use canopa_theme::{ColorRole, Rgb, UiKind};

#[test]
fn theme_file_inline_and_kind_overrides_resolve_per_property() {
    let dir = tempfile::tempdir().unwrap();
    let theme = dir.path().join("theme.kdl");
    let config = dir.path().join("config.kdl");
    std::fs::write(
        &theme,
        r##"
        palette {
            background "#112233"; accent "#445566";
            spine "#123456"; accent-surface "#234567"; brand "#345678";
        }
        typography { family "Inter" "sans-serif"; body-size 18; }
        ui { surface { radius 10; }; panel { padding 20; radius 14; } }
    "##,
    )
    .unwrap();
    std::fs::write(
        &config,
        r##"
        schema-version 1
        appearance {
            theme "theme.kdl"
            palette { accent "#abcdef"; }
            ui { surface { radius 8; }; panel { padding 0; } }
        }
    "##,
    )
    .unwrap();
    for component in [Component::Shell, Component::Notifications, Component::Files] {
        let document = Document::load(&Request::common(&config), component, "").unwrap();
        let appearance = document.appearance();
        assert_eq!(appearance.palette().background, Rgb(17, 34, 51));
        assert_eq!(appearance.palette().accent, Rgb(171, 205, 239));
        assert_eq!(appearance.role(ColorRole::Spine), Rgb(18, 52, 86));
        assert_eq!(appearance.role(ColorRole::AccentSurface), Rgb(35, 69, 103));
        assert_eq!(appearance.role(ColorRole::Brand), Rgb(52, 86, 120));
        assert_eq!(appearance.typography().body_size.px(), 18.0);
        let panel = appearance.style(UiKind::Panel);
        assert_eq!(panel.radius[0].px(), 8.0);
        assert_eq!(panel.padding.top.px(), 0.0);
        assert_eq!(document.info().dependencies[0].path, theme);
    }
}

#[test]
fn invalid_selected_theme_never_falls_back_to_a_preset() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    std::fs::write(
        &config,
        "schema-version 1\nappearance { theme \"missing.kdl\"; }\n",
    )
    .unwrap();
    assert!(Document::load(&Request::common(&config), Component::Shell, "").is_err());
}

#[test]
fn theme_edits_change_combined_provenance_but_not_main_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    let theme = dir.path().join("theme.kdl");
    std::fs::write(
        &config,
        "schema-version 1\nappearance { theme \"theme.kdl\"; }\n",
    )
    .unwrap();
    std::fs::write(&theme, "palette { accent \"#112233\"; }\n").unwrap();
    let before = Document::load(&Request::common(&config), Component::Shell, "").unwrap();
    let captured =
        canopa_config::capture_theme(&std::fs::read_to_string(&config).unwrap(), Some(&config))
            .unwrap()
            .unwrap();
    let snapshot_theme = dir.path().join("captured.kdl");
    std::fs::write(&snapshot_theme, &captured.text).unwrap();
    let snapshot =
        captured.snapshot_text(&std::fs::read_to_string(&config).unwrap(), &snapshot_theme);
    std::fs::write(&theme, "palette { accent \"#abcdef\"; }\n").unwrap();
    let after = Document::load(&Request::common(&config), Component::Shell, "").unwrap();
    assert_eq!(before.info().fingerprint, after.info().fingerprint);
    assert_ne!(
        before.info().combined_fingerprint,
        after.info().combined_fingerprint
    );
    let frozen = Document::parse(
        snapshot,
        None,
        canopa_config::Format::CanopaV1,
        Component::Notifications,
        "",
    )
    .unwrap();
    assert_eq!(frozen.appearance().palette().accent, Rgb(17, 34, 51));
}

#[test]
fn invalid_dimensions_states_and_recursive_themes_have_safe_diagnostics() {
    for option in [
        "ui { panel { padding -1; }; }",
        "ui { panel { radius 1 2; }; }",
        "ui { panel { gap #inf; }; }",
        "ui { button { state \"hover\" { padding 8; }; }; }",
        "ui { panel { background \"#bad\"; }; }",
        "typography { body-size 0; }",
        "typography { family; }",
        "accent \"#112233\"; palette { accent \"#445566\"; }",
    ] {
        let source = format!("schema-version 1\nappearance {{ {option}; }}\n");
        assert!(
            Document::parse(
                source,
                None,
                canopa_config::Format::CanopaV1,
                Component::Shell,
                ""
            )
            .is_err(),
            "{option}"
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    std::fs::write(
        &config,
        "schema-version 1\nappearance { theme \"theme.kdl\"; }\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("theme.kdl"), "theme \"private-name.kdl\"\n").unwrap();
    let error = Document::load(&Request::common(&config), Component::Shell, "").unwrap_err();
    assert_eq!(error.source, Some(canopa_config::DiagnosticSource::Theme));
    assert!(!error.to_string().contains("private-name"));
}
