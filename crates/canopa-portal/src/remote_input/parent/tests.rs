use super::*;
use std::io::Write;
use tokio::io::AsyncReadExt;

struct Helper {
    _root: tempfile::TempDir,
    process: DialogProcess,
    marker: PathBuf,
}
impl Helper {
    fn new(mode: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("helper.py");
        let marker = root.path().join("state");
        std::fs::write(&script, r#"
import array, json, os, select, socket, struct, sys, time
mode, marker = sys.argv[1:3]
request = json.loads(sys.stdin.buffer.readline())
open(marker + '.request', 'w').write(json.dumps(request))
open(marker + '.started', 'w').close()
if mode == 'stall':
    select.select([0], [], [], 5)
    sys.exit(0)
output = socket.socket(fileno=1)
if mode == 'foreign':
    left, right = socket.socketpair()
    delegate = os.fork()
    if not delegate:
        left.close()
        server, client = socket.socketpair()
        right.sendmsg([b'1'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array('i', [client.fileno()]))])
        client.close(); right.close()
        server.settimeout(5)
        try: server.recv(1)
        except TimeoutError: pass
        os._exit(0)
    right.close()
    _, control, _, _ = left.recvmsg(1, socket.CMSG_SPACE(4))
    rights = array.array('i'); rights.frombytes(control[0][2]); left.close()
    output.sendmsg([b'\1'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, rights)])
    for fd in rights: os.close(fd)
    select.select([0], [], [], 5)
    os.waitpid(delegate, 0)
    sys.exit(0)
server, client = socket.socketpair()
rights = [client.fileno()]
if mode == 'multiple': rights.append(server.fileno())
if mode == 'file': rights = [os.open(marker, os.O_CREAT | os.O_RDWR, 0o600)]
if mode == 'truncated': rights = rights * 4
output.sendmsg([b'\2' if mode == 'payload' else b'\1'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array('i', rights))])
client.close()
if mode.startswith('legacy'):
    assert request['transport'] == 'legacy_keyboard'
    while True:
        readable, _, _ = select.select([0, server], [], [], 5)
        if 0 in readable or not readable: break
        data = b''
        while len(data) < 16:
            chunk = server.recv(16 - len(data))
            if not chunk: break
            data += chunk
        if len(data) != 16: break
        sequence, kind, value, state = struct.unpack('<4I', data)
        open(marker + '.commanded', 'w').close()
        if mode == 'legacy-no-ack':
            select.select([0], [], [], 5)
            break
        status = value if kind == 1 and value <= 4 else 0
        if mode == 'legacy-bad-ack': sequence += 1
        try:
            server.sendall(bytes(24) + struct.pack('<6I', sequence, status, 0, 0, 0, 0))
        except (BrokenPipeError, ConnectionResetError): break
    open(marker + '.closed', 'w').close()
    sys.exit(0)
try: server.sendall(b'probe')
except (BrokenPipeError, ConnectionResetError): pass
readable, _, _ = select.select([0, server], [], [], 5)
if server in readable:
    try: server.recv(1)
    except ConnectionResetError: pass
open(marker + '.closed', 'w').close()
if mode == 'late-exit':
    deadline = time.monotonic() + 5
    while not os.path.exists(marker + '.gate') and time.monotonic() < deadline: time.sleep(.01)
"#).unwrap();
        Self {
            process: DialogProcess {
                executable: "python3".into(),
                arguments: vec![
                    script.into_os_string(),
                    mode.into(),
                    marker.clone().into_os_string(),
                ],
            },
            _root: root,
            marker,
        }
    }
    fn input(&self) -> Input {
        Input {
            compositor: ":1.1".into(),
            compositor_pid: std::process::id(),
            ipc_socket: "/tmp/unused-input-test".into(),
            backend: ":1.2".into(),
            caller: ":1.3".into(),
            grants: 1,
            transport: Transport::Eis,
            capture: None,
        }
    }
    fn path(&self, suffix: &str) -> PathBuf {
        PathBuf::from(format!("{}.{}", self.marker.display(), suffix))
    }
}
async fn until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
async fn start(
    helper: &Helper,
    slots: Arc<Semaphore>,
    cancelled: watch::Receiver<bool>,
) -> anyhow::Result<(OwnedFd, InputLease)> {
    let permit = slots.try_acquire_owned()?;
    launch(&helper.process, helper.input(), cancelled, permit).await
}
async fn start_legacy(
    helper: &Helper,
    slots: Arc<Semaphore>,
    cancelled: watch::Receiver<bool>,
) -> anyhow::Result<super::super::LegacyInput> {
    let mut input = helper.input();
    input.transport = Transport::LegacyKeyboard;
    let permit = slots.try_acquire_owned()?;
    let (fd, lease) = launch(&helper.process, input, cancelled, permit).await?;
    super::super::LegacyInput::new(fd, lease)
}

#[tokio::test]
async fn legacy_statuses_preserve_the_lease_and_share_admission_with_eis() {
    use crate::remote_input::KeyboardStatus;
    use crate::screencast::remote::KeyboardInput;
    let helper = Helper::new("legacy");
    let other = Helper::new("valid");
    let slots = Arc::new(Semaphore::new(1));
    let (_cancel, cancelled) = watch::channel(false);
    let mut input = start_legacy(&helper, slots.clone(), cancelled.clone())
        .await
        .unwrap();
    assert!(start(&other, slots.clone(), cancelled).await.is_err());
    assert!(!other.path("started").exists());
    assert!(
        input
            .send(KeyboardInput::Keycode {
                code: 0,
                pressed: true
            })
            .await
            .is_err()
    );
    assert!(!*input.closed().borrow());
    assert_eq!(
        input
            .send(KeyboardInput::Keycode {
                code: 42,
                pressed: true
            })
            .await
            .unwrap(),
        KeyboardStatus::Accepted
    );
    for (symbol, expected) in [
        (1, KeyboardStatus::Invalid),
        (2, KeyboardStatus::Unavailable),
        (3, KeyboardStatus::Conflict),
        (4, KeyboardStatus::Limit),
    ] {
        assert_eq!(
            input
                .send(KeyboardInput::Keysym {
                    symbol,
                    pressed: true
                })
                .await
                .unwrap(),
            expected
        );
        assert!(!*input.closed().borrow());
    }
    input.stop().await.unwrap();
    assert_eq!(slots.available_permits(), 1);
    assert!(helper.path("closed").exists());
}

#[tokio::test]
async fn a_legacy_ack_for_another_command_revokes_the_worker_and_reaps_it() {
    use crate::screencast::remote::KeyboardInput;
    let helper = Helper::new("legacy-bad-ack");
    let slots = Arc::new(Semaphore::new(1));
    let (_cancel, cancelled) = watch::channel(false);
    let mut input = start_legacy(&helper, slots.clone(), cancelled)
        .await
        .unwrap();
    assert!(
        input
            .send(KeyboardInput::Keycode {
                code: 42,
                pressed: true
            })
            .await
            .is_err()
    );
    assert!(*input.closed().borrow());
    input.stop().await.unwrap();
    assert!(helper.path("closed").exists());
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn dropping_legacy_input_during_a_pending_ack_releases_the_worker_and_admission() {
    use crate::screencast::remote::KeyboardInput;
    let helper = Helper::new("legacy-no-ack");
    let slots = Arc::new(Semaphore::new(1));
    let (_cancel, cancelled) = watch::channel(false);
    let mut input = start_legacy(&helper, slots.clone(), cancelled)
        .await
        .unwrap();
    {
        let send = input.send(KeyboardInput::Keycode {
            code: 42,
            pressed: true,
        });
        tokio::pin!(send);
        tokio::select! {
            result = &mut send => panic!("unexpected acknowledgement: {result:?}"),
            _ = until(|| helper.path("commanded").exists()) => {},
        }
    }
    drop(input);
    until(|| helper.path("closed").exists() && slots.available_permits() == 1).await;
}
#[tokio::test]
async fn descriptor_transfer_keeps_only_the_parent_lease_and_client_eof_reaps_the_worker() {
    let helper = Helper::new("valid");
    // Older worker decoders deny unknown fields. Default EIS bootstrap remains
    // byte-contract compatible when the new optional legacy mode is unused.
    assert!(
        serde_json::to_value(helper.input())
            .unwrap()
            .get("transport")
            .is_none()
    );
    let slots = Arc::new(Semaphore::new(1));
    let (cancel, cancelled) = watch::channel(false);
    let (fd, lease) = start(&helper, slots.clone(), cancelled).await.unwrap();
    drop(cancel); // A completed request does not revoke its active worker.
    let stream = UnixStream::from(fd);
    stream.set_nonblocking(true).unwrap();
    let mut stream = tokio::net::UnixStream::from_std(stream).unwrap();
    let mut bytes = [0; 5];
    stream.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"probe");
    assert_eq!(slots.available_permits(), 0);
    drop(stream);
    let mut closed = lease.closed();
    until(|| *closed.borrow_and_update()).await;
    lease.stop().await.unwrap();
    assert_eq!(slots.available_permits(), 1);
}
#[tokio::test]
async fn malformed_or_foreign_descriptors_are_rejected_and_admission_recovers() {
    for mode in ["payload", "multiple", "truncated", "file", "foreign"] {
        let helper = Helper::new(mode);
        let slots = Arc::new(Semaphore::new(1));
        let (_cancel, cancelled) = watch::channel(false);
        assert!(
            start(&helper, slots.clone(), cancelled).await.is_err(),
            "{mode}"
        );
        until(|| slots.available_permits() == 1).await;
    }
}
#[tokio::test]
async fn dropping_a_pending_start_cancels_and_reaps_its_child() {
    let helper = Arc::new(Helper::new("stall"));
    let slots = Arc::new(Semaphore::new(1));
    let (_cancel, cancelled) = watch::channel(false);
    let task = {
        let helper = helper.clone();
        let slots = slots.clone();
        tokio::spawn(async move { start(&helper, slots, cancelled).await })
    };
    until(|| helper.path("started").exists()).await;
    assert_eq!(slots.available_permits(), 0);
    task.abort();
    let _ = task.await;
    until(|| slots.available_permits() == 1).await;
}
#[tokio::test]
async fn cancellation_before_ready_and_active_drop_both_close_the_pipe() {
    let helper = Helper::new("stall");
    let slots = Arc::new(Semaphore::new(1));
    let (cancel, cancelled) = watch::channel(false);
    let pending = start(&helper, slots.clone(), cancelled);
    tokio::pin!(pending);
    tokio::select! { _=&mut pending=>panic!("stalled helper became ready"), _=async { until(|| helper.path("started").exists()).await; }=>{} }
    cancel.send_replace(true);
    assert!(pending.await.is_err());
    until(|| slots.available_permits() == 1).await;
    let helper = Helper::new("valid");
    let (_cancel, cancelled) = watch::channel(false);
    let (fd, lease) = start(&helper, slots.clone(), cancelled).await.unwrap();
    let mut closed = lease.closed();
    drop(lease);
    until(|| *closed.borrow_and_update()).await;
    drop(fd);
    assert!(helper.path("closed").exists());
}
#[tokio::test]
async fn admission_is_retained_until_the_process_exits_after_its_pipe_closed() {
    let helper = Helper::new("late-exit");
    let slots = Arc::new(Semaphore::new(1));
    let (_cancel, cancelled) = watch::channel(false);
    let (fd, lease) = start(&helper, slots.clone(), cancelled).await.unwrap();
    drop(lease);
    until(|| helper.path("closed").exists()).await;
    assert_eq!(slots.available_permits(), 0);
    assert!(slots.clone().try_acquire_owned().is_err());
    std::fs::File::create(helper.path("gate"))
        .unwrap()
        .write_all(b"exit")
        .unwrap();
    until(|| slots.available_permits() == 1).await;
    drop(fd);
}

#[test]
#[ignore = "requires scripts/test-remote-input-parent.py and its private Niri/bus"]
fn native_parent_fixture() {
    assert_eq!(
        std::env::var("CANOPA_INPUT_PARENT_NATIVE").as_deref(),
        Ok("1")
    );
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
    let input: serde_json::Value = serde_json::from_str(&line).unwrap();
    let caller: OwnedUniqueName = input["caller"].as_str().unwrap().try_into().unwrap();
    let expected = input["worker_success"].as_bool().unwrap_or(true);
    let executable = PathBuf::from(std::env::var_os("CANOPA_INPUT_WORKER_BINARY").unwrap());
    let socket = PathBuf::from(std::env::var_os("NIRI_SOCKET").unwrap());
    let output = UnixStream::from(rustix::io::dup(std::io::stderr().as_fd()).unwrap());
    let (retire, mut retiring) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0]);
        let _ = retire.send(());
    });
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let connection=Connection::session().await.unwrap();
        let endpoint=Endpoint::resolve(&connection, socket).await.unwrap();
        let connector=Connector::new(Arc::new(DialogProcess { executable, arguments:vec![] }));
        let (_cancel, cancelled)=watch::channel(false);
        let (fd, lease)=connector.start(&connection, &endpoint, caller, 1, cancelled).await.unwrap();
        let client=UnixStream::from(fd);
        let worker=rustix::net::sockopt::socket_peercred(&client).unwrap().pid.as_raw_nonzero().get();
        super::super::send_ready(&output, &client).unwrap();
        drop(client); // This parent must not hide client loss by keeping an EI FD.
        println!("input-parent-ready {}",serde_json::json!({"worker":worker,"backend":connection.unique_name().unwrap().as_str(),"compositor_pid":endpoint.process_id()}));
        std::io::stdout().flush().unwrap();
        let mut closed=lease.closed();
        tokio::select! {
            _=&mut retiring=>{},
            _=async { if !*closed.borrow() { let _=closed.changed().await; } }=>{},
        }
        let result=lease.stop().await;
        println!("input-parent-retired {}",result.is_ok());
        assert_eq!(result.is_ok(), expected, "{result:?}");
        assert_eq!(connector.slots.available_permits(),8);
    });
}

#[test]
#[ignore = "requires scripts/test-remote-input-legacy-parent.py and its private desktop"]
fn native_legacy_parent_fixture() {
    use crate::remote_input::KeyboardStatus;
    use crate::screencast::remote::KeyboardInput;
    assert_eq!(
        std::env::var("CANOPA_INPUT_LEGACY_NATIVE").as_deref(),
        Ok("1")
    );
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
    let input: serde_json::Value = serde_json::from_str(&line).unwrap();
    let caller: OwnedUniqueName = input["caller"].as_str().unwrap().try_into().unwrap();
    let expected = input["worker_success"].as_bool().unwrap_or(true);
    let executable = PathBuf::from(std::env::var_os("CANOPA_INPUT_WORKER_BINARY").unwrap());
    let socket = PathBuf::from(std::env::var_os("NIRI_SOCKET").unwrap());
    let (retire, mut retiring) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0]);
        let _ = retire.send(());
    });
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let connection = Connection::session().await.unwrap();
        let endpoint = Endpoint::resolve(&connection, socket).await.unwrap();
        let connector = Connector::new(Arc::new(DialogProcess { executable, arguments: vec![] }));
        let (_cancel, cancelled) = watch::channel(false);
        let mut keyboard = connector.start_legacy(&connection, &endpoint, caller, cancelled).await.unwrap();
        let worker = keyboard.worker_pid();
        assert_eq!(keyboard.send(KeyboardInput::Keysym { symbol: 0x0101_f600, pressed: true }).await.unwrap(), KeyboardStatus::Unavailable);
        for (symbol, pressed) in [(0x41, true), (0x42, true), (0x41, false)] {
            assert_eq!(keyboard.send(KeyboardInput::Keysym { symbol, pressed }).await.unwrap(), KeyboardStatus::Accepted);
        }
        assert_eq!(keyboard.send(KeyboardInput::Keycode { code: 42, pressed: true }).await.unwrap(), KeyboardStatus::Accepted);
        println!("legacy-parent-ready {}", serde_json::json!({"worker": worker, "backend": connection.unique_name().unwrap().as_str(), "compositor_pid": endpoint.process_id()}));
        std::io::stdout().flush().unwrap();
        let mut closed = keyboard.closed();
        tokio::select! {
            _ = &mut retiring => {},
            _ = async { if !*closed.borrow() { let _ = closed.changed().await; } } => {},
        }
        let result = keyboard.stop().await;
        println!("legacy-parent-retired {}", result.is_ok());
        assert_eq!(result.is_ok(), expected, "{result:?}");
        assert_eq!(connector.slots.available_permits(), 8);
    });
}
