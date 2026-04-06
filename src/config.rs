mod defaults;
mod parse;

use anyhow::Context;
use iced_layershell::reexport::Anchor;

/// Known module names for FR37 validation.
/// Hardcoded so validation works before any module is implemented.
const KNOWN_MODULE_TYPES: &[&str] = &[
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

/// Command and polling interval for the script module.
#[derive(Debug, Clone)]
pub struct ScriptConfig {
    pub command: String,
    pub interval: u64, // seconds between executions
}

/// Configuration for the button module — all fields required.
#[derive(Debug, Clone)]
pub struct ButtonConfig {
    pub label: String,
    pub icon: String,
    pub on_click: String,
}

/// Per-workspace label and icon for the workspaces module.
#[derive(Debug, Clone)]
pub struct WorkspaceLabelConfig {
    pub label: Option<String>,
    pub icon: Option<String>,
}

/// Configuration for the workspaces module — optional label/icon per workspace number.
#[derive(Debug, Clone, Default)]
pub struct WorkspacesConfig {
    /// Maps workspace number (1-based: ws.idx + 1) to label/icon config.
    /// Absent key → workspace shows number only.
    pub workspaces: std::collections::HashMap<u32, WorkspaceLabelConfig>,
}

/// Configuration for the battery module.
#[derive(Debug, Clone)]
pub struct BatteryConfig {
    /// Percentage at or below which the critical badge and red icon appear.
    /// Default: 15. Valid range: 0–100.
    pub critical_threshold: u8,
}

impl Default for BatteryConfig {
    fn default() -> Self {
        BatteryConfig {
            critical_threshold: 15,
        }
    }
}

/// Configuration for the network module (no user-configurable params for MVP).
#[derive(Debug, Clone, Default)]
pub struct NetworkConfig;

/// Configuration for the volume module — scroll step in percent.
#[derive(Debug, Clone)]
pub struct VolumeConfig {
    /// Volume adjustment step per scroll event (percent, 1–100). Default: 5.
    pub step: u8,
}

impl Default for VolumeConfig {
    fn default() -> Self {
        VolumeConfig { step: 5 }
    }
}

/// Module-specific parameters. Extensible for Stories 2.4–2.5.
#[derive(Debug, Clone)]
pub enum ModuleParams {
    None,
    Battery(BatteryConfig),
    Network(NetworkConfig),
    Volume(VolumeConfig),
    Clock(ClockConfig),
    Script(ScriptConfig),
    Button(ButtonConfig),
    Workspaces(WorkspacesConfig),
}

/// A single module entry from the config, specifying name, type, and module params.
/// Zone placement is determined by `bar.modules`, not by this struct.
#[derive(Debug, Clone)]
pub struct ModuleConfig {
    /// Instance name — unique identifier for this module instance (e.g. "clock", "work-status").
    pub name: String,
    /// Module type — determines which module implementation to use (e.g. "clock", "script").
    /// Inferred from `name` when the name matches a known module type; otherwise explicit.
    pub module_type: String,
    pub params: ModuleParams,
}

/// Parsed configuration for niribar.
/// Fully populated before any surface is created (AR7).
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub bar: BarConfig,
    pub modules: Vec<ModuleConfig>,
}

/// Per-zone ordered list of module names for the bar layout.
#[derive(Debug, Clone, Default)]
pub struct BarModules {
    pub top: Vec<String>,
    pub center: Vec<String>,
    pub bottom: Vec<String>,
}

/// Bar-level configuration: placement edge, width targets, and expand trigger.
#[derive(Debug, Clone)]
pub struct BarConfig {
    pub edge: Edge,
    pub collapsed_width: u32,
    pub expanded_width: u32,
    /// Whether the expand/collapse overlay feature is enabled. Default: true.
    /// When false, the bar is permanently collapsed and all expand actions are ignored.
    pub expand: bool,
    /// What user action triggers expand/collapse. Default: Click.
    pub expand_trigger: ExpandTrigger,
    /// Dwell delay (ms) before hover expands the bar. Only used when expand_trigger = Hover.
    pub hover_delay: u64,
    /// Keybind for in-bar expand/collapse while the bar has keyboard focus.
    /// Also documents the compositor keybind the user should bind to `niribar --toggle-expand`
    /// for global (unfocused) toggling.
    pub expand_keybind: Option<String>,
    /// Ordered module names per zone. Empty lists mean no modules in that zone.
    pub modules: BarModules,
}

impl Default for BarConfig {
    fn default() -> Self {
        BarConfig {
            edge: Edge::Left,
            collapsed_width: 48,
            expanded_width: 200,
            expand: true,
            expand_trigger: ExpandTrigger::Click,
            hover_delay: 0,
            expand_keybind: None,
            modules: BarModules::default(),
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

    /// Returns `true` for edges that span the full screen width (top/bottom).
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Edge::Top | Edge::Bottom)
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

/// Determines what user action triggers bar expand/collapse.
///
/// Hover and click are mutually exclusive: when `Hover` is set, click has no effect.
/// Default: `Click`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ExpandTrigger {
    #[default]
    Click,
    Hover,
}

impl ExpandTrigger {
    pub(crate) fn from_str(s: &str) -> anyhow::Result<ExpandTrigger> {
        match s {
            "click" => Ok(ExpandTrigger::Click),
            "hover" => Ok(ExpandTrigger::Hover),
            other => anyhow::bail!(
                "bar.expand-trigger: unknown value {other:?}. \
                 Expected: one of: click, hover."
            ),
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
    if config.bar.expanded_width < config.bar.collapsed_width {
        anyhow::bail!(
            "bar.expanded-width: value {} must be >= bar.collapsed-width ({}). \
             Expected: a width greater than or equal to the collapsed width.",
            config.bar.expanded_width,
            config.bar.collapsed_width
        );
    }
    for (i, mc) in config.modules.iter().enumerate() {
        if !KNOWN_MODULE_TYPES.contains(&mc.module_type.as_str()) {
            anyhow::bail!(
                "modules[{i}].type: unknown module type {:?}. \
                 Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button.",
                mc.module_type
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
            module_type: "volumee".into(),
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
            module_type: "clock".into(),
            params: ModuleParams::None,
        }];
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_error_includes_expected_list() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "unknown".into(),
            module_type: "unknown".into(),
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
                module_type: "clock".into(),
                params: ModuleParams::None,
            },
            ModuleConfig {
                name: "bad-module".into(),
                module_type: "bad-module".into(),
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
    fn validate_rejects_expanded_width_less_than_collapsed() {
        let mut config = Config::default();
        config.bar.collapsed_width = 100;
        config.bar.expanded_width = 50; // less than collapsed
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("bar.expanded-width:"),
            "expected 'bar.expanded-width:' prefix, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn validate_accepts_expanded_width_equal_to_collapsed() {
        let mut config = Config::default();
        config.bar.collapsed_width = 48;
        config.bar.expanded_width = 48; // equal is OK
        assert!(validate(&config).is_ok());
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

    #[test]
    fn script_config_stores_command_and_interval() {
        let cfg = ScriptConfig {
            command: "echo hi".into(),
            interval: 10,
        };
        assert_eq!(cfg.command, "echo hi");
        assert_eq!(cfg.interval, 10);
    }

    #[test]
    fn module_params_script_stores_config() {
        let params = ModuleParams::Script(ScriptConfig {
            command: "date".into(),
            interval: 30,
        });
        match params {
            ModuleParams::Script(cfg) => {
                assert_eq!(cfg.command, "date");
                assert_eq!(cfg.interval, 30);
            }
            _ => panic!("expected ModuleParams::Script"),
        }
    }

    #[test]
    fn button_config_stores_fields() {
        let cfg = ButtonConfig {
            label: "Launch Terminal".into(),
            icon: ">".into(),
            on_click: "kitty".into(),
        };
        assert_eq!(cfg.label, "Launch Terminal");
        assert_eq!(cfg.icon, ">");
        assert_eq!(cfg.on_click, "kitty");
    }

    #[test]
    fn module_params_button_stores_config() {
        let params = ModuleParams::Button(ButtonConfig {
            label: "launch".into(),
            icon: ">".into(),
            on_click: "kitty".into(),
        });
        match params {
            ModuleParams::Button(cfg) => {
                assert_eq!(cfg.label, "launch");
                assert_eq!(cfg.icon, ">");
                assert_eq!(cfg.on_click, "kitty");
            }
            _ => panic!("expected ModuleParams::Button"),
        }
    }

    #[test]
    fn bar_config_default_expand_keybind_is_none() {
        assert!(BarConfig::default().expand_keybind.is_none());
    }

    #[test]
    fn bar_config_default_expand_is_true() {
        assert!(BarConfig::default().expand);
    }
}
