use ::kdl::KdlDocument;

use super::super::kdl;
use crate::config::{FormatConfig, TextTemplate};

pub(super) fn parse_optional(
    children: &KdlDocument,
    path: &str,
    allowed: &[&str],
) -> anyhow::Result<FormatConfig> {
    Ok(FormatConfig {
        collapsed: parse_field(children, "format", &format!("{path}.format"), allowed)?,
        expanded: parse_field(
            children,
            "expanded-format",
            &format!("{path}.expanded-format"),
            allowed,
        )?,
        horizontal: parse_field(
            children,
            "horizontal-format",
            &format!("{path}.horizontal-format"),
            allowed,
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
) -> anyhow::Result<Option<TextTemplate>> {
    let Some(node) = children.get(field) else {
        return Ok(None);
    };
    let value = kdl::required_string_child(children, field, path)?;
    Ok(Some(
        TextTemplate::parse(&value, allowed, path).map_err(|err| anyhow::anyhow!("{err}"))?,
    ))
}
