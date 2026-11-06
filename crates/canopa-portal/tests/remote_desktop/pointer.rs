//! Real D-Bus receive ordering and session policy; the controlled transport
//! acknowledges input without injecting into the user's compositor.
use super::*;
use remote::{Input, PointerInput};

struct Logged {
    inner: Box<dyn remote::Control>,
    inputs: Arc<Mutex<Vec<Input>>>,
    entered: watch::Sender<bool>,
    ready: Option<watch::Receiver<bool>>,
}
impl remote::Control for Logged {
    fn devices(&self) -> u32 {
        self.inner.devices()
    }
    fn streams(&self) -> &[Stream] {
        self.inner.streams()
    }
    fn closed(&self) -> watch::Receiver<bool> {
        self.inner.closed()
    }
    fn connect<'a>(
        &'a mut self,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<OwnedFd>> {
        self.inner.connect(cancelled)
    }
    fn keyboard<'a>(
        &'a mut self,
        input: remote::KeyboardInput,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, zbus::fdo::Result<()>> {
        self.inner.keyboard(input, cancelled)
    }
    fn input<'a>(
        &'a mut self,
        input: Input,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, zbus::fdo::Result<()>> {
        Box::pin(async move {
            self.inputs.lock().unwrap().push(input);
            self.entered.send_replace(true);
            if let Some(ready) = self.ready.as_mut() {
                while !*ready.borrow() {
                    tokio::select! {
                        biased;
                        _=cancelled.changed()=>return Err(zbus::fdo::Error::Failed("Input cancelled".into())),
                        result=ready.changed()=>{result.map_err(|_|zbus::fdo::Error::Failed("Acknowledgement ended".into()))?;},
                    }
                }
            }
            match input {
                Input::Keyboard(input) => self.inner.keyboard(input, cancelled).await,
                Input::Pointer(_) => Ok(()),
                Input::Touch(_) => Ok(()),
            }
        })
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        self.inner.stop()
    }
}
type LoggedGrant = (
    Box<dyn remote::Control>,
    watch::Sender<bool>,
    Arc<AtomicUsize>,
    watch::Receiver<bool>,
    Arc<Mutex<Vec<Input>>>,
);
fn logged_grant(
    devices: u32,
    streams: Vec<Stream>,
    ready: Option<watch::Receiver<bool>>,
) -> LoggedGrant {
    let (inner, closed, stops, _) = grant(devices, streams, false);
    let inputs = Arc::new(Mutex::new(Vec::new()));
    let (entered, entering) = watch::channel(false);
    (
        Box::new(Logged {
            inner,
            inputs: inputs.clone(),
            entered,
            ready,
        }),
        closed,
        stops,
        entering,
        inputs,
    )
}
async fn notify<B: serde::Serialize + zbus::zvariant::DynamicType>(
    connection: &Connection,
    method: &str,
    body: &B,
) -> zbus::Result<()> {
    zbus::Proxy::new(connection, BUS_NAME, OBJECT_PATH, REMOTE)
        .await?
        .call(method, body)
        .await
}
async fn entered(value: &mut watch::Receiver<bool>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !*value.borrow() {
            value.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn pointer_wire_validates_arguments_owner_grant_stream_and_eis_transition() {
    let mut f = Fixture::new().await;
    f.create("pointer", REMOTE).await.unwrap();
    f.select("pointer", REMOTE, "SelectDevices", types(2))
        .await
        .unwrap();
    f.select("pointer", CAPTURE, "SelectSources", types(1))
        .await
        .unwrap();
    let motion = (path("pointer"), Options::new(), 1.25f64, -2.5f64);
    error(
        notify(&f.client, "NotifyPointerMotion", &motion).await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
    let started = f.start("pointer", REMOTE);
    let request = f.presentation().await;
    let (control, _source, stops, _, inputs) = logged_grant(2, vec![stream()], None);
    request.reply.send(Ok(Some(control))).ok().unwrap();
    let (status, mut values) = started.await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert_eq!(u32::try_from(values.remove("devices").unwrap()).unwrap(), 2);
    let stranger = f.bus.connect().await;
    error(
        notify(&stranger, "NotifyPointerMotion", &motion).await,
        "org.freedesktop.DBus.Error.AccessDenied",
    );
    error(
        keyboard(&f.client, "pointer", "NotifyKeyboardKeycode", 42, 1).await,
        "org.freedesktop.DBus.Error.AccessDenied",
    );
    error(
        notify(
            &f.client,
            "NotifyPointerMotionAbsolute",
            &(path("pointer"), Options::new(), 42u32, 3f64, 4f64),
        )
        .await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
    for (x, y) in [(f64::NAN, 0.), (0., f64::INFINITY), (1_000_001., 0.)] {
        error(
            notify(
                &f.client,
                "NotifyPointerMotion",
                &(path("pointer"), Options::new(), x, y),
            )
            .await,
            "org.freedesktop.DBus.Error.InvalidArgs",
        );
    }
    for (code, state) in [(-1i32, 1u32), (0, 1), (272, 2), (0x10000, 1)] {
        error(
            notify(
                &f.client,
                "NotifyPointerButton",
                &(path("pointer"), Options::new(), code, state),
            )
            .await,
            "org.freedesktop.DBus.Error.InvalidArgs",
        );
    }
    for (node, x, y) in [
        (0u32, 1., 1.),
        (u32::MAX, 1., 1.),
        (41, -1., 1.),
        (41, 1., f64::NAN),
    ] {
        error(
            notify(
                &f.client,
                "NotifyPointerMotionAbsolute",
                &(path("pointer"), Options::new(), node, x, y),
            )
            .await,
            "org.freedesktop.DBus.Error.InvalidArgs",
        );
    }
    for (axis, steps) in [(2u32, 1i32), (0, 33), (1, i32::MIN)] {
        error(
            notify(
                &f.client,
                "NotifyPointerAxisDiscrete",
                &(path("pointer"), Options::new(), axis, steps),
            )
            .await,
            "org.freedesktop.DBus.Error.InvalidArgs",
        );
    }
    error(
        notify(
            &f.client,
            "NotifyPointerAxis",
            &(
                path("pointer"),
                Options::from([("finish".into(), 1u32.into())]),
                0f64,
                0f64,
            ),
        )
        .await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
    assert!(inputs.lock().unwrap().is_empty());
    notify(&f.client, "NotifyPointerMotion", &motion)
        .await
        .unwrap();
    notify(
        &f.client,
        "NotifyPointerMotionAbsolute",
        &(path("pointer"), Options::new(), 41u32, 3f64, 4f64),
    )
    .await
    .unwrap();
    for state in [1u32, 0] {
        notify(
            &f.client,
            "NotifyPointerButton",
            &(path("pointer"), Options::new(), 272i32, state),
        )
        .await
        .unwrap();
    }
    notify(
        &f.client,
        "NotifyPointerAxis",
        &(path("pointer"), Options::new(), 1.5f64, -2.25f64),
    )
    .await
    .unwrap();
    notify(
        &f.client,
        "NotifyPointerAxis",
        &(
            path("pointer"),
            Options::from([("finish".into(), true.into())]),
            0f64,
            0f64,
        ),
    )
    .await
    .unwrap();
    for (axis, steps) in [(0u32, 2i32), (1, -3), (0, 0)] {
        notify(
            &f.client,
            "NotifyPointerAxisDiscrete",
            &(path("pointer"), Options::new(), axis, steps),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        *inputs.lock().unwrap(),
        vec![
            Input::Pointer(PointerInput::Relative { x: 1.25, y: -2.5 }),
            Input::Pointer(PointerInput::Absolute {
                stream: 41,
                x: 3.,
                y: 4.
            }),
            Input::Pointer(PointerInput::Button {
                code: 272,
                pressed: true
            }),
            Input::Pointer(PointerInput::Button {
                code: 272,
                pressed: false
            }),
            Input::Pointer(PointerInput::Axis {
                x: 1.5,
                y: -2.25,
                finish: false
            }),
            Input::Pointer(PointerInput::Axis {
                x: 0.,
                y: 0.,
                finish: true
            }),
            Input::Pointer(PointerInput::Discrete { axis: 0, steps: 2 }),
            Input::Pointer(PointerInput::Discrete { axis: 1, steps: -3 }),
            Input::Pointer(PointerInput::Discrete { axis: 0, steps: 0 }),
        ]
    );
    let fd = connect(&f.client, "pointer", APP).await.unwrap();
    error(
        notify(&f.client, "NotifyPointerMotion", &motion).await,
        "org.freedesktop.DBus.Error.AccessDenied",
    );
    assert_eq!(inputs.lock().unwrap().len(), 9);
    drop(fd);
    close(
        &f.client,
        path("pointer"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    retired(&f.client, path("pointer")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
    for devices in [0, 1] {
        let name = format!("denied{devices}");
        f.create(&name, REMOTE).await.unwrap();
        f.select(&name, REMOTE, "SelectDevices", types(3))
            .await
            .unwrap();
        f.select(&name, CAPTURE, "SelectSources", types(1))
            .await
            .unwrap();
        let started = f.start(&name, REMOTE);
        let request = f.presentation().await;
        let (control, _source, _, _, inputs) = logged_grant(devices, vec![stream()], None);
        request.reply.send(Ok(Some(control))).ok().unwrap();
        assert_eq!(started.await.unwrap().unwrap().0, 0);
        error(
            notify(
                &f.client,
                "NotifyPointerMotion",
                &(path(&name), Options::new(), 1f64, 1f64),
            )
            .await,
            "org.freedesktop.DBus.Error.AccessDenied",
        );
        assert!(inputs.lock().unwrap().is_empty());
    }
    f.portal.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mixed_keyboard_and_pointer_calls_preserve_one_receive_order() {
    let mut f = Fixture::new().await;
    f.create("mixed", REMOTE).await.unwrap();
    f.select("mixed", REMOTE, "SelectDevices", types(3))
        .await
        .unwrap();
    let started = f.start("mixed", REMOTE);
    let request = f.presentation().await;
    let (control, _source, stops, _, inputs) = logged_grant(3, vec![], None);
    request.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let expected = [
        Input::Pointer(PointerInput::Button {
            code: 272,
            pressed: true,
        }),
        Input::Keyboard(remote::KeyboardInput::Keycode {
            code: 29,
            pressed: true,
        }),
        Input::Pointer(PointerInput::Relative { x: 2., y: -1. }),
        Input::Keyboard(remote::KeyboardInput::Keycode {
            code: 29,
            pressed: false,
        }),
        Input::Pointer(PointerInput::Button {
            code: 272,
            pressed: false,
        }),
    ];
    let proxy = zbus::Proxy::new(&f.client, BUS_NAME, OBJECT_PATH, REMOTE)
        .await
        .unwrap();
    for round in 0..32 {
        inputs.lock().unwrap().clear();
        let futures = (0..40).map(|index| {
            let proxy = proxy.clone();
            async move {
                match index % 5 {
                    0 | 4 => {
                        proxy
                            .call::<_, _, ()>(
                                "NotifyPointerButton",
                                &(
                                    path("mixed"),
                                    Options::new(),
                                    272i32,
                                    u32::from(index % 5 == 0),
                                ),
                            )
                            .await
                    }
                    1 | 3 => {
                        proxy
                            .call::<_, _, ()>(
                                "NotifyKeyboardKeycode",
                                &(
                                    path("mixed"),
                                    Options::new(),
                                    29i32,
                                    u32::from(index % 5 == 1),
                                ),
                            )
                            .await
                    }
                    _ => {
                        proxy
                            .call::<_, _, ()>(
                                "NotifyPointerMotion",
                                &(path("mixed"), Options::new(), 2f64, -1f64),
                            )
                            .await
                    }
                }
            }
        });
        let replies = futures_util::future::join_all(futures).await;
        assert!(
            replies.iter().all(Result::is_ok),
            "round{round}: {replies:?}"
        );
        assert_eq!(
            *inputs.lock().unwrap(),
            expected.into_iter().cycle().take(40).collect::<Vec<_>>(),
            "round{round}"
        );
    }
    f.portal.shutdown().await.unwrap();
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn pending_pointer_input_retires_on_session_caller_source_and_backend_loss() {
    for reason in ["session", "caller", "source", "backend"] {
        let mut f = Fixture::new().await;
        let observer = f.bus.connect().await;
        f.create("pendingpointer", REMOTE).await.unwrap();
        f.select("pendingpointer", REMOTE, "SelectDevices", types(2))
            .await
            .unwrap();
        let started = f.start("pendingpointer", REMOTE);
        let request = f.presentation().await;
        let (_release, ready) = watch::channel(false);
        let (control, source, stops, mut entering, inputs) = logged_grant(2, vec![], Some(ready));
        request.reply.send(Ok(Some(control))).ok().unwrap();
        assert_eq!(started.await.unwrap().unwrap().0, 0);
        let caller = f.client.clone();
        let pending = tokio::spawn(async move {
            notify(
                &caller,
                "NotifyPointerButton",
                &(path("pendingpointer"), Options::new(), 272i32, 1u32),
            )
            .await
        });
        entered(&mut entering).await;
        match reason {
            "session" => close(
                &f.client,
                path("pendingpointer"),
                "org.freedesktop.impl.portal.Session",
            )
            .await
            .unwrap(),
            "caller" => f.client.clone().close().await.unwrap(),
            "backend" => f._server.clone().close().await.unwrap(),
            _ => {
                source.send_replace(true);
            }
        }
        assert!(
            tokio::time::timeout(Duration::from_secs(3), pending)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        retired(&observer, path("pendingpointer")).await;
        assert_eq!(inputs.lock().unwrap().len(), 1);
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn blocked_pointer_input_shares_the_keyboard_queue_bound_and_overflow_retires_the_grant() {
    let mut f = Fixture::new().await;
    f.create("boundedpointer", REMOTE).await.unwrap();
    f.select("boundedpointer", REMOTE, "SelectDevices", types(3))
        .await
        .unwrap();
    let started = f.start("boundedpointer", REMOTE);
    let request = f.presentation().await;
    let (_release, ready) = watch::channel(false);
    let (control, _source, stops, mut entering, inputs) = logged_grant(3, vec![], Some(ready));
    request.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let mut tasks = tokio::task::JoinSet::new();
    let caller = f.client.clone();
    tasks.spawn(async move {
        notify(
            &caller,
            "NotifyPointerButton",
            &(path("boundedpointer"), Options::new(), 272i32, 1u32),
        )
        .await
    });
    entered(&mut entering).await;
    for index in 0..65 {
        let caller = f.client.clone();
        tasks.spawn(async move {
            if index % 2 == 0 {
                notify(
                    &caller,
                    "NotifyPointerButton",
                    &(path("boundedpointer"), Options::new(), 272i32, 0u32),
                )
                .await
            } else {
                keyboard(&caller, "boundedpointer", "NotifyKeyboardKeycode", 29, 0).await
            }
        });
    }
    let mut exceeded = false;
    while let Some(result) = tokio::time::timeout(Duration::from_secs(3), tasks.join_next())
        .await
        .unwrap()
    {
        let result = result.unwrap();
        if let Err(zbus::Error::MethodError(name, _, _)) = &result {
            exceeded |= name.as_str() == "org.freedesktop.DBus.Error.LimitsExceeded";
        }
        assert!(result.is_err());
    }
    assert!(exceeded);
    retired(&f.client, path("boundedpointer")).await;
    assert_eq!(inputs.lock().unwrap().len(), 1);
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn touch_wire_dispatches_all_three_methods_in_order_and_checks_owned_streams() {
    let mut f = Fixture::new().await;
    f.create("touch", REMOTE).await.unwrap();
    f.select("touch", REMOTE, "SelectDevices", types(4))
        .await
        .unwrap();
    f.select("touch", CAPTURE, "SelectSources", types(1))
        .await
        .unwrap();
    let started = f.start("touch", REMOTE);
    let request = f.presentation().await;
    let (control, _source, _stops, _, inputs) = logged_grant(4, vec![stream()], None);
    request.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let down = (
        path("touch"),
        Options::new(),
        41u32,
        7u32,
        10.25f64,
        20.5f64,
    );
    notify(&f.client, "NotifyTouchDown", &down).await.unwrap();
    notify(
        &f.client,
        "NotifyTouchMotion",
        &(path("touch"), Options::new(), 41u32, 7u32, 30f64, 40f64),
    )
    .await
    .unwrap();
    notify(
        &f.client,
        "NotifyTouchUp",
        &(path("touch"), Options::new(), 7u32),
    )
    .await
    .unwrap();
    assert_eq!(
        *inputs.lock().unwrap(),
        vec![
            Input::Touch(remote::TouchInput::Down {
                stream: 41,
                slot: 7,
                x: 10.25,
                y: 20.5
            }),
            Input::Touch(remote::TouchInput::Motion {
                stream: 41,
                slot: 7,
                x: 30.,
                y: 40.
            }),
            Input::Touch(remote::TouchInput::Up { slot: 7 }),
        ]
    );
    let stranger = f.bus.connect().await;
    error(
        notify(&stranger, "NotifyTouchDown", &down).await,
        "org.freedesktop.DBus.Error.AccessDenied",
    );
    error(
        notify(
            &f.client,
            "NotifyTouchDown",
            &(path("touch"), Options::new(), 42u32, 7u32, 1f64, 2f64),
        )
        .await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
    assert_eq!(inputs.lock().unwrap().len(), 3);
}
