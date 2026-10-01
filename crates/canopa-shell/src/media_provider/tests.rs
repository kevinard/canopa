use super::*;
use tokio::io::AsyncWriteExt;

fn state() -> Snapshot {
    let p = media::Player {
        id: media::PlayerId {
            bus: "org.mpris.MediaPlayer2.test".into(),
            owner: ":1.42".into(),
            server: "test".into(),
        },
        name: "Player".into(),
        title: "Song".into(),
        artist: "Artist".into(),
        track: "/track/one".into(),
        status: "Playing".into(),
        can_play: true,
        can_pause: true,
        can_next: false,
        can_previous: false,
    };
    Snapshot {
        available: true,
        selected: Some(p.id.clone()),
        players: vec![p],
        pinned: false,
    }
}

fn event(revision: u64, target: InteractionTarget, key: &str, value: Option<&str>) -> HostMessage {
    HostMessage::Event {
        protocol_version: 5,
        revision,
        interaction: Interaction::Rich(canopa_panel_protocol::InteractionEvent::Invoke {
            target,
            action_key: key.into(),
            value: value.map(str::to_owned),
        }),
    }
}

#[test]
fn panel_only_advertises_supported_actions_and_rejects_retired_revisions() {
    let mut panel = MediaPanel::default();
    let mut state = state();
    let view = panel.publish(&state, None).unwrap();
    let Content::Rich(content) = &view.content else {
        panic!("rich media panel")
    };
    content.validate().unwrap();
    assert_eq!(content.sections[0].items[0].title, "Song");
    assert_eq!(
        content.sections[0].items[0]
            .actions
            .iter()
            .map(|a| a.key.as_str())
            .collect::<Vec<_>>(),
        ["pause"]
    );
    let pause = event(
        view.revision,
        InteractionTarget::Item { id: "track".into() },
        "pause",
        None,
    );
    assert!(matches!(
        panel.request(&pause),
        Some(Request::Control {
            operation: Operation::Pause,
            ..
        })
    ));
    let select = event(
        view.revision,
        InteractionTarget::Section { id: "media".into() },
        "player",
        Some("0"),
    );
    assert!(matches!(
        panel.request(&select),
        Some(Request::Select(Some(_)))
    ));
    state.players[0].status = "Paused".into();
    panel.publish(&state, None).unwrap();
    assert!(panel.request(&pause).is_none());
    assert!(panel.request(&select).is_none());
}

#[tokio::test]
async fn provider_closes_without_a_media_connection() {
    let input =
        tokio::io::BufReader::new(b"{\"type\":\"close\",\"protocol_version\":5}\n".as_slice());
    let mut output = vec![];
    tokio::time::timeout(Duration::from_secs(2), run(input, &mut output))
        .await
        .unwrap()
        .unwrap();
    let view: v5::Response = serde_json::from_slice(&output).unwrap();
    let Content::Rich(content) = view.content else {
        panic!("rich media panel")
    };
    assert_eq!(content.title, "Media");
    assert!(content.sections[0].items[0].title.contains("unavailable"));
}

#[derive(Clone)]
struct DelayedMedia {
    state: std::sync::Arc<std::sync::Mutex<Snapshot>>,
    delayed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    entered: std::sync::Arc<tokio::sync::Notify>,
    release: std::sync::Arc<tokio::sync::Notify>,
}

#[zbus::interface(name = "org.canopa.Shell.Media")]
impl DelayedMedia {
    async fn snapshot(&self) -> String {
        if self.delayed.load(std::sync::atomic::Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        serde_json::to_string(&*self.state.lock().unwrap()).unwrap()
    }

    async fn apply(&self, _request: String) {}
}

#[tokio::test]
async fn host_action_during_snapshot_does_not_lose_the_consumed_change() {
    if std::env::var_os("CANOPA_MEDIA_CANCELLATION_FIXTURE").is_none() {
        let status = tokio::process::Command::new("dbus-run-session")
            .args(["--"])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", "media_provider::tests::host_action_during_snapshot_does_not_lose_the_consumed_change", "--nocapture"])
            .env("CANOPA_MEDIA_CANCELLATION_FIXTURE", "1")
            .kill_on_drop(true)
            .status().await.unwrap();
        assert!(status.success(), "private media fixture failed");
        return;
    }
    let service = DelayedMedia {
        state: std::sync::Arc::new(std::sync::Mutex::new(state())),
        delayed: Default::default(),
        entered: Default::default(),
        release: Default::default(),
    };
    let bus = zbus::connection::Builder::session()
        .unwrap()
        .name(media::SERVICE)
        .unwrap()
        .serve_at(media::PATH, service.clone())
        .unwrap()
        .build()
        .await
        .unwrap();
    let (host, provider) = tokio::io::duplex(65536);
    let (read, write) = tokio::io::split(provider);
    let running = tokio::spawn(run(tokio::io::BufReader::new(read), write));
    let (read, mut write) = tokio::io::split(host);
    let mut read = tokio::io::BufReader::new(read);
    async fn titled(read: &mut (impl AsyncBufRead + Unpin), title: &str) -> v5::Response {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let response = v5::Response::read_async(read).await.unwrap().unwrap();
                if matches!(&response.content, Content::Rich(content) if content.sections[0].items[0].title == title) {
                    return response;
                }
            }
        }).await.expect("media update without another Changed signal")
    }
    let initial = titled(&mut read, "Song").await;
    service
        .delayed
        .store(true, std::sync::atomic::Ordering::SeqCst);
    service.state.lock().unwrap().players[0].title = "Updated song".into();
    bus.emit_signal(None::<&str>, media::PATH, media::INTERFACE, "Changed", &())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), service.entered.notified())
        .await
        .unwrap();
    write
        .write_all(
            &event(
                initial.revision,
                InteractionTarget::Item { id: "track".into() },
                "pause",
                None,
            )
            .encode()
            .unwrap(),
        )
        .await
        .unwrap();
    // A control response proves the host frame cancelled the in-flight refresh.
    let after_action = titled(&mut read, "Song").await;
    assert!(after_action.revision > initial.revision);
    service
        .delayed
        .store(false, std::sync::atomic::Ordering::SeqCst);
    service.release.notify_waiters();
    titled(&mut read, "Updated song").await;
    write
        .write_all(&HostMessage::close().encode().unwrap())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
