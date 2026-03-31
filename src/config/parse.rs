use anyhow::Context;
use kdl::KdlDocument;

use super::{BarConfig, Config, Edge};

/// Parse a KDL string into a fully-typed `Config`.
///
/// Missing sections fall back to defaults — validation of required fields
/// and formatted error messages are added in Story 1.4.
pub fn parse(input: &str) -> anyhow::Result<Config> {
    let doc: KdlDocument = input.parse().context("failed to parse KDL config")?;
    let bar = parse_bar_section(&doc)?;
    Ok(Config { bar })
}

fn parse_bar_section(doc: &KdlDocument) -> anyhow::Result<BarConfig> {
    let Some(bar_node) = doc.get("bar") else {
        return Ok(BarConfig::default());
    };

    let children = match bar_node.children() {
        Some(c) => c,
        None => return Ok(BarConfig::default()),
    };

    let defaults = BarConfig::default();

    let edge = children
        .get_arg("edge")
        .and_then(|v| v.as_string())
        .map(Edge::from_str)
        .transpose()
        .context("bar.edge")?
        .unwrap_or(defaults.edge);

    let collapsed_width = children
        .get_arg("collapsed-width")
        .and_then(|v| v.as_integer())
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(defaults.collapsed_width);

    let expanded_width = children
        .get_arg("expanded-width")
        .and_then(|v| v.as_integer())
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(defaults.expanded_width);

    Ok(BarConfig {
        edge,
        collapsed_width,
        expanded_width,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_bar_section() {
        let kdl = r#"
bar {
    edge "right"
    collapsed-width 56
    expanded-width 240
}
"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bar.edge, Edge::Right));
        assert_eq!(config.bar.collapsed_width, 56);
        assert_eq!(config.bar.expanded_width, 240);
    }

    #[test]
    fn parse_edge_left() {
        let kdl = r#"bar { edge "left" }"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bar.edge, Edge::Left));
    }

    #[test]
    fn parse_unknown_edge_errors() {
        let kdl = r#"bar { edge "diagonal" }"#;
        assert!(parse(kdl).is_err());
    }

    #[test]
    fn parse_empty_string_uses_defaults() {
        let config = parse("").unwrap();
        assert!(matches!(config.bar.edge, Edge::Left));
        assert_eq!(config.bar.collapsed_width, 48);
        assert_eq!(config.bar.expanded_width, 200);
    }

    #[test]
    fn parse_bar_without_children_uses_defaults() {
        // A bar node with no children block
        let config = parse("bar\n").unwrap();
        assert_eq!(config.bar.collapsed_width, 48);
    }
}
