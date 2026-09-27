//! Fixtures use the published bytes, independent of the type constructors.
use canopa_panel_protocol::{ProtocolError, v1, v4};

const V4: &[u8] = b"{\"type\":\"replace_content\",\"protocol_version\":4,\"content\":{\"title\":\"Ready\",\"sections\":[],\"actions\":[],\"error\":null}}\n";

#[test]
fn v1_frames_keep_the_existing_header_and_field_names() {
    let payload = br#"{"type":"event","payload":{"type":"text-input-submitted","element_id":"credential","value":"fixture-only"}}"#;
    let mut fixture = b"NIRIBAR_EXTERNAL_PANEL_JSON_V1\n".to_vec();
    fixture.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    fixture.extend_from_slice(payload);
    let decoded: v1::HostMessage = v1::decode_frame(&fixture).unwrap();
    assert_eq!(
        decoded,
        v1::HostMessage::Event(v1::ProviderEvent::TextInputSubmitted {
            element_id: "credential".into(),
            value: "fixture-only".into(),
        })
    );
    assert_eq!(v1::encode_frame(&decoded).unwrap(), fixture);
}

#[test]
fn v1_content_contains_only_provider_fields() {
    let wire = br#"{"elements":[{"type":"selectable-list","id":"networks","entries":[{"id":"home","label":"Home","disabled":false,"active":false}],"allow_custom":true},{"type":"text-input","id":"credential","label":"Password","value":"","secret":true}]}"#;
    let content: v1::ExternalPanelDocument = serde_json::from_slice(wire).unwrap();
    content.validate().unwrap();
    assert_eq!(serde_json::to_vec(&content).unwrap(), wire);
    // The old serde-skipped search field was ignored on input. Preserve that
    // behavior while keeping local search state out of the shared Rust model.
    let with_search = String::from_utf8(wire.to_vec()).unwrap().replace(
        "\"allow_custom\":true",
        "\"allow_custom\":true,\"search_query\":\"ignored\"",
    );
    assert_eq!(
        serde_json::from_str::<v1::ExternalPanelDocument>(&with_search).unwrap(),
        content
    );
    assert!(
        serde_json::from_str::<v1::ExternalPanelDocument>(
            r#"{"elements":[{"type":"canopa","snapshot":{}}]}"#
        )
        .is_err()
    );
}

#[test]
fn v4_forwarding_preserves_bytes_and_reads_one_frame_at_a_time() {
    let bytes = [V4, V4].concat();
    let mut reader = bytes.as_slice();
    assert_eq!(
        v4::Frame::read(&mut reader).unwrap().unwrap().into_bytes(),
        V4
    );
    let content = v4::Frame::read(&mut reader)
        .unwrap()
        .unwrap()
        .into_content();
    assert_eq!(content, v4::PanelContent::new("Ready"));
    assert!(v4::Frame::read(&mut reader).unwrap().is_none());
    // Existing root imports remain v4 for out-of-workspace providers.
    assert_eq!(
        canopa_panel_protocol::PROTOCOL_VERSION,
        v4::PROTOCOL_VERSION
    );
}

fn invalid_frames() -> Vec<(Vec<u8>, ProtocolError)> {
    vec![
        (V4[..V4.len() - 1].to_vec(), ProtocolError::FrameTooShort),
        (
            vec![b' '; v4::MAX_FRAME_BYTES + 1],
            ProtocolError::FrameTooLarge,
        ),
        (
            String::from_utf8(V4.to_vec())
                .unwrap()
                .replace("version\":4", "version\":5")
                .into_bytes(),
            ProtocolError::IncompatibleVersion(5),
        ),
        (
            b"{\n".to_vec(),
            ProtocolError::Json("invalid Canopa provider message".into()),
        ),
        (
            String::from_utf8(V4.to_vec())
                .unwrap()
                .replace(
                    "\"actions\":[]",
                    "\"actions\":[{\"key\":\"\",\"label\":\"Invalid\",\"value\":null}]",
                )
                .into_bytes(),
            ProtocolError::InvalidContent("invalid Canopa content".into()),
        ),
    ]
}

#[test]
fn v4_rejects_invalid_frames_with_the_existing_limits() {
    for (bytes, error) in invalid_frames() {
        assert_eq!(v4::Frame::read(&mut bytes.as_slice()).err(), Some(error));
    }
    let mut exact = vec![b' '; v4::MAX_FRAME_BYTES - V4.len()];
    exact.extend_from_slice(V4);
    assert_eq!(
        v4::Frame::read(&mut exact.as_slice())
            .unwrap()
            .unwrap()
            .into_bytes(),
        exact
    );
    // The overflow probe consumes at most one byte beyond the frame budget.
    let bytes = vec![b' '; v4::MAX_FRAME_BYTES + 8];
    let mut reader = bytes.as_slice();
    assert!(v4::Frame::read(&mut reader).is_err());
    assert_eq!(reader.len(), 7);
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn v4_async_host_and_sync_provider_accept_and_reject_the_same_frames() {
    for (bytes, expected) in invalid_frames() {
        assert_eq!(
            v4::Frame::read_async(&mut bytes.as_slice()).await.err(),
            Some(expected)
        );
    }
    let bytes = [V4, V4].concat();
    let mut reader = bytes.as_slice();
    assert_eq!(
        v4::Frame::read_async(&mut reader)
            .await
            .unwrap()
            .unwrap()
            .into_bytes(),
        V4
    );
    assert_eq!(
        v4::Frame::read_async(&mut reader)
            .await
            .unwrap()
            .unwrap()
            .into_content(),
        v4::PanelContent::new("Ready")
    );
    assert!(v4::Frame::read_async(&mut reader).await.unwrap().is_none());
}
