pub mod battery;
pub mod button;
pub mod clock;
pub mod focused_window;
pub mod network;
pub mod script;
pub mod tray;
pub mod view;
pub mod volume;
pub mod workspaces;

use std::collections::HashMap;

use crate::config::{
    BatteryConfig, ClockConfig, ModuleParams, NetworkConfig, TrayConfig, VolumeConfig,
    WorkspacesConfig,
};

/// Unique identifier for a module instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(pub String);

impl ModuleId {
    // Used in tests and by Stories 2.2+ when module instances are constructed from config.
    pub(crate) fn new(s: impl Into<String>) -> Self {
        ModuleId(s.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleKind {
    Battery,
    Button,
    Network,
    Volume,
    Clock,
    Script,
    Workspaces,
    WorkspaceItem,
    FocusedWindow,
    Tray,
    TrayIcon,
}

impl ModuleKind {
    pub(crate) const CONFIG_KINDS: &'static [ModuleKind] = _CONFIG_KINDS_FROM_DESCRIPTORS;

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ModuleKind::Battery => "battery",
            ModuleKind::Button => "button",
            ModuleKind::Network => "network",
            ModuleKind::Volume => "volume",
            ModuleKind::Clock => "clock",
            ModuleKind::Script => "script",
            ModuleKind::Workspaces => "workspaces",
            ModuleKind::WorkspaceItem => "workspace_item",
            ModuleKind::FocusedWindow => "focused-window",
            ModuleKind::Tray => "tray",
            ModuleKind::TrayIcon => "tray_icon",
        }
    }

    pub(crate) fn from_config_name(s: &str) -> Option<Self> {
        match s {
            "battery" => Some(ModuleKind::Battery),
            "button" => Some(ModuleKind::Button),
            "network" => Some(ModuleKind::Network),
            "volume" => Some(ModuleKind::Volume),
            "clock" => Some(ModuleKind::Clock),
            "script" => Some(ModuleKind::Script),
            "workspaces" => Some(ModuleKind::Workspaces),
            "workspace_item" => Some(ModuleKind::WorkspaceItem),
            "focused-window" => Some(ModuleKind::FocusedWindow),
            "tray" => Some(ModuleKind::Tray),
            "tray_icon" => Some(ModuleKind::TrayIcon),
            _ => None,
        }
    }
}

/// Zone placement for a module in the bar layout.
// Used in Story 2.2 when modules are placed into zones from config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zone {
    Top,
    Center,
    Bottom,
}

/// Per-module runtime state. Variants added per concrete module (Stories 2.3+).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ModuleState {
    /// Module is starting up; no data yet. Render as empty space — no spinner, no text.
    #[default]
    Loading,
    /// Module's data source is unavailable. Render as nothing — zero height, no gap left.
    Unavailable,
    Battery(battery::BatteryState),
    Network(network::NetworkState),
    Volume(volume::VolumeState),
    Clock(clock::ClockState),
    Script(script::ScriptState),
    Button(button::ButtonState),
    FocusedWindow(focused_window::FocusedWindowState),
    /// Surface output name passed to workspace_item modules so they can filter rendering.
    WorkspaceOutput(String),
}

/// Module data events routed through the Iced message bus.
#[derive(Debug, Clone)]
pub enum ModuleEvent {
    Clock(clock::ClockEvent),
    Script(script::ScriptEvent),
    Button(button::ButtonEvent),
}

/// Input event from pointer or keyboard interaction.
// Used in Story 2.2+ when `on_interaction` handlers are implemented.
#[derive(Debug, Clone)]
pub enum InteractionEvent {
    Clicked,
    RightClicked(iced::window::Id),
    Activated(iced::window::Id),
    ScrollUp,
    ScrollDown,
}

/// The core abstraction for all bar content modules.
///
/// # Subscription Contract
///
/// Every implementation must:
/// - Emit its first message within 1 second of subscription start, or emit `Unavailable`.
/// - Never panic — wrap all fallible operations in `match`/`if let`.
/// - Use `std::any::TypeId::of::<{Name}Subscription>()` as the subscription ID — no magic strings.
/// - Treat D-Bus service disappearance as `{Name}Event::Unavailable`, not a fatal error.
/// - Support reconnection: if a D-Bus service restarts, recover and resume emitting.
// Implemented in Stories 2.3+ when concrete modules are added.
pub trait Module: Send + Sync {
    fn id(&self) -> ModuleId;
    fn kind(&self) -> ModuleKind;
    /// Returns the module type name (e.g. `"clock"`, `"script"`).
    /// Used for type-based state routing; distinct from the instance name returned by `id()`.
    fn module_type(&self) -> &str {
        self.kind().as_str()
    }
    fn subscription(&self) -> iced::Subscription<crate::app::Message>;
    fn view_collapsed(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
    fn view_expanded(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
    /// View for horizontal bars (top/bottom). Modules render single-line, shrink-width content
    /// suited for a side-by-side row layout. Defaults to `view_expanded`.
    fn view_horizontal(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message> {
        self.view_expanded(state)
    }
    fn view_collapsed_with_style(
        &self,
        state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        self.view_collapsed(state)
    }
    fn view_expanded_with_style(
        &self,
        state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        self.view_expanded(state)
    }
    fn view_horizontal_with_style(
        &self,
        state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        self.view_horizontal(state)
    }
    /// View used when rendering this module inside a bar surface.
    ///
    /// `window` is the bar's window ID; modules that assign widget IDs (e.g. tray icons)
    /// must embed it so `FindBounds` only matches widgets in the correct bar.
    /// The default dispatches to the appropriate view method.
    fn view_for_bar(
        &self,
        state: &ModuleState,
        window: iced::window::Id,
        horizontal: bool,
        expanded: bool,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        let _ = window;
        if expanded {
            self.view_expanded_with_style(state, bar_style)
        } else if horizontal {
            self.view_horizontal_with_style(state, bar_style)
        } else {
            self.view_collapsed_with_style(state, bar_style)
        }
    }
    fn on_interaction(&self, _event: InteractionEvent) -> Option<crate::app::Message> {
        None
    }
    /// Update dynamic workspace item data without rebuilding module instances.
    /// Non-workspace modules ignore this hook.
    fn update_workspace(&mut self, _workspace: &niri_ipc::Workspace) {}
    /// For dynamically-expanded modules (e.g. tray icons), returns the config module name
    /// this instance was expanded from (e.g. `"tray"`). Used for zone and nav-list lookup.
    /// Returns `None` for all statically-configured modules.
    fn config_name(&self) -> Option<&str> {
        None
    }
}

/// Type alias for a module constructor function.
pub(crate) type ModuleConstructor = fn(&crate::config::ModuleConfig) -> Box<dyn Module>;

/// Type alias for a synthesized-params factory used when a built-in module is undeclared.
pub(crate) type SynthesizedParamsFactory = fn() -> ModuleParams;

macro_rules! config_module_descriptors {
    ($macro:ident) => {
        $macro! {
            (ModuleKind::Battery, "battery", battery_ctor, Some(battery_synthesized_params as SynthesizedParamsFactory)),
            (ModuleKind::Button, "button", button_ctor, None),
            (ModuleKind::Clock, "clock", clock_ctor, Some(clock_synthesized_params as SynthesizedParamsFactory)),
            (ModuleKind::FocusedWindow, "focused-window", focused_window_ctor, Some(focused_window_synthesized_params as SynthesizedParamsFactory)),
            (ModuleKind::Network, "network", network_ctor, Some(network_synthesized_params as SynthesizedParamsFactory)),
            (ModuleKind::Script, "script", script_ctor, None),
            (ModuleKind::Tray, "tray", tray_ctor, Some(tray_synthesized_params as SynthesizedParamsFactory)),
            (ModuleKind::Volume, "volume", volume_ctor, Some(volume_synthesized_params as SynthesizedParamsFactory)),
            (ModuleKind::Workspaces, "workspaces", workspaces_ctor, Some(workspaces_synthesized_params as SynthesizedParamsFactory)),
        }
    };
}

/// Typed metadata for a built-in config-facing module.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ModuleDescriptor {
    pub kind: ModuleKind,
    pub config_name: &'static str,
    pub constructor: ModuleConstructor,
    pub synthesized_params_factory: Option<SynthesizedParamsFactory>,
}

impl ModuleDescriptor {
    pub(crate) fn synthesized_params(&self) -> Option<ModuleParams> {
        self.synthesized_params_factory.map(|factory| factory())
    }
}

macro_rules! build_config_kinds {
    ($(($kind:expr, $config_name:expr, $ctor:ident, $factory:expr)),* $(,)?) => {
        &[$($kind),*]
    };
}

macro_rules! build_module_descriptors {
    ($(($kind:expr, $config_name:expr, $ctor:ident, $factory:expr)),* $(,)?) => {
        &[
            $(ModuleDescriptor {
                kind: $kind,
                config_name: $config_name,
                constructor: $ctor,
                synthesized_params_factory: $factory,
            }),*
        ]
    };
}

pub(crate) const MODULE_DESCRIPTORS: &[ModuleDescriptor] =
    config_module_descriptors!(build_module_descriptors);

const _CONFIG_KINDS_FROM_DESCRIPTORS: &[ModuleKind] =
    config_module_descriptors!(build_config_kinds);

/// Build the static module registry mapping config name → constructor.
pub(crate) fn build_registry() -> HashMap<&'static str, ModuleConstructor> {
    MODULE_DESCRIPTORS
        .iter()
        .map(|descriptor| {
            debug_assert_eq!(descriptor.kind.as_str(), descriptor.config_name);
            (descriptor.config_name, descriptor.constructor)
        })
        .collect()
}

fn network_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    let (icons, format) = match &mc.params {
        ModuleParams::Network(cfg) => (cfg.icons.clone(), cfg.format.clone()),
        _ => {
            let default = NetworkConfig::default();
            (default.icons, default.format)
        }
    };
    Box::new(network::NetworkModule::new(
        ModuleId::new(&mc.name),
        icons,
        format,
    ))
}

fn battery_synthesized_params() -> ModuleParams {
    ModuleParams::Battery(BatteryConfig::default())
}

fn clock_synthesized_params() -> ModuleParams {
    ModuleParams::Clock(ClockConfig::default())
}

fn focused_window_synthesized_params() -> ModuleParams {
    ModuleParams::None
}

fn network_synthesized_params() -> ModuleParams {
    ModuleParams::Network(NetworkConfig::default())
}

fn tray_synthesized_params() -> ModuleParams {
    ModuleParams::Tray(TrayConfig)
}

fn volume_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    let (step, states, icons, format) = match &mc.params {
        ModuleParams::Volume(cfg) => (
            cfg.step,
            cfg.states.clone(),
            cfg.icons.clone(),
            cfg.format.clone(),
        ),
        _ => {
            let default = VolumeConfig::default();
            (default.step, default.states, default.icons, default.format)
        }
    };
    Box::new(volume::VolumeModule::new(
        ModuleId::new(&mc.name),
        step,
        states,
        icons,
        format,
    ))
}

fn battery_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    let (threshold, states, icons, format) = match &mc.params {
        ModuleParams::Battery(cfg) => (
            cfg.critical_threshold,
            cfg.states.clone(),
            cfg.icons.clone(),
            cfg.format.clone(),
        ),
        _ => {
            let default = BatteryConfig::default();
            (
                default.critical_threshold,
                default.states,
                default.icons,
                default.format,
            )
        }
    };
    Box::new(battery::BatteryModule::new(
        ModuleId::new(&mc.name),
        threshold,
        states,
        icons,
        format,
    ))
}

fn button_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    match &mc.params {
        ModuleParams::Button(cfg) => Box::new(button::ButtonModule::new(
            ModuleId::new(&mc.name),
            cfg.label.clone(),
            cfg.icon.clone(),
            cfg.on_click.clone(),
            cfg.format.clone(),
        )),
        _ => panic!("button_ctor called with non-Button params — this is a programming error"),
    }
}

fn clock_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    let (time_fmt, date_fmt, format) = match &mc.params {
        ModuleParams::Clock(cfg) => (
            cfg.time_format.clone(),
            cfg.date_format.clone(),
            cfg.format.clone(),
        ),
        _ => {
            let d = ClockConfig::default();
            (d.time_format, d.date_format, d.format)
        }
    };
    Box::new(clock::ClockModule::new(
        ModuleId::new(&mc.name),
        time_fmt,
        date_fmt,
        format,
    ))
}

fn script_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    match &mc.params {
        ModuleParams::Script(cfg) => Box::new(script::ScriptModule::new(
            ModuleId::new(&mc.name),
            cfg.command.clone(),
            cfg.interval,
            cfg.format.clone(),
        )),
        _ => panic!("script_ctor called with non-Script params — this is a programming error"),
    }
}

fn workspaces_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    let config = match &mc.params {
        ModuleParams::Workspaces(c) => c.clone(),
        _ => crate::config::WorkspacesConfig::default(),
    };
    Box::new(workspaces::WorkspacesModule::new(
        ModuleId::new(&mc.name),
        config,
    ))
}

fn focused_window_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    let format = match &mc.params {
        ModuleParams::FocusedWindow(cfg) => cfg.format.clone(),
        _ => crate::config::FocusedWindowConfig::default().format,
    };
    Box::new(focused_window::FocusedWindowModule::new(
        ModuleId::new(&mc.name),
        format,
    ))
}

fn tray_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    Box::new(tray::TrayModule::new(ModuleId::new(&mc.name)))
}

fn volume_synthesized_params() -> ModuleParams {
    ModuleParams::Volume(VolumeConfig::default())
}

fn workspaces_synthesized_params() -> ModuleParams {
    ModuleParams::Workspaces(WorkspacesConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn module_id_new_stores_string() {
        assert_eq!(ModuleId::new("clock").0, "clock");
    }

    #[test]
    fn module_id_equality() {
        assert_eq!(ModuleId::new("clock"), ModuleId::new("clock"));
        assert_ne!(ModuleId::new("clock"), ModuleId::new("battery"));
    }

    #[test]
    fn interaction_event_activated_variant_exists() {
        let _ = InteractionEvent::Activated;
    }

    #[test]
    fn module_state_default_is_loading() {
        assert!(matches!(ModuleState::default(), ModuleState::Loading));
    }

    #[test]
    fn module_kind_round_trips_known_config_names() {
        for (kind, name) in [
            (ModuleKind::Battery, "battery"),
            (ModuleKind::Button, "button"),
            (ModuleKind::Clock, "clock"),
            (ModuleKind::FocusedWindow, "focused-window"),
            (ModuleKind::Network, "network"),
            (ModuleKind::Script, "script"),
            (ModuleKind::Tray, "tray"),
            (ModuleKind::Volume, "volume"),
            (ModuleKind::Workspaces, "workspaces"),
            (ModuleKind::WorkspaceItem, "workspace_item"),
            (ModuleKind::TrayIcon, "tray_icon"),
        ] {
            assert_eq!(kind.as_str(), name);
            assert_eq!(ModuleKind::from_config_name(name), Some(kind));
        }
    }

    #[test]
    fn every_config_facing_kind_has_exactly_one_descriptor() {
        let expected = HashSet::from([
            ModuleKind::Battery,
            ModuleKind::Button,
            ModuleKind::Clock,
            ModuleKind::FocusedWindow,
            ModuleKind::Network,
            ModuleKind::Script,
            ModuleKind::Tray,
            ModuleKind::Volume,
            ModuleKind::Workspaces,
        ]);
        let descriptor_kinds: Vec<_> = MODULE_DESCRIPTORS.iter().map(|d| d.kind).collect();
        let actual = descriptor_kinds.iter().copied().collect::<HashSet<_>>();

        assert_eq!(descriptor_kinds.len(), actual.len());
        assert_eq!(actual, expected);
    }

    #[test]
    fn descriptor_config_names_round_trip_through_module_kind() {
        for descriptor in MODULE_DESCRIPTORS {
            assert_eq!(descriptor.kind.as_str(), descriptor.config_name);
            assert_eq!(
                ModuleKind::from_config_name(descriptor.config_name),
                Some(descriptor.kind)
            );
        }
    }

    #[test]
    fn descriptor_synthesized_params_cover_known_builtins() {
        let clock = MODULE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.config_name == "clock")
            .expect("clock descriptor should exist");
        assert!(matches!(
            clock.synthesized_params(),
            Some(ModuleParams::Clock(_))
        ));

        let button = MODULE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.config_name == "button")
            .expect("button descriptor should exist");
        assert!(button.synthesized_params().is_none());
    }

    #[test]
    fn dynamic_only_kinds_are_excluded_from_config_descriptors() {
        let descriptor_kinds: HashSet<_> = MODULE_DESCRIPTORS.iter().map(|d| d.kind).collect();
        assert!(!descriptor_kinds.contains(&ModuleKind::WorkspaceItem));
        assert!(!descriptor_kinds.contains(&ModuleKind::TrayIcon));
    }

    #[test]
    fn build_registry_contains_clock() {
        let registry = build_registry();
        assert!(registry.contains_key("clock"));
    }

    #[test]
    fn module_id_hashable() {
        let mut map = HashMap::new();
        map.insert(ModuleId::new("clock"), ModuleState::Loading);
        assert!(map.contains_key(&ModuleId::new("clock")));
        assert!(!map.contains_key(&ModuleId::new("battery")));
    }
}
