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
