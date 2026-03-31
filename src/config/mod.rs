mod defaults;
mod parse;

use anyhow::Context;
use iced_layershell::reexport::Anchor;

/// Parsed configuration for niribar.
/// Fully populated before any surface is created (AR7).
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub bar: BarConfig,
}

/// Bar-level configuration: placement edge and width targets.
#[derive(Debug, Clone)]
pub struct BarConfig {
    pub edge: Edge,
    pub collapsed_width: u32,
    // Used in Story 3.1 (expand/collapse animation)
    #[allow(dead_code)]
    pub expanded_width: u32,
}

impl Default for BarConfig {
    fn default() -> Self {
        BarConfig {
            edge: Edge::Left,
            collapsed_width: 48,
            expanded_width: 200,
        }
    }
}

/// Which screen edge the bar is anchored to.
#[derive(Debug, Clone, Default)]
pub enum Edge {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// Parse a KDL string value into an Edge variant.
    pub(crate) fn from_str(s: &str) -> anyhow::Result<Edge> {
        match s {
            "left" => Ok(Edge::Left),
            "right" => Ok(Edge::Right),
            "top" => Ok(Edge::Top),
            "bottom" => Ok(Edge::Bottom),
            other => {
                anyhow::bail!("unknown edge {other:?}; expected one of: left, right, top, bottom")
            }
        }
    }

    /// Map this edge to iced_layershell anchor bits.
    ///
    /// Side edges (left/right) stretch the full screen height.
    /// Top/bottom edges stretch the full screen width.
    pub fn anchor(&self) -> Anchor {
        match self {
            Edge::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
            Edge::Right => Anchor::Right | Anchor::Top | Anchor::Bottom,
            Edge::Top => Anchor::Top | Anchor::Left | Anchor::Right,
            Edge::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
        }
    }
}

/// Return the path to the user config file, following XDG conventions.
///
/// Returns an error if neither `XDG_CONFIG_HOME` nor `HOME` is set or non-empty.
fn config_file_path() -> anyhow::Result<std::path::PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            let home = std::env::var("HOME")
                .context("neither XDG_CONFIG_HOME nor HOME environment variable is set")?;
            anyhow::ensure!(!home.is_empty(), "HOME environment variable is empty");
            Ok::<_, anyhow::Error>(std::path::PathBuf::from(home).join(".config"))
        })?;
    Ok(base.join("niribar").join("config.kdl"))
}

/// Load configuration from disk or fall back to the embedded default.
///
/// Config is fully parsed before returning — no lazy evaluation (AR7).
pub fn load() -> anyhow::Result<Config> {
    let path = config_file_path()?;
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => defaults::DEFAULT_CONFIG.to_string(),
        Err(e) => {
            return Err(e)
                .with_context(|| format!("failed to read config file: {}", path.display()));
        }
    };
    parse::parse(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_config_succeeds() {
        let config = parse::parse(defaults::DEFAULT_CONFIG).unwrap();
        assert_eq!(config.bar.collapsed_width, 48);
        assert_eq!(config.bar.expanded_width, 200);
    }

    #[test]
    fn missing_bar_section_uses_defaults() {
        let config = parse::parse("").unwrap();
        assert_eq!(config.bar.collapsed_width, 48);
        assert_eq!(config.bar.expanded_width, 200);
    }

    #[test]
    fn edge_left_gives_correct_anchor() {
        let edge = Edge::Left;
        let anchor = edge.anchor();
        assert!(anchor.contains(Anchor::Left));
        assert!(anchor.contains(Anchor::Top));
        assert!(anchor.contains(Anchor::Bottom));
    }

    #[test]
    fn edge_from_str_known_values() {
        assert!(matches!(Edge::from_str("left").unwrap(), Edge::Left));
        assert!(matches!(Edge::from_str("right").unwrap(), Edge::Right));
        assert!(matches!(Edge::from_str("top").unwrap(), Edge::Top));
        assert!(matches!(Edge::from_str("bottom").unwrap(), Edge::Bottom));
    }

    #[test]
    fn edge_from_str_unknown_returns_error() {
        assert!(Edge::from_str("center").is_err());
    }
}
