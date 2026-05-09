use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::style::parse_style_props_ctx;
use crate::config::ConfigResult;
use crate::config::style::{SegmentStyles, Style};
use crate::config::{StyledSegment, TextTemplate};

pub(super) fn parse_segments_ctx(
    node: &KdlNode,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
    ctx: &ParseCtx,
) -> ConfigResult<Vec<StyledSegment>> {
    let mut out = Vec::new();
    let has_positional = node.entries().iter().any(|e| e.name().is_none());
    let has_segment_children = node
        .children()
        .map(|c| c.nodes().iter().any(|n| n.name().value() == "segment"))
        .unwrap_or(false);

    if has_positional && has_segment_children {
        return Err(ctx.node_err(
            node,
            "positional segment strings and `segment {}` children are mutually exclusive",
            None,
        ));
    }

    for entry in node.entries() {
        if entry.name().is_some() {
            return Err(ctx.entry_err(entry, "unexpected named property on format node", None));
        }
        let Some(raw) = entry.value().as_string() else {
            return Err(ctx.entry_err(entry, "expected string value", None));
        };
        out.push(StyledSegment::new(parse_template_ctx(
            raw,
            allowed_placeholders,
            ctx,
            Some(entry.span()),
        )?));
    }

    if let Some(children) = node.children() {
        for (i, child) in children.nodes().iter().enumerate() {
            if child.name().value() != "segment" {
                return Err(ctx.node_err(
                    child,
                    format!(
                        "unexpected child node {:?}; expected `segment`",
                        child.name().value()
                    ),
                    None,
                ));
            }
            let seg_ctx = ctx.child("segment").index(i);
            out.push(parse_segment_ctx(
                child,
                allowed_placeholders,
                allowed_states,
                &seg_ctx,
            )?);
        }
    }

    if out.is_empty() {
        return Err(ctx.node_err(node, "expected at least one segment", None));
    }
    Ok(out)
}

fn parse_segment_ctx(
    node: &KdlNode,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
    ctx: &ParseCtx,
) -> ConfigResult<StyledSegment> {
    let Some(entry) = node.entries().iter().find(|e| e.name().is_none()) else {
        return Err(ctx.node_err(node, "expected one positional string", None));
    };
    let Some(raw) = entry.value().as_string() else {
        return Err(ctx.entry_err(entry, "expected one positional string", None));
    };
    let template = parse_template_ctx(raw, allowed_placeholders, ctx, Some(entry.span()))?;

    let inline = parse_inline_style_props_ctx(node, ctx)?;
    let has_inline = !inline.is_empty();
    let has_children = node.children().is_some();

    if has_inline && has_children {
        return Err(ctx.node_err(
            node,
            "inline style props and `style` children are mutually exclusive",
            None,
        ));
    }

    let mut segment = StyledSegment::new(template);
    if has_inline {
        segment.set_styles(SegmentStyles::Static(inline));
    } else if has_children {
        segment.set_styles(parse_style_children_ctx(node, allowed_states, ctx)?);
    }
    Ok(segment)
}

fn parse_template_ctx(
    raw: &str,
    allowed_placeholders: &[&str],
    ctx: &ParseCtx,
    span: Option<miette::SourceSpan>,
) -> ConfigResult<TextTemplate> {
    TextTemplate::parse(raw, allowed_placeholders, ctx.path()).map_err(|error| {
        let mut message = error.to_string();
        let prefix = format!("{}: ", ctx.path());
        if let Some(stripped) = message.strip_prefix(&prefix) {
            message = stripped.to_string();
        }
        let help = if message.contains("unknown placeholder") {
            Some(format!(
                "expected one of: {}",
                allowed_placeholders.join(", ")
            ))
        } else {
            None
        };
        ctx.err(span, message, help)
    })
}

/// Parse style props directly on a `segment` node. The positional template arg
/// is ignored. `state` is rejected here (only valid on `style` children).
fn parse_inline_style_props_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Style> {
    let (style, _) = parse_style_props_ctx(node, ctx, false)?;
    Ok(style)
}

fn parse_style_children_ctx(
    parent: &KdlNode,
    allowed_states: &[&str],
    ctx: &ParseCtx,
) -> ConfigResult<SegmentStyles> {
    let children = parent.children().expect("checked by caller");
    let mut default: Option<Style> = None;
    let mut by_state: Vec<(String, Style)> = Vec::new();

    for (i, child) in children.nodes().iter().enumerate() {
        if child.name().value() != "style" {
            return Err(ctx.node_err(
                child,
                format!(
                    "unexpected child {:?}; expected `style`",
                    child.name().value()
                ),
                None,
            ));
        }
        let style_ctx = ctx.child("style").index(i);
        let (style, state) = parse_one_style_ctx(child, &style_ctx)?;

        match state {
            None => {
                if default.is_some() {
                    return Err(style_ctx.node_err(
                        child,
                        "duplicate default style for segment",
                        None,
                    ));
                }
                default = Some(style);
            }
            Some(state) => {
                if allowed_states.is_empty() {
                    return Err(style_ctx.node_err(
                        child,
                        "this module does not support `state` selectors",
                        None,
                    ));
                }
                if !allowed_states.contains(&state.as_str()) {
                    return Err(style_ctx.child("state").node_err(
                        child,
                        format!("unknown state {state:?}"),
                        Some(format!("expected one of: {}", allowed_states.join(", "))),
                    ));
                }
                if by_state.iter().any(|(k, _)| k == &state) {
                    return Err(style_ctx.child("state").node_err(
                        child,
                        format!("duplicate state {state:?} for segment"),
                        None,
                    ));
                }
                by_state.push((state, style));
            }
        }
    }

    Ok(SegmentStyles::Stateful { default, by_state })
}

fn parse_one_style_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<(Style, Option<String>)> {
    if node.entries().iter().any(|e| e.name().is_none()) {
        return Err(ctx.node_err(node, "unexpected positional argument on `style`", None));
    }
    parse_style_props_ctx(node, ctx, true)
}
