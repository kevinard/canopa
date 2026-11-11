use super::*;
use crate::{
    ServiceState,
    testing::{
        TestBus,
        bluez::{ADAPTER_PATH, DEVICE_PATH, FakeBluez},
    },
};
use futures_util::StreamExt;
use std::sync::atomic::Ordering::SeqCst;

#[tokio::test]
async fn snapshot_lists_powered_adapters_devices_and_watch_follows_changes() {
    let bus = TestBus::new();
    let client = bus.connect().await;
    assert_eq!(Snapshot::read(&client).await, Snapshot::default());
    let mut states = watch(client.clone());
    let next = async |states: &mut StateStream<Snapshot>| {
        tokio::time::timeout(Duration::from_secs(3), states.next())
            .await
            .unwrap()
            .unwrap()
    };
    assert_eq!(next(&mut states).await, ServiceState::Unavailable);
    let bluez = FakeBluez::serve(&bus, false).await;
    let ServiceState::Available(off) = next(&mut states).await else {
        panic!("snapshot expected");
    };
    assert_eq!(off.adapters.len(), 1);
    assert_eq!(off.adapters[0].label, "Test adapter");
    assert!(
        off.devices.is_empty(),
        "devices of an off adapter are hidden"
    );
    let owner = off.owner.clone().unwrap();
    set_powered(&client, &owner, ADAPTER_PATH, true)
        .await
        .unwrap();
    let ServiceState::Available(on) = next(&mut states).await else {
        panic!("snapshot expected");
    };
    assert!(on.adapters[0].powered);
    assert_eq!(on.devices[0].label, "Test headphones");
    start_discovery(&client, &owner, ADAPTER_PATH)
        .await
        .unwrap();
    assert!(bluez.state.scanning.load(SeqCst));
    stop_discovery(&client, &owner, ADAPTER_PATH).await.unwrap();
    assert!(!bluez.state.scanning.load(SeqCst));
    assert!(connect(&client, &owner, DEVICE_PATH).await.is_err());
    bluez.connection.close().await.unwrap();
    assert_eq!(next(&mut states).await, ServiceState::Unavailable);
}

#[tokio::test]
async fn pairing_asks_the_agent_trusts_and_connects() {
    let bus = TestBus::new();
    let bluez = FakeBluez::serve(&bus, true).await;
    let client = bus.connect().await;
    let snapshot = Snapshot::query(&client).await.unwrap();
    let attempt = Attempt {
        serial: 7,
        owner: snapshot.owner.unwrap(),
        device: snapshot.devices[0].path.clone(),
    };
    let (events, mut requests) = mpsc::channel(8);
    let (_stop, stop) = oneshot::channel();
    let address = bus.address.clone();
    let pairing = tokio::spawn(pair_with(attempt, events, stop, move || async move {
        Ok(zbus::connection::Builder::address(address.as_str())?
            .build()
            .await?)
    }));
    let Some(AgentEvent::Request(7, request)) = requests.recv().await else {
        panic!("confirmation expected");
    };
    assert_eq!(request.question, Question::Confirm(42));
    assert!(!request.question.needs_entry());
    request.reply.unwrap().send("yes".into()).unwrap();
    pairing.await.unwrap().unwrap();
    assert!(bluez.state.paired.load(SeqCst));
    assert!(bluez.state.trusted.load(SeqCst));
    assert!(bluez.state.connected.load(SeqCst));
    assert!(bluez.state.agent_owner.lock().unwrap().is_some());
}

#[test]
fn replies_follow_bluez_formats() {
    assert_eq!(Question::Pin.accepts("ab12"), Some("ab12".into()));
    assert_eq!(Question::Pin.accepts(""), None);
    assert_eq!(Question::Pin.accepts(&"1".repeat(17)), None);
    assert_eq!(Question::Passkey.accepts("123456"), Some("123456".into()));
    assert_eq!(Question::Passkey.accepts("12345a"), None);
    assert_eq!(
        Question::DisplayPinCode("0000".into()).accepts("0000"),
        None
    );
    assert_eq!(Question::Authorize.accepts(""), Some(String::new()));
}

#[tokio::test]
async fn devices_report_kind_battery_and_can_be_trusted_and_forgotten() {
    let bus = TestBus::new();
    let bluez = FakeBluez::serve(&bus, true).await;
    let client = bus.connect().await;
    let snapshot = Snapshot::query(&client).await.unwrap();
    let owner = snapshot.owner.clone().unwrap();
    let adapter = &snapshot.adapters[0];
    assert_eq!(adapter.address, "AA:BB:CC:DD:EE:FF");
    assert!(!adapter.discoverable && !adapter.discovering);
    let device = &snapshot.devices[0];
    assert_eq!(device.address, "00:11:22:33:44:55");
    assert_eq!(device.kind, DeviceKind::Headphones);
    assert_eq!(device.kind.icon(), "audio-headphones");
    assert_eq!(device.battery, Some(80));
    assert!(!device.trusted && !device.blocked);

    set_discoverable(&client, &owner, ADAPTER_PATH, true)
        .await
        .unwrap();
    set_trusted(&client, &owner, DEVICE_PATH, true)
        .await
        .unwrap();
    set_blocked(&client, &owner, DEVICE_PATH, true)
        .await
        .unwrap();
    start_discovery(&client, &owner, ADAPTER_PATH)
        .await
        .unwrap();
    let snapshot = Snapshot::query(&client).await.unwrap();
    assert!(snapshot.adapters[0].discoverable && snapshot.adapters[0].discovering);
    assert!(snapshot.devices[0].trusted && snapshot.devices[0].blocked);

    remove_device(&client, &owner, ADAPTER_PATH, DEVICE_PATH)
        .await
        .unwrap();
    assert!(bluez.state.removed.load(SeqCst));
    assert!(Snapshot::query(&client).await.unwrap().devices.is_empty());
    // A device that is gone is refused, not an outage.
    assert!(matches!(
        remove_device(&client, &owner, ADAPTER_PATH, DEVICE_PATH).await,
        Err(Error::Refused(_))
    ));
}

#[test]
fn icon_names_map_to_kinds() {
    assert_eq!(DeviceKind::from_icon("audio-headset"), DeviceKind::Headset);
    assert_eq!(DeviceKind::from_icon("input-mouse"), DeviceKind::Mouse);
    assert_eq!(DeviceKind::from_icon("input-gaming"), DeviceKind::Gamepad);
    assert_eq!(DeviceKind::from_icon("phone"), DeviceKind::Phone);
    assert_eq!(DeviceKind::from_icon(""), DeviceKind::Other);
    assert_eq!(
        DeviceKind::from_icon("audio-x-unknown"),
        DeviceKind::Speaker
    );
}
