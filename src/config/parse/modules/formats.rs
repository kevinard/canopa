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

pub(super) fn reject_if_present(children: Option<&KdlDocument>, path: &str) -> anyhow::Result<()> {
    let Some(children) = children else {
        return Ok(());
    };
    for field in ["format", "expanded-format", "horizontal-format"] {
        if children.get(field).is_some() {
            anyhow::bail!("{path}.{field}: tray does not support format templates.");
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
    let segments =
        super::segment_styles::parse_segments(node, allowed, allowed_states, path)?;
    Ok(Some(FormatTemplate::from_styled(segments)))
}
