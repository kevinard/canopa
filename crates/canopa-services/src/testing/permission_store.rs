//! A fake portal permission store with in-memory tables.
use super::TestBus;
use crate::permission_store::{PATH, Permissions, SERVICE};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use zbus::{
    fdo,
    object_server::SignalEmitter,
    zvariant::{OwnedValue, Value},
};

type Tables = BTreeMap<String, BTreeMap<String, Permissions>>;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.portal.Error")]
enum StoreError {
    #[zbus(error)]
    ZBus(zbus::Error),
    NotFound(String),
}

struct Store(Arc<Mutex<Tables>>);

impl Store {
    async fn announce(
        &self,
        emitter: &SignalEmitter<'_>,
        table: &str,
        id: &str,
        deleted: bool,
    ) -> zbus::Result<()> {
        let permissions = self
            .0
            .lock()
            .unwrap()
            .get(table)
            .and_then(|entries| entries.get(id))
            .cloned()
            .unwrap_or_default();
        Self::changed(emitter, table, id, deleted, Value::from(0u8), permissions).await
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.PermissionStore")]
impl Store {
    fn list(&self, table: &str) -> Result<Vec<String>, StoreError> {
        let tables = self.0.lock().unwrap();
        let entries = tables
            .get(table)
            .ok_or_else(|| StoreError::NotFound(format!("No such table: {table}")))?;
        Ok(entries.keys().cloned().collect())
    }
    fn lookup(&self, table: &str, id: &str) -> Result<(Permissions, OwnedValue), StoreError> {
        let tables = self.0.lock().unwrap();
        let permissions = tables
            .get(table)
            .and_then(|entries| entries.get(id))
            .ok_or_else(|| StoreError::NotFound(format!("No entry for {id}")))?;
        Ok((permissions.clone(), OwnedValue::from(0u8)))
    }
    async fn set_permission(
        &self,
        table: &str,
        _create: bool,
        id: &str,
        app: &str,
        permissions: Vec<String>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.0
            .lock()
            .unwrap()
            .entry(table.into())
            .or_default()
            .entry(id.into())
            .or_default()
            .insert(app.into(), permissions);
        self.announce(&emitter, table, id, false).await?;
        Ok(())
    }
    async fn delete_permission(
        &self,
        table: &str,
        id: &str,
        app: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), StoreError> {
        let removed = self
            .0
            .lock()
            .unwrap()
            .get_mut(table)
            .and_then(|entries| entries.get_mut(id))
            .and_then(|apps| apps.remove(app));
        if removed.is_none() {
            return Err(StoreError::NotFound(format!("No entry for {id}")));
        }
        self.announce(&emitter, table, id, false).await?;
        Ok(())
    }
    async fn delete(
        &self,
        table: &str,
        id: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), StoreError> {
        let removed = self
            .0
            .lock()
            .unwrap()
            .get_mut(table)
            .and_then(|entries| entries.remove(id));
        if removed.is_none() {
            return Err(StoreError::NotFound(format!("No entry for {id}")));
        }
        self.announce(&emitter, table, id, true).await?;
        Ok(())
    }
    #[zbus(signal)]
    async fn changed(
        emitter: &SignalEmitter<'_>,
        table: &str,
        id: &str,
        deleted: bool,
        data: Value<'_>,
        permissions: Permissions,
    ) -> zbus::Result<()>;
}

/// The permission store on a private bus.
pub struct FakePermissionStore {
    pub connection: zbus::Connection,
    pub tables: Arc<Mutex<Tables>>,
}

impl FakePermissionStore {
    pub async fn serve(bus: &TestBus) -> Self {
        let tables = Arc::new(Mutex::new(Tables::new()));
        let connection = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(PATH, Store(tables.clone()))
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self { connection, tables }
    }

    /// Record a grant, as xdg-desktop-portal would.
    pub fn grant(&self, table: &str, id: &str, app: &str, permissions: &[&str]) {
        self.tables
            .lock()
            .unwrap()
            .entry(table.into())
            .or_default()
            .entry(id.into())
            .or_default()
            .insert(
                app.into(),
                permissions.iter().map(|p| (*p).to_owned()).collect(),
            );
    }
}
