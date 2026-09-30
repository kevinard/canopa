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
    BarConfig, BarLayer, BarModules, DrawerMode, Edge, OutputTarget, PanelAnimationConfig,
    PanelConfig, PanelModules, PanelPresentation,
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
    ModuleConfig, ModuleParams, NetworkConfig, NotificationsConfig, PanelAction,
    PanelProviderConfig, ScriptConfig, StateRule, StatesConfig, TrayConfig, VolumeConfig,
    WorkspaceLabelConfig, WorkspacesConfig,
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

/// Parsed configuration for canopa-shell.
/// Fully populated before any surface is created (AR7).
#[derive(Debug, Clone)]
pub struct Config {
    pub appearance: canopa_theme::Appearance,
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
            appearance: Default::default(),
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

/// Prepare a complete configuration before creating surfaces. Live reload is not supported.
pub fn load(
    request: &canopa_config::Request,
    show_source: bool,
) -> ConfigResult<(Config, canopa_config::SourceInfo)> {
    let input = canopa_config::Input::load(
        request,
        canopa_config::Component::Shell,
        defaults::DEFAULT_CONFIG,
    )
    .map_err(ConfigError::Intake)?;
    let document = input
        .select(canopa_config::Component::Shell, defaults::DEFAULT_CONFIG)
        .map_err(|error| {
            if show_source
                && error.code == canopa_config::Code::Syntax
                && let Err(detail) = input.text().parse::<kdl::KdlDocument>()
            {
                let name = input
                    .path()
                    .map(|path| ConfigSourceName::UserFile(path.into()))
                    .unwrap_or(ConfigSourceName::EmbeddedDefault);
                return ConfigError::from_kdl(ConfigSource::new(name, input.text()), detail);
            }
            ConfigError::Intake(error)
        })?;
    let source_name = if document.info().defaults {
        ConfigSourceName::EmbeddedDefault
    } else {
        document
            .info()
            .path
            .clone()
            .map(ConfigSourceName::UserFile)
            .unwrap_or(ConfigSourceName::EmbeddedDefault)
    };
    let source = ConfigSource::new(source_name, document.text());
    let mut config = parse::parse_document(document.selected(), source.clone())?;
    config.appearance = document.appearance();
    validate_with_source(&config, source)?;
    Ok((config, document.info().clone()))
}

pub mod states {
    pub const NETWORK: &[&str] = &["full", "limited", "none", "unknown", "connected", "portal"];
    pub const WORKSPACES: &[&str] = &["focused", "active", "inactive"];
    pub const EMPTY: &[&str] = &[];
}

#[cfg(test)]
mod tests;
