use ::kdl::{KdlDocument, KdlNode};

use super::super::{
    BarConfig, BarLayer, BarModules, Edge, OutputTarget, PanelAnimationConfig, PanelConfig,
    PanelModules, PanelPresentation,
};
use super::actions;
use super::error::ParseCtx;
use super::fields;
use super::style::parse_style_props_ctx;
use crate::config::style::Style;
use crate::config::{CommandPolicy, ConfigResult};

pub(super) fn parse(
    doc: &KdlDocument,
    root_ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<BarConfig>> {
    let mut result = Vec::new();
    let mut bar_idx = 0;
    for node in doc.nodes() {
        if node.name().value() != "bar" {
            continue;
        }
        let bar_ctx = root_ctx.child("bar").index(bar_idx);
        bar_idx += 1;
        let name = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                bar_ctx.node_err(
                    node,
                    "bar: missing name argument. \
                     Expected: bar \"<name>\" { ... }, e.g. bar \"main\" { ... }",
                    None,
                )
            })?
            .to_string();
        let bar = parse_single_bar_node(node, &name, &bar_ctx, command_policy)?;
        result.push(bar);
    }
    Ok(result)
}

fn parse_single_bar_node(
    node: &KdlNode,
    name: &str,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<BarConfig> {
    let children = match node.children() {
        Some(c) => c,
        None => {
            return Ok(BarConfig {
                name: name.to_string(),
                ..BarConfig::default()
            });
        }
    };

    ctx.validate_child_schema(
        children,
        &[
            "output",
            "edge",
            "layer",
            "collapsed-width",
            "tooltip-delay",
            "style",
            "modules",
            "actions",
            "panel",
        ],
        &[
            "output",
            "edge",
            "layer",
            "collapsed-width",
            "tooltip-delay",
            "style",
            "modules",
            "actions",
            "panel",
        ],
    )?;

    let defaults = BarConfig::default();

    let output_ctx = ctx.child("output");
    let output_target = match fields::field(children, ctx, "output")
        .optional()
        .positional_strings("expected string Output Target entries")
    {
        fields::FieldValue::Missing => defaults.output_target.clone(),
        fields::FieldValue::Present(entries) => {
            let entries = entries?;
            let output_node =
                fields::field(children, ctx, "output").required_node("missing output", None)?;
            OutputTarget::from_entries(entries, output_ctx.path())
                .map_err(|e| output_ctx.node_err(output_node, e.to_string(), None))?
        }
    };

    let edge = fields::field(children, ctx, "edge")
        .optional()
        .one_of("left, right, top, bottom", |value| match value {
            "left" => Some(Edge::Left),
            "right" => Some(Edge::Right),
            "top" => Some(Edge::Top),
            "bottom" => Some(Edge::Bottom),
            _ => None,
        })
        .unwrap_or(defaults.edge)?;

    let layer = fields::field(children, ctx, "layer")
        .optional()
        .one_of("background, bottom, top, overlay", |value| match value {
            "background" => Some(BarLayer::Background),
            "bottom" => Some(BarLayer::Bottom),
            "top" => Some(BarLayer::Top),
            "overlay" => Some(BarLayer::Overlay),
            _ => None,
        })
        .unwrap_or(defaults.layer)?;

    let collapsed_width = fields::field(children, ctx, "collapsed-width")
        .optional()
        .help("expected a width in logical pixels, e.g. 48")
        .integer_message(
            "expected a positive integer. Expected: expected a width in logical pixels, e.g. 48",
        )
        .u32()
        .unwrap_or(defaults.collapsed_width)?;

    let tooltip_delay_ms = fields::field(children, ctx, "tooltip-delay")
        .optional()
        .help("expected a non-negative integer (milliseconds)")
        .integer_message(
            "expected a positive integer. Expected: expected a non-negative integer (milliseconds)",
        )
        .u32()
        .unwrap_or(defaults.tooltip_delay_ms)?;

    let style = match fields::field(children, ctx, "style").optional_node()? {
        None => Style::default(),
        Some(style_node) => {
            let style_ctx = ctx.child("style");
            parse_style_node_ctx(style_node, &style_ctx)?
        }
    };

    let modules = parse_bar_modules(children, &ctx.child("modules"))?;
    let actions = actions::parse(node, 0, "bar", ctx, command_policy)?;
    let panel = parse_panel(children, &ctx.child("panel"))?;

    Ok(BarConfig {
        name: name.to_string(),
        output_target,
        edge,
        layer,
        collapsed_width,
        tooltip_delay_ms,
        style,
        modules,
        actions,
        panel,
    })
}

fn parse_style_node_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Style> {
    if node.children().is_some() {
        return Err(ctx.node_err(node, "style does not support child nodes", None));
    }
    if node.entries().iter().any(|e| e.name().is_none()) {
        return Err(ctx.node_err(node, "unexpected positional argument on `style`", None));
    }
    let (style, _) = parse_style_props_ctx(node, ctx, false)?;
    Ok(style)
}

fn parse_bar_modules(children: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<BarModules> {
    let Some(modules_node) = fields::field(children, ctx, "modules").optional_node()? else {
        return Ok(BarModules::default());
    };
    let Some(zone_doc) = modules_node.children() else {
        return Ok(BarModules::default());
    };
    parse_bar_modules_from_doc(zone_doc, ctx)
}

fn parse_panel(children: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<PanelConfig> {
    let Some(panel_node) = fields::field(children, ctx, "panel").optional_node()? else {
        return Ok(PanelConfig::default());
    };
    let Some(panel_children) = panel_node.children() else {
        return Ok(PanelConfig::default());
    };

    ctx.validate_child_schema(
        panel_children,
        &[
            "presentation",
            "width",
            "max-width",
            "height",
            "max-height",
            "start",
            "center",
            "end",
            "animation",
        ],
        &[
            "presentation",
            "width",
            "max-width",
            "height",
            "max-height",
            "start",
            "center",
            "end",
            "animation",
        ],
    )?;

    let defaults = PanelConfig::default();
    let presentation = fields::field(panel_children, ctx, "presentation")
        .optional()
        .one_of("auto, drawer", |value| match value {
            "auto" => Some(PanelPresentation::Auto),
            "drawer" => Some(PanelPresentation::Drawer),
            _ => None,
        })
        .unwrap_or(defaults.presentation)?;
    let width = fields::field(panel_children, ctx, "width")
        .optional()
        .help("expected a width in logical pixels, e.g. 360")
        .integer_message(
            "expected a positive integer. Expected: expected a width in logical pixels, e.g. 360",
        )
        .u32()
        .transpose()?
        .or(defaults.width);
    let max_width = fields::field(panel_children, ctx, "max-width")
        .optional()
        .help("expected a max width in logical pixels")
        .u32()
        .transpose()?
        .or(defaults.max_width);
    let height = fields::field(panel_children, ctx, "height")
        .optional()
        .help("expected a height in logical pixels")
        .u32()
        .transpose()?
        .or(defaults.height);
    let max_height = fields::field(panel_children, ctx, "max-height")
        .optional()
        .help("expected a max height in logical pixels")
        .u32()
        .transpose()?
        .or(defaults.max_height);
    let modules = parse_panel_modules_from_doc(panel_children, ctx)?;
    let animation = match fields::field(panel_children, ctx, "animation").optional_node()? {
        None => PanelAnimationConfig::default(),
        Some(animation_node) => parse_panel_animation(animation_node, &ctx.child("animation"))?,
    };

    Ok(PanelConfig {
        presentation,
        width,
        max_width,
        height,
        max_height,
        modules,
        animation,
    })
}

/// Parse `start`, `center`, `end` zone nodes from a `KdlDocument`.
fn parse_bar_modules_from_doc(zone_doc: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<BarModules> {
    let mut result = BarModules::default();
    for zone_node in zone_doc.nodes() {
        let zone_name = zone_node.name().value();
        let names: Vec<String> = zone_node
            .entries()
            .iter()
            .filter(|e| e.name().is_none())
            .filter_map(|e| e.value().as_string())
            .map(|s| s.to_string())
            .collect();
        match zone_name {
            "start" => result.start = names,
            "center" => result.center = names,
            "end" => result.end = names,
            other => {
                return Err(ctx.child(other).node_err(
                    zone_node,
                    format!("unknown zone {other:?}. Expected: one of: start, center, end"),
                    Some("expected one of: start, center, end".into()),
                ));
            }
        }
    }
    Ok(result)
}

fn parse_panel_modules_from_doc(
    zone_doc: &KdlDocument,
    _ctx: &ParseCtx,
) -> ConfigResult<PanelModules> {
    let mut result = PanelModules::default();
    for zone_node in zone_doc.nodes() {
        let zone_name = zone_node.name().value();
        let names: Vec<String> = zone_node
            .entries()
            .iter()
            .filter(|entry| entry.name().is_none())
            .filter_map(|entry| entry.value().as_string())
            .map(ToString::to_string)
            .collect();
        match zone_name {
            "start" => result.start = names,
            "center" => result.center = names,
            "end" => result.end = names,
            _ => {}
        }
    }
    Ok(result)
}

fn parse_panel_animation(
    animation_node: &KdlNode,
    ctx: &ParseCtx,
) -> ConfigResult<PanelAnimationConfig> {
    let Some(animation_children) = animation_node.children() else {
        return Ok(PanelAnimationConfig::default());
    };
    ctx.validate_child_schema(
        animation_children,
        &["open-ms", "close-ms"],
        &["open-ms", "close-ms"],
    )?;
    let defaults = PanelAnimationConfig::default();
    let open_ms = fields::field(animation_children, ctx, "open-ms")
        .optional()
        .help("expected a duration in milliseconds, e.g. 180")
        .integer_message(
            "expected a positive integer. Expected: a duration in milliseconds, e.g. 180",
        )
        .u32()
        .map(|value| value as f32)
        .unwrap_or(defaults.open_ms)?;
    let close_ms = fields::field(animation_children, ctx, "close-ms")
        .optional()
        .help("expected a duration in milliseconds, e.g. 120")
        .integer_message(
            "expected a positive integer. Expected: a duration in milliseconds, e.g. 120",
        )
        .u32()
        .map(|value| value as f32)
        .unwrap_or(defaults.close_ms)?;

    Ok(PanelAnimationConfig { open_ms, close_ms })
}

#[cfg(test)]
#[path = "bars_tests.rs"]
mod bars_tests;
