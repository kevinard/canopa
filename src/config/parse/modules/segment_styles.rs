use ::kdl::KdlNode;

use crate::config::style::{ColorValue, SegmentStyles, Style, Weight};
use crate::config::{StyledSegment, TextTemplate};

pub(super) fn parse_segments(
    node: &KdlNode,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
    path: &str,
) -> anyhow::Result<Vec<StyledSegment>> {
    let mut out = Vec::new();

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

    if let Some(children) = node.children() {
        for (i, child) in children.nodes().iter().enumerate() {
            if child.name().value() != "segment" {
                anyhow::bail!(
                    "{path}: unexpected child node {:?}; expected `segment`.",
                    child.name().value()
                );
            }
            let seg_path = format!("{path}.segment[{i}]");
            out.push(parse_segment(
                child,
                allowed_placeholders,
                allowed_states,
                &seg_path,
            )?);
        }
    }

    if out.is_empty() {
        anyhow::bail!("{path}: expected at least one segment.");
    }
    Ok(out)
}

fn parse_segment(
    node: &KdlNode,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
    path: &str,
) -> anyhow::Result<StyledSegment> {
    let raw = node
        .entries()
        .iter()
        .find(|e| e.name().is_none())
        .and_then(|e| e.value().as_string())
        .ok_or_else(|| anyhow::anyhow!("{path}: expected one positional string."))?;
    let template = TextTemplate::parse(raw, allowed_placeholders, path)?;

    let inline = parse_inline_style_props(node, path)?;
    let has_inline = !inline.is_empty();
    let has_children = node.children().is_some();

    if has_inline && has_children {
        anyhow::bail!("{path}: inline style props and `style` children are mutually exclusive.");
    }

    let mut segment = StyledSegment::new(template);
    if has_inline {
        segment.set_styles(SegmentStyles::Static(inline));
    } else if has_children {
        segment.set_styles(parse_style_children(node, allowed_states, path)?);
    }
    Ok(segment)
}

/// Parse style props directly on a `segment` node. The positional template arg
/// is ignored. `state` is rejected here (only valid on `style` children).
fn parse_inline_style_props(node: &KdlNode, path: &str) -> anyhow::Result<Style> {
    let mut style = Style::default();
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
            "state" => {
                anyhow::bail!("{path}: `state` is only valid on `style` children, not inline.");
            }
            other => {
                anyhow::bail!("{path}: unknown property {other:?}.");
            }
        }
    }
    Ok(style)
}

fn parse_style_children(
    parent: &KdlNode,
    allowed_states: &[&str],
    path: &str,
) -> anyhow::Result<SegmentStyles> {
    let children = parent.children().expect("checked by caller");
    let mut default: Option<Style> = None;
    let mut by_state: Vec<(String, Style)> = Vec::new();

    for (i, child) in children.nodes().iter().enumerate() {
        if child.name().value() != "style" {
            anyhow::bail!(
                "{path}: unexpected child {:?}; expected `style`.",
                child.name().value()
            );
        }
        let style_path = format!("{path}.style[{i}]");
        let (style, state) = parse_one_style(child, &style_path)?;

        match state {
            None => {
                if default.is_some() {
                    anyhow::bail!("{style_path}: duplicate default style for segment.");
                }
                default = Some(style);
            }
            Some(state) => {
                if allowed_states.is_empty() {
                    anyhow::bail!("{style_path}: this module does not support `state` selectors.");
                }
                if !allowed_states.contains(&state.as_str()) {
                    anyhow::bail!(
                        "{style_path}: unknown state {state:?}; expected one of {:?}.",
                        allowed_states
                    );
                }
                if by_state.iter().any(|(k, _)| k == &state) {
                    anyhow::bail!("{style_path}: duplicate state {state:?} for segment.");
                }
                by_state.push((state, style));
            }
        }
    }

    Ok(SegmentStyles::Stateful { default, by_state })
}

fn parse_one_style(node: &KdlNode, path: &str) -> anyhow::Result<(Style, Option<String>)> {
    let mut style = Style::default();
    let mut state: Option<String> = None;
    for entry in node.entries() {
        let Some(name) = entry.name() else {
            anyhow::bail!("{path}: unexpected positional argument on `style`.");
        };
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
            "state" => {
                let raw = entry
                    .value()
                    .as_string()
                    .ok_or_else(|| anyhow::anyhow!("{path}.state: expected string."))?;
                state = Some(raw.to_string());
            }
            other => {
                anyhow::bail!("{path}: unknown property {other:?}.");
            }
        }
    }
    Ok((style, state))
}
