use ::kdl::KdlNode;

use super::super::error;
use super::formats;
use crate::config::FocusedWindowConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<FocusedWindowConfig> {
    let Some(children) = node.children() else {
        return Ok(FocusedWindowConfig::default());
    };
    let module_path = error::module_path(idx, "focused-window");
    let (format, tooltip) = formats::parse_format_and_tooltip(
        children,
        &module_path,
        &["icon", "value", "title"],
        crate::config::states::EMPTY,
    )?;
    Ok(FocusedWindowConfig { format, tooltip })
}

#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    fn focused_window_config(kdl: &str) -> crate::config::FocusedWindowConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "focused-window")
            .expect("focused-window module");
        match &m.params {
            crate::config::ModuleParams::FocusedWindow(f) => f.clone(),
            _ => panic!("expected FocusedWindow params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg =
            focused_window_config(r#"modules { module "focused-window" { tooltip "{title}" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg =
            focused_window_config(r#"modules { module "focused-window" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "focused-window" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = focused_window_config(r#"modules { module "focused-window" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }
}
