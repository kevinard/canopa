use ::kdl::KdlNode;

use super::super::{error, kdl};
use super::formats;
use crate::config::{ButtonConfig, ModuleActionsConfig};

pub(super) fn parse(
    node: &KdlNode,
    idx: usize,
    actions: &ModuleActionsConfig,
) -> anyhow::Result<ButtonConfig> {
    let button_path = error::module_path(idx, "button");
    let label_path = error::field_path(idx, "button", "label");
    let icon_path = error::field_path(idx, "button", "icon");
    let on_click_path = error::field_path(idx, "button", "on-click");

    let children = node.children().ok_or_else(|| {
        anyhow::anyhow!(
            "{button_path}: missing required parameters block. Expected child nodes: label, icon, on-click."
        )
    })?;

    if children.get("label").is_none() {
        anyhow::bail!("{label_path}: missing required parameter. Expected: a string value.");
    }
    let label = kdl::required_string_child(children, "label", &label_path)?;

    if children.get("icon").is_none() {
        anyhow::bail!("{icon_path}: missing required parameter. Expected: a string value.");
    }
    let icon = kdl::required_string_child(children, "icon", &icon_path)?;

    let on_click = if children.get("on-click").is_some() {
        kdl::required_string_child(children, "on-click", &on_click_path)?
    } else if let Some(command) = actions.click.as_ref() {
        command.clone()
    } else {
        anyhow::bail!(
            "{on_click_path}: missing required parameter. Expected: a string value or actions.click."
        );
    };
    let (format, tooltip) = formats::parse_format_and_tooltip(
        children,
        &button_path,
        &["icon", "value", "label"],
        crate::config::states::EMPTY,
    )?;

    Ok(ButtonConfig {
        label,
        icon,
        on_click,
        format,
        tooltip,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::ModuleParams;
    use crate::config::parse::parse;

    fn button_config(kdl: &str) -> crate::config::ButtonConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "button")
            .expect("button module");
        match &m.params {
            ModuleParams::Button(b) => b.clone(),
            _ => panic!("expected Button params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = button_config(
            "modules { module \"button\" { label \"L\"; icon \">\"; on-click \"x\"; tooltip \"{label}\" } }",
        );
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = button_config(
            "modules { module \"button\" { label \"L\"; icon \">\"; on-click \"x\"; tooltip #false } }",
        );
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = "modules { module \"button\" { label \"L\"; icon \">\"; on-click \"x\"; tooltip #true } }";
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = button_config(
            "modules { module \"button\" { label \"L\"; icon \">\"; on-click \"x\" } }",
        );
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn parse_button_with_all_params() {
        let kdl = r#"
modules {
    module "button" {
        label "Launch Terminal"
        icon ">"
        on-click "kitty"
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Button(cfg) => {
                assert_eq!(cfg.label, "Launch Terminal");
                assert_eq!(cfg.icon, ">");
                assert_eq!(cfg.on_click, "kitty");
            }
            _ => panic!("expected ModuleParams::Button"),
        }
    }

    #[test]
    fn parse_button_missing_block_errors() {
        // No child block at all -- distinct from a missing individual field.
        let kdl = r#"modules { module "button" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button:") && msg.contains("missing required parameters block"),
            "expected 'button: missing required parameters block' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_missing_label_errors() {
        // Child block present but label node absent.
        let kdl = r#"
modules {
    module "button" {
        icon ">"
        on-click "kitty"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.label"),
            "expected 'button.label' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_required_string_child_reports_missing_name() {
        let kdl = r#"modules { module "button" { icon ">" on-click "kitty" } }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.label"),
            "expected 'button.label' in error, got: {msg}"
        );
        assert!(
            msg.contains("missing required parameter"),
            "expected 'missing required parameter' in error, got: {msg}"
        );
        assert!(
            msg.contains("Expected: a string value"),
            "expected 'Expected: a string value' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_missing_icon_errors() {
        // Child block present but icon node absent.
        let kdl = r#"
modules {
    module "button" {
        label "Launch"
        on-click "kitty"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.icon"),
            "expected 'button.icon' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_missing_on_click_errors() {
        let kdl = r#"
modules {
    module "button" {
        label "Launch"
        icon ">"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.on-click"),
            "expected 'button.on-click' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_wrong_type_errors() {
        let kdl = r#"
modules {
    module "button" {
        label 42
        icon ">"
        on-click "kitty"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.label"),
            "expected 'button.label' in error, got: {msg}"
        );
    }
}
