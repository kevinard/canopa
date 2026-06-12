use niribar::external_panel::protocol::{
    ExternalPanelDocument, ExternalPanelElement, HostHandshake, HostMessage, PROTOCOL_VERSION,
    ProviderEvent, ProviderMessage, ProviderOrigin, decode_frame, encode_frame,
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
