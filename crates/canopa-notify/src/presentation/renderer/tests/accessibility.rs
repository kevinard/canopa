use super::*;
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
async fn native_accessibility_exposes_content_actions_and_retires_old_controls() {
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
"$1" --exact presentation::renderer::tests::accessibility::native_accessibility_exposes_content_actions_and_retires_old_controls --nocapture
"#;
        let fixture = tempfile::tempdir().unwrap();
        assert_cmd::Command::new("dbus-run-session")
            .env("CANOPA_ACCESSIBILITY_ROOT", fixture.path())
            .args(["--", "bash", "-c", FIXTURE, "--"])
            .arg(std::env::current_exe().unwrap())
            .timeout(Duration::from_secs(30))
            .assert()
            .success()
            .stdout(predicates::str::contains("1 passed"));
        return;
    }
    let session = zbus::Connection::session().await.unwrap();
    let bus = zbus::Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus")
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

    let mut cache_events = zbus::MessageStream::for_match_rule(
        zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.a11y.atspi.Cache")
            .unwrap()
            .build(),
        &connection,
        Some(512),
    )
    .await
    .unwrap();
    let mut announcements = zbus::MessageStream::for_match_rule(
        zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.a11y.atspi.Event.Object")
            .unwrap()
            .member("Announcement")
            .unwrap()
            .build(),
        &connection,
        Some(16),
    )
    .await
    .unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
    let mut bridge = crate::presentation::renderer::accessibility::BannerAccessibility::new(sender);
    let mut banner = banner_view(42, "Message body and image alternative");
    banner.body_content = crate::intake::interpreted_body_content(
        "Message body and image alternative <a href='https://example.org/page'>Read online</a>",
    );
    banner.body_segments = banner_body_segments_from_body_content(&banner.body_content);
    banner.actions = ["Reply", "Archive", "Read", "Fourth action"]
        .into_iter()
        .enumerate()
        .map(|(index, label)| BannerAction {
            key: format!("opaque/{index}/α"),
            label: label.into(),
            icon: None,
        })
        .collect();
    bridge.update(&[banner.clone()]);
    tokio::time::timeout(Duration::from_secs(5), bridge.activation())
        .await
        .unwrap()
        .unwrap();
    bridge.publish_after_activation();
    tokio::time::timeout(Duration::from_secs(3), announcements.next())
        .await
        .expect("first displayed banner was not announced")
        .unwrap()
        .unwrap();
    bridge.publish_after_activation();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), announcements.next())
            .await
            .is_err(),
        "unchanged banner repeated its announcement"
    );
    use iced::advanced::{renderer::Headless, widget::Tree};
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let bounds = {
        let mut card = notification_banner_widget(&banner, &Default::default());
        let mut widgets = Tree::new(card.as_widget());
        let layout = card.as_widget_mut().layout(
            &mut widgets,
            &renderer,
            &iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(360.0, 1000.0)),
        );
        let mut measure = crate::presentation::renderer::accessibility_bounds::Measure::new(
            (0..8)
                .chain(std::iter::once(accessibility_bounds::link_slot(1)))
                .map(|slot| (banner.target(), slot)),
            Default::default(),
        );
        card.as_widget_mut().operate(
            &mut widgets,
            iced::advanced::Layout::new(&layout),
            &renderer,
            &mut iced::advanced::widget::operation::black_box(&mut measure),
        );
        measure.bounds
    };
    let expected = bounds[&(banner.target(), 7)];
    let expected_link = bounds[&(banner.target(), accessibility_bounds::link_slot(1))];
    bridge.set_bounds(bounds);
    named(&connection, "Message body and image alternative").await;
    let (bus, path) = named(&connection, "Fourth action").await;
    let accessible = zbus::Proxy::new(
        &connection,
        bus.as_str(),
        path.as_str(),
        "org.a11y.atspi.Accessible",
    )
    .await
    .unwrap();
    let role: u32 = accessible.call("GetRole", &()).await.unwrap();
    assert_eq!(role, 43, "AT-SPI push-button role");
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
    let action = zbus::Proxy::new(
        &connection,
        bus.as_str(),
        path.as_str(),
        "org.a11y.atspi.Action",
    )
    .await
    .unwrap();
    let invoked: bool = action.call("DoAction", &(0i32,)).await.unwrap();
    assert!(invoked);
    let received = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(received, BannerInteraction::InvokeAction { target, action_key, .. }
        if target == banner.target() && action_key == "opaque/3/α")
    );
    let (link_bus, link_path) = named(&connection, "Read online").await;
    let accessible_link = zbus::Proxy::new(
        &connection,
        link_bus.as_str(),
        link_path.as_str(),
        "org.a11y.atspi.Accessible",
    )
    .await
    .unwrap();
    assert_eq!(
        accessible_link
            .call::<_, _, u32>("GetRole", &())
            .await
            .unwrap(),
        88,
        "AT-SPI link role"
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
    let link = zbus::Proxy::new(
        &connection,
        link_bus.as_str(),
        link_path.as_str(),
        "org.a11y.atspi.Action",
    )
    .await
    .unwrap();
    assert!(link.call::<_, _, bool>("DoAction", &(0i32,)).await.unwrap());
    let event = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(event, BannerInteraction::OpenLink { target, segment_index: 1, .. } if target == banner.target())
    );
    banner.revision += 1;
    banner.actions[3].label = "Replacement fourth action".into();
    bridge.update(&[banner]);
    named(&connection, "Replacement fourth action").await;
    tokio::time::timeout(Duration::from_secs(5), async {
        let (mut added, mut removed) = (false, false);
        while !added || !removed {
            let message = cache_events.next().await.unwrap().unwrap();
            let header = message.header();
            match header.member().unwrap().as_str() {
                "AddAccessible" => {
                    assert_eq!(wire_signature(&message), "((so)(so)(so)iiassusau)");
                    added = true;
                }
                "RemoveAccessible" => {
                    assert_eq!(wire_signature(&message), "(so)");
                    removed = true;
                }
                _ => {}
            }
        }
    })
    .await
    .expect("missing accessibility cache lifecycle signals");
    let _ = action.call::<_, _, bool>("DoAction", &(0i32,)).await;
    let _ = link.call::<_, _, bool>("DoAction", &(0i32,)).await;
    assert!(
        receiver.try_recv().is_err(),
        "retired accessible control invoked replacement"
    );
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
    .expect("closed surface stayed in the accessibility tree");
}

// zvariant normalizes a bare argument list and a structure to the same Signature.
// Read header field 8 verbatim so a missing structure boundary cannot pass this test.
fn wire_signature(message: &zbus::Message) -> &str {
    let bytes = message.data().bytes();
    let length = bytes[12..16].try_into().unwrap();
    let length = match bytes[0] {
        b'l' => u32::from_le_bytes(length),
        b'B' => u32::from_be_bytes(length),
        _ => panic!("invalid message endianness"),
    } as usize;
    let header = &bytes[..16 + length];
    let offset = (16..header.len())
        .step_by(8)
        .find(|&offset| header.get(offset..offset + 4) == Some(&[8, 1, b'g', 0]))
        .expect("missing signature header field");
    let length = header[offset + 4] as usize;
    assert_eq!(header[offset + 5 + length], 0);
    std::str::from_utf8(&header[offset + 5..offset + 5 + length]).unwrap()
}

#[test]
fn cache_wire_check_distinguishes_structure_from_flat_arguments() {
    let reference = (
        "bus",
        zbus::zvariant::ObjectPath::try_from("/object").unwrap(),
    );
    let flat = zbus::Message::signal("/cache", "org.a11y.atspi.Cache", "RemoveAccessible")
        .unwrap()
        .build(&reference)
        .unwrap();
    let structured = zbus::Message::signal("/cache", "org.a11y.atspi.Cache", "RemoveAccessible")
        .unwrap()
        .build(&(reference,))
        .unwrap();
    assert_eq!(wire_signature(&flat), "so");
    assert_eq!(wire_signature(&structured), "(so)");
}
