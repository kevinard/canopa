use super::*;
use niribar::external_panel::canopa::CanopaSession;
use std::time::Duration;
use zbus::zvariant::OwnedObjectPath;

type Accessible = (String, OwnedObjectPath);

async fn find_named(connection: &zbus::Connection, name: &str) -> Option<Accessible> {
    let mut pending = vec![(
        "org.a11y.atspi.Registry".to_string(),
        OwnedObjectPath::try_from("/org/a11y/atspi/accessible/root").unwrap(),
    )];
    for _ in 0..1024 {
        let (bus, path) = pending.pop()?;
        let proxy = zbus::Proxy::new(
            connection,
            bus.as_str(),
            path.as_str(),
            "org.a11y.atspi.Accessible",
        )
        .await
        .ok()?;
        if proxy.get_property::<String>("Name").await.ok().as_deref() == Some(name) {
            return Some((bus, path));
        }
        if let Ok(children) = proxy
            .call::<_, _, Vec<Accessible>>("GetChildren", &())
            .await
        {
            pending.extend(children);
        }
    }
    None
}

async fn named(connection: &zbus::Connection, name: &str) -> Accessible {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(found) = find_named(connection, name).await {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("AT-SPI never exposed {name}"))
}

#[tokio::test]
async fn native_center_accessibility_exposes_geometry_focus_and_revision_owned_actions() {
    if std::env::var_os("CANOPA_ACCESSIBILITY_FIXTURE").is_none() {
        // Isolate both D-Bus and the AT-SPI socket, which is based on XDG_RUNTIME_DIR.
        const FIXTURE: &str = r#"
set -euo pipefail
fixture=$CANOPA_ACCESSIBILITY_ROOT
export XDG_RUNTIME_DIR="$fixture/runtime" XDG_CONFIG_HOME="$fixture/config"
export DCONF_BACKEND=memory CANOPA_ACCESSIBILITY_FIXTURE=1
unset AT_SPI_BUS_ADDRESS NO_AT_BRIDGE
mkdir -m 700 "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME"
launcher= registry=
cleanup() {
    if [[ -n "$registry" ]]; then kill "$registry" 2>/dev/null || true; wait "$registry" 2>/dev/null || true; fi
    if [[ -n "$launcher" ]]; then kill "$launcher" 2>/dev/null || true; wait "$launcher" 2>/dev/null || true; fi
}
trap cleanup EXIT
launcher_bin=/usr/lib/at-spi-bus-launcher
registry_bin=/usr/lib/at-spi2-registryd
if [[ ! -x "$launcher_bin" ]]; then launcher_bin=/usr/libexec/at-spi-bus-launcher; fi
if [[ ! -x "$registry_bin" ]]; then registry_bin=/usr/libexec/at-spi2-registryd; fi
"$launcher_bin" --launch-immediately --a11y=1 &
launcher=$!
for attempt in {1..100}; do
    if dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.GetNameOwner string:org.a11y.Bus >/dev/null 2>&1; then break; fi
    sleep 0.02
done
"$registry_bin" &
registry=$!
"$1" --exact app::canopa_accessibility::tests::native_center_accessibility_exposes_geometry_focus_and_revision_owned_actions --nocapture
"#;
        let fixture = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("timeout")
            .args(["30", "dbus-run-session"])
            .env("CANOPA_ACCESSIBILITY_ROOT", fixture.path())
            .args(["--", "bash", "-c", FIXTURE, "--"])
            .arg(std::env::current_exe().unwrap())
            .status()
            .unwrap();
        assert!(status.success(), "isolated AT-SPI test failed");

        return;
    }
    let session_bus = zbus::Connection::session().await.unwrap();
    let bus = zbus::Proxy::new(
        &session_bus,
        "org.a11y.Bus",
        "/org/a11y/bus",
        "org.a11y.Bus",
    )
    .await
    .unwrap();
    let address: String = bus.call("GetAddress", &()).await.unwrap();
    let connection = zbus::connection::Builder::address(address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let registry_bus = zbus::fdo::DBusProxy::new(&connection).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !registry_bus
            .name_has_owner(zbus::names::BusName::try_from("org.a11y.atspi.Registry").unwrap())
            .await
            .unwrap()
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("private AT-SPI registry did not acquire its name");

    let mut session = CanopaSession::default();
    let mut snapshot = content(&mut session, "Fourth action").await;
    let mut bridge = PanelAccessibility::new(iced::window::Id::unique());
    bridge.update(&snapshot, "notifications", true);
    use iced::advanced::{renderer::Headless, widget::Tree};
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let bounds = {
        let mut view = crate::app::canopa_panel::render(&snapshot, "notifications");
        let mut widgets = Tree::new(view.as_widget());
        let layout = view.as_widget_mut().layout(
            &mut widgets,
            &renderer,
            &iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(360.0, 1000.0)),
        );
        let mut measure = Measure::new(bridge.ids.keys().cloned(), Bounds::new());
        view.as_widget_mut().operate(
            &mut widgets,
            iced::advanced::Layout::new(&layout),
            &renderer,
            &mut iced::advanced::widget::operation::black_box(&mut measure),
        );
        measure.bounds
    };
    let rich_key = crate::app::canopa_panel::semantic_key(
        &snapshot,
        "notifications",
        &SemanticNode::Body("x".into(), Some(0)),
    );
    let plain_key = crate::app::canopa_panel::semantic_key(
        &snapshot,
        "notifications",
        &SemanticNode::Body("x/0".into(), None),
    );
    assert_ne!(
        bridge.ids[&rich_key], bridge.ids[&plain_key],
        "provider IDs aliased native nodes"
    );
    assert_ne!(
        bounds[&rich_key], bounds[&plain_key],
        "provider IDs aliased rendered bounds"
    );
    let selected = snapshot.action_id(
        InteractionTarget::Item {
            id: "message".into(),
        },
        &snapshot.content.sections[0].items[0].actions[3],
    );
    let expected = bounds[&crate::app::canopa_panel::semantic_key(
        &snapshot,
        "notifications",
        &SemanticNode::Action(selected.clone()),
    )];
    let canopa_panel_protocol::RichBodySegment::Link {
        action: link_control,
        ..
    } = &snapshot.content.sections[0].items[0]
        .rich_body
        .as_ref()
        .unwrap()
        .segments[3]
    else {
        panic!("body link")
    };
    let link_id = snapshot.action_id(
        InteractionTarget::Item {
            id: "message".into(),
        },
        link_control,
    );
    let expected_link = bounds[&crate::app::canopa_panel::semantic_key(
        &snapshot,
        "notifications",
        &SemanticNode::Action(link_id.clone()),
    )];
    bridge.set_bounds(bounds);
    named(&connection, "Before image").await;
    named(&connection, "After image").await;
    named(&connection, "Chart alternative").await;
    named(&connection, "Styled body only").await;
    named(&connection, "Rich identity body").await;
    named(&connection, "Plain identity body").await;
    assert!(
        find_named(&connection, "Obsolete plain fallback")
            .await
            .is_none()
    );
    let (bus, path) = named(&connection, "Fourth action").await;
    let accessible = zbus::Proxy::new(
        &connection,
        bus.as_str(),
        path.as_str(),
        "org.a11y.atspi.Accessible",
    )
    .await
    .unwrap();
    assert_eq!(
        accessible.call::<_, _, u32>("GetRole", &()).await.unwrap(),
        43
    );
    let component = zbus::Proxy::new(
        &connection,
        bus.as_str(),
        path.as_str(),
        "org.a11y.atspi.Component",
    )
    .await
    .unwrap();
    let ((x, y, width, height),): ((i32, i32, i32, i32),) =
        component.call("GetExtents", &(1u32,)).await.unwrap();
    assert!((x as f32 - expected.x).abs() <= 1.0 && (y as f32 - expected.y).abs() <= 1.0);
    assert!(
        (width as f32 - expected.width).abs() <= 1.0
            && (height as f32 - expected.height).abs() <= 1.0
    );
    assert!(
        component
            .call::<_, _, bool>("GrabFocus", &())
            .await
            .unwrap()
    );
    let message = tokio::time::timeout(Duration::from_secs(2), async {
        bridge.events.receiver.lock().await.recv().await
    })
    .await
    .unwrap()
    .unwrap();
    let Message::CanopaAccessibilityFocus {
        provider,
        action_id,
    } = message
    else {
        panic!("wrong focus event");
    };
    assert_eq!(provider, "notifications");
    assert_eq!(action_id, selected);
    assert!(snapshot.focus_action(&action_id));
    bridge.update(&snapshot, "notifications", true);
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let states: Vec<u32> = accessible.call("GetState", &()).await.unwrap();
            if states[0] & (1 << 12) != 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("native focus did not follow keyboard selection");
    let action = zbus::Proxy::new(
        &connection,
        bus.as_str(),
        path.as_str(),
        "org.a11y.atspi.Action",
    )
    .await
    .unwrap();
    assert!(
        action
            .call::<_, _, bool>("DoAction", &(0i32,))
            .await
            .unwrap()
    );
    let message = tokio::time::timeout(Duration::from_secs(2), async {
        bridge.events.receiver.lock().await.recv().await
    })
    .await
    .unwrap()
    .unwrap();
    assert!(
        matches!(message, Message::ExternalPanel(ExternalPanelEvent::ButtonActivated { provider_name, element_id })
        if provider_name == "notifications" && element_id == selected)
    );
    let (link_bus, link_path) = named(&connection, "Read online").await;
    let link_accessible = zbus::Proxy::new(
        &connection,
        link_bus.as_str(),
        link_path.as_str(),
        "org.a11y.atspi.Accessible",
    )
    .await
    .unwrap();
    assert_eq!(
        link_accessible
            .call::<_, _, u32>("GetRole", &())
            .await
            .unwrap(),
        88
    );
    let hyperlink = zbus::Proxy::new(
        &connection,
        link_bus.as_str(),
        link_path.as_str(),
        "org.a11y.atspi.Hyperlink",
    )
    .await
    .unwrap();
    assert_eq!(
        hyperlink
            .call::<_, _, String>("GetURI", &(0i32,))
            .await
            .unwrap(),
        "https://example.org/page"
    );
    let link_component = zbus::Proxy::new(
        &connection,
        link_bus.as_str(),
        link_path.as_str(),
        "org.a11y.atspi.Component",
    )
    .await
    .unwrap();
    let ((x, y, width, height),): ((i32, i32, i32, i32),) =
        link_component.call("GetExtents", &(1u32,)).await.unwrap();
    assert!((x as f32 - expected_link.x).abs() <= 1.0 && (y as f32 - expected_link.y).abs() <= 1.0);
    assert!(
        (width as f32 - expected_link.width).abs() <= 1.0
            && (height as f32 - expected_link.height).abs() <= 1.0
    );
    assert!(
        link_component
            .call::<_, _, bool>("GrabFocus", &())
            .await
            .unwrap()
    );
    let message = tokio::time::timeout(Duration::from_secs(2), async {
        bridge.events.receiver.lock().await.recv().await
    })
    .await
    .unwrap()
    .unwrap();
    assert!(
        matches!(message, Message::CanopaAccessibilityFocus { action_id, .. } if action_id == link_id)
    );
    assert!(snapshot.focus_action(&link_id));
    bridge.update(&snapshot, "notifications", true);
    let link = zbus::Proxy::new(
        &connection,
        link_bus.as_str(),
        link_path.as_str(),
        "org.a11y.atspi.Action",
    )
    .await
    .unwrap();
    assert!(link.call::<_, _, bool>("DoAction", &(0i32,)).await.unwrap());
    let message = tokio::time::timeout(Duration::from_secs(2), async {
        bridge.events.receiver.lock().await.recv().await
    })
    .await
    .unwrap()
    .unwrap();
    assert!(
        matches!(message, Message::ExternalPanel(ExternalPanelEvent::ButtonActivated { element_id, .. }) if element_id == link_id)
    );
    let replacement = content(&mut session, "Replacement fourth action").await;
    bridge.update(&replacement, "notifications", true);
    named(&connection, "Replacement fourth action").await;
    let _ = action.call::<_, _, bool>("DoAction", &(0i32,)).await;
    let _ = link.call::<_, _, bool>("DoAction", &(0i32,)).await;
    assert!(bridge.events.receiver.lock().await.try_recv().is_err());
    drop(bridge);
    tokio::time::timeout(Duration::from_secs(5), async {
        while find_named(&connection, "Replacement fourth action")
            .await
            .is_some()
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("closed Center retained accessible controls");
}

async fn content(session: &mut CanopaSession, fourth: &str) -> CanopaSnapshot {
    let content = serde_json::json!({
        "type": "replace_content", "protocol_version": 4,
        "content": { "title": "Notifications", "error": null, "actions": [],
            "sections": [{ "id": "mail", "title": "Mail", "controls": [],
                "items": [{ "id": "message", "title": "New message", "body": "Before image Chart alternative After image",
                    "rich_body": {"segments": [
                        {"kind": "text", "text": "Before image", "style": {"bold": false, "italic": false, "underline": false}},
                        {"kind": "image", "image": {"source": {"kind": "local_path", "path": "/missing-chart.png"}, "alt": "Chart alternative"}},
                        {"kind": "text", "text": "After image", "style": {"bold": false, "italic": false, "underline": false}},
                        {"kind":"link", "uri":"https://example.org/page", "action":{"key":"body-link", "label":"Read online", "value":"link-revision"}, "style":{"bold":true,"italic":false,"underline":false}}
                    ]},
                    "metadata": [], "actions": [
                        {"key": "first", "label": "First action", "value": null},
                        {"key": "second", "label": "Second action", "value": null},
                        {"key": "third", "label": "Third action", "value": null},
                        {"key": "opaque/α", "label": fourth, "value": "revision-bound"}
                    ]
                },
                {"id":"x", "title":"Rich identity", "rich_body":{"segments":[
                    {"kind":"text", "text":"Rich identity body", "style":{"bold":false,"italic":false,"underline":false}}
                ]}, "metadata":[], "actions":[]},
                {"id":"x/0", "title":"Plain identity", "body":"Plain identity body", "metadata":[], "actions":[]},
                {"id":"styled", "title":"Styled identity", "body":"Obsolete plain fallback",
                 "styled_body":{"runs":[{"text":"Styled body only", "style":{"bold":true,"italic":false,"underline":false}}]},
                 "metadata":[], "actions":[]}
                ]
            }]
        }
    });
    let mut bytes = serde_json::to_vec(&content).unwrap();
    bytes.push(b'\n');
    session
        .read_content(&mut tokio::io::BufReader::new(bytes.as_slice()))
        .await
        .unwrap()
}
