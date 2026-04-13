use zbus::zvariant::OwnedObjectPath;

/// A live SNI tray item, fully resolved from D-Bus properties.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TrayItem {
    /// D-Bus service name, e.g. `"org.kde.konsole"` or `":1.42"`.
    pub service: String,
    /// Object path the item is registered at, e.g. `"/StatusNotifierItem"`.
    pub object_path: OwnedObjectPath,
    /// Resolved icon — Named preferred, Pixmap as fallback.
    pub icon: TrayIcon,
    /// Human-readable tooltip text (the `ToolTip.title` field), if any.
    pub tooltip: Option<String>,
    pub status: ItemStatus,
    /// D-Bus path for the context menu (`org.kde.DBusMenu`), if the item exposes one.
    pub menu_path: Option<OwnedObjectPath>,
}

/// Resolved icon data for a tray item.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TrayIcon {
    /// XDG icon theme name. Render via icon theme lookup at display time.
    Named(String),
    /// Raw ARGB32 pixmap data: list of (width, height, bytes) at different sizes.
    /// Bytes are ARGB big-endian; convert to RGBA before passing to iced.
    Pixmap(Vec<(i32, i32, Vec<u8>)>),
}

/// SNI item visibility status.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ItemStatus {
    /// Normal visible state.
    Active,
    /// Item requests to be hidden. Render nothing.
    Passive,
    /// Item requests attention (e.g. new notification).
    NeedsAttention,
}

impl ItemStatus {
    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "Passive" => ItemStatus::Passive,
            "NeedsAttention" => ItemStatus::NeedsAttention,
            _ => ItemStatus::Active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_item_has_service_and_status() {
        let item = TrayItem {
            service: "org.kde.konsole".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("utilities-terminal".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };
        assert_eq!(item.service, "org.kde.konsole");
        assert!(matches!(item.status, ItemStatus::Active));
    }

    #[test]
    fn item_status_from_str_passive() {
        assert_eq!(ItemStatus::from_str("Passive"), ItemStatus::Passive);
    }

    #[test]
    fn item_status_from_str_needs_attention() {
        assert_eq!(ItemStatus::from_str("NeedsAttention"), ItemStatus::NeedsAttention);
    }

    #[test]
    fn item_status_from_str_unknown_is_active() {
        assert_eq!(ItemStatus::from_str("SomeUnknown"), ItemStatus::Active);
    }

    #[test]
    fn tray_icon_named_stores_string() {
        let icon = TrayIcon::Named("konsole".into());
        match icon {
            TrayIcon::Named(n) => assert_eq!(n, "konsole"),
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn tray_icon_pixmap_stores_vec() {
        let icon = TrayIcon::Pixmap(vec![(22, 22, vec![0u8; 22 * 22 * 4])]);
        match icon {
            TrayIcon::Pixmap(v) => assert_eq!(v[0].0, 22),
            _ => panic!("expected Pixmap"),
        }
    }
}
