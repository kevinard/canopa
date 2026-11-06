use super::*;
pub(crate) mod fake;

#[tokio::test]
async fn missing_or_unsupported_creator_cleanup_is_rejected_before_capture_mutation() {
    use std::sync::atomic::Ordering;

    for version in [0, 2] {
        let fixture = fake::Fixture::new().await;
        fixture
            .state
            .capture_owner_version
            .store(version, Ordering::SeqCst);
        assert!(Owner::resolve(&fixture.backend).await.is_err());
        let (_stop, ready, task) = launch(&fixture, vec![Source::Monitor("DP-1".into())]).await;
        assert!(
            tokio::time::timeout(Duration::from_secs(3), task)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert!(ready.await.is_err());
        assert_eq!(fixture.state.created_sessions.load(Ordering::SeqCst), 0);
        assert!(fixture.state.recorded.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn creator_cleanup_is_rechecked_after_owner_resolution_before_worker_creation() {
    use std::sync::atomic::Ordering;

    let fixture = fake::Fixture::new().await;
    assert!(Owner::resolve(&fixture.backend).await.is_ok());
    fixture
        .state
        .capture_owner_version
        .store(0, Ordering::SeqCst);
    let (_stop, ready, task) = launch(&fixture, vec![Source::Monitor("DP-1".into())]).await;
    assert!(
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(ready.await.is_err());
    assert_eq!(fixture.state.created_sessions.load(Ordering::SeqCst), 0);
}

async fn launch(
    fixture: &fake::Fixture,
    sources: Vec<Source>,
) -> (
    watch::Sender<bool>,
    oneshot::Receiver<Vec<Stream>>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
) {
    let input = Input {
        compositor: fixture.service.unique_name().unwrap().to_string(),
        backend: fixture.backend.unique_name().unwrap().to_string(),
        caller: fixture.caller.unique_name().unwrap().to_string(),
        sources,
        cursor: Cursor::Metadata,
    };
    let (stop, _) = watch::channel(false);
    let (ready, streams) = oneshot::channel();
    let connection = fixture.bus.connect().await;
    let control = stop.clone();
    let task = tokio::spawn(async move {
        worker::run(&connection, input, control, |streams| {
            ready
                .send(streams)
                .map_err(|_| anyhow::anyhow!("No consumer"))
        })
        .await
    });
    (stop, streams, task)
}

#[tokio::test]
async fn monitor_and_window_subscribe_before_start_and_preserve_order_and_geometry() {
    let fixture = fake::Fixture::new().await;
    assert!(Owner::resolve(&fixture.backend).await.is_ok());
    let (stop, ready, task) = launch(
        &fixture,
        vec![Source::Monitor("DP-1".into()), Source::Window(42)],
    )
    .await;
    let streams = tokio::time::timeout(Duration::from_secs(3), ready)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        streams.iter().map(|v| v.node).collect::<Vec<_>>(),
        [101, 102]
    );
    assert_eq!(streams[0].geometry.unwrap().position, (-640, 0));
    assert_eq!(streams[0].geometry.unwrap().size, (640, 480));
    assert!(streams[1].geometry.is_none());
    assert_eq!(
        fixture.state.recorded.lock().unwrap().as_slice(),
        [("DP-1".into(), 2), ("window:42".into(), 2)]
    );
    stop.send_replace(true);
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn partial_record_failure_and_early_compositor_close_retire_the_created_session() {
    let fixture = fake::Fixture::new().await;
    let (_stop, _ready, task) = launch(
        &fixture,
        vec![
            Source::Monitor("DP-1".into()),
            Source::Monitor("unavailable".into()),
        ],
    )
    .await;
    assert!(
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    fixture
        .state
        .close_on_start
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (_stop, _ready, task) = launch(&fixture, vec![Source::Window(42)]).await;
    assert!(
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        2
    );
}

#[tokio::test]
async fn cancellation_during_creation_still_stops_the_late_returned_session() {
    let fixture = fake::Fixture::new().await;
    let gate = Arc::new(Semaphore::new(0));
    *fixture.state.create_gate.lock().unwrap() = Some(gate.clone());
    let (stop, ready, task) = launch(&fixture, vec![Source::Monitor("DP-1".into())]).await;
    tokio::time::timeout(Duration::from_secs(3), fixture.state.created.notified())
        .await
        .unwrap();
    stop.send_replace(true);
    gate.add_permits(1);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(ready.await.is_err());
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert!(fixture.state.recorded.lock().unwrap().is_empty());
}

#[tokio::test]
async fn caller_loss_and_service_replacement_stop_the_original_owner() {
    let fixture = fake::Fixture::new().await;
    let (_stop, ready, task) = launch(&fixture, vec![Source::Window(42)]).await;
    tokio::time::timeout(Duration::from_secs(3), ready)
        .await
        .unwrap()
        .unwrap();
    fixture.caller.clone().close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );

    let fixture = fake::Fixture::new().await;
    let (_stop, ready, task) = launch(&fixture, vec![Source::Window(42)]).await;
    tokio::time::timeout(Duration::from_secs(3), ready)
        .await
        .unwrap()
        .unwrap();
    fixture.service.release_name(SERVICE).await.unwrap();
    let replacement = fixture.bus.connect().await;
    replacement.request_name(SERVICE).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn cancellation_waits_for_inflight_record_before_retiring_its_session() {
    let fixture = fake::Fixture::new().await;
    let gate = Arc::new(Semaphore::new(0));
    *fixture.state.record_gate.lock().unwrap() = Some(gate.clone());
    let (stop, ready, task) = launch(&fixture, vec![Source::Window(42)]).await;
    tokio::time::timeout(Duration::from_secs(3), fixture.state.recording.notified())
        .await
        .unwrap();
    stop.send_replace(true);
    gate.add_permits(1);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(ready.await.is_err());
    assert_eq!(
        fixture
            .state
            .starts
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    let retired = fixture
        .backend
        .call_method(
            Some(fixture.service.unique_name().unwrap().as_str()),
            "/org/gnome/Mutter/ScreenCast/Stream/u1",
            Some("org.freedesktop.DBus.Introspectable"),
            "Introspect",
            &(),
        )
        .await;
    assert!(
        matches!(retired, Err(zbus::Error::MethodError(name,_,_)) if name.as_str()=="org.freedesktop.DBus.Error.UnknownObject")
    );
}

#[test]
fn source_limits_and_unique_bus_names_reject_ambiguous_inputs() {
    let mut input = Input {
        compositor: ":1.1".into(),
        backend: ":1.2".into(),
        caller: ":1.3".into(),
        sources: vec![Source::Window(1)],
        cursor: Cursor::Hidden,
    };
    assert!(input.validate().is_ok());
    for source in [
        Source::Window(0),
        Source::Monitor("".into()),
        Source::Monitor("bad\nname".into()),
        Source::Monitor("x".repeat(257).into()),
    ] {
        input.sources = vec![source];
        assert!(input.validate().is_err());
    }
    input.sources = vec![Source::Window(1); 2];
    assert!(input.validate().is_err());
    input.sources = (1..=17).map(Source::Window).collect();
    assert!(input.validate().is_err());
    input.sources = vec![Source::Window(1)];
    input.compositor = SERVICE.into();
    assert!(input.validate().is_err());
    assert_eq!(Cursor::Hidden.compositor_value(), 0);
    assert_eq!(Cursor::Embedded.compositor_value(), 1);
    assert_eq!(Cursor::Metadata.compositor_value(), 2);
}

#[test]
fn monitor_serialization_preserves_live_identity_and_rejects_connector_aliases() {
    let pinned = Source::Monitor(Monitor::pinned("DP-1", 77).unwrap());
    let bytes = serde_json::to_vec(&pinned).unwrap();
    assert_eq!(serde_json::from_slice::<Source>(&bytes).unwrap(), pinned);
    assert_ne!(
        pinned,
        Source::Monitor(Monitor::pinned("DP-1", 78).unwrap())
    );
    let legacy = Source::Monitor("DP-1".into());
    assert_eq!(
        serde_json::to_string(&legacy).unwrap(),
        r#"{"Monitor":"DP-1"}"#
    );
    for value in [
        r#"{"Monitor":{"name":"DP-1","identity":0}}"#,
        r#"{"Monitor":{"name":"DP-1","identity":77,"extra":true}}"#,
        r#"{"Monitor":{"name":"bad\nname","identity":77}}"#,
    ] {
        assert!(serde_json::from_str::<Source>(value).is_err());
    }
    for alias in [
        legacy,
        Source::Monitor(Monitor::pinned("DP-1", 78).unwrap()),
    ] {
        let input = Input {
            compositor: ":1.1".into(),
            backend: ":1.2".into(),
            caller: ":1.3".into(),
            sources: vec![pinned.clone(), alias],
            cursor: Cursor::Hidden,
        };
        assert!(input.validate().is_err());
    }
}

#[tokio::test]
async fn monitor_inventory_negotiates_live_ids_and_does_not_downgrade_errors() {
    use std::sync::atomic::Ordering;
    let fixture = fake::Fixture::new().await;
    let owner = Owner::resolve(&fixture.backend).await.unwrap();
    let legacy = owner
        .monitors(&fixture.backend, vec!["DP-1".into()])
        .await
        .unwrap();
    assert_eq!(legacy, [Monitor::from("DP-1")]);
    fixture.state.source_version.store(1, Ordering::SeqCst);
    fixture.state.monitor_identity.store(77, Ordering::SeqCst);
    let initial = owner
        .monitors(&fixture.backend, vec!["DP-1".into()])
        .await
        .unwrap();
    assert_eq!(initial, [Monitor::pinned("DP-1", 77).unwrap()]);
    fixture.state.monitor_identity.store(78, Ordering::SeqCst);
    assert_ne!(
        owner
            .monitors(&fixture.backend, vec!["DP-1".into()])
            .await
            .unwrap(),
        initial
    );
    fixture.state.monitor_identity.store(0, Ordering::SeqCst);
    assert!(
        owner
            .monitors(&fixture.backend, vec!["DP-1".into()])
            .await
            .is_err()
    );
    fixture.state.monitor_identity.store(77, Ordering::SeqCst);
    assert!(
        owner
            .monitors(&fixture.backend, vec!["missing".into()])
            .await
            .is_err()
    );
    fixture.state.identity_failure.store(true, Ordering::SeqCst);
    assert!(
        owner
            .monitors(&fixture.backend, vec!["DP-1".into()])
            .await
            .is_err()
    );
    fixture
        .state
        .identity_failure
        .store(false, Ordering::SeqCst);
    fixture.state.source_version.store(2, Ordering::SeqCst);
    assert!(
        owner
            .monitors(&fixture.backend, vec!["DP-1".into()])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn pinned_monitor_reaches_record_and_geometry_is_read_after_start() {
    use std::sync::atomic::Ordering;
    let fixture = fake::Fixture::new().await;
    fixture.state.source_version.store(1, Ordering::SeqCst);
    fixture.state.monitor_identity.store(77, Ordering::SeqCst);
    fixture.state.resize_on_start.store(true, Ordering::SeqCst);
    let source = Source::Monitor(Monitor::pinned("DP-1", 77).unwrap());
    let (stop, ready, task) = launch(&fixture, vec![source.clone()]).await;
    let streams = tokio::time::timeout(Duration::from_secs(3), ready)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(streams[0].source, source);
    assert_eq!(streams[0].geometry.unwrap().size, (1024, 768));
    assert_eq!(
        *fixture.state.identity_preconditions.lock().unwrap(),
        [Some(77)]
    );
    stop.send_replace(true);
    task.await.unwrap().unwrap();
    assert_eq!(fixture.state.stops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn monitor_replacement_after_consent_or_invalid_stream_echo_never_reaches_ready() {
    use std::sync::atomic::Ordering;
    for ignored_precondition in [false, true] {
        let fixture = fake::Fixture::new().await;
        fixture.state.source_version.store(1, Ordering::SeqCst);
        fixture.state.monitor_identity.store(77, Ordering::SeqCst);
        fixture
            .state
            .ignore_precondition
            .store(ignored_precondition, Ordering::SeqCst);
        let gate = Arc::new(Semaphore::new(0));
        *fixture.state.record_gate.lock().unwrap() = Some(gate.clone());
        let (_stop, ready, task) = launch(
            &fixture,
            vec![Source::Monitor(Monitor::pinned("DP-1", 77).unwrap())],
        )
        .await;
        tokio::time::timeout(Duration::from_secs(3), fixture.state.recording.notified())
            .await
            .unwrap();
        fixture.state.monitor_identity.store(78, Ordering::SeqCst);
        gate.add_permits(1);
        assert!(
            tokio::time::timeout(Duration::from_secs(3), task)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert!(ready.await.is_err());
        assert_eq!(fixture.state.stops.load(Ordering::SeqCst), 1);
        assert_eq!(
            fixture.state.starts.load(Ordering::SeqCst),
            u32::from(ignored_precondition)
        );
    }
    let fixture = fake::Fixture::new().await;
    fixture.state.source_version.store(1, Ordering::SeqCst);
    fixture.state.monitor_identity.store(77, Ordering::SeqCst);
    fixture
        .state
        .wrong_stream_identity
        .store(true, Ordering::SeqCst);
    let (_stop, ready, task) = launch(
        &fixture,
        vec![Source::Monitor(Monitor::pinned("DP-1", 77).unwrap())],
    )
    .await;
    assert!(task.await.unwrap().is_err());
    assert!(ready.await.is_err());
    assert_eq!(fixture.state.stops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn pinned_capture_cannot_fall_back_to_a_legacy_record_call() {
    let fixture = fake::Fixture::new().await;
    let (_stop, ready, task) = launch(
        &fixture,
        vec![Source::Monitor(Monitor::pinned("DP-1", 77).unwrap())],
    )
    .await;
    assert!(task.await.unwrap().is_err());
    assert!(ready.await.is_err());
    assert!(fixture.state.recorded.lock().unwrap().is_empty());
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn live_monitor_and_window_mappings_survive_worker_readiness_as_one_binding() {
    use std::sync::atomic::Ordering;
    let fixture = fake::Fixture::new().await;
    fixture.state.source_version.store(1, Ordering::SeqCst);
    fixture.state.monitor_identity.store(77, Ordering::SeqCst);
    let (stop, ready, task) = launch(
        &fixture,
        vec![
            Source::Monitor(Monitor::pinned("DP-1", 77).unwrap()),
            Source::Window(42),
        ],
    )
    .await;
    let streams = ready.await.unwrap();
    assert!(streams[1].geometry.is_none());
    assert!(streams.iter().all(|s| s.mapping.is_some()));
    let bindings =
        Bindings::from_streams(fixture.service.unique_name().unwrap().as_str(), &streams)
            .unwrap()
            .unwrap();
    assert_eq!(bindings.regions.len(), 2);
    assert_eq!(bindings.regions[1].source_id, 42);
    assert_eq!(
        bindings
            .current_shapes(&fixture.backend, std::process::id())
            .await
            .unwrap(),
        [
            bindings::Shape {
                node: 101,
                size: (640, 480),
                physical_scale: None
            },
            bindings::Shape {
                node: 102,
                size: (640, 480),
                physical_scale: None
            }
        ]
    );
    stop.send_replace(true);
    task.await.unwrap().unwrap();
    assert!(
        bindings
            .current_shapes(&fixture.backend, std::process::id())
            .await
            .is_err()
    );
}

#[test]
fn capture_binding_paths_aliases_and_response_budget_are_bounded() {
    for path in [
        "/org/gnome/Mutter/ScreenCast/Stream/u0",
        "/org/gnome/Mutter/ScreenCast/Stream/u01",
        "/org/gnome/Mutter/ScreenCast/Session/u1/Stream/u1",
        "/org/gnome/Mutter/ScreenCast/Stream/u1/extra",
    ] {
        assert!(bindings::path_id(path, "Stream").is_none());
    }
    let streams: Vec<_> = (0..16)
        .map(|i| Stream {
            node: i + 1,
            source: Source::Monitor(
                Monitor::pinned(
                    format!("{}{:03}", "x".repeat(253), i),
                    u64::MAX - u64::from(i),
                )
                .unwrap(),
            ),
            geometry: Some(Geometry {
                position: (-1_000_000, 1_000_000),
                size: (32768, 32768),
            }),
            mapping: Some(Mapping {
                session: format!("/org/gnome/Mutter/ScreenCast/Session/u{}", u64::MAX),
                stream: format!(
                    "/org/gnome/Mutter/ScreenCast/Stream/u{}",
                    u64::MAX - u64::from(i)
                ),
                source_id: u64::MAX - u64::from(i),
                mapping_id: format!("{}{:03}", "y".repeat(253), i),
                size: (32768, 32768),
                physical_scale: Some(64.),
            }),
        })
        .collect();
    let bytes = serde_json::to_vec(&streams).unwrap();
    assert!(bytes.len() < RESPONSE_BYTES);
    let binding = Bindings::from_streams(":1.1", &streams).unwrap().unwrap();
    binding.validate().unwrap();
    let mut invalid = binding.clone();
    invalid.regions[1].node = invalid.regions[0].node;
    assert!(invalid.validate().is_err());
    let mut invalid = binding.clone();
    invalid.regions[1].mapping_id = invalid.regions[0].mapping_id.clone();
    assert!(invalid.validate().is_err());
    let mut invalid = binding;
    invalid.regions[1].stream = invalid.regions[0].stream.clone();
    assert!(invalid.validate().is_err());
    let mut legacy = streams;
    legacy[0].source = Source::Monitor("legacy".into());
    assert!(Bindings::from_streams(":1.1", &legacy).is_err());
}

#[tokio::test]
async fn scaled_bindings_revalidate_live_projection_without_replacing_source_identity() {
    use std::sync::atomic::Ordering;
    let fixture = fake::Fixture::new().await;
    fixture.state.source_version.store(1, Ordering::SeqCst);
    *fixture.state.physical_scale.lock().unwrap() = Some(1.25);
    let (stop, ready, task) = launch(&fixture, vec![Source::Window(42)]).await;
    let streams = ready.await.unwrap();
    assert_eq!(
        streams[0].mapping.as_ref().unwrap().physical_scale,
        Some(1.25)
    );
    let binding = Bindings::from_streams(fixture.service.unique_name().unwrap().as_str(), &streams)
        .unwrap()
        .unwrap();
    let before = binding
        .current_shapes(&fixture.backend, std::process::id())
        .await
        .unwrap();
    *fixture.state.physical_scale.lock().unwrap() = Some(1.5);
    let after = binding
        .current_shapes(&fixture.backend, std::process::id())
        .await
        .unwrap();
    assert_eq!(before[0].size, after[0].size);
    assert_eq!(before[0].physical_scale, Some(1.25));
    assert_eq!(after[0].physical_scale, Some(1.5));
    for scale in [
        None,
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(0.),
        Some(-1.),
        Some(64.1),
    ] {
        *fixture.state.physical_scale.lock().unwrap() = scale;
        assert!(
            binding
                .current_shapes(&fixture.backend, std::process::id())
                .await
                .is_err()
        );
    }
    stop.send_replace(true);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn malformed_optional_scale_cannot_publish_a_ready_recording() {
    use std::sync::atomic::Ordering;
    for scale in [f64::NAN, f64::INFINITY, 0., -1., 64.1] {
        let fixture = fake::Fixture::new().await;
        fixture.state.source_version.store(1, Ordering::SeqCst);
        *fixture.state.physical_scale.lock().unwrap() = Some(scale);
        let (_stop, ready, task) = launch(&fixture, vec![Source::Window(42)]).await;
        assert!(ready.await.is_err());
        assert!(task.await.unwrap().is_err());
        assert_eq!(fixture.state.stops.load(Ordering::SeqCst), 1);
    }
}
