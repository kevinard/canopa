use super::*;

fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    value.into().try_into_owned().unwrap()
}

fn draft(options: Options) -> fdo::Result<LauncherDraft> {
    LauncherDraft::parse(
        "org.canopa.Fixture".into(),
        "wayland:parent".into(),
        "Fixture".into(),
        options,
    )
}

fn icon(bytes: Vec<u8>) -> OwnedValue {
    value(("bytes", Value::from(bytes)))
}

#[test]
fn defaults_and_webapp_context_preserve_exact_review_options() {
    let application = draft(Options::new()).unwrap();
    assert!(application.modal && application.editable_name);
    assert_eq!(application.launcher_type, LauncherType::Application);
    assert!(application.target.is_empty());
    let webapp = draft(Options::from([
        ("launcher_type".into(), 2_u32.into()),
        ("target".into(), value("https://example.test/a?q=one#two")),
        ("editable_name".into(), false.into()),
        ("editable_icon".into(), true.into()),
        ("modal".into(), false.into()),
        ("future-option".into(), true.into()),
    ]))
    .unwrap();
    assert_eq!(webapp.launcher_type, LauncherType::Webapp);
    assert_eq!(webapp.target, "https://example.test/a?q=one#two");
    assert!(!webapp.modal && !webapp.editable_name);
}

#[test]
fn invalid_types_bounds_and_missing_webapp_target_are_rejected() {
    for options in [
        Options::from([("launcher_type".into(), 3_u32.into())]),
        Options::from([("launcher_type".into(), 2_u32.into())]),
        Options::from([("launcher_type".into(), 1_i32.into())]),
        Options::from([("modal".into(), value("true"))]),
        Options::from([("editable_name".into(), 1_u32.into())]),
        Options::from([("editable_icon".into(), value("true"))]),
        Options::from([("target".into(), value("not a URI"))]),
        Options::from([("target".into(), value("https://example.test/\nmisleading"))]),
        (0..65)
            .map(|i| (format!("option{i}"), true.into()))
            .collect(),
    ] {
        assert!(draft(options).is_err());
    }
    for name in [
        "x".repeat(NAME_BYTES + 1),
        "line\nbreak".into(),
        "zero\0byte".into(),
    ] {
        assert!(LauncherDraft::parse("app".into(), "".into(), name, Options::new()).is_err());
    }
    assert!(
        LauncherDraft::parse(
            "x".repeat(FIELD_BYTES + 1),
            "".into(),
            "Name".into(),
            Options::new()
        )
        .is_err()
    );
    assert!(LauncherDraft::parse("app".into(), "".into(), "".into(), Options::new()).is_ok());
    assert!(
        LauncherDraft::parse(
            "app".into(),
            "".into(),
            "".into(),
            Options::from([("editable_name".into(), false.into())])
        )
        .is_err()
    );
}

#[test]
fn only_exact_bounded_bytes_icon_shape_is_accepted() {
    assert_eq!(
        LauncherIcon::parse(icon(vec![1, 2, 3])).unwrap().bytes(),
        [1, 2, 3]
    );
    for value in [
        value("themed"),
        value(("themed", Value::from(vec!["icon"]))),
        value(("bytes", "not a variant")),
        value(("bytes", Value::from(vec![1_u32, 2]))),
        value(("bytes", Value::from(vec![1_u8]), 1_u32)),
        value(("bytes", Value::Value(Box::new(Value::from(vec![1_u8]))))),
        icon(Vec::new()),
        icon(vec![1; ICON_BYTES + 1]),
    ] {
        assert!(LauncherIcon::parse(value).is_err());
    }
}

#[test]
fn successful_result_has_frontend_required_nested_icon_and_original_bytes() {
    let bytes = b"original format and private fixture metadata".to_vec();
    let request = LauncherRequest {
        draft: draft(Options::new()).unwrap(),
        icon: LauncherIcon::parse(icon(bytes.clone())).unwrap(),
    };
    let (status, mut results) = request.response("Edited name".into());
    assert_eq!(status, 0);
    assert_eq!(results.len(), 2);
    assert_eq!(
        String::try_from(results.remove("name").unwrap()).unwrap(),
        "Edited name"
    );
    let result = Value::from(results.remove("icon").unwrap());
    assert_eq!(result.value_signature().to_string(), "v");
    let Value::Value(serialized) = result else {
        panic!("missing icon variant")
    };
    let returned = LauncherIcon::parse(serialized.try_into_owned().unwrap()).unwrap();
    assert_eq!(returned.bytes(), bytes);
}

#[test]
fn invalid_or_disallowed_helper_edits_cannot_return_success() {
    let mut request = LauncherRequest {
        draft: draft(Options::new()).unwrap(),
        icon: LauncherIcon::parse(icon(vec![1])).unwrap(),
    };
    for name in [
        "".into(),
        "  ".into(),
        "bad\nname".into(),
        "é".repeat(NAME_BYTES / 2 + 1),
    ] {
        let (status, values) = request.response(name);
        assert_eq!(status, 2);
        assert!(values.is_empty());
    }
    request.draft.editable_name = false;
    assert_eq!(request.response("Altered by helper".into()).0, 2);
    assert_eq!(request.response("Fixture".into()).0, 0);
}
