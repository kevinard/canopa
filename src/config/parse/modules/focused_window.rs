use ::kdl::KdlNode;

use crate::config::FocusedWindowConfig;

pub(super) fn parse(_node: &KdlNode, _idx: usize) -> anyhow::Result<FocusedWindowConfig> {
    Ok(FocusedWindowConfig)
}
