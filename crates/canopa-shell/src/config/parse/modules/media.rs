use super::super::error::ParseCtx;
use crate::config::{ConfigResult, MediaConfig};
use ::kdl::KdlNode;

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<MediaConfig> {
    let Some(children) = node.children() else {
        return Ok(MediaConfig::default());
    };
    let fields = &[
        "type",
        "actions",
        "tooltip",
        "tooltip-id",
        "format",
        "expanded-format",
        "horizontal-format",
        "content-rotation",
    ];
    ctx.validate_child_schema(children, fields, fields)?;
    let (format, tooltip) = super::formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &[
            "icon", "value", "label", "state", "title", "artist", "player",
        ],
        &["playing", "paused", "stopped"],
    )?;
    Ok(MediaConfig {
        format,
        tooltip,
        content_rotation: super::formats::parse_content_rotation_ctx(children, ctx)?,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn media_templates_rotation_and_panel_action_work_for_a_named_instance() {
        let config = crate::config::parse::parse(
            r#"
            bar "main" { modules { center "playing"; }; }
            modules {
                module "playing" {
                    type "media"
                    content-rotation "auto"
                    horizontal-format "{icon}" "{value}"
                    expanded-format "{title}" "{artist}"
                    tooltip "{label}" "{player}" "{state}"
                    actions { click { open-panel; }; }
                }
            }
        "#,
        )
        .unwrap();
        let module = config.modules.iter().find(|m| m.name == "playing").unwrap();
        assert!(matches!(
            module.params,
            crate::config::ModuleParams::Media(_)
        ));
        assert!(
            crate::config::parse::parse(r#"modules { module "media" { format "{unknown}"; }; }"#)
                .is_err()
        );
    }
}
