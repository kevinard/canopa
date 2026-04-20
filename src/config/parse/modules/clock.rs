use ::kdl::KdlNode;

use super::super::{error, kdl};
use crate::config::ClockConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<ClockConfig> {
    let Some(children) = node.children() else {
        return Ok(ClockConfig::default());
    };

    let defaults = ClockConfig::default();

    let time_format = kdl::optional_nonempty_string_child(
        children,
        "time-format",
        defaults.time_format,
        &error::field_path(idx, "clock", "time-format"),
        "a chrono format string such as \"%H:%M\"",
    )?;

    let date_format = kdl::optional_nonempty_string_child(
        children,
        "date-format",
        defaults.date_format,
        &error::field_path(idx, "clock", "date-format"),
        "a chrono format string such as \"%a %d %b\"",
    )?;

    Ok(ClockConfig {
        time_format,
        date_format,
    })
}
