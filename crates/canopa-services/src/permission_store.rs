//! The portal permission store (`org.freedesktop.impl.portal.PermissionStore`
//! on the session bus): what applications were allowed or denied through
//! portals, such as the camera, location or screenshots.
//!
//! xdg-desktop-portal writes the grants; this client reads them and revokes
//! or changes them. A table or entry that does not exist is empty, not an
//! error.
//!
//! ```no_run
//! use canopa_services::permission_store;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let session = canopa_services::bus::session().await?;
//! let camera = permission_store::lookup(&session, "devices", "camera").await?;
//! for (app, permissions) in &camera {
//!     println!("{app}: {permissions:?}");
//!     permission_store::delete_permission(&session, "devices", "camera", app).await?;
//! }
//! # Ok(()) }
//! ```
use crate::{Error, bus::DEADLINE};
use futures_util::{Stream, StreamExt};
use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};

pub const SERVICE: &str = "org.freedesktop.impl.portal.PermissionStore";
pub const PATH: &str = "/org/freedesktop/impl/portal/PermissionStore";
pub const INTERFACE: &str = "org.freedesktop.impl.portal.PermissionStore";
/// At most this many entries or applications are read per table.
pub const MAX_ENTRIES: usize = 256;

/// Applications and their permission strings for one entry.
pub type Permissions = HashMap<String, Vec<String>>;

async fn proxy(connection: &zbus::Connection) -> Result<zbus::Proxy<'static>, Error> {
    Ok(zbus::Proxy::new_owned(connection.clone(), SERVICE, PATH, INTERFACE).await?)
}

fn not_found(error: &zbus::Error) -> bool {
    matches!(error, zbus::Error::MethodError(name, ..)
        if name.as_str() == "org.freedesktop.portal.Error.NotFound")
}

fn valid(name: &str) -> bool {
    !name.is_empty() && name.len() <= 255 && !name.chars().any(char::is_control)
}

/// The entry ids of `table`.
pub async fn list(connection: &zbus::Connection, table: &str) -> Result<Vec<String>, Error> {
    let reply =
        tokio::time::timeout(DEADLINE, proxy(connection).await?.call("List", &(table,))).await?;
    let mut ids: Vec<String> = match reply {
        Ok(ids) => ids,
        Err(error) if not_found(&error) => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    ids.retain(|id| valid(id));
    ids.truncate(MAX_ENTRIES);
    Ok(ids)
}

/// The applications and permissions of one entry.
pub async fn lookup(
    connection: &zbus::Connection,
    table: &str,
    id: &str,
) -> Result<Permissions, Error> {
    let reply: zbus::Result<(Permissions, OwnedValue)> = tokio::time::timeout(
        DEADLINE,
        proxy(connection).await?.call("Lookup", &(table, id)),
    )
    .await?;
    let mut permissions = match reply {
        Ok((permissions, _)) => permissions,
        Err(error) if not_found(&error) => return Ok(Permissions::new()),
        Err(error) => return Err(error.into()),
    };
    permissions.retain(|app, values| {
        values.truncate(8);
        values.iter_mut().for_each(|value| value.truncate(64));
        valid(app)
    });
    if permissions.len() > MAX_ENTRIES {
        let mut apps: Vec<_> = permissions.keys().cloned().collect();
        apps.sort();
        for app in apps.into_iter().skip(MAX_ENTRIES) {
            permissions.remove(&app);
        }
    }
    Ok(permissions)
}

/// Set one application's permissions in an entry, creating it if needed.
pub async fn set_permission(
    connection: &zbus::Connection,
    table: &str,
    id: &str,
    app: &str,
    permissions: &[&str],
) -> Result<(), Error> {
    Ok(tokio::time::timeout(
        DEADLINE,
        proxy(connection)
            .await?
            .call::<_, _, ()>("SetPermission", &(table, true, id, app, permissions)),
    )
    .await??)
}

/// Forget one application's permissions: the portal asks again next time.
pub async fn delete_permission(
    connection: &zbus::Connection,
    table: &str,
    id: &str,
    app: &str,
) -> Result<(), Error> {
    let reply = tokio::time::timeout(
        DEADLINE,
        proxy(connection)
            .await?
            .call::<_, _, ()>("DeletePermission", &(table, id, app)),
    )
    .await?;
    match reply {
        Err(error) if not_found(&error) => Ok(()),
        other => Ok(other?),
    }
}

/// Remove a whole entry.
pub async fn delete(connection: &zbus::Connection, table: &str, id: &str) -> Result<(), Error> {
    let reply = tokio::time::timeout(
        DEADLINE,
        proxy(connection)
            .await?
            .call::<_, _, ()>("Delete", &(table, id)),
    )
    .await?;
    match reply {
        Err(error) if not_found(&error) => Ok(()),
        other => Ok(other?),
    }
}

/// The table of each change the store announces.
pub async fn changes(
    connection: &zbus::Connection,
) -> Result<impl Stream<Item = String> + Send + 'static, Error> {
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(INTERFACE)?
        .member("Changed")?
        .path(PATH)?
        .build();
    let messages = zbus::MessageStream::for_match_rule(rule, connection, Some(64)).await?;
    Ok(messages.filter_map(|message| async move {
        let message = message.ok()?;
        let (table, ..): (String, String, bool, Value<'_>, Permissions) =
            message.body().deserialize().ok()?;
        Some(table)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestBus, permission_store::FakePermissionStore};

    #[tokio::test]
    async fn reads_changes_and_revokes_grants() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert_eq!(list(&client, "devices").await, Err(Error::Unavailable));
        let store = FakePermissionStore::serve(&bus).await;
        store.grant("devices", "camera", "org.example.Chat", &["yes"]);
        store.grant("location", "location", "org.example.Maps", &["EXACT", "0"]);
        assert_eq!(list(&client, "devices").await.unwrap(), ["camera"]);
        assert!(list(&client, "nothing").await.unwrap().is_empty());
        assert!(
            lookup(&client, "devices", "microphone")
                .await
                .unwrap()
                .is_empty()
        );
        let camera = lookup(&client, "devices", "camera").await.unwrap();
        assert_eq!(camera["org.example.Chat"], ["yes"]);

        let mut changes = Box::pin(changes(&client).await.unwrap());
        set_permission(&client, "devices", "camera", "org.example.Chat", &["no"])
            .await
            .unwrap();
        assert_eq!(changes.next().await.as_deref(), Some("devices"));
        assert_eq!(
            lookup(&client, "devices", "camera").await.unwrap()["org.example.Chat"],
            ["no"]
        );
        delete_permission(&client, "devices", "camera", "org.example.Chat")
            .await
            .unwrap();
        assert!(
            lookup(&client, "devices", "camera")
                .await
                .unwrap()
                .is_empty()
        );
        delete(&client, "location", "location").await.unwrap();
        assert!(list(&client, "location").await.unwrap().is_empty());
        delete(&client, "location", "location").await.unwrap();
    }
}
