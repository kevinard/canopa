use crate::dbus::DbusConnections;

#[derive(Debug, Clone, Default)]
pub(super) struct SharedContext {
    pub(super) dbus: DbusConnections,
}

impl SharedContext {
    pub(super) fn new() -> Self {
        Self {
            dbus: DbusConnections::default(),
        }
    }
}
