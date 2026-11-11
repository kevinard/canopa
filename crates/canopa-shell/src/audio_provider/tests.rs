use super::*;
use canopa_services::testing::audio::AudioServer;
use std::time::Duration;

#[tokio::test]
async fn unavailable_provider_publishes_and_closes_on_a_split_frame() {
    use tokio::io::{AsyncWriteExt, BufReader};
    let (host, provider) = tokio::io::duplex(8192);
    let (input, output) = tokio::io::split(provider);
    let running = tokio::spawn(run(
        BufReader::new(input),
        output,
        Some("unix:/nonexistent-canopa-audio-test".into()),
    ));
    let (input, mut output) = tokio::io::split(host);
    let mut input = BufReader::new(input);
    let response =
        tokio::time::timeout(Duration::from_secs(3), v5::Response::read_async(&mut input))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    let Content::Rich(content) = response.content else {
        panic!("rich content")
    };
    assert_eq!(content.title, "Sound");
    assert!(content.error.as_deref().unwrap().contains("unavailable"));
    output.write_all(b"{\"type\":\"cl").await.unwrap();
    tokio::time::sleep(Duration::from_millis(75)).await;
    output
        .write_all(b"ose\",\"protocol_version\":5}\n")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

/// The panel over a private PipeWire Pulse server with synthetic sinks.
#[tokio::test]
async fn audio_panel_v5_controls_devices_and_ignores_old_revisions() {
    use canopa_panel_protocol::{InteractionEvent, InteractionTarget, v5};
    use tokio::io::{AsyncWriteExt, BufReader};
    let server = AudioServer::new().await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    server
        .pactl(&["set-sink-volume", "speakers", "100%", "50%"])
        .await;
    let (host, provider) = tokio::io::duplex(65536);
    let (input, output) = tokio::io::split(provider);
    let address = server.server.clone();
    let running = tokio::spawn(run(BufReader::new(input), output, Some(address)));
    let (input, mut output) = tokio::io::split(host);
    let mut input = BufReader::new(input);
    async fn next(
        reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
        muted: bool,
        volume: Option<u16>,
    ) -> v5::Response {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let response = v5::Response::read_async(reader).await.unwrap().unwrap();
                let v5::Content::Rich(content) = &response.content else {
                    panic!("rich content")
                };
                if content
                    .sections
                    .iter()
                    .flat_map(|section| &section.items)
                    .any(|item| {
                        volume.is_none_or(|value| {
                            item.slider
                                .as_ref()
                                .is_some_and(|slider| slider.value == value)
                        }) && item.actions.iter().any(|action| {
                            action.key == "mute"
                                && action.label == if muted { "Unmute" } else { "Mute" }
                        }) && !item
                            .body
                            .as_deref()
                            .unwrap_or_default()
                            .contains("Applying")
                    })
                {
                    return response;
                }
            }
        })
        .await
        .unwrap()
    }
    let initial = next(&mut input, false, None).await;
    let v5::Content::Rich(content) = &initial.content else {
        unreachable!()
    };
    let item = &content.sections[0].items[0];
    assert!(
        !item.actions.iter().any(|action| action.key == "louder"),
        "a capped channel must hide volume increase even when the average is lower"
    );
    assert_eq!(
        item.slider.as_ref().unwrap().value,
        100,
        "slider shows the loudest channel, not its average"
    );
    let item_id = item.id.clone();
    let invoke = v5::HostMessage::event(
        initial.revision,
        v5::Interaction::Rich(InteractionEvent::Invoke {
            target: InteractionTarget::Item {
                id: item.id.clone(),
            },
            action_key: "mute".into(),
            value: None,
        }),
    );
    output.write_all(&invoke.encode().unwrap()).await.unwrap();
    let updated = next(&mut input, true, None).await;
    assert!(updated.revision > initial.revision);
    assert!(
        server
            .pactl(&["get-sink-mute", "speakers"])
            .await
            .contains("yes")
    );
    // Return the hardware to unmuted externally, then submit an old "mute"
    // frame. Accepting it would wrongly mute the device again.
    server.pactl(&["set-sink-mute", "speakers", "0"]).await;
    let current = next(&mut input, false, None).await;
    output.write_all(&invoke.encode().unwrap()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        server
            .pactl(&["get-sink-mute", "speakers"])
            .await
            .contains("no")
    );
    let set_volume = v5::HostMessage::event(
        current.revision,
        v5::Interaction::Rich(InteractionEvent::Invoke {
            target: InteractionTarget::Item {
                id: item_id.clone(),
            },
            action_key: "volume".into(),
            value: Some("64".into()),
        }),
    );
    output
        .write_all(&set_volume.encode().unwrap())
        .await
        .unwrap();
    let current = next(&mut input, false, Some(64)).await;
    let channels = server.pactl(&["get-sink-volume", "speakers"]).await;
    assert!(
        channels.contains("64%") && channels.contains("32%"),
        "{channels}"
    );
    // Stall only this private Pulse server so multiple valid releases arrive
    // while the first command is still pending. Accepted edits must survive the
    // intermediate observer publication, and the following mute must also run.
    let pulse_pid = server.pulse_pid().to_string();
    server.run("kill", &["-STOP", &pulse_pid]).await;
    let mut batch = Vec::new();
    for (action_key, value) in [
        ("volume", Some("42")),
        ("volume", Some("57")),
        ("volume", Some("64")),
        ("mute", None),
    ] {
        batch.extend(
            v5::HostMessage::event(
                current.revision,
                v5::Interaction::Rich(InteractionEvent::Invoke {
                    target: InteractionTarget::Item {
                        id: item_id.clone(),
                    },
                    action_key: action_key.into(),
                    value: value.map(str::to_owned),
                }),
            )
            .encode()
            .unwrap(),
        );
    }
    output.write_all(&batch).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    server.run("kill", &["-CONT", &pulse_pid]).await;
    next(&mut input, true, Some(64)).await;
    let channels = server.pactl(&["get-sink-volume", "speakers"]).await;
    assert!(
        channels.contains("64%") && channels.contains("32%"),
        "{channels}"
    );
    assert!(
        server
            .pactl(&["get-sink-mute", "speakers"])
            .await
            .contains("yes")
    );
    output
        .write_all(&v5::HostMessage::close().encode().unwrap())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
