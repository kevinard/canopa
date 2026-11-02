pub mod battery;
pub mod button;
pub mod clock;
pub mod data_source;
pub mod detail;
pub mod dynamic;
pub mod focused_window;
pub mod idle_inhibitor;
pub mod interaction;
pub mod media;
pub mod network;
pub mod notifications;
pub mod render;
pub mod script;
pub mod tray;
pub mod view;
pub mod volume;
pub mod workspaces;

use std::{collections::HashMap, sync::Arc};

use crate::config::{
    BatteryConfig, ClockConfig, ModuleParams, NetworkConfig, TrayConfig, VolumeConfig,
    WorkspacesConfig,
};

pub type ModuleDetailSession = Box<dyn std::any::Any + Send>;

#[derive(Clone)]
pub struct ModuleDetailMessage(Arc<dyn std::any::Any + Send + Sync>);

impl ModuleDetailMessage {
    pub fn new<T: Send + Sync + 'static>(message: T) -> Self {
        Self(Arc::new(message))
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.0.downcast_ref::<T>()
    }
}

impl std::fmt::Debug for ModuleDetailMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleDetailMessage")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub enum ModuleDetailAppEffect {
    /// Request shared app infrastructure such as command execution, panel
    /// lifecycle, menu opening, or app-owned process launch policy.
    PanelAction(crate::config::PanelAction),
    /// Request closing the active Panel without making the Module know how
    /// Panel windows are tracked.
    ClosePanel,
}

#[derive(Debug, Clone)]
pub enum ModuleDetailEffect {
    /// App-owned work that must run through centralized app infrastructure.
    App(ModuleDetailAppEffect),
    /// Module-owned work for future runtime adapters, such as connecting to a
    /// network or switching an audio source.
    Module(ModuleDetailMessage),
}

#[derive(Debug, Clone, Default)]
pub struct ModuleDetailUpdate {
    pub effects: Vec<ModuleDetailEffect>,
}

/// Structural identity: configured names and runtime children occupy separate namespaces.
/// Cloning an identity shares its immutable allocation, including in layout and state indexes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(Arc<ModuleIdentity>);

#[derive(Debug, PartialEq, Eq, Hash)]
enum ModuleIdentity {
    Configured(String),
    Child { parent: String, key: ModuleChildKey },
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum ModuleChildKey {
    Workspace(u64),
    Tray(String),
}

impl ModuleId {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self(Arc::new(ModuleIdentity::Configured(name.into())))
    }

    pub(crate) fn workspace(parent: &str, workspace: u64) -> Self {
        Self(Arc::new(ModuleIdentity::Child {
            parent: parent.into(),
            key: ModuleChildKey::Workspace(workspace),
        }))
    }

    pub(crate) fn tray(parent: &str, runtime_id: &str) -> Self {
        Self(Arc::new(ModuleIdentity::Child {
            parent: parent.into(),
            key: ModuleChildKey::Tray(runtime_id.into()),
        }))
    }

    pub(crate) fn configured_name(&self) -> &str {
        match self.0.as_ref() {
            ModuleIdentity::Configured(name) => name,
            ModuleIdentity::Child { parent, .. } => parent,
        }
    }

    pub(crate) fn parent_name(&self) -> Option<&str> {
        match self.0.as_ref() {
            ModuleIdentity::Configured(_) => None,
            ModuleIdentity::Child { parent, .. } => Some(parent),
        }
    }

    // Historical human-readable labels are useful in display assertions, never as identity.
    #[cfg(test)]
    pub(crate) fn display_name(&self) -> String {
        match self.0.as_ref() {
            ModuleIdentity::Configured(name) => name.clone(),
            ModuleIdentity::Child {
                parent,
                key: ModuleChildKey::Workspace(id),
            } => {
                if parent == "workspaces" {
                    format!("workspace:{id}")
                } else {
                    format!("workspace:{parent}:{id}")
                }
            }
            ModuleIdentity::Child {
                parent,
                key: ModuleChildKey::Tray(id),
            } => {
                if parent == "tray" {
                    format!("tray:{id}")
                } else {
                    format!("tray:{parent}:{id}")
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleKind {
    Media,
    IdleInhibitor,
    Battery,
    Button,
    Network,
    Notifications,
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
    pub(crate) fn as_str(self) -> &'static str {
        self.config_name().unwrap_or_else(|| self.runtime_name())
    }

    pub(crate) fn config_name(self) -> Option<&'static str> {
        MODULE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.kind == self)
            .map(|descriptor| descriptor.config_name)
    }

    pub(crate) fn runtime_name(self) -> &'static str {
        match self {
            ModuleKind::IdleInhibitor => "idle-inhibitor",
            ModuleKind::Battery => "battery",
            ModuleKind::Button => "button",
            ModuleKind::Network => "network",
            ModuleKind::Notifications => "notifications",
            ModuleKind::Media => "media",
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
        MODULE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.config_name == s)
            .map(|descriptor| descriptor.kind)
    }
}

/// Zone placement for a module in the bar layout.
// Used in Story 2.2 when modules are placed into zones from config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zone {
    Start,
    Center,
    End,
}

/// Per-module runtime state. Variants added per concrete module (Stories 2.3+).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ModuleState {
    Media(canopa_shell::media::Player),
    IdleInhibitor(idle_inhibitor::IdleInhibitorState),
    Stasis(canopa_widgets::idle::Snapshot),
    /// Module has no visible content. Used for available-but-empty Module outcomes
    /// and as the no-state render fallback while first emission is pending inside
    /// the Module Data Source runtime lifecycle.
    #[default]
    Empty,
    /// Module's data source is unavailable. Render as nothing — zero height, no gap left.
    Unavailable,
    Battery(battery::BatteryState),
    Network(network::NetworkState),
    Notifications(canopa_notify_client::Summary),
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ModuleInteractionProfile {
    pub primary_activation: bool,
    pub secondary_activation: bool,
    pub scroll: bool,
}

impl ModuleInteractionProfile {
    pub(crate) const NONE: Self = Self {
        primary_activation: false,
        secondary_activation: false,
        scroll: false,
    };

    pub(crate) const PRIMARY: Self = Self {
        primary_activation: true,
        ..Self::NONE
    };

    pub(crate) const PRIMARY_AND_SECONDARY: Self = Self {
        primary_activation: true,
        secondary_activation: true,
        scroll: false,
    };

    pub(crate) const SCROLL: Self = Self {
        scroll: true,
        ..Self::NONE
    };
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
pub trait Module: render::ModuleRender + Send + Sync {
    fn id(&self) -> ModuleId;
    fn kind(&self) -> ModuleKind;
    /// Returns the module type name (e.g. `"clock"`, `"script"`).
    /// Used for type-based state routing; distinct from the instance name returned by `id()`.
    fn module_type(&self) -> &str {
        self.kind().as_str()
    }
    fn subscription(&self) -> iced::Subscription<crate::app::Message>;
    fn data_source(&self) -> Option<&dyn data_source::ModuleDataSourceMapper> {
        None
    }

    fn module_detail(&self) -> Option<&dyn detail::ModuleDetailLifecycle> {
        None
    }

    fn module_detail_mut(&mut self) -> Option<&mut (dyn detail::ModuleDetailLifecycle + '_)> {
        None
    }

    fn interaction_handler(&self) -> Option<&dyn interaction::ModuleInteractionHandler> {
        None
    }

    fn dynamic_instance(&self) -> Option<&dyn dynamic::DynamicModuleInstance> {
        None
    }

    fn dynamic_instance_mut(&mut self) -> Option<&mut (dyn dynamic::DynamicModuleInstance + '_)> {
        None
    }
}

/// Type alias for a module constructor function.
pub(crate) type ModuleConstructor =
    fn(&crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>>;

/// Type alias for a synthesized-params factory used when a built-in module is undeclared.
pub(crate) type SynthesizedParamsFactory = fn() -> ModuleParams;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ModuleCapabilities {
    pub needs_niri: bool,
    pub needs_system_dbus: bool,
    pub needs_session_dbus: bool,
    pub needs_wireplumber: bool,
}

impl ModuleCapabilities {
    pub(crate) const NONE: Self = Self {
        needs_niri: false,
        needs_system_dbus: false,
        needs_session_dbus: false,
        needs_wireplumber: false,
    };
}

macro_rules! config_module_descriptors {
    ($macro:ident) => {
        $macro! {
            (ModuleKind::IdleInhibitor, "idle-inhibitor", idle_inhibitor_ctor, Some(idle_inhibitor_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities::NONE, ModuleInteractionProfile::PRIMARY),
            (ModuleKind::Battery, "battery", battery_ctor, Some(battery_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities { needs_system_dbus: true, ..ModuleCapabilities::NONE }, ModuleInteractionProfile::NONE),
            (ModuleKind::Button, "button", button_ctor, None, ModuleCapabilities::NONE, ModuleInteractionProfile::PRIMARY),
            (ModuleKind::Clock, "clock", clock_ctor, Some(clock_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities::NONE, ModuleInteractionProfile::NONE),
            (ModuleKind::FocusedWindow, "focused-window", focused_window_ctor, Some(focused_window_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities { needs_niri: true, ..ModuleCapabilities::NONE }, ModuleInteractionProfile::NONE),
            (ModuleKind::Media, "media", media_ctor, Some(media_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities::NONE, ModuleInteractionProfile::PRIMARY),
            (ModuleKind::Notifications, "notifications", notifications_ctor, Some(notifications_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities::NONE, ModuleInteractionProfile::PRIMARY_AND_SECONDARY),
            (ModuleKind::Network, "network", network_ctor, Some(network_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities { needs_system_dbus: true, ..ModuleCapabilities::NONE }, ModuleInteractionProfile::NONE),
            (ModuleKind::Script, "script", script_ctor, None, ModuleCapabilities::NONE, ModuleInteractionProfile::NONE),
            (ModuleKind::Tray, "tray", tray_ctor, Some(tray_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities { needs_session_dbus: true, ..ModuleCapabilities::NONE }, ModuleInteractionProfile::NONE),
            (ModuleKind::Volume, "volume", volume_ctor, Some(volume_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities { needs_wireplumber: true, ..ModuleCapabilities::NONE }, ModuleInteractionProfile::SCROLL),
            (ModuleKind::Workspaces, "workspaces", workspaces_ctor, Some(workspaces_synthesized_params as SynthesizedParamsFactory), ModuleCapabilities { needs_niri: true, ..ModuleCapabilities::NONE }, ModuleInteractionProfile::NONE),
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
    pub capabilities: ModuleCapabilities,
    pub interactions: ModuleInteractionProfile,
}

impl ModuleDescriptor {
    pub(crate) fn synthesized_params(&self) -> Option<ModuleParams> {
        self.synthesized_params_factory.map(|factory| factory())
    }
}

macro_rules! build_module_descriptors {
    ($(($kind:expr, $config_name:expr, $ctor:ident, $factory:expr, $capabilities:expr, $interactions:expr)),* $(,)?) => {
        &[
            $(ModuleDescriptor {
                kind: $kind,
                config_name: $config_name,
                constructor: $ctor,
                synthesized_params_factory: $factory,
                capabilities: $capabilities,
                interactions: $interactions,
            }),*
        ]
    };
}

pub(crate) const MODULE_DESCRIPTORS: &[ModuleDescriptor] =
    config_module_descriptors!(build_module_descriptors);

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

pub(crate) fn descriptor_for_kind(kind: ModuleKind) -> Option<&'static ModuleDescriptor> {
    MODULE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.kind == kind)
}

fn media_synthesized_params() -> ModuleParams {
    ModuleParams::Media(crate::config::MediaConfig::default())
}

fn media_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let config = match &mc.params {
        ModuleParams::Media(config) => config.clone(),
        _ => Default::default(),
    };
    Ok(Box::new(media::MediaModule {
        id: ModuleId::new(&mc.name),
        config,
    }))
}

fn notifications_synthesized_params() -> ModuleParams {
    ModuleParams::Notifications(crate::config::NotificationsConfig::default())
}

fn idle_inhibitor_synthesized_params() -> ModuleParams {
    ModuleParams::IdleInhibitor(crate::config::IdleInhibitorConfig::default())
}

fn idle_inhibitor_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let config = match &mc.params {
        ModuleParams::IdleInhibitor(config) => config.clone(),
        _ => crate::config::IdleInhibitorConfig::default(),
    };
    Ok(Box::new(idle_inhibitor::IdleInhibitorModule {
        id: ModuleId::new(&mc.name),
        config,
    }))
}

fn notifications_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let config = match &mc.params {
        ModuleParams::Notifications(config) => config.clone(),
        _ => crate::config::NotificationsConfig::default(),
    };
    Ok(Box::new(notifications::NotificationsModule {
        id: ModuleId::new(&mc.name),
        config,
    }))
}

fn network_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let (icons, format, content_rotation) = match &mc.params {
        ModuleParams::Network(cfg) => (cfg.icons.clone(), cfg.format.clone(), cfg.content_rotation),
        _ => {
            let default = NetworkConfig::default();
            (default.icons, default.format, default.content_rotation)
        }
    };
    Ok(Box::new(network::NetworkModule::new_with_content_rotation(
        ModuleId::new(&mc.name),
        icons,
        format,
        content_rotation,
    )))
}

fn battery_synthesized_params() -> ModuleParams {
    ModuleParams::Battery(BatteryConfig::default())
}

fn clock_synthesized_params() -> ModuleParams {
    ModuleParams::Clock(ClockConfig::default())
}

fn focused_window_synthesized_params() -> ModuleParams {
    ModuleParams::FocusedWindow(crate::config::FocusedWindowConfig::default())
}

fn network_synthesized_params() -> ModuleParams {
    ModuleParams::Network(NetworkConfig::default())
}

fn tray_synthesized_params() -> ModuleParams {
    ModuleParams::Tray(TrayConfig::default())
}

fn volume_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let (step, states, icons, format, content_rotation) = match &mc.params {
        ModuleParams::Volume(cfg) => (
            cfg.step,
            cfg.states.clone(),
            cfg.icons.clone(),
            cfg.format.clone(),
            cfg.content_rotation,
        ),
        _ => {
            let default = VolumeConfig::default();
            (
                default.step,
                default.states,
                default.icons,
                default.format,
                default.content_rotation,
            )
        }
    };
    Ok(Box::new(volume::VolumeModule::new_with_content_rotation(
        ModuleId::new(&mc.name),
        step,
        states,
        icons,
        format,
        content_rotation,
    )))
}

fn battery_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let (threshold, states, icons, format, content_rotation) = match &mc.params {
        ModuleParams::Battery(cfg) => (
            cfg.critical_threshold,
            cfg.states.clone(),
            cfg.icons.clone(),
            cfg.format.clone(),
            cfg.content_rotation,
        ),
        _ => {
            let default = BatteryConfig::default();
            (
                default.critical_threshold,
                default.states,
                default.icons,
                default.format,
                default.content_rotation,
            )
        }
    };
    Ok(Box::new(battery::BatteryModule::new_with_content_rotation(
        ModuleId::new(&mc.name),
        threshold,
        states,
        icons,
        format,
        content_rotation,
    )))
}

fn button_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    match &mc.params {
        ModuleParams::Button(cfg) => Ok(Box::new(button::ButtonModule::new_with_content_rotation(
            ModuleId::new(&mc.name),
            cfg.label.clone(),
            cfg.icon.clone(),
            cfg.format.clone(),
            cfg.content_rotation,
        ))),
        _ => anyhow::bail!(
            "button constructor called with non-button params for {:?}",
            mc.name
        ),
    }
}

fn clock_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let (time_fmt, date_fmt, format, content_rotation) = match &mc.params {
        ModuleParams::Clock(cfg) => (
            cfg.time_format.clone(),
            cfg.date_format.clone(),
            cfg.format.clone(),
            cfg.content_rotation,
        ),
        _ => {
            let d = ClockConfig::default();
            (d.time_format, d.date_format, d.format, d.content_rotation)
        }
    };
    Ok(Box::new(clock::ClockModule::new_with_content_rotation(
        ModuleId::new(&mc.name),
        time_fmt,
        date_fmt,
        format,
        content_rotation,
    )))
}

fn script_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    match &mc.params {
        ModuleParams::Script(cfg) => Ok(Box::new(script::ScriptModule::new_with_content_rotation(
            ModuleId::new(&mc.name),
            cfg.command.clone(),
            cfg.interval,
            cfg.format.clone(),
            cfg.content_rotation,
        ))),
        _ => anyhow::bail!(
            "script constructor called with non-script params for {:?}",
            mc.name
        ),
    }
}

fn workspaces_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let config = match &mc.params {
        ModuleParams::Workspaces(c) => c.clone(),
        _ => crate::config::WorkspacesConfig::default(),
    };
    Ok(Box::new(workspaces::WorkspacesModule::new(
        ModuleId::new(&mc.name),
        config,
    )))
}

fn focused_window_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    let config = match &mc.params {
        ModuleParams::FocusedWindow(cfg) => cfg.clone(),
        _ => crate::config::FocusedWindowConfig::default(),
    };
    Ok(Box::new(focused_window::FocusedWindowModule::new(
        ModuleId::new(&mc.name),
        config,
    )))
}

fn tray_ctor(mc: &crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>> {
    Ok(Box::new(tray::TrayModule::new(ModuleId::new(&mc.name))))
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
        assert_eq!(ModuleId::new("clock").configured_name(), "clock");
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
    fn module_state_default_is_empty() {
        assert!(matches!(ModuleState::default(), ModuleState::Empty));
    }

    #[test]
    fn module_kind_round_trips_known_config_names() {
        for (kind, name) in [
            (ModuleKind::Battery, "battery"),
            (ModuleKind::Button, "button"),
            (ModuleKind::Clock, "clock"),
            (ModuleKind::FocusedWindow, "focused-window"),
            (ModuleKind::Notifications, "notifications"),
            (ModuleKind::Network, "network"),
            (ModuleKind::Script, "script"),
            (ModuleKind::Tray, "tray"),
            (ModuleKind::Volume, "volume"),
            (ModuleKind::Workspaces, "workspaces"),
        ] {
            assert_eq!(kind.as_str(), name);
            assert_eq!(ModuleKind::from_config_name(name), Some(kind));
        }

        for (kind, name) in [
            (ModuleKind::WorkspaceItem, "workspace_item"),
            (ModuleKind::TrayIcon, "tray_icon"),
        ] {
            assert_eq!(kind.as_str(), name);
            assert_eq!(ModuleKind::from_config_name(name), None);
        }
    }

    #[test]
    fn module_kind_names_are_descriptor_backed_for_config_modules() {
        for descriptor in MODULE_DESCRIPTORS {
            assert_eq!(descriptor.kind.config_name(), Some(descriptor.config_name));
            assert_eq!(
                ModuleKind::from_config_name(descriptor.config_name),
                Some(descriptor.kind)
            );
        }
    }

    #[test]
    fn runtime_only_kinds_are_not_config_names() {
        assert_eq!(ModuleKind::WorkspaceItem.config_name(), None);
        assert_eq!(ModuleKind::TrayIcon.config_name(), None);
    }

    #[test]
    fn script_constructor_reports_wrong_params() {
        let module_config = crate::config::ModuleConfig {
            name: "bad".into(),
            module_type: "script".into(),
            params: crate::config::ModuleParams::None,
            actions: crate::config::ModuleActionsConfig::default(),
        };
        let err = match script_ctor(&module_config) {
            Ok(_) => panic!("expected script constructor error"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("script"));
    }

    #[test]
    fn every_config_facing_kind_has_exactly_one_descriptor() {
        let expected = HashSet::from([
            ModuleKind::IdleInhibitor,
            ModuleKind::Battery,
            ModuleKind::Button,
            ModuleKind::Clock,
            ModuleKind::FocusedWindow,
            ModuleKind::Network,
            ModuleKind::Notifications,
            ModuleKind::Media,
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
    fn descriptors_capture_subscription_capabilities() {
        let tray = descriptor_for_kind(ModuleKind::Tray).expect("tray descriptor");
        assert!(tray.capabilities.needs_session_dbus);

        let focused_window =
            descriptor_for_kind(ModuleKind::FocusedWindow).expect("focused-window descriptor");
        assert!(focused_window.capabilities.needs_niri);
        assert!(!focused_window.capabilities.needs_system_dbus);
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
        map.insert(ModuleId::new("clock"), ModuleState::Empty);
        assert!(map.contains_key(&ModuleId::new("clock")));
        assert!(!map.contains_key(&ModuleId::new("battery")));
    }
}
