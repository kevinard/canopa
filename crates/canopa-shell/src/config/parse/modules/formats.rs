use ::kdl::KdlDocument;

use super::super::error::ParseCtx;
use super::super::fields;
use crate::config::{ConfigResult, ContentRotation, FormatConfig, FormatTemplate};

pub(super) fn parse_format_and_tooltip_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
    placeholders: &[&str],
    allowed_states: &[&str],
) -> ConfigResult<(FormatConfig, TooltipSetting)> {
    Ok((
        parse_optional_ctx(children, module_ctx, placeholders, allowed_states)?,
        parse_tooltip_field_ctx(children, module_ctx, placeholders, allowed_states)?,
    ))
}

pub(super) fn parse_content_rotation_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
) -> ConfigResult<ContentRotation> {
    parse_content_rotation_with_default_ctx(children, module_ctx, ContentRotation::None)
}

pub(super) fn parse_content_rotation_with_default_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
    default: ContentRotation,
) -> ConfigResult<ContentRotation> {
    let expected = ContentRotation::ACCEPTED_VALUES.join(", ");
    fields::field(children, module_ctx, "content-rotation")
        .optional()
        .one_of(&expected, ContentRotation::parse)
        .unwrap_or(default)
}

fn parse_optional_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
    allowed: &[&str],
    allowed_states: &[&str],
) -> ConfigResult<FormatConfig> {
    Ok(FormatConfig {
        collapsed: parse_field_ctx(children, "format", module_ctx, allowed, allowed_states)?,
        expanded: parse_field_ctx(
            children,
            "expanded-format",
            module_ctx,
            allowed,
            allowed_states,
        )?,
        horizontal: parse_field_ctx(
            children,
            "horizontal-format",
            module_ctx,
            allowed,
            allowed_states,
        )?,
    })
}

pub(super) fn reject_format_fields_ctx(
    children: Option<&KdlDocument>,
    module_ctx: &ParseCtx,
    module_label: &str,
) -> ConfigResult<()> {
    let Some(children) = children else {
        return Ok(());
    };
    for field in ["format", "expanded-format", "horizontal-format"] {
        if let Some(node) = fields::field(children, module_ctx, field).optional_node()? {
            return Err(module_ctx.child(field).node_err(
                node,
                format!("{module_label} does not support format templates"),
                Some(format!("remove `{field}` from this module")),
            ));
        }
    }
    Ok(())
}

fn parse_field_ctx(
    children: &KdlDocument,
    field: &str,
    module_ctx: &ParseCtx,
    allowed: &[&str],
    allowed_states: &[&str],
) -> ConfigResult<Option<FormatTemplate>> {
    let Some(node) = fields::field(children, module_ctx, field).optional_node()? else {
        return Ok(None);
    };
    let ctx = module_ctx.child(field);
    let segments = super::segment_styles::parse_segments_ctx(node, allowed, allowed_states, &ctx)?;
    Ok(Some(FormatTemplate::from_styled(segments)))
}

use crate::config::TooltipSetting;

pub(super) fn parse_tooltip_field_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
) -> ConfigResult<TooltipSetting> {
    let Some(node) = fields::field(children, module_ctx, "tooltip").optional_node()? else {
        return Ok(TooltipSetting::Default);
    };
    let ctx = module_ctx.child("tooltip");

    let positional: Vec<_> = node
        .entries()
        .iter()
        .filter(|e| e.name().is_none())
        .collect();
    if node.children().is_none()
        && positional.len() == 1
        && let Some(b) = positional[0].value().as_bool()
    {
        if b {
            return Err(ctx.entry_err(
                positional[0],
                "tooltip true is not allowed; omit the field to use the default",
                None,
            ));
        }
        return Ok(TooltipSetting::Disabled);
    }

    let segments = super::segment_styles::parse_segments_ctx(
        node,
        allowed_placeholders,
        allowed_states,
        &ctx,
    )?;
    Ok(TooltipSetting::Custom(
        crate::config::FormatTemplate::from_styled(segments),
    ))
}

#[cfg(test)]
#[path = "formats_tests.rs"]
mod formats_tests;
