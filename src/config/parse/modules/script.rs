use ::kdl::KdlNode;

use super::super::{error, kdl};
use crate::config::ScriptConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<ScriptConfig> {
    let command_path = error::field_path(idx, "script", "command");
    let children = node.children().ok_or_else(|| {
        anyhow::anyhow!("{command_path}: missing required parameter. Expected: a string value.")
    })?;

    if children.get("command").is_none() {
        anyhow::bail!("{command_path}: missing required parameter. Expected: a string value.");
    }
    let command = kdl::optional_nonempty_string_child(
        children,
        "command",
        String::new(),
        &command_path,
        "a shell command string",
    )?;

    let interval = match children.get("interval") {
        None => 5u64,
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_integer()) {
                Some(i) if i >= 1 => i as u64,
                Some(_) => anyhow::bail!(
                    "{}: value must be at least 1. Expected: a positive integer.",
                    error::field_path(idx, "script", "interval")
                ),
                None => {
                    anyhow::bail!(
                        "{}: expected a positive integer.",
                        error::field_path(idx, "script", "interval")
                    )
                }
            }
        }
    };

    Ok(ScriptConfig { command, interval })
}
