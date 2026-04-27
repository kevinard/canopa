use ::kdl::KdlDocument;

use crate::config::{FormatConfig, FormatTemplate};

pub(super) fn parse_optional(
    children: &KdlDocument,
    path: &str,
    allowed: &[&str],
    allowed_states: &[&str],
) -> anyhow::Result<FormatConfig> {
    Ok(FormatConfig {
        collapsed: parse_field(
            children,
            "format",
            &format!("{path}.format"),
            allowed,
            allowed_states,
        )?,
        expanded: parse_field(
            children,
            "expanded-format",
            &format!("{path}.expanded-format"),
            allowed,
            allowed_states,
        )?,
        horizontal: parse_field(
            children,
            "horizontal-format",
            &format!("{path}.horizontal-format"),
            allowed,
            allowed_states,
        )?,
    })
}

/// Error if any of the format fields (`format`, `expanded-format`,
/// `horizontal-format`) are present. Used by modules that do not render
/// templated text — e.g. tray, which composes its own per-icon view.
pub(super) fn reject_format_fields(
    children: Option<&KdlDocument>,
    path: &str,
    module_label: &str,
) -> anyhow::Result<()> {
    let Some(children) = children else {
        return Ok(());
    };
    for field in ["format", "expanded-format", "horizontal-format"] {
        if children.get(field).is_some() {
            anyhow::bail!("{path}.{field}: {module_label} does not support format templates.");
        }
    }
    Ok(())
}

fn parse_field(
    children: &KdlDocument,
    field: &str,
    path: &str,
    allowed: &[&str],
    allowed_states: &[&str],
) -> anyhow::Result<Option<FormatTemplate>> {
    let Some(node) = children.get(field) else {
        return Ok(None);
    };
    let segments = super::segment_styles::parse_segments(node, allowed, allowed_states, path)?;
    Ok(Some(FormatTemplate::from_styled(segments)))
}

use crate::config::TooltipSetting;

/// Parse the optional `tooltip` field on a module's child block.
///
/// Accepts:
/// - absence => `TooltipSetting::Default`
/// - `tooltip "..."` (positional string, optionally multiple segments) => `Custom`
/// - `tooltip { segment ... }` (styled-segment block) => `Custom`
/// - `tooltip #false` => `Disabled`
/// - `tooltip #true` => error
pub(super) fn parse_tooltip_field(
    children: &KdlDocument,
    module_path: &str,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
) -> anyhow::Result<TooltipSetting> {
    let Some(node) = children.get("tooltip") else {
        return Ok(TooltipSetting::Default);
    };
    let path = format!("{module_path}.tooltip");

    // Single bool entry (no children, one positional bool) => enabled/disabled flag.
    let positional: Vec<_> = node.entries().iter().filter(|e| e.name().is_none()).collect();
    if node.children().is_none() && positional.len() == 1 {
        if let Some(b) = positional[0].value().as_bool() {
            if b {
                anyhow::bail!(
                    "{path}: tooltip true is not allowed; omit the field to use the default."
                );
            }
            return Ok(TooltipSetting::Disabled);
        }
    }

    // Otherwise: delegate to the shared styled-segment parser.
    let segments =
        super::segment_styles::parse_segments(node, allowed_placeholders, allowed_states, &path)?;
    Ok(TooltipSetting::Custom(crate::config::FormatTemplate::from_styled(segments)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::kdl::KdlDocument;

    fn parse(src: &str) -> KdlDocument {
        src.parse().expect("kdl parse")
    }

    fn allowed() -> &'static [&'static str] {
        &["icon", "value", "state"]
    }

    #[test]
    fn tooltip_absent_returns_default_setting() {
        let doc = parse("");
        let s = parse_tooltip_field(&doc, "m", allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Default));
    }

    #[test]
    fn tooltip_string_returns_custom() {
        let doc = parse(r#"tooltip "{value}%""#);
        let s = parse_tooltip_field(&doc, "m", allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_returns_disabled() {
        let doc = parse("tooltip #false");
        let s = parse_tooltip_field(&doc, "m", allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let doc = parse("tooltip #true");
        let err = parse_tooltip_field(&doc, "m", allowed(), &[])
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("tooltip true is not allowed"),
            "got: {err}"
        );
    }

    #[test]
    fn tooltip_block_returns_custom() {
        let doc = parse(
            r#"tooltip {
                segment "{icon}"
                segment "{value}"
            }"#,
        );
        let s = parse_tooltip_field(&doc, "m", allowed(), &[]).unwrap();
        match s {
            crate::config::TooltipSetting::Custom(t) => {
                assert_eq!(t.segments().len(), 2);
            }
            _ => panic!("expected Custom"),
        }
    }
}
