use std::collections::HashMap;

use crate::dbus::DbusConnections;
use crate::modules::{ModuleId, ModuleState};

#[derive(Debug, Clone, Default)]
pub(super) struct SharedContext {
    pub(super) dbus: DbusConnections,
    pub(super) modules: HashMap<ModuleId, ModuleState>,
}

impl SharedContext {
    pub(super) fn new() -> Self {
        Self {
            dbus: DbusConnections::default(),
            modules: HashMap::new(),
        }
    }
}
