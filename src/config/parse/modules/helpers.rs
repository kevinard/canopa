use super::super::error::ParseCtx;
use crate::config::ConfigResult;

pub(super) fn optional_u8_range_child(
    children: &::kdl::KdlDocument,
    name: &str,
    default: u8,
    min: u8,
    max: u8,
    ctx: &ParseCtx,
) -> ConfigResult<u8> {
    let Some(node) = children.get(name) else {
        return Ok(default);
    };
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(
            node,
            "expected an integer value",
            Some(format!("expected integer {min}-{max}")),
        ));
    };
    let Some(value) = entry.value().as_integer() else {
        return Err(ctx.entry_err(
            entry,
            "expected an integer value",
            Some(format!("expected integer {min}-{max}")),
        ));
    };
    let value = u8::try_from(value).map_err(|_| {
        ctx.entry_err(
            entry,
            format!("value {value} out of range. Expected: integer {min}-{max}"),
            None,
        )
    })?;
    if value < min || value > max {
        return Err(ctx.entry_err(
            entry,
            format!("value {value} out of range. Expected: integer {min}-{max}"),
            None,
        ));
    }
    Ok(value)
}
