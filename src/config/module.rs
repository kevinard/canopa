use std::collections::HashMap;

use super::{ContentRotation, FormatConfig, FormatTemplate, TooltipSetting};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CommandConfig {
    Shell(String),
    Argv { program: String, args: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPolicy {
    pub allow_shell: bool,
    pub action_timeout_ms: u64,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        CommandPolicy {
            allow_shell: true,
            action_timeout_ms: 3000,
        }
    }
}

/// Format strings for the clock module (AC: #4, #5).
#[derive(Debug, Clone)]
pub struct ClockConfig {
    pub time_format: String,
    pub date_format: String,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            time_format: String::from("%H:%M"),
            date_format: String::from("%a %d %b"),
            format: FormatConfig::default(),
            content_rotation: ContentRotation::None,
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Command and polling interval for the script module.
#[derive(Debug, Clone)]
pub struct ScriptConfig {
    pub command: CommandConfig,
    pub interval: u64, // seconds between executions
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

/// Configuration for the button module — all fields required.
#[derive(Debug, Clone)]
pub struct ButtonConfig {
    pub label: String,
    pub icon: String,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

/// Per-workspace label and icon for the workspaces module.
#[derive(Debug, Clone)]
pub struct WorkspaceLabelConfig {
    pub label: Option<String>,
    pub icon: Option<String>,
}

/// Configuration for the workspaces module — optional label/icon per workspace number.
#[derive(Debug, Clone)]
pub struct WorkspacesConfig {
    /// Maps workspace number (1-based: ws.idx + 1) to label/icon config.
    /// Absent key → workspace shows number only.
    pub workspaces: std::collections::HashMap<u32, WorkspaceLabelConfig>,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        WorkspacesConfig {
            workspaces: std::collections::HashMap::default(),
            format: FormatConfig::default(),
            content_rotation: ContentRotation::None,
            tooltip: TooltipSetting::Default,
        }
    }
}

/// A single named state with an upper-bound threshold.
/// `value <= threshold` matches; the smallest matching threshold wins.
#[derive(Debug, Clone, PartialEq)]
pub struct StateRule {
    pub name: String,
    pub threshold: u8,
}

/// Ordered list of named state rules sorted by threshold ascending.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatesConfig {
    pub rules: Vec<StateRule>,
}

impl StatesConfig {
    /// Smallest-threshold-that-covers-`value` lookup. Falls back to the
    /// highest-threshold rule when `value` exceeds every threshold.
    pub fn resolve(&self, value: f64) -> Option<&str> {
        let v = value.clamp(0.0, 100.0).round() as u8;
        self.rules
            .iter()
            .find(|rule| rule.threshold >= v)
            .or_else(|| self.rules.last())
            .map(|rule| rule.name.as_str())
    }

    pub fn names(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.name.as_str()).collect()
    }

    /// Replace the rule whose name matches `name`, or insert a new one.
    /// Re-sorts by threshold ascending.
    pub fn upsert(&mut self, name: String, threshold: u8) {
        if let Some(existing) = self.rules.iter_mut().find(|r| r.name == name) {
            existing.threshold = threshold;
        } else {
            self.rules.push(StateRule { name, threshold });
        }
        self.rules.sort_by_key(|r| r.threshold);
    }

    /// Threshold for the rule named `name`, if any.
    pub fn threshold_of(&self, name: &str) -> Option<u8> {
        self.rules
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.threshold)
    }

    pub fn battery_default() -> Self {
        StatesConfig {
            rules: vec![
                StateRule {
                    name: "critical".into(),
                    threshold: 15,
                },
                StateRule {
                    name: "low".into(),
                    threshold: 30,
                },
                StateRule {
                    name: "normal".into(),
                    threshold: 100,
                },
            ],
        }
    }

    pub fn volume_default() -> Self {
        StatesConfig {
            rules: vec![StateRule {
                name: "unmuted".into(),
                threshold: 100,
            }],
        }
    }
}

/// Configuration for the battery module.
#[derive(Debug, Clone)]
pub struct BatteryConfig {
    /// Percentage at or below which the critical badge and red icon appear.
    /// Derived from `states` (the threshold of the "critical" rule). Falls back
    /// to 15 when the user removed the "critical" state entirely.
    pub critical_threshold: u8,
    pub states: StatesConfig,
    pub icons: IconRulesConfig,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

impl Default for BatteryConfig {
    fn default() -> Self {
        let states = StatesConfig::battery_default();
        BatteryConfig {
            critical_threshold: states.threshold_of("critical").unwrap_or(15),
            states,
            icons: IconRulesConfig::default(),
            format: FormatConfig::default(),
            content_rotation: ContentRotation::None,
            tooltip: TooltipSetting::Default,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IconRulesConfig {
    pub states: HashMap<String, String>,
    pub thresholds: Vec<IconThreshold>,
}

impl IconRulesConfig {
    pub fn icon_for_state(&self, state: &str) -> Option<&str> {
        self.states.get(state).map(String::as_str)
    }

    pub fn icon_for_value(&self, value: f64) -> Option<&str> {
        self.thresholds
            .iter()
            .rev()
            .find(|threshold| value >= threshold.at)
            .map(|threshold| threshold.icon.as_str())
    }

    pub fn icon_for_state_or_value(&self, state: Option<&str>, value: Option<f64>) -> Option<&str> {
        state
            .and_then(|state| self.icon_for_state(state))
            .or_else(|| value.and_then(|value| self.icon_for_value(value)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IconThreshold {
    pub at: f64,
    pub icon: String,
}

/// Configuration for the network module.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub icons: IconRulesConfig,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            icons: IconRulesConfig::default(),
            format: FormatConfig::default(),
            content_rotation: ContentRotation::None,
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Configuration for the tray module.
#[derive(Debug, Clone)]
pub struct TrayConfig {
    pub tooltip: TooltipSetting,
    pub menu_search: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        TrayConfig {
            tooltip: TooltipSetting::Default,
            menu_search: false,
        }
    }
}

/// Configuration for the focused-window module.
#[derive(Debug, Clone)]
pub struct FocusedWindowConfig {
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub fallback_icon: String,
    pub tooltip: TooltipSetting,
}

impl Default for FocusedWindowConfig {
    fn default() -> Self {
        FocusedWindowConfig {
            format: FormatConfig {
                collapsed: Some(
                    FormatTemplate::parse(
                        &["{icon}", "{title}"],
                        &["icon", "value", "title", "app-id"],
                        "focused-window.format",
                    )
                    .expect("focused-window default format is valid"),
                ),
                expanded: None,
                horizontal: None,
            },
            content_rotation: ContentRotation::Auto,
            fallback_icon: "□".into(),
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Configuration for the volume module — scroll step in percent.
#[derive(Debug, Clone)]
pub struct VolumeConfig {
    /// Volume adjustment step per scroll event (percent, 1–100). Default: 5.
    pub step: u8,
    pub states: StatesConfig,
    pub icons: IconRulesConfig,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

impl Default for VolumeConfig {
    fn default() -> Self {
        VolumeConfig {
            step: 5,
            states: StatesConfig::volume_default(),
            icons: IconRulesConfig::default(),
            format: FormatConfig::default(),
            content_rotation: ContentRotation::None,
            tooltip: TooltipSetting::Default,
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionTrigger {
    Click,
    RightClick,
    Key(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelAction {
    Command(CommandConfig),
    OpenPanel,
    TogglePanel,
    ClosePanel,
    OpenMenu(String),
    ToggleMenu(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionsConfig {
    pub bindings: Vec<ActionBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBinding {
    pub trigger: ActionTrigger,
    pub action: PanelAction,
}

impl ActionsConfig {
    pub fn action_for(&self, trigger: &ActionTrigger) -> Option<&PanelAction> {
        self.bindings
            .iter()
            .find(|binding| &binding.trigger == trigger)
            .map(|binding| &binding.action)
    }
}

pub type ModuleActionsConfig = ActionsConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuConfig {
    pub name: String,
    pub show_icons: bool,
    pub prompt: Option<String>,
    pub source: Option<MenuSourceConfig>,
    pub items: Vec<MenuItemConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuSourceKind {
    InstalledApps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSourceConfig {
    pub kind: MenuSourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemConfig {
    pub label: String,
    pub icon: Option<String>,
    pub keywords: Vec<String>,
    pub action: Option<PanelAction>,
    pub children: Vec<MenuItemConfig>,
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
    pub actions: ModuleActionsConfig,
}
