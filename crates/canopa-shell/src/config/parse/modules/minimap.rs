use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use crate::config::{ConfigResult, MinimapConfig};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<MinimapConfig> {
    let defaults = MinimapConfig::default();
    let Some(children) = node.children() else {
        return Ok(defaults);
    };
    let allowed = ["type", "actions", "size", "max-length"];
    ctx.validate_child_schema(children, &allowed, &allowed)?;
    formats::reject_format_fields_ctx(node.children(), ctx, "minimap")?;
    let size = fields::field(children, ctx, "size")
        .optional()
        .u8_range(8, 64)
        .unwrap_or(defaults.size)?;
    let max_length = fields::field(children, ctx, "max-length")
        .optional()
        .integer_message("expected a non-negative integer (0 disables the limit)")
        .u32()
        .unwrap_or(defaults.max_length)?;
    Ok(MinimapConfig { size, max_length })
}

#[cfg(test)]
mod tests {
    fn minimap(kdl: &str) -> Result<crate::config::MinimapConfig, String> {
        let config = crate::config::parse::parse(kdl).map_err(|error| format!("{error:?}"))?;
        match config
            .modules
            .into_iter()
            .find(|module| module.name == "strip")
            .expect("minimap module")
            .params
        {
            crate::config::ModuleParams::Minimap(config) => Ok(config),
            other => panic!("unexpected params {other:?}"),
        }
    }

    #[test]
    fn defaults_apply_without_fields() {
        assert_eq!(
            minimap(r#"modules { module "strip" { type "minimap"; } }"#),
            Ok(crate::config::MinimapConfig::default())
        );
    }

    #[test]
    fn size_and_max_length_are_read() {
        let config =
            minimap(r#"modules { module "strip" { type "minimap"; size 24; max-length 0; } }"#)
                .expect("valid minimap");
        assert_eq!(config.size, 24);
        assert_eq!(config.max_length, 0);
    }

    #[test]
    fn size_out_of_range_is_rejected() {
        assert!(minimap(r#"modules { module "strip" { type "minimap"; size 4; } }"#).is_err());
    }

    #[test]
    fn format_fields_are_rejected() {
        assert!(
            minimap(r#"modules { module "strip" { type "minimap"; format "{title}"; } }"#).is_err()
        );
    }
}
