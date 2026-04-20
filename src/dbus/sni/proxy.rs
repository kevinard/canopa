// zbus proxy trait definitions for the SNI protocol.
// The proxy macro generates <TraitName>Proxy types in this module.
// host.rs imports them via `use super::proxy::*;`.

pub type SniPixmap = (i32, i32, Vec<u8>);
pub(crate) type SniToolTip = (String, Vec<SniPixmap>, String, String);
pub(crate) type Props = std::collections::HashMap<String, zbus::zvariant::OwnedValue>;
pub(crate) type RawMenuItem = (i32, Props, Vec<zbus::zvariant::OwnedValue>);
pub(crate) type RawMenuLayout = (u32, RawMenuItem);

// -- SNI item D-Bus proxy -----------------------------------------------------

#[zbus::proxy(
    interface = "org.kde.StatusNotifierItem",
    default_service = "org.kde.StatusNotifierItem",
    default_path = "/StatusNotifierItem"
)]
pub(crate) trait SniItem {
    fn activate(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn secondary_activate(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn scroll(&self, delta: i32, orientation: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn icon_name(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn icon_pixmap(&self) -> zbus::Result<Vec<(i32, i32, Vec<u8>)>>;

    /// Returns (icon_name, icon_pixmap, title, description).
    #[zbus(property)]
    fn tool_tip(&self) -> zbus::Result<SniToolTip>;

    #[zbus(property)]
    fn status(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn menu(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[zbus(signal)]
    fn new_icon(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn new_status(&self, status: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn new_tool_tip(&self) -> zbus::Result<()>;
}

// -- Watcher proxy (client side) ----------------------------------------------

#[zbus::proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
pub(crate) trait SniWatcher {
    fn register_status_notifier_host(&self, service: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> zbus::Result<Vec<String>>;

    #[zbus(signal)]
    fn status_notifier_item_registered(&self, service: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn status_notifier_item_unregistered(&self, service: String) -> zbus::Result<()>;
}

// -- DBusMenu proxy -----------------------------------------------------------

#[zbus::proxy(
    interface = "com.canonical.dbusmenu",
    default_service = "com.canonical.dbusmenu",
    default_path = "/MenuBar"
)]
pub(crate) trait DbusMenu {
    /// Fetch the full menu layout.
    /// Returns (revision, root_item) where root_item is (id: i32, properties: a{sv}, children: av).
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: &[&str],
    ) -> zbus::Result<RawMenuLayout>;

    fn about_to_show(&self, id: i32) -> zbus::Result<bool>;

    fn event(
        &self,
        id: i32,
        event_id: &str,
        data: &zbus::zvariant::Value<'_>,
        timestamp: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn layout_updated(&self, revision: u32, parent: i32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn items_properties_updated(
        &self,
        updated_props: Vec<(
            i32,
            std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        )>,
        removed_props: Vec<(i32, Vec<String>)>,
    ) -> zbus::Result<()>;
}
