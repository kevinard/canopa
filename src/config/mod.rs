mod defaults;
mod parse;

use anyhow::Context;
use iced_layershell::reexport::Anchor;

/// Known module names for FR37 validation.
/// Hardcoded so validation works before any module is implemented.
const KNOWN_MODULE_NAMES: &[&str] = &[
    "battery",
    "clock",
    "network",
    "volume",
    "workspaces",
    "focused-window",
    "script",
    "button",
];

/// Format strings for the clock module (AC: #4, #5).
#[derive(Debug, Clone)]
pub struct ClockConfig {
    pub time_format: String,
    pub date_format: String,
}

impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            time_format: String::from("%H:%M"),
            date_format: String::from("%a %d %b"),
        }
    }
}

/// Module-specific parameters. Extensible for Stories 2.4–2.5.
#[derive(Debug, Clone)]
pub enum ModuleParams {
    None,
    Clock(ClockConfig),
    // Script(ScriptConfig) — Story 2.4
    // Button(ButtonConfig) — Story 2.5
}

/// A single module entry from the config, specifying type, zone placement, and module params.
#[derive(Debug, Clone)]
pub struct ModuleConfig {
    pub name: String,
    pub zone: crate::modules::Zone,
    pub params: ModuleParams,
}

/// Parsed configuration for niribar.
/// Fully populated before any surface is created (AR7).
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub bar: BarConfig,
    pub modules: Vec<ModuleConfig>,
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
                anyhow::bail!(
                    "bar.edge: unknown value {other:?}. Expected: one of: left, right, top, bottom."
                )
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
            anyhow::bail!("failed to read config file {}: {e}", path.display());
        }
    };
    let config = parse::parse(&content)?;
    validate(&config)?;
    Ok(config)
}

/// Validate a fully-parsed config for constraint violations.
///
/// Returns an error with a spec-formatted message if any constraint is violated.
/// Called by `load()` after parsing. Invalid config → process exits with code 1 (via main).
pub(crate) fn validate(config: &Config) -> anyhow::Result<()> {
    if config.bar.collapsed_width < 2 {
        anyhow::bail!(
            "bar.collapsed-width: value {} is too small. Expected: 2 or greater \
             (minimum 1px content + 1px border).",
            config.bar.collapsed_width
        );
    }
    for (i, mc) in config.modules.iter().enumerate() {
        if !KNOWN_MODULE_NAMES.contains(&mc.name.as_str()) {
            anyhow::bail!(
                "modules[{i}].type: unknown module {:?}. \
                 Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button.",
                mc.name
            );
        }
    }
    Ok(())
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

    #[test]
    fn validate_rejects_collapsed_width_below_2() {
        for w in [0u32, 1u32] {
            let mut config = Config::default();
            config.bar.collapsed_width = w;
            let err = validate(&config).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.starts_with("bar.collapsed-width:"),
                "collapsed_width={w}: expected 'bar.collapsed-width:' prefix, got: {msg}"
            );
            assert!(
                msg.contains("Expected:"),
                "collapsed_width={w}: expected 'Expected:' in message, got: {msg}"
            );
        }
    }

    #[test]
    fn validate_accepts_collapsed_width_at_minimum() {
        let mut config = Config::default();
        config.bar.collapsed_width = 2;
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_unknown_module_name_errors() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "volumee".into(),
            zone: crate::modules::Zone::Bottom,
            params: ModuleParams::None,
        }];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("modules[0].type:"),
            "expected 'modules[0].type:' prefix, got: {msg}"
        );
        assert!(
            msg.contains("volumee"),
            "expected module name in error, got: {msg}"
        );
    }

    #[test]
    fn validate_known_module_name_passes() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: crate::modules::Zone::Bottom,
            params: ModuleParams::None,
        }];
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_error_includes_expected_list() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "unknown".into(),
            zone: crate::modules::Zone::Top,
            params: ModuleParams::None,
        }];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Expected one of:"),
            "expected 'Expected one of:' in: {msg}"
        );
        assert!(
            msg.contains("clock"),
            "expected 'clock' in expected list: {msg}"
        );
    }

    #[test]
    fn validate_error_index_reflects_position() {
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                zone: crate::modules::Zone::Bottom,
                params: ModuleParams::None,
            },
            ModuleConfig {
                name: "bad-module".into(),
                zone: crate::modules::Zone::Top,
                params: ModuleParams::None,
            },
        ];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("modules[1].type:"),
            "expected 'modules[1].type:' prefix, got: {msg}"
        );
    }

    #[test]
    fn clock_config_default_formats() {
        let cfg = ClockConfig::default();
        assert_eq!(cfg.time_format, "%H:%M");
        assert_eq!(cfg.date_format, "%a %d %b");
    }

    #[test]
    fn module_params_clock_stores_config() {
        let params = ModuleParams::Clock(ClockConfig {
            time_format: "%I:%M %p".into(),
            date_format: "%Y-%m-%d".into(),
        });
        match params {
            ModuleParams::Clock(cfg) => {
                assert_eq!(cfg.time_format, "%I:%M %p");
                assert_eq!(cfg.date_format, "%Y-%m-%d");
            }
            _ => panic!("expected ModuleParams::Clock"),
        }
    }
}
