use ::kdl::KdlDocument;

use crate::config::{FormatConfig, FormatTemplate};

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
) -> anyhow::Result<Option<FormatTemplate>> {
    let Some(node) = children.get(field) else {
        return Ok(None);
    };
    let values: Vec<&str> = node
        .entries()
        .iter()
        .filter(|entry| entry.name().is_none())
        .map(|entry| entry.value().as_string())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| anyhow::anyhow!("{path}: expected string value(s)."))?;
    Ok(Some(FormatTemplate::parse(&values, allowed, path)?))
}
