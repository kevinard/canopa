use ::kdl::{KdlDocument, KdlNode, KdlValue};

use crate::config::StatesConfig;

/// Parse an optional `states { state "name" threshold=N ... }` block,
/// merging declared rules into `defaults`. User entries replace built-ins by
/// name; new names are added. Returns `defaults` unchanged when absent.
pub(super) fn parse_optional(
    children: &KdlDocument,
    path: &str,
    defaults: StatesConfig,
) -> anyhow::Result<StatesConfig> {
    let (merged, _) = parse_optional_with_names(children, path, defaults)?;
    Ok(merged)
}

/// Same as [`parse_optional`] but also returns the names supplied by the user.
/// Callers can use the name set to detect overrides without re-walking the KDL.
pub(super) fn parse_optional_with_names(
    children: &KdlDocument,
    path: &str,
    defaults: StatesConfig,
) -> anyhow::Result<(StatesConfig, Vec<String>)> {
    let Some(node) = children.get("states") else {
        return Ok((defaults, Vec::new()));
    };
    let Some(state_children) = node.children() else {
        return Ok((defaults, Vec::new()));
    };

    let mut merged = defaults;
    let mut seen: Vec<String> = Vec::new();

    for (idx, child) in state_children.nodes().iter().enumerate() {
        let rule_path = format!("{path}.states[{idx}]");
        let (name, threshold) = parse_state_node(child, &rule_path)?;
        if seen.iter().any(|n| n == &name) {
            anyhow::bail!("{rule_path}: duplicate state {name:?}.");
        }
        seen.push(name.clone());
        merged.upsert(name, threshold);
    }

    Ok((merged, seen))
}

fn parse_state_node(node: &KdlNode, path: &str) -> anyhow::Result<(String, u8)> {
    if node.name().value() != "state" {
        anyhow::bail!(
            "{path}: expected node name \"state\", got {:?}.",
            node.name().value()
        );
    }

    let positional: Vec<&KdlValue> = node
        .entries()
        .iter()
        .filter(|e| e.name().is_none())
        .map(|e| e.value())
        .collect();

    let name = positional
        .first()
        .and_then(|v| v.as_string())
        .ok_or_else(|| anyhow::anyhow!("{path}: expected a state name string."))?;
    if name.is_empty() {
        anyhow::bail!("{path}: state name must not be empty.");
    }

    let threshold_value = node
        .entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some("threshold"))
        .map(|e| e.value())
        .ok_or_else(|| anyhow::anyhow!("{path}: missing `threshold` property."))?;

    let threshold = threshold_value
        .as_integer()
        .ok_or_else(|| anyhow::anyhow!("{path}: `threshold` must be an integer."))?;
    let threshold = u8::try_from(threshold)
        .ok()
        .filter(|n| *n <= 100)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{path}: `threshold` {threshold} out of range. Expected: integer 0–100."
            )
        })?;

    Ok((name.to_string(), threshold))
}
