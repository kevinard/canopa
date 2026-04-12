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

/// Per-output configuration, parsed from `output "name" { ... }` KDL blocks.
/// A connected output not in this list receives the global defaults.
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// The Wayland output name (e.g. "eDP-1", "DP-1", "HDMI-A-1").
    pub name: String,
    /// Whether to create a bar surface on this output. Default: true.
    pub enabled: bool,
    /// Override for bar.collapsed-width. `None` → inherit from global `bar`.
    pub collapsed_width: Option<u32>,
    /// Override for bar.expanded-width. `None` → inherit from global `bar`.
    pub expanded_width: Option<u32>,
    /// Override for bar.modules zone layout. `None` → inherit from global `bar`.
    pub modules: Option<BarModules>,
    // New in Story 7.5:
    /// Override for bar.edge. `None` → inherit from global.
    pub edge: Option<Edge>,
    /// Override for bar.expand. `None` → inherit from global.
    pub expand: Option<bool>,
    /// Override for bar.expand-trigger. `None` → inherit from global.
    pub expand_trigger: Option<ExpandTrigger>,
    /// Override for bar.hover-delay. `None` → inherit from global.
    pub hover_delay: Option<u64>,
    /// Override for bar.expand-keybind. `None` → inherit from global.
    pub expand_keybind: Option<String>,
    /// Override for bar.animation. `None` → inherit from global.
    pub animation: Option<AnimationConfig>,
}

/// Parsed configuration for niribar.
/// Fully populated before any surface is created (AR7).
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub bar: BarConfig,
    pub modules: Vec<ModuleConfig>,
    /// Per-output overrides. Outputs not listed here use global defaults.
    pub outputs: Vec<OutputConfig>,
}

impl Config {
    /// Return the effective `BarConfig` for the given output name.
    ///
    /// Per-output overrides take precedence; absent fields inherit from the global `bar`.
    /// Returns a clone of the global `bar` when the output has no config block,
    /// or when `output_name` is `""` (AllScreens mode — no output name assigned).
    pub fn effective_bar_for(&self, output_name: &str) -> BarConfig {
        let global = &self.bar;
        let Some(oc) = self.outputs.iter().find(|o| o.name == output_name) else {
            return global.clone();
        };
        BarConfig {
            collapsed_width: oc.collapsed_width.unwrap_or(global.collapsed_width),
            expanded_width: oc.expanded_width.unwrap_or(global.expanded_width),
            modules: oc.modules.clone().unwrap_or_else(|| global.modules.clone()),
            edge: oc.edge.clone().unwrap_or_else(|| global.edge.clone()),
            expand: oc.expand.unwrap_or(global.expand),
            expand_trigger: oc.expand_trigger.unwrap_or(global.expand_trigger),
            hover_delay: oc.hover_delay.unwrap_or(global.hover_delay),
            expand_keybind: oc
                .expand_keybind
                .clone()
                .or_else(|| global.expand_keybind.clone()),
            animation: oc
                .animation
                .clone()
                .unwrap_or_else(|| global.animation.clone()),
        }
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
    // Check for duplicate output names
    let mut seen_output_names = std::collections::HashSet::new();
    for oc in &config.outputs {
        if !seen_output_names.insert(&oc.name) {
            anyhow::bail!(
                "output {:?}: duplicate output name. \
                 Expected: each output name to appear at most once.",
                oc.name
            );
        }
    }
    // Validate per-output width overrides
    for oc in &config.outputs {
        if let Some(cw) = oc.collapsed_width
            && cw < 2
        {
            anyhow::bail!(
                "output {:?}: collapsed-width: value {} is too small. \
                 Expected: 2 or greater.",
                oc.name,
                cw
            );
        }
        // effective collapsed width: per-output override OR global
        let effective_collapsed = oc.collapsed_width.unwrap_or(config.bar.collapsed_width);
        if let Some(ew) = oc.expanded_width
            && ew < effective_collapsed
        {
            anyhow::bail!(
                "output {:?}: expanded-width: value {} must be >= effective collapsed-width ({}). \
                 Expected: a width >= collapsed-width.",
                oc.name,
                ew,
                effective_collapsed
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
        // UX-DR18: zone assignments
        assert_eq!(config.bar.modules.top, vec!["workspaces"]);
        assert_eq!(config.bar.modules.center, vec!["focused-window"]);
        assert_eq!(
            config.bar.modules.bottom,
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

    // --- Story 7.3: Output exclusion config tests ---

    // --- Story 7.3: Output exclusion config tests ---
    // KDL syntax: output "name" { enabled #true } or output "name" { enabled #false }
    // KDL 6.5.0 (v2) uses #true/##false for booleans, matching bar.expand syntax.

    #[test]
    fn output_config_enabled_true_parses() {
        let config = parse::parse(r#"output "eDP-1" { enabled #true }"#).unwrap();
        assert_eq!(config.outputs.len(), 1);
        assert_eq!(config.outputs[0].name, "eDP-1");
        assert!(config.outputs[0].enabled);
    }

    #[test]
    fn output_config_enabled_false_parses() {
        let config = parse::parse(r#"output "DP-1" { enabled #false }"#).unwrap();
        assert_eq!(config.outputs.len(), 1);
        assert_eq!(config.outputs[0].name, "DP-1");
        assert!(!config.outputs[0].enabled);
    }

    #[test]
    fn output_config_empty_block_defaults_enabled_true() {
        let config = parse::parse(r#"output "HDMI-1" {}"#).unwrap();
        assert_eq!(config.outputs.len(), 1);
        assert!(config.outputs[0].enabled);
    }

    #[test]
    fn config_with_no_output_blocks_has_empty_vec() {
        let config = parse::parse("").unwrap();
        assert!(config.outputs.is_empty());
    }

    #[test]
    fn config_with_multiple_output_blocks_parses() {
        let kdl = r#"output "eDP-1" { enabled #true }
output "DP-1" { enabled #false }"#;
        let config = parse::parse(kdl).unwrap();
        assert_eq!(config.outputs.len(), 2);
        let dp1 = config.outputs.iter().find(|o| o.name == "DP-1").unwrap();
        assert!(!dp1.enabled);
    }

    #[test]
    fn output_unknown_key_errors() {
        let result = parse::parse(r#"output "DP-1" { zoom 2 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown key"), "got: {msg}");
    }

    #[test]
    fn output_missing_name_errors() {
        let result = parse::parse("output {}");
        assert!(result.is_err());
    }

    #[test]
    fn validate_duplicate_output_name_errors() {
        let kdl = r#"output "DP-1" { enabled #false }
output "DP-1" { enabled #true }"#;
        let parsed = parse::parse(kdl).unwrap();
        let err = validate(&parsed).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("DP-1"), "got: {msg}");
        assert!(msg.contains("duplicate"), "got: {msg}");
    }

    #[test]
    fn excluded_outputs_computed_from_config() {
        let kdl = r#"output "eDP-1" { enabled #true }
output "DP-1" { enabled #false }"#;
        let config = parse::parse(kdl).unwrap();
        let excluded: std::collections::HashSet<String> = config
            .outputs
            .iter()
            .filter(|o| !o.enabled)
            .map(|o| o.name.clone())
            .collect();
        assert!(excluded.contains("DP-1"));
        assert!(!excluded.contains("eDP-1"));
    }

    #[test]
    fn output_not_in_config_is_implicitly_allowed() {
        let kdl = r#"output "DP-1" { enabled #false }"#;
        let config = parse::parse(kdl).unwrap();
        let excluded: std::collections::HashSet<String> = config
            .outputs
            .iter()
            .filter(|o| !o.enabled)
            .map(|o| o.name.clone())
            .collect();
        // HDMI-1 is not mentioned → not excluded
        assert!(!excluded.contains("HDMI-1"));
    }

    #[test]
    fn validate_unique_output_names_passes() {
        let kdl = r#"output "eDP-1" { enabled #true }
output "DP-1" { enabled #false }"#;
        let config = parse::parse(kdl).unwrap();
        assert!(validate(&config).is_ok());
    }

    // --- Story 7.4: Per-output configuration override tests ---

    #[test]
    fn effective_bar_for_unknown_output_returns_global() {
        let config = Config::default();
        let bar = config.effective_bar_for("DP-1");
        assert_eq!(bar.collapsed_width, config.bar.collapsed_width);
        assert_eq!(bar.expanded_width, config.bar.expanded_width);
    }

    #[test]
    fn effective_bar_for_empty_name_returns_global() {
        // AllScreens mode: output_name is "" — must return global defaults.
        let config = Config::default();
        let bar = config.effective_bar_for("");
        assert_eq!(bar.collapsed_width, config.bar.collapsed_width);
        assert_eq!(bar.expanded_width, config.bar.expanded_width);
    }

    #[test]
    fn effective_bar_for_applies_collapsed_width_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "eDP-1".into(),
            enabled: true,
            collapsed_width: Some(60),
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("eDP-1");
        assert_eq!(bar.collapsed_width, 60);
        // expanded_width inherits from global
        assert_eq!(bar.expanded_width, config.bar.expanded_width);
    }

    #[test]
    fn effective_bar_for_applies_expanded_width_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "eDP-1".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: Some(300),
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("eDP-1");
        assert_eq!(bar.expanded_width, 300);
        // collapsed_width inherits from global
        assert_eq!(bar.collapsed_width, config.bar.collapsed_width);
    }

    #[test]
    fn effective_bar_for_applies_modules_override() {
        let mut config = Config::default();
        let override_mods = BarModules {
            top: vec!["clock".into()],
            center: vec![],
            bottom: vec![],
        };
        config.outputs = vec![OutputConfig {
            name: "eDP-1".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: Some(override_mods.clone()),
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("eDP-1");
        assert_eq!(bar.modules.top, override_mods.top);
        assert!(bar.modules.center.is_empty());
        assert!(bar.modules.bottom.is_empty());
    }

    #[test]
    fn effective_bar_for_partial_override_inherits_rest() {
        let mut config = Config::default();
        config.bar.collapsed_width = 48;
        config.bar.expanded_width = 200;
        config.bar.modules.center = vec!["clock".into()];
        config.outputs = vec![OutputConfig {
            name: "eDP-1".into(),
            enabled: true,
            collapsed_width: Some(60),
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("eDP-1");
        assert_eq!(bar.collapsed_width, 60); // overridden
        assert_eq!(bar.expanded_width, 200); // inherited
        assert_eq!(bar.modules.center, vec!["clock"]); // inherited
    }

    #[test]
    fn validate_per_output_collapsed_width_too_small_errors() {
        let config = Config {
            bar: BarConfig::default(),
            modules: vec![],
            outputs: vec![OutputConfig {
                name: "DP-1".into(),
                enabled: true,
                collapsed_width: Some(1),
                expanded_width: None,
                modules: None,
                edge: None,
                expand: None,
                expand_trigger: None,
                hover_delay: None,
                expand_keybind: None,
                animation: None,
            }],
        };
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("DP-1"), "got: {msg}");
        assert!(msg.contains("collapsed-width"), "got: {msg}");
    }

    #[test]
    fn validate_per_output_expanded_less_than_collapsed_errors() {
        let config = Config {
            bar: BarConfig::default(),
            modules: vec![],
            outputs: vec![OutputConfig {
                name: "DP-1".into(),
                enabled: true,
                collapsed_width: Some(100),
                expanded_width: Some(50),
                modules: None,
                edge: None,
                expand: None,
                expand_trigger: None,
                hover_delay: None,
                expand_keybind: None,
                animation: None,
            }],
        };
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("DP-1"), "got: {msg}");
        assert!(msg.contains("expanded-width"), "got: {msg}");
    }

    #[test]
    fn validate_per_output_expanded_less_than_global_collapsed_errors() {
        // If only expanded-width is overridden, effective collapsed is the global value.
        // expanded < global_collapsed should error.
        let mut config = Config::default();
        config.bar.collapsed_width = 80;
        config.outputs = vec![OutputConfig {
            name: "DP-1".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: Some(50), // 50 < 80 → error
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("DP-1"), "got: {msg}");
        assert!(msg.contains("expanded-width"), "got: {msg}");
    }

    #[test]
    fn validate_per_output_valid_overrides_pass() {
        let config = Config {
            bar: BarConfig::default(),
            modules: vec![],
            outputs: vec![OutputConfig {
                name: "eDP-1".into(),
                enabled: true,
                collapsed_width: Some(60),
                expanded_width: Some(250),
                modules: None,
                edge: None,
                expand: None,
                expand_trigger: None,
                hover_delay: None,
                expand_keybind: None,
                animation: None,
            }],
        };
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn effective_bar_for_non_overridable_fields_always_inherit() {
        let mut config = Config::default();
        config.bar.expand = false;
        config.bar.hover_delay = 500;
        config.outputs = vec![OutputConfig {
            name: "eDP-1".into(),
            enabled: true,
            collapsed_width: Some(60),
            expanded_width: Some(250),
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("eDP-1");
        // Fields are now overridable; None means inherit from global
        assert!(!bar.expand);
        assert_eq!(bar.hover_delay, 500);
    }

    // --- Story 7.5: All bar params per-output override tests ---

    #[test]
    fn effective_bar_for_applies_edge_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: Some(Edge::Top),
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert!(matches!(bar.edge, Edge::Top));
        // other fields inherit global defaults
        assert_eq!(bar.collapsed_width, config.bar.collapsed_width);
    }

    #[test]
    fn effective_bar_for_applies_expand_override() {
        let mut config = Config::default();
        config.bar.expand = true; // global: expand on
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: Some(false), // per-output: expand off
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert!(!bar.expand);
        // global config unchanged
        let global_bar = config.effective_bar_for("unknown");
        assert!(global_bar.expand);
    }

    #[test]
    fn effective_bar_for_applies_expand_trigger_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: Some(ExpandTrigger::Hover),
            hover_delay: None,
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert_eq!(bar.expand_trigger, ExpandTrigger::Hover);
    }

    #[test]
    fn effective_bar_for_applies_hover_delay_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: Some(300),
            expand_keybind: None,
            animation: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert_eq!(bar.hover_delay, 300);
    }

    #[test]
    fn effective_bar_for_applies_expand_keybind_override() {
        let mut config = Config::default();
        config.bar.expand_keybind = Some("Super+e".into());
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: Some("Ctrl+b".into()),
            animation: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert_eq!(bar.expand_keybind, Some("Ctrl+b".into()));
        // global still has Super+e
        let global_bar = config.effective_bar_for("other");
        assert_eq!(global_bar.expand_keybind, Some("Super+e".into()));
    }

    #[test]
    fn effective_bar_for_inherits_expand_keybind_when_not_overridden() {
        let mut config = Config::default();
        config.bar.expand_keybind = Some("Super+e".into());
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None, // not overridden → inherit
            animation: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert_eq!(bar.expand_keybind, Some("Super+e".into()));
    }
}
