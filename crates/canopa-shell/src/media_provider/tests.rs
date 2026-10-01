use super::*;

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
