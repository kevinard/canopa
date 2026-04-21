use std::collections::HashMap;

use crate::dbus::DbusConnections;
use crate::modules::{ModuleId, ModuleState};
use crate::niri::NiriState;

#[derive(Debug, Clone, Default)]
pub(super) struct SharedContext {
    pub(super) dbus: DbusConnections,
    pub(super) modules: HashMap<ModuleId, ModuleState>,
    pub(super) niri: NiriState,
}

impl SharedContext {
    pub(super) fn new() -> Self {
        Self {
            dbus: DbusConnections::default(),
            modules: HashMap::new(),
            niri: NiriState::default(),
        }
    }
}
