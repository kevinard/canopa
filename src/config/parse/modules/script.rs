use ::kdl::KdlNode;

use super::super::{error, kdl};
use super::formats;
use crate::config::ScriptConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<ScriptConfig> {
    let command_path = error::field_path(idx, "script", "command");
    let children = node.children().ok_or_else(|| {
        anyhow::anyhow!("{command_path}: missing required parameter. Expected: a string value.")
    })?;

    if children.get("command").is_none() {
        anyhow::bail!("{command_path}: missing required parameter. Expected: a string value.");
    }
    let command = kdl::optional_nonempty_string_child(
        children,
        "command",
        String::new(),
        &command_path,
        "a shell command string",
    )?;

    let interval = match children.get("interval") {
        None => 5u64,
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_integer()) {
                Some(i) if i >= 1 => i as u64,
                Some(_) => anyhow::bail!(
                    "{}: value must be at least 1. Expected: a positive integer.",
                    error::field_path(idx, "script", "interval")
                ),
                None => {
                    anyhow::bail!(
                        "{}: expected a positive integer.",
                        error::field_path(idx, "script", "interval")
                    )
                }
            }
        }
    };

    let format = formats::parse_optional(
        children,
        &error::module_path(idx, "script"),
        &["icon", "value", "output"],
        crate::config::states::EMPTY,
    )?;
    let tooltip = formats::parse_tooltip_field(
        children,
        &error::module_path(idx, "script"),
        &["icon", "value", "output"],
        crate::config::states::EMPTY,
    )?;

    Ok(ScriptConfig {
        command,
        interval,
        format,
        tooltip,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::ModuleParams;
    use crate::config::parse::parse;

    fn script_config(kdl: &str) -> crate::config::ScriptConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "script")
            .expect("script module");
        match &m.params {
            ModuleParams::Script(s) => s.clone(),
            _ => panic!("expected Script params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = script_config(
            "modules { module \"script\" { command \"date\"; tooltip \"{output}\" } }",
        );
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = script_config(
            "modules { module \"script\" { command \"date\"; tooltip #false } }",
        );
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = "modules { module \"script\" { command \"date\"; tooltip #true } }";
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = script_config(r#"modules { module "script" { command "date" } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Default));
    }

    #[test]
    fn parse_script_with_command_and_interval() {
        let kdl = r#"
modules {
    module "script" {
        command "date"
        interval 10
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Script(cfg) => {
                assert_eq!(cfg.command, "date");
                assert_eq!(cfg.interval, 10);
            }
            _ => panic!("expected ModuleParams::Script"),
        }
    }

    #[test]
    fn parse_script_defaults_interval_to_5() {
        let kdl = r#"
modules {
    module "script" {
        command "date"
    }
}
"#;
        let config = parse(kdl).unwrap();
        match &config.modules[0].params {
            ModuleParams::Script(cfg) => {
                assert_eq!(cfg.command, "date");
                assert_eq!(cfg.interval, 5);
            }
            _ => panic!("expected ModuleParams::Script"),
        }
    }

    #[test]
    fn parse_script_missing_command_errors() {
        let kdl = r#"modules { module "script" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("script.command"),
            "expected 'script.command' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_script_empty_command_errors() {
        let kdl = r#"
modules {
    module "script" {
        command ""
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("script.command"),
            "expected 'script.command' in error, got: {msg}"
        );
        assert!(
            msg.contains("must not be empty"),
            "expected 'must not be empty' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_script_command_without_value_errors() {
        let kdl = r#"
modules {
    module "script" {
        command
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("script.command"),
            "expected 'script.command' in error, got: {msg}"
        );
        assert!(
            msg.contains("expected a string value"),
            "expected 'expected a string value' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_script_zero_interval_errors() {
        let kdl = r#"
modules {
    module "script" {
        command "date"
        interval 0
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("script.interval"),
            "expected 'script.interval' in error, got: {msg}"
        );
        assert!(
            msg.contains("at least 1"),
            "expected 'at least 1' in error, got: {msg}"
        );
    }
}
