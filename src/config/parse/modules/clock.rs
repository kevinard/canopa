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
    )?;

    Ok(ClockConfig {
        time_format,
        date_format,
        format,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::ModuleParams;
    use crate::config::parse::parse;

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
