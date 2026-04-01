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
    // Constructed in Stories 2.3+ when a module's data source is absent.
    #[allow(dead_code)]
    Unavailable,
    // Clock(ClockState) — Story 2.3
    // Script(ScriptState) — Story 2.4
    // Button(ButtonState) — Story 2.5
}

/// Module data events routed through the Iced message bus.
/// Concrete variants added per module (Stories 2.3+).
#[derive(Debug, Clone)]
pub enum ModuleEvent {}

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
// Used in Story 2.2 when the registry is populated.
#[allow(dead_code)]
pub type ModuleConstructor = fn() -> Box<dyn Module>;

/// Build the static module registry mapping config name → constructor.
///
/// Modules are registered here as they are implemented:
/// - clock — Story 2.3
/// - script — Story 2.4
/// - button — Story 2.5
// Called in Story 2.2 during app initialisation.
#[allow(dead_code)]
pub fn build_registry() -> HashMap<&'static str, ModuleConstructor> {
    HashMap::new()
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
    fn build_registry_is_empty_initially() {
        assert!(build_registry().is_empty());
    }

    #[test]
    fn module_id_hashable() {
        let mut map = HashMap::new();
        map.insert(ModuleId::new("clock"), ModuleState::Loading);
        assert!(map.contains_key(&ModuleId::new("clock")));
        assert!(!map.contains_key(&ModuleId::new("battery")));
    }
}
