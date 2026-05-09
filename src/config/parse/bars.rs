use ::kdl::{KdlDocument, KdlNode};

use super::super::{
    AnimationConfig, BarConfig, BarLayer, BarModules, Edge, ExpandTrigger, OutputTarget,
};
use super::animation;
use super::error::ParseCtx;
use super::style::parse_style_props_ctx;
use crate::config::ConfigResult;
use crate::config::style::Style;

pub(super) fn parse(doc: &KdlDocument, root_ctx: &ParseCtx) -> ConfigResult<Vec<BarConfig>> {
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
        let bar = parse_single_bar_node(node, &name, &bar_ctx)?;
        result.push(bar);
    }
    Ok(result)
}

fn parse_single_bar_node(node: &KdlNode, name: &str, ctx: &ParseCtx) -> ConfigResult<BarConfig> {
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
            "expanded-width",
            "expand-trigger",
            "hover-delay",
            "tooltip-delay",
            "expand",
            "expand-keybind",
            "animation",
            "style",
            "modules",
        ],
        &[
            "output",
            "edge",
            "layer",
            "collapsed-width",
            "expanded-width",
            "expand-trigger",
            "hover-delay",
            "tooltip-delay",
            "expand",
            "expand-keybind",
            "animation",
            "style",
            "modules",
        ],
    )?;

    let defaults = BarConfig::default();

    let output_target = match children.get("output") {
        None => defaults.output_target.clone(),
        Some(output_node) => {
            let output_ctx = ctx.child("output");
            let mut entries = Vec::new();
            for entry in output_node
                .entries()
                .iter()
                .filter(|entry| entry.name().is_none())
            {
                let Some(value) = entry.value().as_string() else {
                    return Err(output_ctx.entry_err(
                        entry,
                        "expected string Output Target entries",
                        None,
                    ));
                };
                entries.push(value.to_string());
            }
            OutputTarget::from_entries(entries, output_ctx.path())
                .map_err(|e| output_ctx.node_err(output_node, e.to_string(), None))?
        }
    };

    let edge = optional_enum_child(
        children,
        "edge",
        defaults.edge,
        &ctx.child("edge"),
        "left, right, top, bottom",
        |value| match value {
            "left" => Some(Edge::Left),
            "right" => Some(Edge::Right),
            "top" => Some(Edge::Top),
            "bottom" => Some(Edge::Bottom),
            _ => None,
        },
    )?;

    let layer = optional_enum_child(
        children,
        "layer",
        defaults.layer,
        &ctx.child("layer"),
        "background, bottom, top, overlay",
        |value| match value {
            "background" => Some(BarLayer::Background),
            "bottom" => Some(BarLayer::Bottom),
            "top" => Some(BarLayer::Top),
            "overlay" => Some(BarLayer::Overlay),
            _ => None,
        },
    )?;

    let collapsed_width = optional_u32_child(
        children,
        "collapsed-width",
        defaults.collapsed_width,
        &ctx.child("collapsed-width"),
        "expected a width in logical pixels, e.g. 48",
    )?;

    let expanded_width = optional_u32_child(
        children,
        "expanded-width",
        defaults.expanded_width,
        &ctx.child("expanded-width"),
        "expected a width in logical pixels, e.g. 200",
    )?;

    let expand_trigger = optional_enum_child(
        children,
        "expand-trigger",
        defaults.expand_trigger,
        &ctx.child("expand-trigger"),
        "click, hover",
        |value| match value {
            "click" => Some(ExpandTrigger::Click),
            "hover" => Some(ExpandTrigger::Hover),
            _ => None,
        },
    )?;

    let hover_delay = optional_u64_child(
        children,
        "hover-delay",
        defaults.hover_delay,
        &ctx.child("hover-delay"),
        "expected a non-negative integer (milliseconds)",
    )?;

    let tooltip_delay_ms = optional_u32_child(
        children,
        "tooltip-delay",
        defaults.tooltip_delay_ms,
        &ctx.child("tooltip-delay"),
        "expected a non-negative integer (milliseconds)",
    )?;

    let expand = optional_bool_child(children, "expand", defaults.expand, &ctx.child("expand"))?;

    let expand_keybind = children
        .get_arg("expand-keybind")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let anim_config = match children.get("animation") {
        None => AnimationConfig::default(),
        Some(anim_node) => animation::parse_node(anim_node, &ctx.child("animation"))?,
    };

    let style = match children.get("style") {
        None => Style::default(),
        Some(style_node) => {
            let style_ctx = ctx.child("style");
            parse_style_node_ctx(style_node, &style_ctx)?
        }
    };

    let modules = parse_bar_modules(children, &ctx.child("modules"))?;

    Ok(BarConfig {
        name: name.to_string(),
        output_target,
        edge,
        layer,
        collapsed_width,
        expanded_width,
        expand,
        expand_trigger,
        hover_delay,
        tooltip_delay_ms,
        expand_keybind,
        animation: anim_config,
        style,
        modules,
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

fn optional_enum_child<T: Clone>(
    children: &KdlDocument,
    name: &str,
    default: T,
    ctx: &ParseCtx,
    expected: &str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> ConfigResult<T> {
    let Some(node) = children.get(name) else {
        return Ok(default);
    };
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(
            node,
            format!("expected a string value. Expected: one of: {expected}"),
            Some(format!("expected one of: {expected}")),
        ));
    };
    let Some(raw) = entry.value().as_string() else {
        return Err(ctx.entry_err(
            entry,
            format!("expected a string value. Expected: one of: {expected}"),
            Some(format!("expected one of: {expected}")),
        ));
    };
    parse(raw).ok_or_else(|| {
        ctx.entry_err(
            entry,
            format!("unknown value {raw:?}. Expected: one of: {expected}"),
            Some(format!("expected one of: {expected}")),
        )
    })
}

fn optional_u32_child(
    children: &KdlDocument,
    name: &str,
    default: u32,
    ctx: &ParseCtx,
    expected: &str,
) -> ConfigResult<u32> {
    let Some(node) = children.get(name) else {
        return Ok(default);
    };
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(
            node,
            format!("expected a positive integer. Expected: {expected}"),
            Some(expected.into()),
        ));
    };
    let Some(value) = entry.value().as_integer() else {
        return Err(ctx.entry_err(
            entry,
            format!("expected a positive integer. Expected: {expected}"),
            Some(expected.into()),
        ));
    };
    u32::try_from(value).map_err(|_| {
        ctx.entry_err(
            entry,
            format!("expected a positive integer. Expected: {expected}"),
            Some(expected.into()),
        )
    })
}

fn optional_u64_child(
    children: &KdlDocument,
    name: &str,
    default: u64,
    ctx: &ParseCtx,
    expected: &str,
) -> ConfigResult<u64> {
    let Some(node) = children.get(name) else {
        return Ok(default);
    };
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(
            node,
            format!("expected a non-negative integer. Expected: {expected}"),
            Some(expected.into()),
        ));
    };
    let Some(value) = entry.value().as_integer() else {
        return Err(ctx.entry_err(
            entry,
            format!("expected a non-negative integer. Expected: {expected}"),
            Some(expected.into()),
        ));
    };
    u64::try_from(value).map_err(|_| {
        ctx.entry_err(
            entry,
            format!("expected a non-negative integer. Expected: {expected}"),
            Some(expected.into()),
        )
    })
}

fn optional_bool_child(
    children: &KdlDocument,
    name: &str,
    default: bool,
    ctx: &ParseCtx,
) -> ConfigResult<bool> {
    let Some(node) = children.get(name) else {
        return Ok(default);
    };
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(
            node,
            "expected a boolean value. Expected: #true or #false",
            Some("expected #true or #false".into()),
        ));
    };
    entry.value().as_bool().ok_or_else(|| {
        ctx.entry_err(
            entry,
            "expected a boolean value. Expected: #true or #false",
            Some("expected #true or #false".into()),
        )
    })
}

fn parse_bar_modules(children: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<BarModules> {
    let Some(modules_node) = children.get("modules") else {
        return Ok(BarModules::default());
    };
    let Some(zone_doc) = modules_node.children() else {
        return Ok(BarModules::default());
    };
    parse_bar_modules_from_doc(zone_doc, ctx)
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

#[cfg(test)]
#[path = "bars_tests.rs"]
mod bars_tests;
