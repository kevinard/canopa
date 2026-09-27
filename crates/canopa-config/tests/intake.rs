use canopa_config::{
    Code, Component, Document, Format, MAX_BYTES, Request, config_home_from, discover_in,
};
use std::{
    ffi::OsStr,
    fs,
    os::unix::{fs::FileTypeExt, fs::symlink},
    path::Path,
};

#[test]
fn appearance_is_shared_even_when_feature_namespaces_default() {
    let source = "schema-version 1\nappearance { color-scheme \"light\"; accent \"#123456\"; }\n";
    for component in [Component::Shell, Component::Notifications] {
        let document = Document::parse(
            source.into(),
            None,
            Format::CanopaV1,
            component,
            "fallback\n",
        )
        .unwrap();
        assert!(document.info().defaults);
        assert_eq!(document.selected().nodes()[0].name().value(), "fallback");
        assert_eq!(
            document.appearance(),
            canopa_theme::Appearance {
                color_scheme: canopa_theme::ColorScheme::Light,
                accent: Some(canopa_theme::Rgb(18, 52, 86)),
            }
        );
    }
}

#[test]
fn invalid_appearance_rejects_both_consumers_with_original_safe_location() {
    for option in [
        "color-scheme \"private-value\"",
        "accent \"private-value\"",
        "font \"private-value\"",
        "accent (private-value)\"#123456\"",
        "accent value=\"#123456\"",
        "accent \"#123456\" {}",
        "accent \"#123456\"; accent \"#123456\"",
        "color-scheme 1",
    ] {
        let source = format!("schema-version 1\nappearance {{ {option}; }}\n");
        for component in [Component::Shell, Component::Notifications] {
            let error =
                Document::parse(source.clone(), None, Format::CanopaV1, component, "").unwrap_err();
            assert_eq!(error.code, Code::Appearance, "{source}");
            assert!(error.offset.unwrap() >= source.find(option).unwrap());
            assert!(!format!("{error} {error:?}").contains("private-value"));
        }
    }
}

#[test]
fn discovery_prefers_common_even_when_invalid_and_never_merges_legacy() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        discover_in(dir.path(), Component::Shell).unwrap(),
        Request::Defaults
    );
    fs::create_dir(dir.path().join("niribar")).unwrap();
    let legacy = dir.path().join("niribar/config.kdl");
    fs::write(&legacy, "legacy\n").unwrap();
    assert_eq!(
        discover_in(dir.path(), Component::Shell).unwrap(),
        Request::legacy(&legacy)
    );
    fs::create_dir(dir.path().join("canopa")).unwrap();
    let common = dir.path().join("canopa/config.kdl");
    fs::write(&common, "invalid\n").unwrap();
    let request = discover_in(dir.path(), Component::Shell).unwrap();
    assert_eq!(request, Request::common(&common));
    assert_eq!(
        Document::load(&request, Component::Shell, "default\n")
            .unwrap_err()
            .code,
        Code::Envelope
    );
    fs::write(&common, "schema-version 1\nnotifications {}\n").unwrap();
    let loaded = Document::load(&request, Component::Shell, "default\n").unwrap();
    assert_eq!(loaded.selected().nodes()[0].name().value(), "default");
    assert!(loaded.info().defaults);
    assert_eq!(loaded.info().path.as_deref(), Some(common.as_path()));
    fs::remove_file(&common).unwrap();
    symlink(dir.path().join("missing"), &common).unwrap();
    assert_eq!(discover_in(dir.path(), Component::Shell).unwrap(), request);
    assert_eq!(
        Document::load(&request, Component::Shell, "")
            .unwrap_err()
            .code,
        Code::Read
    );
}

#[test]
fn namespaces_preserve_absolute_spans_and_share_revision() {
    let text = "schema-version 1\nnotifications { private-rule 42; }\nshell { bar \"primary\"; }\n";
    let shell = Document::parse(text.into(), None, Format::CanopaV1, Component::Shell, "").unwrap();
    let notify = Document::parse(
        text.into(),
        None,
        Format::CanopaV1,
        Component::Notifications,
        "",
    )
    .unwrap();
    assert_eq!(
        shell.selected().nodes()[0].span().offset(),
        text.find("bar").unwrap()
    );
    assert_eq!(shell.text(), text);
    assert_eq!(shell.info().fingerprint, notify.info().fingerprint);
    assert_eq!(shell.info().fingerprint.len(), 64);
    assert!(!format!("{shell:?}").contains("private-rule"));
    let changed = Document::parse(
        format!("{text}// comment\n"),
        None,
        Format::CanopaV1,
        Component::Shell,
        "",
    )
    .unwrap();
    assert_ne!(shell.info().fingerprint, changed.info().fingerprint);
}

#[test]
fn envelope_is_strict_but_namespace_semantics_belong_to_owners() {
    for text in [
        "shell {}",
        "schema-version 2",
        "schema-version 1\nshell\n",
        "schema-version 1\nshell 1 {}",
        "schema-version 1\nprivate-value {}",
        "schema-version 1\nappearance {}\nappearance {}",
        "schema-version 1\nshell {}\nshell {}",
        "schema-version 1\n(x)shell {}",
        "schema-version (x)1",
        "schema-version 1\nschema-version 1",
    ] {
        let error =
            Document::parse(text.into(), None, Format::CanopaV1, Component::Shell, "").unwrap_err();
        assert_eq!(error.code, Code::Envelope, "{text}");
        assert!(!format!("{error:?} {error}").contains("private-value"));
    }
    Document::parse(
        "schema-version 1\nnotifications { invalid 1; }\nshell {}".into(),
        None,
        Format::CanopaV1,
        Component::Shell,
        "",
    )
    .unwrap();
    let error = Document::parse(
        "private-value {".into(),
        None,
        Format::Legacy,
        Component::Shell,
        "",
    )
    .unwrap_err();
    assert_eq!(error.code, Code::Syntax);
    assert!(!format!("{error:?} {error}").contains("private-value"));
}

#[test]
fn source_admission_is_bounded_descriptor_checked_and_symlink_friendly() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config");
    fs::write(&file, " ".repeat(MAX_BYTES)).unwrap();
    let link = dir.path().join("link");
    symlink(&file, &link).unwrap();
    assert!(canopa_config::read(&link).is_ok());
    for bytes in [vec![b' '; MAX_BYTES + 1], vec![255]] {
        fs::write(&file, bytes).unwrap();
        assert_eq!(canopa_config::read(&file).unwrap_err().code, Code::Read);
    }
    assert_eq!(
        canopa_config::read(dir.path()).unwrap_err().code,
        Code::Read
    );
    let fifo = dir.path().join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert!(fs::metadata(&fifo).unwrap().file_type().is_fifo());
    let start = std::time::Instant::now();
    assert_eq!(canopa_config::read(&fifo).unwrap_err().code, Code::Read);
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn xdg_paths_require_absolute_nonempty_values() {
    for xdg in [None, Some(OsStr::new("")), Some(OsStr::new("relative"))] {
        assert_eq!(
            config_home_from(xdg, Some(OsStr::new("/home/test"))).unwrap(),
            Path::new("/home/test/.config")
        );
    }
    assert_eq!(
        config_home_from(Some(OsStr::new("/config")), None).unwrap(),
        Path::new("/config")
    );
    assert!(config_home_from(None, Some(OsStr::new("relative"))).is_err());
}

#[test]
fn shared_only_intake_reuses_appearance_validation_without_legacy_or_feature_semantics() {
    use canopa_config::{SharedConfiguration, discover_common_in};
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("niribar")).unwrap();
    fs::write(
        dir.path().join("niribar/config.kdl"),
        "invalid private-value",
    )
    .unwrap();
    assert_eq!(discover_common_in(dir.path()).unwrap(), Request::Defaults);
    assert_eq!(
        SharedConfiguration::load(&Request::Defaults)
            .unwrap()
            .appearance,
        Default::default()
    );
    assert!(
        SharedConfiguration::load(&Request::legacy(dir.path().join("niribar/config.kdl"))).is_err()
    );
    fs::create_dir(dir.path().join("canopa")).unwrap();
    let path = dir.path().join("canopa/config.kdl");
    let text = "schema-version 1\nappearance { color-scheme \"light\"; accent \"#123456\"; }\nshell { unknown \"private-value\"; }\nnotifications { unknown 1; }\n";
    fs::write(&path, text).unwrap();
    let request = discover_common_in(dir.path()).unwrap();
    let shared = SharedConfiguration::load(&request).unwrap();
    let shell = Document::load(&request, Component::Shell, "").unwrap();
    assert_eq!(shared.appearance, shell.appearance());
    assert_eq!(shared.source.fingerprint, shell.info().fingerprint);
    assert!(!shared.source.defaults);
    for text in [
        "schema-version 1\nappearance { accent \"private-value\"; }",
        "private-value {}",
        "schema-version 1\nshell {",
    ] {
        fs::write(&path, text).unwrap();
        let error = SharedConfiguration::load(&request).unwrap_err();
        assert!(!format!("{error} {error:?}").contains("private-value"));
    }
    fs::remove_file(&path).unwrap();
    symlink(dir.path().join("missing"), &path).unwrap();
    assert_eq!(discover_common_in(dir.path()).unwrap(), request);
    assert_eq!(
        SharedConfiguration::load(&request).unwrap_err().code,
        Code::Read
    );
}
