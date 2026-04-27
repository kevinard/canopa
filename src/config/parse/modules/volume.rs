use ::kdl::KdlNode;

use super::super::{error, kdl};
use super::formats;
use super::icons;
use super::states;
use crate::config::{StatesConfig, VolumeConfig};

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
    let Some(children) = node.children() else {
        return Ok(VolumeConfig::default());
    };
    let module_path = error::module_path(idx, "volume");
    let step = kdl::optional_u8_range_child(
        children,
        "step",
        VolumeConfig::default().step,
        1,
        100,
        &error::field_path(idx, "volume", "step"),
    )?;
    let states_config =
        states::parse_optional(children, &module_path, StatesConfig::volume_default())?;
    let icons = icons::parse_optional(children, &module_path)?;

    let mut allowed_states: Vec<&str> = vec!["muted"];
    allowed_states.extend(states_config.names());
    let format = formats::parse_optional(
        children,
        &module_path,
        &["icon", "value", "percent", "state"],
        &allowed_states,
    )?;
    let tooltip = formats::parse_tooltip_field(
        children,
        &module_path,
        &["icon", "value", "percent", "state"],
        &allowed_states,
    )?;
    Ok(VolumeConfig {
        step,
        states: states_config,
        icons,
        format,
        tooltip,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    fn volume_config(kdl: &str) -> crate::config::VolumeConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "volume")
            .expect("volume module");
        match &m.params {
            crate::config::ModuleParams::Volume(v) => v.clone(),
            _ => panic!("expected Volume params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = volume_config(r#"modules { module "volume" { tooltip "{value}%" } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = volume_config(r#"modules { module "volume" { tooltip #false } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "volume" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = volume_config(r#"modules { module "volume" {} }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Default));
    }

    #[test]
    fn parse_optional_u8_range_reports_out_of_range() {
        let kdl = r#"modules { module "volume" { step 101 } }"#;
        let err = parse(kdl).unwrap_err();
        assert!(err.to_string().contains("volume.step"));
        assert!(err.to_string().contains("1\u{2013}100"));
    }

    #[test]
    fn parse_volume_states_adds_custom_rule() {
        let kdl = r#"
modules {
    module "volume" {
        states {
            state "quiet" threshold=33
            state "loud"  threshold=100
        }
    }
}
"#;
        let cfg = parse(kdl).unwrap();
        let volume = cfg
            .modules
            .iter()
            .find(|m| m.name == "volume")
            .expect("volume module");
        let crate::config::ModuleParams::Volume(vc) = &volume.params else {
            panic!("expected Volume params");
        };
        assert_eq!(vc.states.threshold_of("quiet"), Some(33));
        assert_eq!(vc.states.threshold_of("loud"), Some(100));
        assert_eq!(vc.states.threshold_of("unmuted"), Some(100));
        // resolution: value 20 → smallest threshold ≥ 20 → quiet (33)
        assert_eq!(vc.states.resolve(20.0), Some("quiet"));
        // value 50 → loud or unmuted both at 100; quiet (33) doesn't cover.
        // Both have the same threshold 100; first in sorted order wins —
        // both are valid, but the test only locks in *some* result.
        let r = vc.states.resolve(50.0).unwrap();
        assert!(r == "loud" || r == "unmuted", "got {r}");
    }

    #[test]
    fn parse_volume_states_rejects_out_of_range_threshold() {
        let kdl = r#"
modules {
    module "volume" {
        states {
            state "bad" threshold=200
        }
    }
}
"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn parse_volume_step_without_value_errors() {
        let kdl = r#"
modules {
    module "volume" {
        step
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("volume.step"),
            "expected 'volume.step' in error, got: {msg}"
        );
        assert!(
            msg.contains("expected an integer value"),
            "expected 'expected an integer value' in error, got: {msg}"
        );
    }
}
