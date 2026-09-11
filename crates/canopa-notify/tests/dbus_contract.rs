mod support;

use canopa_notify::{ControlCommand, ControlReply};
use futures_util::StreamExt;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::process::Child;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const INTERFACE: &str = "org.freedesktop.Notifications";
const OBJECT_PATH: &str = "/org/freedesktop/Notifications";

// Only the public executable, D-Bus and control socket participate in this fixture.
// Keep process cleanup alive before readiness checks so assertion failures cannot
// leave a daemon behind. Each case owns isolated state and a distinct bus name.
struct Server {
    process: Child,
    socket: PathBuf,
    proxy: zbus::Proxy<'static>,
    _temp: tempfile::TempDir,
}

impl Server {
    async fn start(case: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("control.sock");
        let name = format!("org.canopa.Contract.{case}{}", std::process::id());
        let connection = zbus::connection::Builder::session()
            .expect("run this suite through bash scripts/test.sh for a private bus")
            .method_timeout(Duration::from_secs(3))
            .build()
            .await
            .unwrap();
        let proxy = zbus::Proxy::new(&connection, name.clone(), OBJECT_PATH, INTERFACE)
            .await
            .unwrap();
        let process = support::spawn_canopa_notify([
            "daemon",
            "--headless",
            "--socket",
            support::path_arg(&socket),
            "--dev-bus-name",
            &name,
        ]);
        let mut server = Self {
            process,
            socket,
            proxy,
            _temp: temp,
        };
        support::wait_for_status_with_child(&server.socket, &mut server.process);
        server
    }

    async fn notify(&self, replaces_id: u32, summary: &str) -> zbus::Message {
        self.proxy
            .call_method(
                "Notify",
                &(
                    "Contract test",
                    replaces_id,
                    "",
                    summary,
                    "",
                    Vec::<String>::new(),
                    HashMap::<String, zbus::zvariant::OwnedValue>::new(),
                    0i32,
                ),
            )
            .await
            .unwrap()
    }

    async fn control(&self, command: ControlCommand) -> ControlReply {
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut stream = tokio::net::UnixStream::connect(&self.socket).await.unwrap();
            stream
                .write_all(
                    format!("control {}\n", serde_json::to_string(&command).unwrap()).as_bytes(),
                )
                .await
                .unwrap();
            let mut response = String::new();
            BufReader::new(stream)
                .read_line(&mut response)
                .await
                .unwrap();
            serde_json::from_str(&response).unwrap()
        })
        .await
        .expect("control reply deadline")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.process.kill().ok();
        self.process.wait().ok();
    }
}

#[tokio::test]
async fn introspection_matches_the_complete_notifications_1_3_wire_contract() {
    let server = Server::start("Introspection").await;
    let introspection = zbus::Proxy::new(
        server.proxy.connection(),
        server.proxy.destination().clone(),
        OBJECT_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap();
    let xml: String = introspection.call("Introspect", &()).await.unwrap();
    // D-Bus introspection includes its standard DOCTYPE. The parser is local-only
    // and does not fetch that external DTD.
    let document = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .unwrap();
    let interface = document
        .root_element()
        .children()
        .find(|node| node.has_tag_name("interface") && node.attribute("name") == Some(INTERFACE))
        .expect("Notifications interface at the standard object path");
    let methods: BTreeMap<_, _> = interface
        .children()
        .filter(|node| node.has_tag_name("method"))
        .map(|node| {
            let mut input = Vec::new();
            let mut output = Vec::new();
            for argument in node.children().filter(|arg| arg.has_tag_name("arg")) {
                let signature = argument.attribute("type").unwrap();
                match argument.attribute("direction").unwrap_or("in") {
                    "in" => input.push(signature),
                    "out" => output.push(signature),
                    direction => panic!("invalid argument direction {direction}"),
                }
            }
            (node.attribute("name").unwrap(), (input, output))
        })
        .collect();
    // Independent literals from Desktop Notifications 1.3 sections 9.1 and 9.2.
    // Preserve argument boundaries: one (ssss) struct is not four output strings.
    assert_eq!(
        methods,
        BTreeMap::from([
            (
                "Notify",
                (vec!["s", "u", "s", "s", "s", "as", "a{sv}", "i"], vec!["u"])
            ),
            ("CloseNotification", (vec!["u"], vec![])),
            ("GetCapabilities", (vec![], vec!["as"])),
            ("GetServerInformation", (vec![], vec!["s", "s", "s", "s"])),
        ])
    );
    let signals: BTreeMap<_, _> = interface
        .children()
        .filter(|node| node.has_tag_name("signal"))
        .map(|node| {
            (
                node.attribute("name").unwrap(),
                node.children()
                    .filter(|arg| arg.has_tag_name("arg"))
                    .map(|arg| {
                        assert_eq!(arg.attribute("direction").unwrap_or("out"), "out");
                        arg.attribute("type").unwrap()
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        signals,
        BTreeMap::from([
            ("ActionInvoked", vec!["u", "s"]),
            ("ActivationToken", vec!["u", "s"]),
            ("NotificationClosed", vec!["u", "u"]),
        ])
    );
    assert!(
        !interface
            .children()
            .any(|node| node.has_tag_name("property"))
    );
}

fn assert_reply(message: &zbus::Message, signature: &str) {
    assert_eq!(
        message.header().message_type(),
        zbus::message::Type::MethodReturn
    );
    assert!(message.header().reply_serial().is_some());
    assert_eq!(message.body().signature(), signature);
}

#[tokio::test]
async fn all_four_methods_return_the_documented_types_and_server_identity() {
    let server = Server::start("Replies").await;
    let information = server
        .proxy
        .call_method("GetServerInformation", &())
        .await
        .unwrap();
    assert_reply(&information, "ssss");
    assert_eq!(
        information
            .body()
            .deserialize::<(String, String, String, String)>()
            .unwrap(),
        (
            "Canopa Notify".into(),
            "Canopa".into(),
            env!("CARGO_PKG_VERSION").into(),
            "1.3".into(),
        )
    );
    let capabilities = server
        .proxy
        .call_method("GetCapabilities", &())
        .await
        .unwrap();
    assert_reply(&capabilities, "as");
    let capabilities: Vec<String> = capabilities.body().deserialize().unwrap();
    let unique: BTreeSet<_> = capabilities.iter().map(String::as_str).collect();
    assert_eq!(
        unique.len(),
        capabilities.len(),
        "duplicate advertised capability"
    );
    assert_eq!(
        unique,
        BTreeSet::from([
            "actions",
            "action-icons",
            "body",
            "body-markup",
            "icon-static"
        ])
    );

    let notification = server.notify(0, "Original").await;
    assert_reply(&notification, "u");
    let id: u32 = notification.body().deserialize().unwrap();
    assert_ne!(id, 0);
    let replacement = server.notify(id, "Replacement").await;
    assert_reply(&replacement, "u");
    assert_eq!(replacement.body().deserialize::<u32>().unwrap(), id);
    let closure = server
        .proxy
        .call_method("CloseNotification", &(id,))
        .await
        .unwrap();
    assert_reply(&closure, "");
    assert!(closure.body().is_empty());
}

#[tokio::test]
async fn independent_dbus_client_reads_four_top_level_server_information_strings() {
    let server = Server::start("IndependentClient").await;
    // dbus-send is shipped by the same dbus package as the private-bus runner.
    // Its decoder is independent of the server/client Rust serialization stack.
    let output = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--reply-timeout=3000",
            &format!("--dest={}", server.proxy.destination()),
            OBJECT_PATH,
            "org.freedesktop.Notifications.GetServerInformation",
        ])
        .output()
        .expect("dbus-send is required; install the documented dbus test dependency");
    assert!(
        output.status.success(),
        "dbus-send failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).unwrap();
    let mut lines = output.lines();
    assert!(lines.next().unwrap().starts_with("method return "));
    assert_eq!(
        lines.map(str::trim).collect::<Vec<_>>(),
        vec![
            "string \"Canopa Notify\"".to_string(),
            "string \"Canopa\"".into(),
            format!("string \"{}\"", env!("CARGO_PKG_VERSION")),
            "string \"1.3\"".into(),
        ]
    );
}

fn method_error(result: zbus::Result<zbus::Message>) -> (String, zbus::Message) {
    let zbus::Error::MethodError(name, _, message) = result.expect_err("invalid call was accepted")
    else {
        panic!("expected an actual D-Bus error reply, not a client/transport failure");
    };
    assert_eq!(message.header().message_type(), zbus::message::Type::Error);
    assert_eq!(
        message.header().error_name().unwrap().as_str(),
        name.as_str()
    );
    assert!(message.header().reply_serial().is_some());
    (name.to_string(), message)
}

async fn next_signal(signals: &mut zbus::proxy::SignalStream<'_>) -> zbus::Message {
    let message = tokio::time::timeout(Duration::from_secs(3), signals.next())
        .await
        .expect("missing notification signal")
        .expect("signal stream ended");
    assert_eq!(message.header().message_type(), zbus::message::Type::Signal);
    assert_eq!(message.header().interface().unwrap().as_str(), INTERFACE);
    assert_eq!(message.header().path().unwrap().as_str(), OBJECT_PATH);
    message
}

#[tokio::test]
async fn invalid_closures_return_empty_errors_and_do_not_consume_notification_ids() {
    let server = Server::start("InvalidClosure").await;
    let mut signals = server.proxy.receive_all_signals().await.unwrap();
    for id in [0u32, 42, u32::MAX] {
        let (name, error) =
            method_error(server.proxy.call_method("CloseNotification", &(id,)).await);
        assert_eq!(name, "org.freedesktop.DBus.Error.Failed");
        assert_eq!(error.body().deserialize::<String>().unwrap(), "");
    }
    let id: u32 = server.notify(0, "Live").await.body().deserialize().unwrap();
    assert_eq!(id, 1, "invalid closure consumed an ID");
    server
        .proxy
        .call::<_, _, ()>("CloseNotification", &(id,))
        .await
        .unwrap();
    let closed = next_signal(&mut signals).await;
    assert_eq!(
        closed.header().member().unwrap().as_str(),
        "NotificationClosed"
    );
    assert_eq!(closed.body().signature(), "uu");
    assert_eq!(closed.body().deserialize::<(u32, u32)>().unwrap(), (id, 3));
    let dropped: u32 = server.notify(0, "").await.body().deserialize().unwrap();
    let closed = next_signal(&mut signals).await;
    assert_eq!(
        closed.body().deserialize::<(u32, u32)>().unwrap(),
        (dropped, 4)
    );
    for id in [id, dropped] {
        let (name, error) =
            method_error(server.proxy.call_method("CloseNotification", &(id,)).await);
        assert_eq!(name, "org.freedesktop.DBus.Error.Failed");
        assert_eq!(error.body().deserialize::<String>().unwrap(), "");
    }
    assert_eq!(
        server
            .notify(0, "Still available")
            .await
            .body()
            .deserialize::<u32>()
            .unwrap(),
        dropped + 1
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), signals.next())
            .await
            .is_err(),
        "invalid closure emitted an event"
    );
}

#[tokio::test]
async fn malformed_calls_and_admission_errors_leave_live_state_and_id_allocation_unchanged() {
    let server = Server::start("RejectedCalls").await;
    let id: u32 = server
        .notify(0, "Preserve me")
        .await
        .body()
        .deserialize()
        .unwrap();
    let before = server.control(ControlCommand::ActiveNotifications).await;
    let mut signals = server.proxy.receive_all_signals().await.unwrap();
    let empty_hints = HashMap::<String, zbus::zvariant::OwnedValue>::new();
    // These are malformed wire signatures, not malformed optional hint values.
    // The exact binding-generated error name is not prescribed by Notifications 1.3.
    for result in [
        server.proxy.call_method("Notify", &()).await,
        server
            .proxy
            .call_method(
                "Notify",
                &(
                    "Contract",
                    -1i32,
                    "",
                    "PRIVATE",
                    "",
                    Vec::<String>::new(),
                    &empty_hints,
                    0i32,
                ),
            )
            .await,
        server
            .proxy
            .call_method(
                "Notify",
                &(
                    "Contract",
                    id,
                    "",
                    "PRIVATE",
                    "",
                    "not-an-action-array",
                    &empty_hints,
                    0i32,
                ),
            )
            .await,
        server
            .proxy
            .call_method(
                "Notify",
                &(
                    "Contract",
                    id,
                    "",
                    "PRIVATE",
                    "",
                    Vec::<String>::new(),
                    &empty_hints,
                    u32::MAX,
                ),
            )
            .await,
        server
            .proxy
            .call_method("CloseNotification", &(-1i32,))
            .await,
    ] {
        let (_, error) = method_error(result);
        assert!(
            !error
                .body()
                .deserialize::<String>()
                .unwrap()
                .contains("PRIVATE")
        );
    }
    for method in ["GetCapabilities", "GetServerInformation"] {
        let (_, error) = method_error(server.proxy.call_method(method, &("PRIVATE",)).await);
        assert!(
            !error
                .body()
                .deserialize::<String>()
                .unwrap()
                .contains("PRIVATE")
        );
    }
    let (name, _) = method_error(server.proxy.call_method("NotAMethod", &()).await);
    assert_eq!(name, "org.freedesktop.DBus.Error.UnknownMethod");
    // Canopa's local admission error must be an actual error reply, not a zero ID
    // or an accepted replacement followed by an error-shaped control response.
    let (name, _) = method_error(
        server
            .proxy
            .call_method(
                "Notify",
                &(
                    "Contract",
                    id,
                    "",
                    "x".repeat(8_193),
                    "",
                    Vec::<String>::new(),
                    &empty_hints,
                    0i32,
                ),
            )
            .await,
    );
    assert_eq!(name, "org.freedesktop.DBus.Error.LimitsExceeded");
    assert_eq!(
        server.control(ControlCommand::ActiveNotifications).await,
        before
    );
    assert_eq!(
        server
            .notify(0, "Fresh")
            .await
            .body()
            .deserialize::<u32>()
            .unwrap(),
        id + 1
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), signals.next())
            .await
            .is_err(),
        "rejected call emitted an event"
    );
}

#[tokio::test]
async fn action_signals_preserve_opaque_keys_and_precede_nonresident_closure() {
    let server = Server::start("ActionOrder").await;
    let mut signals = server.proxy.receive_all_signals().await.unwrap();
    // Separate the wire namespace from retained thread IDs before invoking an action.
    let dropped: u32 = server.notify(0, "").await.body().deserialize().unwrap();
    let closed = next_signal(&mut signals).await;
    assert_eq!(
        closed.body().deserialize::<(u32, u32)>().unwrap(),
        (dropped, 4)
    );
    let action_key = " Open:λ-CASE ";
    let mut wire_id = 0u32;
    for resident in [true, false] {
        let id: u32 = server
            .proxy
            .call(
                "Notify",
                &(
                    "Contract",
                    wire_id,
                    "",
                    "Actionable",
                    "",
                    vec![action_key, "Open"],
                    HashMap::from([("resident", zbus::zvariant::OwnedValue::from(resident))]),
                    0i32,
                ),
            )
            .await
            .unwrap();
        if wire_id != 0 {
            assert_eq!(id, wire_id, "replacement changed the sender ID");
        }
        wire_id = id;
        let ControlReply::ActiveNotifications(active) =
            server.control(ControlCommand::ActiveNotifications).await
        else {
            panic!("expected active notifications");
        };
        assert_eq!(active.len(), 1);
        // Use the public control ID; never cast the sender-visible wire ID.
        let command = ControlCommand::InvokeNotificationAction {
            notification_id: active[0].id,
            action_key: action_key.into(),
        };
        for _ in 0..if resident { 2 } else { 1 } {
            assert_eq!(
                server.control(command.clone()).await,
                ControlReply::CommandApplied
            );
            let action = next_signal(&mut signals).await;
            assert_eq!(action.header().member().unwrap().as_str(), "ActionInvoked");
            assert_eq!(action.body().signature(), "us");
            assert_eq!(
                action.body().deserialize::<(u32, String)>().unwrap(),
                (wire_id, action_key.into())
            );
        }
        if resident {
            assert_eq!(
                server.control(ControlCommand::ActiveNotifications).await,
                ControlReply::ActiveNotifications(active)
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(100), signals.next())
                    .await
                    .is_err(),
                "resident action closed its notification"
            );
        } else {
            let closed = next_signal(&mut signals).await;
            assert_eq!(
                closed.header().member().unwrap().as_str(),
                "NotificationClosed"
            );
            assert_eq!(closed.body().signature(), "uu");
            assert_eq!(
                closed.body().deserialize::<(u32, u32)>().unwrap(),
                (wire_id, 2)
            );
            assert_eq!(
                server.control(ControlCommand::ActiveNotifications).await,
                ControlReply::ActiveNotifications(vec![])
            );
            let (_, error) = method_error(
                server
                    .proxy
                    .call_method("CloseNotification", &(wire_id,))
                    .await,
            );
            assert_eq!(error.body().deserialize::<String>().unwrap(), "");
        }
    }
}
