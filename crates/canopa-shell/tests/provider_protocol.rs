use canopa_panel_protocol::v1::{
    ExternalPanelDocument, ExternalPanelElement, ExternalPanelSeverity, HostHandshake, HostMessage,
    PROTOCOL_VERSION, ProviderEvent, ProviderMessage, ProviderOrigin, decode_frame, encode_frame,
};

#[test]
fn provider_side_code_uses_shared_protocol_and_content_model() {
    let host = HostMessage::Handshake(HostHandshake {
        protocol_version: PROTOCOL_VERSION,
        session_id: "external-panel-1".into(),
        provider_name: "wifi".into(),
        origin: ProviderOrigin {
            bar_name: "main".into(),
            output_name: "DP-1".into(),
            origin_kind: "module".into(),
        },
    });
    let host_frame = encode_frame(&host).unwrap();
    assert_eq!(decode_frame::<HostMessage>(&host_frame).unwrap(), host);

    let content = ExternalPanelDocument {
        elements: vec![ExternalPanelElement::Text {
            text: "Wi-Fi ready".into(),
        }],
    };
    content.validate().unwrap();

    let provider = ProviderMessage::Handshake {
        protocol_version: PROTOCOL_VERSION,
        content,
    };
    let provider_frame = encode_frame(&provider).unwrap();
    assert_eq!(
        decode_frame::<ProviderMessage>(&provider_frame).unwrap(),
        provider
    );

    let event = HostMessage::Event(ProviderEvent::Close);
    assert_eq!(
        decode_frame::<HostMessage>(&encode_frame(&event).unwrap()).unwrap(),
        event
    );
}

#[tokio::test]
async fn shipped_wifi_provider_binary_completes_handshake_with_shared_protocol() {
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_canopa-wifi-provider"))
        .env("CANOPA_WIFI_PROVIDER_BACKEND", "none")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    canopa_panel_protocol::v1::write_frame(
        &mut stdin,
        &HostMessage::Handshake(HostHandshake {
            protocol_version: PROTOCOL_VERSION,
            session_id: "external-panel-1".into(),
            provider_name: "wifi".into(),
            origin: ProviderOrigin {
                bar_name: "main".into(),
                output_name: "DP-1".into(),
                origin_kind: "module".into(),
            },
        }),
    )
    .await
    .unwrap();

    let message = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        canopa_panel_protocol::v1::read_frame::<_, ProviderMessage>(&mut stdout),
    )
    .await
    .expect("provider handshake should not wait for scan")
    .unwrap();

    match message {
        ProviderMessage::Handshake {
            protocol_version,
            content,
        } => {
            assert_eq!(protocol_version, PROTOCOL_VERSION);
            content.validate().unwrap();
            assert!(matches!(
                &content.elements[0],
                ExternalPanelElement::Status {
                    title,
                    severity: ExternalPanelSeverity::Error,
                    ..
                } if title == "Wi-Fi unavailable"
            ));
        }
        other => panic!("expected provider handshake, got {other:?}"),
    }

    canopa_panel_protocol::v1::write_frame(&mut stdin, &HostMessage::Event(ProviderEvent::Close))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), child.wait())
        .await
        .expect("provider should exit after close")
        .unwrap();
}

#[tokio::test]
async fn shipped_wifi_provider_v5_is_provider_first_and_closes_cleanly() {
    for (current, legacy) in [
        (Some("none"), None),
        (None, Some("none")),
        (Some("none"), Some("iwd")),
    ] {
        assert_wifi_provider_v5_closes(current, legacy).await;
    }
}

async fn assert_wifi_provider_v5_closes(current: Option<&str>, legacy: Option<&str>) {
    use canopa_panel_protocol::v5;
    use tokio::io::AsyncWriteExt;
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_canopa-wifi-provider"));
    command
        .args(["--protocol", "canopa-v5"])
        .env_remove("CANOPA_WIFI_PROVIDER_BACKEND")
        .env_remove("NIRIBAR_WIFI_PROVIDER_BACKEND")
        .kill_on_drop(true)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    if let Some(value) = current {
        command.env("CANOPA_WIFI_PROVIDER_BACKEND", value);
    }
    if let Some(value) = legacy {
        command.env("NIRIBAR_WIFI_PROVIDER_BACKEND", value);
    }
    let mut child = command.spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = tokio::io::BufReader::new(child.stdout.take().unwrap());
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        v5::Response::read_async(&mut output),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(response.revision, 1);
    let v5::Content::Form(form) = response.content else {
        panic!("form")
    };
    assert!(
        matches!(&form.elements[0], v5::FormElement::Status { title, .. } if title == "Wi-Fi unavailable")
    );
    input
        .write_all(&v5::HostMessage::close().encode().unwrap())
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}
