use crate::config::parse::parse;

#[test]
fn parse_modules_section_absent() {
    let config = parse("").unwrap();
    assert!(config.modules.is_empty());
}

#[test]
fn parse_single_module() {
    let kdl = r#"modules { module "clock" }"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules.len(), 1);
    assert_eq!(config.modules[0].name, "clock");
}

#[test]
fn parse_multiple_modules_order_preserved() {
    let kdl = r#"
modules {
    module "clock"
    module "battery"
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules.len(), 2);
    assert_eq!(config.modules[0].name, "clock");
    assert_eq!(config.modules[1].name, "battery");
}

#[test]
fn parse_modules_empty_block() {
    let kdl = r#"modules { }"#;
    let config = parse(kdl).unwrap();
    assert!(config.modules.is_empty());
}

#[test]
fn parse_wrong_node_name_errors() {
    let kdl = r#"modules { widget "clock" }"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.starts_with("modules[0]:"),
        "expected 'modules[0]:' prefix, got: {msg}"
    );
    assert!(
        msg.contains("widget"),
        "expected wrong node name in error: {msg}"
    );
}

#[test]
fn parse_named_module_infers_type_from_name() {
    // When name matches a known type, module_type is inferred automatically.
    let kdl = r#"modules { module "clock" }"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules[0].name, "clock");
    assert_eq!(config.modules[0].module_type, "clock");
}

#[test]
fn parse_named_module_explicit_type() {
    // When name doesn't match a known type, explicit `type` child is required.
    let kdl = r#"
modules {
    module "work-clock" {
        type "clock"
        time-format "%H:%M"
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules[0].name, "work-clock");
    assert_eq!(config.modules[0].module_type, "clock");
    use crate::config::ModuleParams;
    match &config.modules[0].params {
        ModuleParams::Clock(cfg) => assert_eq!(cfg.time_format, "%H:%M"),
        _ => panic!("expected Clock params"),
    }
}

#[test]
fn parse_named_module_missing_explicit_type_errors() {
    let kdl = r#"modules { module "my-custom" }"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("my-custom"),
        "expected instance name in error, got: {msg}"
    );
    assert!(
        msg.contains("type"),
        "expected 'type' hint in error, got: {msg}"
    );
}

#[test]
fn parse_two_instances_same_type() {
    let kdl = r#"
modules {
    module "clock"
    module "clock2" {
        type "clock"
        time-format "%I:%M %p"
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules.len(), 2);
    assert_eq!(config.modules[0].name, "clock");
    assert_eq!(config.modules[0].module_type, "clock");
    assert_eq!(config.modules[1].name, "clock2");
    assert_eq!(config.modules[1].module_type, "clock");
}

#[test]
fn parse_bar_and_modules_sections_coexist() {
    let kdl = r#"
bar "main" { }
modules {
    module "clock"
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.bars.len(), 1);
    assert_eq!(config.modules.len(), 1);
}
