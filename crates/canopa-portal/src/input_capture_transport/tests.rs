//! A private transport fixture; it does not register an InputCapture backend.
use super::*;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
use tokio::sync::Mutex as AsyncMutex;
use zbus::{fdo, message::Header};

#[test]
fn provider_negotiation_keeps_v1_pointer_only_and_declines_unknown_versions_or_touch_only() {
    assert_eq!(provider_capabilities(1, 3).unwrap(), 2);
    assert_eq!(provider_capabilities(2, 7).unwrap(), 3);
    assert_eq!(provider_capabilities(2, 1).unwrap(), 1);
    for (version, mask) in [(0, 3), (3, 3), (1, 1), (2, 4), (2, 0)] {
        assert!(provider_capabilities(version, mask).is_err());
    }
}

#[test]
fn worker_bootstrap_requires_nonempty_approved_input_classes() {
    let mut input = Input {
        compositor: ":1.1".into(),
        compositor_pid: 1,
        ipc_socket: "/tmp/capture-test".into(),
        backend: ":1.2".into(),
        caller: ":1.3".into(),
        grants: 2,
    };
    for grants in [1, 2, 3] {
        input.grants = grants;
        input.validate().unwrap();
    }
    for grants in [0, 4, 7, u32::MAX] {
        input.grants = grants;
        assert!(input.validate().is_err());
    }
}

struct Slot {
    owner: OwnedUniqueName,
    bridge: AsyncMutex<Option<Bridge>>,
    revoker: Revoker,
    snapshot: watch::Receiver<Snapshot>,
    closed: watch::Receiver<bool>,
    events: Arc<Mutex<VecDeque<Notice>>>,
    pid: u32,
}
struct State {
    connection: Connection,
    connector: Connector,
    endpoint: Endpoint,
    slots: Mutex<HashMap<u32, Arc<Slot>>>,
    next: AtomicU32,
}
struct Probe(Arc<State>);
fn fail(error: impl std::fmt::Display) -> fdo::Error {
    fdo::Error::Failed(error.to_string())
}
fn caller(header: &Header<'_>) -> fdo::Result<OwnedUniqueName> {
    Ok(header
        .sender()
        .ok_or_else(|| fdo::Error::AccessDenied("Missing sender".into()))?
        .to_owned()
        .into())
}
impl Probe {
    fn slot(&self, id: u32, header: &Header<'_>) -> fdo::Result<Arc<Slot>> {
        let slot = self
            .0
            .slots
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| fdo::Error::Failed("Unknown fixture lease".into()))?;
        if slot.owner != caller(header)? {
            return Err(fdo::Error::AccessDenied(
                "Fixture lease belongs to another owner".into(),
            ));
        }
        Ok(slot)
    }
}
#[zbus::interface(name = "org.canopa.InputCaptureFixture")]
impl Probe {
    async fn start(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<(u32, u32)> {
        let owner = caller(&header)?;
        let (_cancel, cancelled) = watch::channel(false);
        let mut bridge = self
            .0
            .connector
            .start(
                &self.0.connection,
                &self.0.endpoint,
                owner.clone(),
                2,
                cancelled,
            )
            .await
            .map_err(fail)?;
        let pid = bridge.worker_pid();
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let recording = events.clone();
        let mut notices = bridge.take_events().unwrap();
        tokio::spawn(async move {
            while let Some(notice) = notices.recv().await {
                let mut recording = recording.lock().unwrap();
                recording.push_back(notice);
                if recording.len() > 128 {
                    recording.pop_front();
                }
            }
        });
        let slot = Arc::new(Slot {
            owner,
            revoker: bridge.revoker(),
            snapshot: bridge.snapshot(),
            closed: bridge.closed(),
            bridge: AsyncMutex::new(Some(bridge)),
            events,
            pid,
        });
        let id = self.0.next.fetch_add(1, Ordering::Relaxed);
        self.0.slots.lock().unwrap().insert(id, slot);
        Ok((id, pid))
    }
    #[zbus(out_args("fd"))]
    async fn connect(
        &self,
        id: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<zbus::zvariant::OwnedFd> {
        let slot = self.slot(id, &header)?;
        let mut bridge = slot.bridge.lock().await;
        let bridge = bridge
            .as_mut()
            .ok_or_else(|| fail("Fixture lease closed"))?;
        Ok(bridge.connect().await.map_err(fail)?.into())
    }
    async fn control(
        &self,
        id: u32,
        command: String,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<String> {
        if command.len() > wire::MAX_FRAME {
            return Err(fdo::Error::InvalidArgs("Command too large".into()));
        }
        let command: Command = serde_json::from_str(&command).map_err(fail)?;
        let slot = self.slot(id, &header)?;
        let bridge = slot.bridge.lock().await;
        let bridge = bridge
            .as_ref()
            .ok_or_else(|| fail("Fixture lease closed"))?;
        serde_json::to_string(&bridge.command(command).await.map_err(fail)?).map_err(fail)
    }
    async fn state(&self, id: u32, #[zbus(header)] header: Header<'_>) -> fdo::Result<String> {
        let slot = self.slot(id, &header)?;
        let snapshot = slot.snapshot.borrow().clone();
        serde_json::to_string(
            &serde_json::json!({"state":snapshot,"closed":*slot.closed.borrow(),"pid":slot.pid,
            "events":slot.events.lock().unwrap().iter().collect::<Vec<_>>()}),
        )
        .map_err(fail)
    }
    async fn stop(&self, id: u32, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        let slot = self.slot(id, &header)?;
        slot.revoker.revoke();
        if let Some(bridge) = slot.bridge.lock().await.take() {
            bridge.stop().await.map_err(fail)?;
        }
        self.0.slots.lock().unwrap().remove(&id);
        Ok(())
    }
}

#[tokio::test]
#[ignore = "private compositor/EI process fixture; driven by test-input-capture-worker.py"]
async fn native_receiver_fixture() {
    assert_eq!(
        std::env::var("CANOPA_INPUT_CAPTURE_NATIVE").as_deref(),
        Ok("1")
    );
    let connection = Connection::session().await.unwrap();
    let endpoint = Endpoint::resolve(
        &connection,
        PathBuf::from(std::env::var_os("NIRI_SOCKET").unwrap()),
    )
    .await
    .unwrap();
    let process = Arc::new(crate::process::DialogProcess {
        executable: PathBuf::from(std::env::var_os("CANOPA_PORTAL_BINARY").unwrap()),
        arguments: Vec::new(),
    });
    let state = Arc::new(State {
        connection: connection.clone(),
        connector: Connector::new(process),
        endpoint,
        slots: Mutex::new(HashMap::new()),
        next: AtomicU32::new(1),
    });
    connection
        .object_server()
        .at("/org/canopa/InputCaptureFixture", Probe(state.clone()))
        .await
        .unwrap();
    connection
        .request_name("org.canopa.InputCaptureFixture")
        .await
        .unwrap();
    println!("capture-receiver-fixture-ready");
    tokio::task::spawn_blocking(|| {
        let _ = std::io::stdin().read_line(&mut String::new());
    })
    .await
    .unwrap();
    let slots = std::mem::take(&mut *state.slots.lock().unwrap());
    for slot in slots.into_values() {
        slot.revoker.revoke();
        if let Some(bridge) = slot.bridge.lock().await.take() {
            let _ = bridge.stop().await;
        }
    }
}
