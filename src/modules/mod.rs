pub mod battery;
pub mod button;
pub mod clock;
pub mod focused_window;
pub mod network;
pub mod script;
pub mod volume;
pub mod workspaces;

use std::collections::HashMap;

/// Unique identifier for a module instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(pub String);

impl ModuleId {
    // Used in tests and by Stories 2.2+ when module instances are constructed from config.
    #[allow(dead_code)]
    pub fn new(s: impl Into<String>) -> Self {
        ModuleId(s.into())
    }
}

/// Zone placement for a module in the bar layout.
// Used in Story 2.2 when modules are placed into zones from config.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zone {
    Top,
    Center,
    Bottom,
}

/// Per-module runtime state. Variants added per concrete module (Stories 2.3+).
#[derive(Debug, Clone, Default)]
pub enum ModuleState {
    /// Module is starting up; no data yet. Render as empty space — no spinner, no text.
    #[default]
    Loading,
    /// Module's data source is unavailable. Render as nothing — zero height, no gap left.
    #[allow(dead_code)]
    Unavailable,
    Battery(battery::BatteryState),
    Network(network::NetworkState),
    Volume(volume::VolumeState),
    Clock(clock::ClockState),
    Script(script::ScriptState),
    Button(button::ButtonState),
    Workspace(workspaces::WorkspaceState),
    FocusedWindow(focused_window::FocusedWindowState),
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
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum InteractionEvent {
    Clicked,
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
#[allow(dead_code)]
pub trait Module: Send + Sync {
    fn id(&self) -> ModuleId;
    fn subscription(&self) -> iced::Subscription<crate::app::Message>;
    fn view_collapsed(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
    fn view_expanded(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
    fn on_interaction(&self, _event: InteractionEvent) -> Option<crate::app::Message> {
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
    map
}

fn network_ctor(_mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    Box::new(network::NetworkModule::new())
}

fn volume_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::{ModuleParams, VolumeConfig};
    let step = match &mc.params {
        ModuleParams::Volume(cfg) => cfg.step,
        _ => VolumeConfig::default().step,
    };
    Box::new(volume::VolumeModule::new(step))
}

fn battery_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::{BatteryConfig, ModuleParams};
    let threshold = match &mc.params {
        ModuleParams::Battery(cfg) => cfg.critical_threshold,
        _ => BatteryConfig::default().critical_threshold,
    };
    Box::new(battery::BatteryModule::new(threshold))
}

fn button_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::ModuleParams;
    match &mc.params {
        ModuleParams::Button(cfg) => Box::new(button::ButtonModule::new(
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
    Box::new(clock::ClockModule::new(time_fmt, date_fmt))
}

fn script_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::ModuleParams;
    match &mc.params {
        ModuleParams::Script(cfg) => {
            Box::new(script::ScriptModule::new(cfg.command.clone(), cfg.interval))
        }
        _ => panic!("script_ctor called with non-Script params — this is a programming error"),
    }
}

fn workspaces_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    use crate::config::ModuleParams;
    let config = match &mc.params {
        ModuleParams::Workspaces(c) => c.clone(),
        _ => crate::config::WorkspacesConfig::default(),
    };
    Box::new(workspaces::WorkspacesModule::new(config))
}

fn focused_window_ctor(_mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    Box::new(focused_window::FocusedWindowModule::new())
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
    fn module_state_default_is_loading() {
        assert!(matches!(ModuleState::default(), ModuleState::Loading));
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
    fn module_id_hashable() {
        let mut map = HashMap::new();
        map.insert(ModuleId::new("clock"), ModuleState::Loading);
        assert!(map.contains_key(&ModuleId::new("clock")));
        assert!(!map.contains_key(&ModuleId::new("battery")));
    }
}
