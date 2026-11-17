use std::collections::HashMap;

use super::{ContentRotation, FormatConfig, FormatTemplate, TooltipSetting};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfiguredCommand {
    spec: ConfiguredCommandSpec,
    timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ConfiguredCommandSpec {
    Shell(String),
    Argv { program: String, args: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandPolicyError {
    ShellDisabled,
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

impl ConfiguredCommand {
    pub(crate) fn shell(
        command: String,
        policy: &CommandPolicy,
    ) -> Result<Self, CommandPolicyError> {
        if !policy.allow_shell {
            return Err(CommandPolicyError::ShellDisabled);
        }
        Ok(Self {
            spec: ConfiguredCommandSpec::Shell(command),
            timeout_ms: policy.action_timeout_ms,
        })
    }

    pub(crate) fn argv(program: String, args: Vec<String>, policy: &CommandPolicy) -> Self {
        Self {
            spec: ConfiguredCommandSpec::Argv { program, args },
            timeout_ms: policy.action_timeout_ms,
        }
    }

    pub(crate) fn spec(&self) -> &ConfiguredCommandSpec {
        &self.spec
    }

    pub(crate) fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    #[cfg(test)]
    pub(crate) fn shell_for_test(command: impl Into<String>) -> Self {
        Self {
            spec: ConfiguredCommandSpec::Shell(command.into()),
            timeout_ms: CommandPolicy::default().action_timeout_ms,
        }
    }

    #[cfg(test)]
    pub(crate) fn shell_with_timeout_for_test(command: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            spec: ConfiguredCommandSpec::Shell(command.into()),
            timeout_ms,
        }
    }

    #[cfg(test)]
    pub(crate) fn argv_for_test(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            spec: ConfiguredCommandSpec::Argv {
                program: program.into(),
                args,
            },
            timeout_ms: CommandPolicy::default().action_timeout_ms,
        }
    }

    #[cfg(test)]
    pub(crate) fn argv_with_timeout_for_test(
        program: impl Into<String>,
        args: Vec<String>,
        timeout_ms: u64,
    ) -> Self {
        Self {
            spec: ConfiguredCommandSpec::Argv {
                program: program.into(),
                args,
            },
            timeout_ms,
        }
    }
}

/// Format strings for the clock module (AC: #4, #5). `None` follows the
/// language's time and date formats; an explicit Chrono pattern is the
/// person's own and is used as written.
#[derive(Debug, Clone, PartialEq)]
pub struct ClockConfig {
    pub time_format: Option<String>,
    pub date_format: Option<String>,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            time_format: None,
            date_format: None,
            format: FormatConfig::default(),
            content_rotation: ContentRotation::None,
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Command and polling interval for the script module.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptConfig {
    pub command: ConfiguredCommand,
    pub interval: u64, // seconds between executions
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

/// Configuration for the button module — all fields required.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonConfig {
    pub label: String,
    pub icon: String,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
}

/// Per-workspace label and icon for the workspaces module.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceLabelConfig {
    pub label: Option<String>,
    pub icon: Option<String>,
}

/// Configuration for the workspaces module — optional label/icon per workspace number.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkConfig {
    pub icons: IconRulesConfig,
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub tooltip: TooltipSetting,
    pub default_provider: Option<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            icons: IconRulesConfig::default(),
            format: FormatConfig::default(),
            content_rotation: ContentRotation::None,
            tooltip: TooltipSetting::Default,
            default_provider: Some("wifi".into()),
        }
    }
}

/// Configuration for the tray module.
#[derive(Debug, Clone, PartialEq)]
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

/// Configuration for the taskbar: pinned applications, then running ones.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskbarConfig {
    /// Desktop IDs shown even when not running, in this order. Missing
    /// applications are skipped.
    pub pinned: Vec<String>,
    pub tooltip: TooltipSetting,
}

/// Configuration for the minimap module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapConfig {
    /// The strip's thickness across the bar, in logical pixels. A screen's
    /// height is drawn at this size and widths keep its aspect.
    pub size: u8,
    /// The strip's longest extent along the bar; wider strips shrink to fit.
    /// Zero disables the limit.
    pub max_length: u32,
}

impl Default for MinimapConfig {
    fn default() -> Self {
        Self {
            size: 18,
            max_length: 240,
        }
    }
}

/// Configuration for the focused-window module.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusedWindowConfig {
    pub format: FormatConfig,
    pub content_rotation: ContentRotation,
    pub fallback_icon: String,
    /// Maximum displayed title graphemes, including the middle ellipsis. Zero disables the limit.
    pub max_length: u32,
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
            max_length: 50,
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Configuration for the volume module — scroll step in percent.
#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaConfig {
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
    pub content_rotation: ContentRotation,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NotificationsConfig {
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IdleBackend {
    #[default]
    Wayland,
    Stasis,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IdleInhibitorConfig {
    pub backend: IdleBackend,
    pub icons: IconRulesConfig,
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
    pub content_rotation: ContentRotation,
}

/// Module-specific parameters. Extensible for Stories 2.4–2.5.
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleParams {
    IdleInhibitor(IdleInhibitorConfig),
    Notifications(NotificationsConfig),
    Media(MediaConfig),
    #[cfg(test)]
    None,
    Battery(BatteryConfig),
    Network(NetworkConfig),
    Volume(VolumeConfig),
    Clock(ClockConfig),
    Script(ScriptConfig),
    Button(ButtonConfig),
    Workspaces(WorkspacesConfig),
    Tray(TrayConfig),
    Taskbar(TaskbarConfig),
    FocusedWindow(FocusedWindowConfig),
    Minimap(MinimapConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionTrigger {
    Click,
    RightClick,
    Key(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelAction {
    Command(ConfiguredCommand),
    /// An argv application launch, owned by the compositor/session rather than the bar.
    Launch(Vec<String>),
    /// Suspend, hibernate, reboot or power off through logind.
    Session(crate::session::SessionAction),
    /// An action of the compositor, sent over the Shell's own Niri IPC socket
    /// instead of running a `niri` executable.
    Compositor(CompositorAction),
    OpenPanel,
    TogglePanel,
    OpenBarPanel,
    ToggleBarPanel,
    ClosePanel,
    OpenMenu(String),
    ToggleMenu(String),
    OpenProvider(String),
}

/// Compositor actions a menu item or module click can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorAction {
    /// `session "log-out"`: end the session. The menu asking for confirmation
    /// is the Shell's, so Niri's own dialog is skipped.
    LogOut,
    /// `toggle-overview`.
    ToggleOverview,
}

impl CompositorAction {
    pub fn niri_action(self) -> niri_ipc::Action {
        match self {
            Self::LogOut => niri_ipc::Action::Quit {
                skip_confirmation: true,
            },
            Self::ToggleOverview => niri_ipc::Action::ToggleOverview {},
        }
    }
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
    /// The title shown instead of the name, from `title` or `title-id`.
    pub title: Option<String>,
    pub show_icons: bool,
    pub prompt: Option<String>,
    pub source: Option<MenuSourceConfig>,
    pub items: Vec<MenuItemConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuSourceKind {
    InstalledApps,
    Launcher(crate::launcher::Config),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelProviderConfig {
    pub name: String,
    pub command: ConfiguredCommand,
    pub protocol: crate::external_panel::ProviderProtocol,
}

/// A single module entry from the config, specifying name, type, and module params.
/// Zone placement is determined by `bar.modules`, not by this struct.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleConfig {
    /// Instance name — unique identifier for this module instance (e.g. "clock", "work-status").
    pub name: String,
    /// Module type — determines which module implementation to use (e.g. "clock", "script").
    /// Inferred from `name` when the name matches a known module type; otherwise explicit.
    pub module_type: String,
    pub params: ModuleParams,
    pub actions: ModuleActionsConfig,
}
