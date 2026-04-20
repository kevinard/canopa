mod defaults;
mod parse;

use anyhow::Context;
use iced_layershell::reexport::Anchor;

use crate::modules::ModuleKind;

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
    "tray",
];

fn is_known_config_module_type(module_type: &str) -> bool {
    ModuleKind::from_config_name(module_type)
        .is_some_and(|kind| ModuleKind::CONFIG_KINDS.contains(&kind))
}

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

/// Configuration for the tray module.
#[derive(Debug, Clone, Default)]
pub struct TrayConfig;

/// Configuration for the focused-window module (no user-configurable params).
#[derive(Debug, Clone, Default)]
pub struct FocusedWindowConfig;

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
    Tray(TrayConfig),
    FocusedWindow(FocusedWindowConfig),
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
#[derive(Debug, Clone)]
pub struct Config {
    /// All named bar sections. At least one is always present (falls back to defaults when
    /// no `bar` sections appear in the config file).
    pub bars: Vec<BarConfig>,
    pub modules: Vec<ModuleConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bars: vec![BarConfig::default()],
            modules: vec![],
        }
    }
}

impl Config {
    /// Return all bar configs that should appear on the given output.
    ///
    /// A bar with `output == "all"` appears on every connected output.
    /// A bar with a specific output name appears only on that output.
    /// In AllScreens mode (`output_name == ""`), returns all bars with `output == "all"`.
    pub fn bars_for_output(&self, output_name: &str) -> Vec<&BarConfig> {
        self.bars
            .iter()
            .filter(|b| b.output == "all" || b.output == output_name)
            .collect()
    }
}

/// Per-zone ordered list of module names for the bar layout.
#[derive(Debug, Clone, Default)]
pub struct BarModules {
    pub top: Vec<String>,
    pub center: Vec<String>,
    pub bottom: Vec<String>,
}

/// Animation timing for expand/collapse transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationConfig {
    /// Duration of the expand animation in milliseconds. Default: 280.0.
    pub expand_ms: f32,
    /// Duration of the collapse animation in milliseconds. Default: 200.0.
    pub collapse_ms: f32,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        AnimationConfig {
            expand_ms: 280.0,
            collapse_ms: 200.0,
        }
    }
}

/// Bar-level configuration: placement edge, width targets, and expand trigger.
#[derive(Debug, Clone)]
pub struct BarConfig {
    /// Instance name — unique identifier for this bar (e.g. "main", "top-strip").
    pub name: String,
    /// Wayland output name this bar targets, or `"all"` to appear on every output.
    /// Default: `"all"`.
    pub output: String,
    pub edge: Edge,
    /// Collapsed bar width, in **logical pixels**.
    /// The compositor multiplies this by the output scale factor for physical rendering (NFR11).
    pub collapsed_width: u32,
    /// Expanded bar width, in **logical pixels**.
    /// The compositor multiplies this by the output scale factor for physical rendering (NFR11).
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
    /// Animation timing for expand/collapse transitions.
    pub animation: AnimationConfig,
    /// Ordered module names per zone. Empty lists mean no modules in that zone.
    pub modules: BarModules,
}

impl Default for BarConfig {
    fn default() -> Self {
        BarConfig {
            name: "main".to_string(),
            output: "all".to_string(),
            edge: Edge::Left,
            collapsed_width: 48,
            expanded_width: 200,
            expand: true,
            expand_trigger: ExpandTrigger::Click,
            hover_delay: 0,
            expand_keybind: None,
            animation: AnimationConfig::default(),
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
    // Duplicate bar name check
    let mut seen_bar_names = std::collections::HashSet::new();
    for bar in &config.bars {
        if !seen_bar_names.insert(bar.name.as_str()) {
            anyhow::bail!(
                "bar {:?}: duplicate name. Each bar section must have a unique name.",
                bar.name
            );
        }
    }

    // Per-bar width constraints
    for bar in &config.bars {
        if bar.collapsed_width < 2 {
            anyhow::bail!(
                "bar {:?}: collapsed-width: value {} is too small. Expected: 2 or greater \
                 (minimum 1px content + 1px border).",
                bar.name,
                bar.collapsed_width
            );
        }
        if bar.expanded_width < bar.collapsed_width {
            anyhow::bail!(
                "bar {:?}: expanded-width: value {} must be >= collapsed-width ({}). \
                 Expected: a width greater than or equal to the collapsed width.",
                bar.name,
                bar.expanded_width,
                bar.collapsed_width
            );
        }
    }

    for (i, mc) in config.modules.iter().enumerate() {
        if !is_known_config_module_type(&mc.module_type) {
            anyhow::bail!(
                "modules[{i}].type: unknown module type {:?}. \
                 Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button, tray.",
                mc.module_type
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn known_module_types_include_all_config_module_kinds() {
        for kind in ModuleKind::CONFIG_KINDS {
            assert!(
                KNOWN_MODULE_TYPES.contains(&kind.as_str()),
                "missing known module type {:?}",
                kind
            );
        }
    }

    #[test]
    fn parse_default_config_succeeds() {
        let config = parse::parse(defaults::DEFAULT_CONFIG).unwrap();
        assert_eq!(config.bars.len(), 1);
        assert_eq!(config.bars[0].collapsed_width, 48);
        assert_eq!(config.bars[0].expanded_width, 200);
        // UX-DR18: zone assignments
        assert_eq!(config.bars[0].modules.top, vec!["workspaces"]);
        assert_eq!(config.bars[0].modules.center, vec!["focused-window"]);
        assert_eq!(
            config.bars[0].modules.bottom,
            vec!["volume", "network", "battery", "clock"]
        );
        // All 6 default modules declared
        assert_eq!(config.modules.len(), 6);
        let names: Vec<&str> = config.modules.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"workspaces"));
        assert!(names.contains(&"focused-window"));
        assert!(names.contains(&"volume"));
        assert!(names.contains(&"network"));
        assert!(names.contains(&"battery"));
        assert!(names.contains(&"clock"));
    }

    #[test]
    fn missing_bar_section_uses_defaults() {
        // Empty input falls back to a single default bar.
        let config = parse::parse("").unwrap();
        assert_eq!(config.bars[0].collapsed_width, 48);
        assert_eq!(config.bars[0].expanded_width, 200);
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
            config.bars[0].collapsed_width = w;
            let err = validate(&config).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("collapsed-width:"),
                "collapsed_width={w}: expected 'collapsed-width:' in message, got: {msg}"
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
        config.bars[0].collapsed_width = 2;
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
    fn validate_tray_module_type_passes() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "tray".into(),
            module_type: "tray".into(),
            params: ModuleParams::Tray(TrayConfig),
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
        config.bars[0].collapsed_width = 100;
        config.bars[0].expanded_width = 50; // less than collapsed
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("expanded-width:"),
            "expected 'expanded-width:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn validate_accepts_expanded_width_equal_to_collapsed() {
        let mut config = Config::default();
        config.bars[0].collapsed_width = 48;
        config.bars[0].expanded_width = 48; // equal is OK
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

    // --- Story 9.1: Named bar sections with output targeting ---

    #[test]
    fn bars_for_output_all_matches_any_output() {
        let config = Config::default(); // single bar "main" with output "all"
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].name, "main");
    }

    #[test]
    fn bars_for_output_specific_name_matches_only_that_output() {
        let mut config = Config::default();
        config.bars = vec![
            BarConfig {
                name: "main".into(),
                output: "all".into(),
                ..BarConfig::default()
            },
            BarConfig {
                name: "side".into(),
                output: "DP-1".into(),
                ..BarConfig::default()
            },
        ];
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars.len(), 2); // both "main" (all) and "side" (DP-1)
        let bars_edp = config.bars_for_output("eDP-1");
        assert_eq!(bars_edp.len(), 1); // only "main"
    }

    #[test]
    fn bars_for_output_no_all_bars_specific_output_not_connected() {
        let mut config = Config::default();
        config.bars = vec![BarConfig {
            name: "side".into(),
            output: "DP-1".into(),
            ..BarConfig::default()
        }];
        let bars = config.bars_for_output("eDP-1");
        assert!(bars.is_empty());
    }

    #[test]
    fn bars_for_output_empty_string_matches_only_all() {
        // AllScreens mode: output_name is ""
        let config = Config::default();
        let bars = config.bars_for_output("");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].output, "all");
    }

    #[test]
    fn validate_duplicate_bar_name_errors() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "main".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("duplicate name"), "got: {err}");
    }

    #[test]
    fn validate_bar_collapsed_width_too_small_errors() {
        let config = Config {
            bars: vec![BarConfig {
                name: "x".into(),
                output: "all".into(),
                collapsed_width: 1,
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("collapsed-width"), "got: {err}");
    }

    // --- Additional Story 9.1 config tests ---

    #[test]
    fn bar_config_default_name_is_main() {
        assert_eq!(BarConfig::default().name, "main");
    }

    #[test]
    fn bar_config_default_output_is_all() {
        assert_eq!(BarConfig::default().output, "all");
    }

    #[test]
    fn config_default_has_one_bar() {
        assert_eq!(Config::default().bars.len(), 1);
    }

    #[test]
    fn config_default_bar_name_is_main() {
        assert_eq!(Config::default().bars[0].name, "main");
    }

    #[test]
    fn config_default_bar_output_is_all() {
        assert_eq!(Config::default().bars[0].output, "all");
    }

    #[test]
    fn bars_for_output_all_targets_any_named_output() {
        let config = Config::default();
        // "all" should match any output name
        assert_eq!(config.bars_for_output("eDP-1").len(), 1);
        assert_eq!(config.bars_for_output("HDMI-A-1").len(), 1);
        assert_eq!(config.bars_for_output("DP-2").len(), 1);
    }

    #[test]
    fn bars_for_output_multiple_all_bars_returns_all() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "a".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "b".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let bars = config.bars_for_output("eDP-1");
        assert_eq!(bars.len(), 2);
    }

    #[test]
    fn bars_for_output_mixed_returns_both_matching() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "extra".into(),
                    output: "eDP-1".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "other".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let bars = config.bars_for_output("eDP-1");
        assert_eq!(bars.len(), 2); // "main" (all) + "extra" (eDP-1)
        let names: Vec<&str> = bars.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"extra"));
        assert!(!names.contains(&"other"));
    }

    #[test]
    fn bars_for_output_preserves_order() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "first".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "second".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "third".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars[0].name, "first");
        assert_eq!(bars[1].name, "second");
        assert_eq!(bars[2].name, "third");
    }

    #[test]
    fn validate_multiple_bars_no_duplicate_names_passes() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_duplicate_name_error_includes_bar_name() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "side".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("side"), "got: {err}");
    }

    #[test]
    fn validate_second_bar_expanded_less_than_collapsed_errors() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output: "DP-1".into(),
                    collapsed_width: 100,
                    expanded_width: 50,
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(
            err.to_string().contains("side"),
            "error should name the offending bar: {err}"
        );
        assert!(err.to_string().contains("expanded-width"), "got: {err}");
    }

    #[test]
    fn bars_for_output_case_sensitive() {
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output: "dp-1".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        // "DP-1" (uppercase) does not match "dp-1" (lowercase)
        assert!(config.bars_for_output("DP-1").is_empty());
        assert_eq!(config.bars_for_output("dp-1").len(), 1);
    }
}
