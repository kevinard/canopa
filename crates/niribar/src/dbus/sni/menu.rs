use super::proxy::{DbusMenuProxy, Props};

const MAX_MENU_ITEMS: usize = 256;
const MAX_MENU_LABEL_BYTES: usize = 512;

fn truncate_label(label: String) -> String {
    if label.len() <= MAX_MENU_LABEL_BYTES {
        label
    } else {
        String::from_utf8_lossy(&label.as_bytes()[..MAX_MENU_LABEL_BYTES]).to_string()
    }
}

fn limit_menu_items(
    items: Vec<crate::surface::menu_popup::MenuItem>,
) -> Vec<crate::surface::menu_popup::MenuItem> {
    items.into_iter().take(MAX_MENU_ITEMS).collect()
}

/// Extract the `label` string from a DBusMenu item's property map.
///
/// Dict values in `a{sv}` are D-Bus variants, so each OwnedValue wraps
/// `Value::Value(Box<inner>)`. `downcast_ref` unwraps that layer before casting.
fn extract_label_from_props(props: &Props) -> String {
    props
        .get("label")
        .and_then(|ov| ov.downcast_ref::<String>().ok())
        .unwrap_or_default()
}

fn prop_bool(props: &Props, key: &str, default: bool) -> bool {
    props
        .get(key)
        .and_then(|ov| ov.downcast_ref::<bool>().ok())
        .unwrap_or(default)
}

fn prop_str(props: &Props, key: &str) -> String {
    props
        .get(key)
        .and_then(|ov| ov.downcast_ref::<String>().ok())
        .unwrap_or_default()
}

/// Convert one `OwnedValue` (which should be a Structure) to a `MenuItem`.
///
/// DBusMenu `GetLayout` returns children as `av` (array of variants). Each element
/// is `Value::Value(Box<Value::Structure(...)>)`. We unwrap the variant layer before
/// casting to Structure. If the value is already a Structure (no variant wrapper),
/// the direct cast is used instead.
///
/// Returns `None` if the value is not the expected DBusMenu item structure.
fn owned_value_to_menu_item(
    v: &zbus::zvariant::OwnedValue,
) -> Option<crate::surface::menu_popup::MenuItem> {
    use zbus::zvariant::Value;
    let s: &zbus::zvariant::Structure<'_> = match &**v {
        Value::Value(inner) => <&zbus::zvariant::Structure<'_>>::try_from(inner.as_ref()).ok()?,
        _ => <&zbus::zvariant::Structure<'_>>::try_from(v).ok()?,
    };
    let fields = s.fields();
    if fields.len() < 2 {
        return None;
    }
    let id = if let zbus::zvariant::Value::I32(i) = &fields[0] {
        *i
    } else {
        return None;
    };
    let props: Props = if let zbus::zvariant::Value::Dict(dict) = &fields[1] {
        dict.iter()
            .filter_map(|(k, v)| {
                let key = if let zbus::zvariant::Value::Str(s) = k {
                    s.to_string()
                } else {
                    return None;
                };
                Some((key, zbus::zvariant::OwnedValue::try_from(v.clone()).ok()?))
            })
            .collect()
    } else {
        Props::new()
    };

    let item_type = prop_str(&props, "type");
    let is_separator = item_type == "separator";
    let label = truncate_label(extract_label_from_props(&props));
    let enabled = prop_bool(&props, "enabled", true);
    let visible = prop_bool(&props, "visible", true);
    let icon_name = {
        let s = prop_str(&props, "icon-name");
        if s.is_empty() { None } else { Some(s) }
    };

    Some(crate::surface::menu_popup::MenuItem {
        id: id.to_string(),
        label,
        enabled,
        visible,
        icon_name,
        children: vec![],
        is_separator,
    })
}

/// Fetch a DBusMenu layout from `menu_path` on `service` and convert it to a `MenuTree`.
///
/// Returns `None` if the menu D-Bus call fails.
pub(crate) async fn fetch_menu(
    conn: &zbus::Connection,
    service: &str,
    menu_path: &zbus::zvariant::OwnedObjectPath,
) -> Option<crate::surface::menu_popup::MenuTree> {
    let proxy = DbusMenuProxy::builder(conn)
        .destination(service)
        .ok()?
        .path(menu_path.as_ref())
        .ok()?
        .build()
        .await
        .ok()?;

    let _ = proxy.about_to_show(0).await;

    let (_, root) = proxy.get_layout(0, 1, &[]).await.ok()?;
    let (_, _, children) = &root;

    let items: Vec<crate::surface::menu_popup::MenuItem> = children
        .iter()
        .filter_map(owned_value_to_menu_item)
        .take(MAX_MENU_ITEMS)
        .collect();

    Some(crate::surface::menu_popup::MenuTree {
        items: limit_menu_items(items),
        anchor: (0, 0),
        screen_size: (0, 0),
    })
}

/// Send `Event("clicked")` to the DBusMenu for the selected item.
pub(crate) async fn send_menu_event(
    conn: &zbus::Connection,
    service: &str,
    menu_path: &zbus::zvariant::OwnedObjectPath,
    item_id: i32,
    event_id: &str,
) -> zbus::Result<()> {
    let proxy = DbusMenuProxy::builder(conn)
        .destination(service)?
        .path(menu_path.as_ref())?
        .build()
        .await?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    proxy
        .event(item_id, event_id, &zbus::zvariant::Value::I32(0), ts)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_dbusmenu_props_extracts_label() {
        use std::collections::HashMap;
        use zbus::zvariant::{OwnedValue, Value};

        let mut props: HashMap<String, OwnedValue> = HashMap::new();
        props.insert(
            "label".to_string(),
            OwnedValue::try_from(Value::from("Open")).unwrap(),
        );

        assert_eq!(extract_label_from_props(&props), "Open");
    }

    #[test]
    fn convert_dbusmenu_props_returns_empty_for_missing_label() {
        use std::collections::HashMap;

        let props: HashMap<String, zbus::zvariant::OwnedValue> = HashMap::new();
        assert_eq!(extract_label_from_props(&props), "");
    }

    #[test]
    fn truncate_label_keeps_short_label() {
        assert_eq!(truncate_label("Short".to_owned()), "Short");
    }

    #[test]
    fn truncate_label_caps_long_label() {
        let input = "x".repeat(MAX_MENU_LABEL_BYTES + 10);
        let out = truncate_label(input);
        assert_eq!(out.len(), MAX_MENU_LABEL_BYTES);
    }

    #[test]
    fn items_from_root_children_limits_item_count() {
        let children: Vec<crate::surface::menu_popup::MenuItem> = (0..MAX_MENU_ITEMS + 10)
            .map(|i| crate::surface::menu_popup::MenuItem {
                id: i.to_string(),
                label: format!("item {i}"),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            })
            .collect();

        let items = limit_menu_items(children);
        assert_eq!(items.len(), MAX_MENU_ITEMS);
        assert_eq!(items[0].id, "0");
        assert_eq!(
            items[MAX_MENU_ITEMS - 1].id,
            (MAX_MENU_ITEMS - 1).to_string()
        );
    }
}
