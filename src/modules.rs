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

/// Unique identifier for a module instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(pub String);

impl ModuleId {
    // Used in tests and by Stories 2.2+ when module instances are constructed from config.
    pub fn new(s: impl Into<String>) -> Self {
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
    pub const CONFIG_KINDS: &'static [ModuleKind] = &[
        ModuleKind::Battery,
        ModuleKind::Clock,
        ModuleKind::Network,
        ModuleKind::Volume,
        ModuleKind::Workspaces,
        ModuleKind::FocusedWindow,
        ModuleKind::Script,
        ModuleKind::Button,
        ModuleKind::Tray,
    ];

    pub fn as_str(self) -> &'static str {
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

    pub fn from_config_name(s: &str) -> Option<Self> {
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
    ) -> iced::Element<'_, crate::app::Message> {
        let _ = window;
        if expanded {
            self.view_expanded(state)
        } else if horizontal {
            self.view_horizontal(state)
        } else {
            self.view_collapsed(state)
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
pub type ModuleConstructor = fn(&crate::config::ModuleConfig) -> Box<dyn Module>;

/// Build the static module registry mapping config name → constructor.
pub fn build_registry() -> HashMap<&'static str, ModuleConstructor> {
    let mut map = HashMap::new();
    map.insert("battery", battery_ctor as ModuleConstructor);
    map.insert("button", button_ctor as ModuleConstructor);
    map.insert("network", network_ctor as ModuleConstructor);
    map.insert("volume", volume_ctor as ModuleConstructor);
    map.insert("clock", clock_ctor as ModuleConstructor);
    map.insert("script", script_ctor as ModuleConstructor);
    map.insert("workspaces", workspaces_ctor as ModuleConstructor);
    map.insert("focused-window", focused_window_ctor as ModuleConstructor);
    map.insert("tray", tray_ctor as ModuleConstructor);
    map
}

fn network_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    Box::new(network::NetworkModule::new(ModuleId::new(&mc.name)))
}

fn volume_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::{ModuleParams, VolumeConfig};
    let step = match &mc.params {
        ModuleParams::Volume(cfg) => cfg.step,
        _ => VolumeConfig::default().step,
    };
    Box::new(volume::VolumeModule::new(ModuleId::new(&mc.name), step))
}

fn battery_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::{BatteryConfig, ModuleParams};
    let threshold = match &mc.params {
        ModuleParams::Battery(cfg) => cfg.critical_threshold,
        _ => BatteryConfig::default().critical_threshold,
    };
    Box::new(battery::BatteryModule::new(
        ModuleId::new(&mc.name),
        threshold,
    ))
}

fn button_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::ModuleParams;
    match &mc.params {
        ModuleParams::Button(cfg) => Box::new(button::ButtonModule::new(
            ModuleId::new(&mc.name),
            cfg.label.clone(),
            cfg.icon.clone(),
            cfg.on_click.clone(),
        )),
        _ => panic!("button_ctor called with non-Button params — this is a programming error"),
    }
}

fn clock_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::{ClockConfig, ModuleParams};
    let (time_fmt, date_fmt) = match &mc.params {
        ModuleParams::Clock(cfg) => (cfg.time_format.clone(), cfg.date_format.clone()),
        _ => {
            let d = ClockConfig::default();
            (d.time_format, d.date_format)
        }
    };
    Box::new(clock::ClockModule::new(
        ModuleId::new(&mc.name),
        time_fmt,
        date_fmt,
    ))
}

fn script_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::ModuleParams;
    match &mc.params {
        ModuleParams::Script(cfg) => Box::new(script::ScriptModule::new(
            ModuleId::new(&mc.name),
            cfg.command.clone(),
            cfg.interval,
        )),
        _ => panic!("script_ctor called with non-Script params — this is a programming error"),
    }
}

fn workspaces_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::ModuleParams;
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
    Box::new(focused_window::FocusedWindowModule::new(ModuleId::new(
        &mc.name,
    )))
}

fn tray_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    Box::new(tray::TrayModule::new(ModuleId::new(&mc.name)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
            (ModuleKind::Clock, "clock"),
            (ModuleKind::Network, "network"),
            (ModuleKind::Volume, "volume"),
            (ModuleKind::Workspaces, "workspaces"),
            (ModuleKind::FocusedWindow, "focused-window"),
            (ModuleKind::Script, "script"),
            (ModuleKind::Button, "button"),
            (ModuleKind::Tray, "tray"),
            (ModuleKind::WorkspaceItem, "workspace_item"),
            (ModuleKind::TrayIcon, "tray_icon"),
        ] {
            assert_eq!(kind.as_str(), name);
            assert_eq!(ModuleKind::from_config_name(name), Some(kind));
        }
    }

    #[test]
    fn build_registry_contains_clock() {
        let registry = build_registry();
        assert!(registry.contains_key("clock"));
    }

    #[test]
    fn build_registry_contains_script() {
        let registry = build_registry();
        assert!(registry.contains_key("script"));
    }

    #[test]
    fn build_registry_contains_button() {
        let registry = build_registry();
        assert!(registry.contains_key("button"));
    }

    #[test]
    fn build_registry_contains_workspaces() {
        let registry = build_registry();
        assert!(registry.contains_key("workspaces"));
    }

    #[test]
    fn build_registry_contains_focused_window() {
        let registry = build_registry();
        assert!(registry.contains_key("focused-window"));
    }

    #[test]
    fn build_registry_contains_tray() {
        let registry = build_registry();
        assert!(registry.contains_key("tray"));
    }

    #[test]
    fn module_id_hashable() {
        let mut map = HashMap::new();
        map.insert(ModuleId::new("clock"), ModuleState::Loading);
        assert!(map.contains_key(&ModuleId::new("clock")));
        assert!(!map.contains_key(&ModuleId::new("battery")));
    }
}
