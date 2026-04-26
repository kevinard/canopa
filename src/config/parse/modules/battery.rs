use ::kdl::KdlNode;

use super::super::{error, kdl};
use super::formats;
use super::icons;
use super::states;
use crate::config::{BatteryConfig, StatesConfig};

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
    let Some(children) = node.children() else {
        return Ok(BatteryConfig::default());
    };

    let module_path = error::module_path(idx, "battery");

    // Start from built-in defaults and apply the user's `states` block (if any).
    let states_config =
        states::parse_optional(children, &module_path, StatesConfig::battery_default())?;

    // Backward-compat: `critical-threshold N`. If `states` redefines `critical`,
    // that wins; otherwise this overrides the built-in critical threshold.
    let critical_override = if children.get("critical-threshold").is_some() {
        Some(kdl::optional_u8_range_child(
            children,
            "critical-threshold",
            15,
            0,
            100,
            &error::field_path(idx, "battery", "critical-threshold"),
        )?)
    } else {
        None
    };

    let mut states_config = states_config;
    let user_set_critical_via_states = children
        .get("states")
        .and_then(|n| n.children())
        .map(|doc| {
            doc.nodes()
                .iter()
                .any(|c| c.entries().iter().any(|e| e.value().as_string() == Some("critical")))
        })
        .unwrap_or(false);

    if let Some(t) = critical_override {
        if !user_set_critical_via_states {
            states_config.upsert("critical".into(), t);
        }
        // If both set "critical", `states` wins silently (deprecation path).
    }

    let critical_threshold = states_config.threshold_of("critical").unwrap_or(15);

    let icons = icons::parse_optional(children, &module_path)?;

    // Format selectors may reference any state the module can emit:
    // built-in non-numeric override `charging` plus all numeric state names.
    let mut allowed_states: Vec<&str> = vec!["charging"];
    allowed_states.extend(states_config.names());
    let format = formats::parse_optional(
        children,
        &module_path,
        &["icon", "value", "percent", "state"],
        &allowed_states,
    )?;

    Ok(BatteryConfig {
        critical_threshold,
        states: states_config,
        icons,
        format,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    fn battery_config(kdl: &str) -> crate::config::BatteryConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "battery")
            .expect("battery module");
        match &m.params {
            crate::config::ModuleParams::Battery(b) => b.clone(),
            _ => panic!("expected Battery params"),
        }
    }

    #[test]
    fn states_block_overrides_builtin_critical() {
        let cfg = battery_config(
            r#"modules { module "battery" { states { state "critical" threshold=10 } } }"#,
        );
        assert_eq!(cfg.states.threshold_of("critical"), Some(10));
        assert_eq!(cfg.critical_threshold, 10);
    }

    #[test]
    fn states_block_adds_custom_state() {
        let cfg = battery_config(
            r#"modules { module "battery" { states { state "needs-charge" threshold=60 } } }"#,
        );
        assert_eq!(cfg.states.threshold_of("needs-charge"), Some(60));
        // Built-in critical retained
        assert_eq!(cfg.states.threshold_of("critical"), Some(15));
    }

    #[test]
    fn resolution_picks_smallest_threshold_above_value() {
        let cfg = battery_config(
            r#"modules { module "battery" { states { state "needs-charge" threshold=60 } } }"#,
        );
        // value 45 → matches needs-charge (60) and normal (100); smallest = 60
        assert_eq!(cfg.states.resolve(45.0), Some("needs-charge"));
        // value 10 → critical (15)
        assert_eq!(cfg.states.resolve(10.0), Some("critical"));
        // value 100 → normal (100)
        assert_eq!(cfg.states.resolve(100.0), Some("normal"));
    }

    #[test]
    fn critical_threshold_field_still_overrides_when_no_states() {
        let cfg = battery_config(r#"modules { module "battery" { critical-threshold 5 } }"#);
        assert_eq!(cfg.critical_threshold, 5);
        assert_eq!(cfg.states.threshold_of("critical"), Some(5));
    }

    #[test]
    fn states_block_wins_over_critical_threshold_field() {
        let cfg = battery_config(
            r#"modules { module "battery" { critical-threshold 5; states { state "critical" threshold=20 } } }"#,
        );
        assert_eq!(cfg.states.threshold_of("critical"), Some(20));
        assert_eq!(cfg.critical_threshold, 20);
    }

    #[test]
    fn duplicate_state_name_in_user_block_errors() {
        let kdl = r#"modules { module "battery" { states { state "low" threshold=20; state "low" threshold=25 } } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("duplicate state"), "got: {err}");
    }

    #[test]
    fn missing_threshold_property_errors() {
        let kdl = r#"modules { module "battery" { states { state "x" } } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("missing `threshold`"), "got: {err}");
    }

    #[test]
    fn empty_state_name_errors() {
        let kdl = r#"modules { module "battery" { states { state "" threshold=10 } } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("must not be empty"), "got: {err}");
    }

    #[test]
    fn custom_state_usable_in_format_selector() {
        let kdl = r#"
modules {
    module "battery" {
        states { state "needs-charge" threshold=60 }
        format {
            segment "{icon}" {
                style state="needs-charge" color="warning"
            }
        }
    }
}
"#;
        // Should parse without "unknown state" error.
        battery_config(kdl);
    }
}

