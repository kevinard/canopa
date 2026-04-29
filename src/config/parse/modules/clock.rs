use ::kdl::KdlNode;

use super::super::{error, kdl};
use super::formats;
use crate::config::ClockConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<ClockConfig> {
    let Some(children) = node.children() else {
        return Ok(ClockConfig::default());
    };

    let defaults = ClockConfig::default();

    let time_format = kdl::optional_nonempty_string_child(
        children,
        "time-format",
        defaults.time_format,
        &error::field_path(idx, "clock", "time-format"),
        "a chrono format string such as \"%H:%M\"",
    )?;

    let date_format = kdl::optional_nonempty_string_child(
        children,
        "date-format",
        defaults.date_format,
        &error::field_path(idx, "clock", "date-format"),
        "a chrono format string such as \"%a %d %b\"",
    )?;
    let format = formats::parse_optional(
        children,
        &error::module_path(idx, "clock"),
        &["icon", "value", "time", "date"],
        crate::config::states::EMPTY,
    )?;
    let tooltip = formats::parse_tooltip_field(
        children,
        &error::module_path(idx, "clock"),
        &["icon", "value", "time", "date"],
        crate::config::states::EMPTY,
    )?;

    Ok(ClockConfig {
        time_format,
        date_format,
        format,
        tooltip,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::ModuleParams;
    use crate::config::parse::parse;

    fn clock_config(kdl: &str) -> crate::config::ClockConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "clock")
            .expect("clock module");
        match &m.params {
            ModuleParams::Clock(c) => c.clone(),
            _ => panic!("expected Clock params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = clock_config(r#"modules { module "clock" { tooltip "{date}" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = clock_config(r#"modules { module "clock" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "clock" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = clock_config(r#"modules { module "clock" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn parse_clock_with_custom_formats() {
        let kdl = r#"
modules {
    module "clock" {
        time-format "%I:%M %p"
        date-format "%Y-%m-%d"
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Clock(cfg) => {
                assert_eq!(cfg.time_format, "%I:%M %p");
                assert_eq!(cfg.date_format, "%Y-%m-%d");
            }
            _ => panic!("expected ModuleParams::Clock"),
        }
    }

    #[test]
    fn parse_clock_uses_defaults_without_children() {
        let kdl = r#"modules { module "clock" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Clock(cfg) => {
                assert_eq!(cfg.time_format, "%H:%M");
                assert_eq!(cfg.date_format, "%a %d %b");
            }
            _ => panic!("expected ModuleParams::Clock"),
        }
    }

    #[test]
    fn parse_clock_rejects_empty_time_format() {
        let kdl = r#"
modules {
    module "clock" {
        time-format ""
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("time-format"),
            "expected 'time-format' in error, got: {msg}"
        );
        assert!(
            msg.contains("must not be empty"),
            "expected 'must not be empty' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_clock_rejects_empty_date_format() {
        let kdl = r#"
modules {
    module "clock" {
        date-format ""
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("date-format"),
            "expected 'date-format' in error, got: {msg}"
        );
        assert!(
            msg.contains("must not be empty"),
            "expected 'must not be empty' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_clock_time_format_without_value_errors() {
        let kdl = r#"
modules {
    module "clock" {
        time-format
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("clock.time-format"),
            "expected 'clock.time-format' in error, got: {msg}"
        );
        assert!(
            msg.contains("expected a string value"),
            "expected 'expected a string value' in error, got: {msg}"
        );
    }
}
