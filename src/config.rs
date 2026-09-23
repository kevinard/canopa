mod bar;
mod defaults;
mod error;
mod module;
mod parse;
pub mod style;
mod template;
pub mod tooltip_defaults;
mod validate;

pub use bar::{
    BarConfig, BarLayer, BarModules, Edge, OutputTarget, PanelAnimationConfig, PanelConfig,
    PanelModules,
};
pub(crate) use error::{
    ConfigDiagnostic, ConfigDiagnosticKind, ConfigError, ConfigResult, ConfigSource,
    ConfigSourceName,
};
#[allow(unused_imports)]
pub use module::{
    ActionBinding, ActionTrigger, ActionsConfig, BatteryConfig, ButtonConfig, ClockConfig,
    CommandPolicy, ConfiguredCommand, FocusedWindowConfig, IconRulesConfig, IconThreshold,
    MenuConfig, MenuItemConfig, MenuSourceConfig, MenuSourceKind, ModuleActionsConfig,
    ModuleConfig, ModuleParams, NetworkConfig, PanelAction, PanelProviderConfig, ScriptConfig,
    StateRule, StatesConfig, TrayConfig, VolumeConfig, WorkspaceLabelConfig, WorkspacesConfig,
};
pub(crate) use module::{CommandPolicyError, ConfiguredCommandSpec};
#[allow(unused_imports)]
pub use template::{
    ContentRotation, FormatConfig, FormatMode, FormatTemplate, ResolvedContentRotation,
    StyledSegment, TextTemplate, TooltipSetting,
};
#[cfg(test)]
pub(crate) use validate::validate;
pub(crate) use validate::validate_with_source;

#[allow(unused_imports)]
pub use style::{ColorValue, ResolvedStyle, SegmentStyles, Style, ThemeToken, Weight};

pub type FormatValue<'a> = (&'static str, std::borrow::Cow<'a, str>);

fn is_known_config_module_type(module_type: &str) -> bool {
    crate::modules::ModuleKind::from_config_name(module_type).is_some()
}

fn module_descriptor_for_config_name(
    config_name: &str,
) -> Option<&'static crate::modules::ModuleDescriptor> {
    crate::modules::MODULE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.config_name == config_name)
}

fn bar_module_placements(bar: &BarConfig) -> [(crate::modules::Zone, &[String]); 3] {
    [
        (crate::modules::Zone::Start, &bar.modules.start),
        (crate::modules::Zone::Center, &bar.modules.center),
        (crate::modules::Zone::End, &bar.modules.end),
    ]
}

/// Parsed configuration for niribar.
/// Fully populated before any surface is created (AR7).
#[derive(Debug, Clone)]
pub struct Config {
    /// All named bar sections. At least one is always present (falls back to defaults when
    /// no `bar` sections appear in the config file).
    pub bars: Vec<BarConfig>,
    pub modules: Vec<ModuleConfig>,
    pub menus: Vec<MenuConfig>,
    pub panel_providers: Vec<PanelProviderConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bars: vec![BarConfig::default()],
            modules: vec![],
            menus: vec![],
            panel_providers: vec![],
        }
    }
}

impl Config {
    /// Return all bar configs whose Output Target matches the given Output name.
    #[cfg(test)]
    pub fn bars_for_output(&self, output_name: &str) -> Vec<&BarConfig> {
        self.bars
            .iter()
            .filter(|b| b.matches_output(output_name))
            .collect()
    }
}

/// Return the path to the user config file, following XDG conventions.
///
/// Returns an error if neither `XDG_CONFIG_HOME` nor `HOME` is set or non-empty.
fn config_file_path() -> ConfigResult<std::path::PathBuf> {
    let base = match std::env::var("XDG_CONFIG_HOME") {
        Ok(value) => std::path::PathBuf::from(value),
        Err(_) => {
            let home = std::env::var("HOME").map_err(|_| {
                ConfigError::message(
                    ConfigSource::new(ConfigSourceName::EmbeddedDefault, ""),
                    "neither XDG_CONFIG_HOME nor HOME environment variable is set",
                )
            })?;
            if home.is_empty() {
                return Err(ConfigError::message(
                    ConfigSource::new(ConfigSourceName::EmbeddedDefault, ""),
                    "HOME environment variable is empty",
                ));
            }
            std::path::PathBuf::from(home).join(".config")
        }
    };
    Ok(base.join("niribar").join("config.kdl"))
}

/// Load configuration from disk or fall back to the embedded default.
///
/// Config is fully parsed before returning — no lazy evaluation (AR7).
pub fn load() -> ConfigResult<Config> {
    let path = config_file_path()?;
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => defaults::DEFAULT_CONFIG.to_string(),
        Err(e) => {
            return Err(ConfigError::Io { path, source: e });
        }
    };
    let source_name = if path.exists() {
        ConfigSourceName::UserFile(path)
    } else {
        ConfigSourceName::EmbeddedDefault
    };
    let validation_source_name = source_name.clone();
    let config = parse::parse_input(parse::ConfigInput {
        name: source_name,
        text: &content,
    })?;
    validate_with_source(
        &config,
        ConfigSource::new(validation_source_name, content.clone()),
    )?;
    Ok(config)
}

pub mod states {
    pub const NETWORK: &[&str] = &["full", "limited", "none"];
    pub const WORKSPACES: &[&str] = &["focused", "active", "inactive"];
    pub const EMPTY: &[&str] = &[];
}

#[cfg(test)]
mod tests;
