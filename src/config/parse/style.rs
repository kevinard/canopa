use ::kdl::KdlNode;

use crate::config::style::{ColorValue, Style, Weight};

/// Parse `color`, `size`, `weight`, and (optionally) `state` named props on a node.
/// Positional entries are ignored — callers strip or validate them as needed.
/// `state` is only consumed when `allow_state` is true; otherwise it errors.
pub(super) fn parse_style_props(
    node: &KdlNode,
    path: &str,
    allow_state: bool,
) -> anyhow::Result<(Style, Option<String>)> {
    let mut style = Style::default();
    let mut state: Option<String> = None;
    for entry in node.entries() {
        let Some(name) = entry.name() else { continue };
        match name.value() {
            "color" => {
                let raw = entry
                    .value()
                    .as_string()
                    .ok_or_else(|| anyhow::anyhow!("{path}.color: expected string."))?;
                style.color =
                    Some(ColorValue::parse(raw).map_err(|e| anyhow::anyhow!("{path}.color: {e}"))?);
            }
            "size" => {
                let f = entry
                    .value()
                    .as_float()
                    .or_else(|| entry.value().as_integer().map(|i| i as f64))
                    .ok_or_else(|| anyhow::anyhow!("{path}.size: expected number."))?;
                if f <= 0.0 {
                    anyhow::bail!("{path}.size: must be positive, got {f}.");
                }
                style.size = Some(f as f32);
            }
            "weight" => {
                let raw = entry
                    .value()
                    .as_string()
                    .ok_or_else(|| anyhow::anyhow!("{path}.weight: expected string."))?;
                style.weight =
                    Some(Weight::parse(raw).map_err(|e| anyhow::anyhow!("{path}.weight: {e}"))?);
            }
            "state" if allow_state => {
                let raw = entry
                    .value()
                    .as_string()
                    .ok_or_else(|| anyhow::anyhow!("{path}.state: expected string."))?;
                state = Some(raw.to_string());
            }
            "state" => {
                anyhow::bail!("{path}: `state` is not valid here.");
            }
            other => {
                anyhow::bail!("{path}: unknown property {other:?}.");
            }
        }
    }
    Ok((style, state))
}
