use ::kdl::KdlNode;

use super::super::{error, kdl};
use super::formats;
use super::icons;
use crate::config::VolumeConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
    let Some(children) = node.children() else {
        return Ok(VolumeConfig::default());
    };
    let step = kdl::optional_u8_range_child(
        children,
        "step",
        VolumeConfig::default().step,
        1,
        100,
        &error::field_path(idx, "volume", "step"),
    )?;
    let icons = icons::parse_optional(children, &error::module_path(idx, "volume"))?;
    let format = formats::parse_optional(
        children,
        &error::module_path(idx, "volume"),
        &["icon", "value", "percent", "state"],
    )?;
    Ok(VolumeConfig {
        step,
        icons,
        format,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    #[test]
    fn parse_optional_u8_range_reports_out_of_range() {
        let kdl = r#"modules { module "volume" { step 101 } }"#;
        let err = parse(kdl).unwrap_err();
        assert!(err.to_string().contains("volume.step"));
        assert!(err.to_string().contains("1\u{2013}100"));
    }

    #[test]
    fn parse_volume_step_without_value_errors() {
        let kdl = r#"
modules {
    module "volume" {
        step
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("volume.step"),
            "expected 'volume.step' in error, got: {msg}"
        );
        assert!(
            msg.contains("expected an integer value"),
            "expected 'expected an integer value' in error, got: {msg}"
        );
    }
}
