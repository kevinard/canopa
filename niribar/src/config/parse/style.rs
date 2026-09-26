use ::kdl::KdlNode;

use super::error::ParseCtx;
use crate::config::ConfigResult;
use crate::config::style::{ColorValue, Style, Weight};

pub(super) fn parse_style_props_ctx(
    node: &KdlNode,
    ctx: &ParseCtx,
    allow_state: bool,
) -> ConfigResult<(Style, Option<String>)> {
    let mut style = Style::default();
    let mut state: Option<String> = None;
    for entry in node.entries() {
        let Some(name) = entry.name() else { continue };
        match name.value() {
            "color" => {
                let field_ctx = ctx.child("color");
                let Some(raw) = entry.value().as_string() else {
                    return Err(field_ctx.entry_err(entry, "expected string", None));
                };
                style.color = Some(
                    ColorValue::parse(raw)
                        .map_err(|e| field_ctx.entry_err(entry, e.to_string(), None))?,
                );
            }
            "size" => {
                let field_ctx = ctx.child("size");
                let Some(f) = entry
                    .value()
                    .as_float()
                    .or_else(|| entry.value().as_integer().map(|i| i as f64))
                else {
                    return Err(field_ctx.entry_err(entry, "expected number", None));
                };
                if f <= 0.0 {
                    return Err(field_ctx.entry_err(
                        entry,
                        format!("must be positive, got {f}"),
                        None,
                    ));
                }
                style.size = Some(f as f32);
            }
            "weight" => {
                let field_ctx = ctx.child("weight");
                let Some(raw) = entry.value().as_string() else {
                    return Err(field_ctx.entry_err(entry, "expected string", None));
                };
                style.weight = Some(
                    Weight::parse(raw)
                        .map_err(|e| field_ctx.entry_err(entry, e.to_string(), None))?,
                );
            }
            "state" if allow_state => {
                let field_ctx = ctx.child("state");
                let Some(raw) = entry.value().as_string() else {
                    return Err(field_ctx.entry_err(entry, "expected string", None));
                };
                state = Some(raw.to_string());
            }
            "state" => {
                return Err(ctx
                    .child("state")
                    .entry_err(entry, "`state` is not valid here", None));
            }
            other => {
                return Err(ctx.child(other).entry_err(
                    entry,
                    format!("unknown property {other:?}"),
                    Some("expected one of: color, size, weight".into()),
                ));
            }
        }
    }
    Ok((style, state))
}
