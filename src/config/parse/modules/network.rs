use ::kdl::KdlNode;

use super::super::error;
use super::formats;
use super::icons;
use crate::config::NetworkConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<NetworkConfig> {
    let Some(children) = node.children() else {
        return Ok(NetworkConfig::default());
    };
    let icons = icons::parse_optional(children, &error::module_path(idx, "network"))?;
    let format = formats::parse_optional(
        children,
        &error::module_path(idx, "network"),
        &["icon", "value", "label", "state"],
        crate::config::states::NETWORK,
    )?;
    let tooltip = formats::parse_tooltip_field(
        children,
        &error::module_path(idx, "network"),
        &["icon", "value", "label", "state"],
        crate::config::states::NETWORK,
    )?;
    Ok(NetworkConfig {
        icons,
        format,
        tooltip,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    fn network_config(kdl: &str) -> crate::config::NetworkConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "network")
            .expect("network module");
        match &m.params {
            crate::config::ModuleParams::Network(n) => n.clone(),
            _ => panic!("expected Network params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = network_config(r#"modules { module "network" { tooltip "{label}" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = network_config(r#"modules { module "network" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "network" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = network_config(r#"modules { module "network" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }
}
