use ::kdl::KdlNode;

use super::super::error;
use super::formats;
use crate::config::TrayConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<TrayConfig> {
    let module_path = error::module_path(idx, "tray");
    formats::reject_format_fields(node.children(), &module_path, "tray")?;

    let tooltip = if let Some(children) = node.children() {
        formats::parse_tooltip_field(children, &module_path, &["title", "description"], &[])?
    } else {
        crate::config::TooltipSetting::Default
    };

    Ok(TrayConfig { tooltip })
}

#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    fn tray_config(kdl: &str) -> crate::config::TrayConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "tray")
            .expect("tray module");
        match &m.params {
            crate::config::ModuleParams::Tray(t) => t.clone(),
            _ => panic!("expected Tray params"),
        }
    }

    #[test]
    fn format_field_rejected() {
        let kdl = r#"modules { module "tray" { format "{title}" } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("format"), "got: {err}");
    }

    #[test]
    fn tooltip_title_string_parsed() {
        let cfg = tray_config(r#"modules { module "tray" { tooltip "{title}" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = tray_config(r#"modules { module "tray" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_unknown_placeholder_errors() {
        let kdl = r#"modules { module "tray" { tooltip "{value}" } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("value"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = tray_config(r#"modules { module "tray" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }
}
