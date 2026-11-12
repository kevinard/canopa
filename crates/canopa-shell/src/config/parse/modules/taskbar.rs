use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use crate::config::{ConfigResult, TaskbarConfig};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<TaskbarConfig> {
    let Some(children) = node.children() else {
        return Ok(TaskbarConfig::default());
    };
    let allowed = ["type", "actions", "pinned", "tooltip", "tooltip-id"];
    ctx.validate_child_schema(children, &allowed, &allowed)?;
    formats::reject_format_fields_ctx(node.children(), ctx, "taskbar")?;
    let tooltip = formats::parse_tooltip_field_ctx(children, ctx, &["name", "windows"], &[])?;
    let mut pinned = Vec::new();
    for id in fields::field(children, ctx, "pinned")
        .optional()
        .positional_strings(
            "taskbar.pinned entries must be desktop IDs, such as \"org.gnome.Nautilus.desktop\"",
        )
        .unwrap_or(Vec::new())?
    {
        let id = id.trim();
        let id = if id.ends_with(".desktop") {
            id.to_owned()
        } else {
            format!("{id}.desktop")
        };
        if !id.is_empty() && !pinned.contains(&id) {
            pinned.push(id);
        }
    }
    Ok(TaskbarConfig { pinned, tooltip })
}

#[cfg(test)]
mod tests {
    fn taskbar(kdl: &str) -> crate::config::TaskbarConfig {
        let config = crate::config::parse::parse(kdl).expect("valid taskbar config");
        match config
            .modules
            .into_iter()
            .find(|module| module.name == "apps")
            .expect("taskbar module")
            .params
        {
            crate::config::ModuleParams::Taskbar(config) => config,
            other => panic!("unexpected params {other:?}"),
        }
    }

    #[test]
    fn pinned_ids_keep_their_order_gain_the_desktop_suffix_and_drop_duplicates() {
        let config = taskbar(
            r#"modules { module "apps" { type "taskbar"; pinned "org.gnome.Nautilus.desktop" "firefox" "org.gnome.Nautilus"; } }"#,
        );
        assert_eq!(
            config.pinned,
            ["org.gnome.Nautilus.desktop", "firefox.desktop"]
        );
    }
}
