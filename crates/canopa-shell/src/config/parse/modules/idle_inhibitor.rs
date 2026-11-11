use super::super::error::ParseCtx;
use crate::config::{ConfigResult, IdleInhibitorConfig};
use ::kdl::KdlNode;

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<IdleInhibitorConfig> {
    let Some(children) = node.children() else {
        return Ok(IdleInhibitorConfig::default());
    };
    let fields = &[
        "type",
        "backend",
        "actions",
        "tooltip",
        "tooltip-id",
        "icons",
        "format",
        "expanded-format",
        "horizontal-format",
        "content-rotation",
    ];
    ctx.validate_child_schema(children, fields, fields)?;
    let (format, tooltip) = super::formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "label", "state"],
        &["active", "inactive", "pending", "unavailable"],
    )?;
    let backend = if let Some(node) = children.get("backend") {
        if node.children().is_some()
            || node.entries().len() != 1
            || node.entries()[0].name().is_some()
            || node.entries()[0].ty().is_some()
        {
            return Err(ctx.node_err(node, "idle backend requires one string", None));
        }
        let value = node.entries().first().and_then(|e| e.value().as_string());
        match value {
            Some("wayland") => crate::config::IdleBackend::Wayland,
            Some("stasis") => crate::config::IdleBackend::Stasis,
            _ => return Err(ctx.node_err(node, "idle backend must be wayland or stasis", None)),
        }
    } else {
        crate::config::IdleBackend::Wayland
    };
    Ok(IdleInhibitorConfig {
        backend,
        icons: super::icons::parse_optional_ctx(children, ctx)?,
        format,
        tooltip,
        content_rotation: super::formats::parse_content_rotation_ctx(children, ctx)?,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::{IdleBackend, ModuleParams, parse::parse};
    #[test]
    fn stasis_is_opt_in_and_backend_shape_is_strict() {
        for (field, expected) in [
            ("", IdleBackend::Wayland),
            ("backend \"stasis\";", IdleBackend::Stasis),
            ("backend \"wayland\";", IdleBackend::Wayland),
        ] {
            let config = parse(&format!(
                "modules {{ module \"awake\" {{ type \"idle-inhibitor\"; {field} }} }}"
            ))
            .unwrap();
            let module = config.modules.iter().find(|m| m.name == "awake").unwrap();
            assert!(matches!(&module.params,ModuleParams::IdleInhibitor(c) if c.backend==expected));
        }
        for field in [
            "backend \"unknown\";",
            "backend #true;",
            "backend \"stasis\" \"extra\";",
            "backend \"stasis\" {};",
            "backend \"stasis\"; backend \"wayland\";",
        ] {
            assert!(
                parse(&format!(
                    "modules {{ module \"awake\" {{ type \"idle-inhibitor\"; {field} }} }}"
                ))
                .is_err()
            );
        }
    }
}
