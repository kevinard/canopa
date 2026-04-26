use ::kdl::{KdlDocument, KdlNode};

use crate::config::{StyledSegment, TextTemplate};

/// Parse a `format`-family node, supporting both bare positional strings and
/// child `segment` nodes. Style props are not yet handled (Task 6).
pub(super) fn parse_segments(
    node: &KdlNode,
    allowed_placeholders: &[&str],
    _allowed_states: &[&str],
    path: &str,
) -> anyhow::Result<Vec<StyledSegment>> {
    let mut out = Vec::new();

    // Positional string args become unstyled segments.
    for entry in node.entries() {
        if entry.name().is_some() {
            anyhow::bail!("{path}: unexpected named property on format node.");
        }
        let raw = entry
            .value()
            .as_string()
            .ok_or_else(|| anyhow::anyhow!("{path}: expected string value."))?;
        out.push(StyledSegment::new(TextTemplate::parse(
            raw,
            allowed_placeholders,
            path,
        )?));
    }

    // Child `segment` nodes appended in order.
    if let Some(children) = node.children() {
        for (i, child) in children.nodes().iter().enumerate() {
            if child.name().value() != "segment" {
                anyhow::bail!(
                    "{path}: unexpected child node {:?}; expected `segment`.",
                    child.name().value()
                );
            }
            let seg_path = format!("{path}.segment[{i}]");
            let raw = child
                .entries()
                .iter()
                .find(|e| e.name().is_none())
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| {
                    anyhow::anyhow!("{seg_path}: expected one positional string.")
                })?;
            out.push(StyledSegment::new(TextTemplate::parse(
                raw,
                allowed_placeholders,
                &seg_path,
            )?));
            for e in child.entries() {
                if let Some(name) = e.name() {
                    anyhow::bail!(
                        "{seg_path}: unknown property {:?} (styling not yet supported).",
                        name.value()
                    );
                }
            }
            if child.children().is_some() {
                anyhow::bail!("{seg_path}: child nodes not yet supported.");
            }
        }
    }

    if out.is_empty() {
        anyhow::bail!("{path}: expected at least one segment.");
    }
    Ok(out)
}

// Suppress unused-imports lint until Task 6 brings this in via additional types.
#[allow(dead_code)]
fn _ensure_imports(_: &KdlDocument) {}
