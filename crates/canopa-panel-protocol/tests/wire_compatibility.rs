//! Fixtures use the published bytes, independent of the type constructors.
use canopa_panel_protocol::{ProtocolError, v1, v4};

const V4: &[u8] = b"{\"type\":\"replace_content\",\"protocol_version\":4,\"content\":{\"title\":\"Ready\",\"sections\":[],\"actions\":[],\"error\":null}}\n";

#[test]
fn optional_section_icons_preserve_legacy_sections_and_validate_in_v5() {
    let legacy = serde_json::json!({"id":"mail", "title":"Mail", "items":[], "controls":[]});
    let section: v4::PanelSection = serde_json::from_value(legacy.clone()).unwrap();
    assert!(section.icon.is_none());
    assert_eq!(serde_json::to_value(section).unwrap(), legacy);
    let mut with_icon = legacy;
    with_icon["icon"] =
        serde_json::json!({"kind":"theme", "names":["mail"], "theme":"hicolor", "scale":1});
    #[derive(serde::Deserialize)]
    struct LegacySection {
        id: String,
        title: String,
        items: Vec<v4::PanelItem>,
        controls: Vec<v4::PanelControl>,
    }
    let old: LegacySection = serde_json::from_value(with_icon.clone()).unwrap();
    assert_eq!(
        (
            old.id.as_str(),
            old.title.as_str(),
            old.items.len(),
            old.controls.len()
        ),
        ("mail", "Mail", 0, 0)
    );
    let mut content = v4::PanelContent::new("Settings");
    content
        .sections
        .push(serde_json::from_value(with_icon).unwrap());
    let mut session = canopa_panel_protocol::v5::Session::default();
    session
        .replace(
            canopa_panel_protocol::v5::Content::Rich(content.clone()),
            canopa_panel_protocol::v5::Update::Replace,
        )
        .unwrap();
    content.sections[0].icon = Some(v4::PanelIcon::LocalPath {
        path: "relative.png".into(),
    });
    assert!(content.validate().is_err());
}

#[test]
fn optional_item_attribution_preserves_legacy_content_and_v5_interactions() {
    let legacy = serde_json::json!({"id":"message", "title":"Hello", "body":"Body",
        "styled_body":null, "rich_body":null, "metadata":[], "actions":[]});
    let item: v4::PanelItem = serde_json::from_value(legacy.clone()).unwrap();
    assert!(item.attribution.is_none());
    assert_eq!(serde_json::to_value(item).unwrap(), legacy);
    let mut with_source = legacy.clone();
    with_source["attribution"] = serde_json::json!({"label":"Mail", "icon":{
        "kind":"theme", "names":["org.example.Mail.desktop", "org.example.Mail"],
        "theme":"hicolor", "scale":2}});
    // The published item decoder ignores unknown fields. Existing readers keep
    // the original content, and new readers accept items without attribution.
    #[derive(serde::Deserialize)]
    struct LegacyItem {
        id: String,
        title: String,
        body: Option<String>,
        actions: Vec<v4::PanelAction>,
    }
    let old: LegacyItem = serde_json::from_value(with_source.clone()).unwrap();
    assert_eq!(
        (
            old.id.as_str(),
            old.title.as_str(),
            old.body.as_deref(),
            old.actions.len()
        ),
        ("message", "Hello", Some("Body"), 0)
    );
    let mut content = v4::PanelContent::new("Messages");
    content.sections.push(v4::PanelSection {
        id: "mail".into(),
        title: "Mail".into(),
        icon: None,
        controls: vec![],
        items: vec![serde_json::from_value(with_source).unwrap()],
    });
    let mut session = canopa_panel_protocol::v5::Session::default();
    let response = session
        .replace(
            canopa_panel_protocol::v5::Content::Rich(content.clone()),
            canopa_panel_protocol::v5::Update::Replace,
        )
        .unwrap();
    assert!(response.content.validate().is_ok());
    for icon in [
        v4::PanelIcon::LocalPath {
            path: "https://example.org/icon.png".into(),
        },
        v4::PanelIcon::Theme {
            names: vec!["../escape".into()],
            theme: "hicolor".into(),
            scale: 1,
        },
        v4::PanelIcon::Theme {
            names: vec!["mail".into(); 3],
            theme: "hicolor".into(),
            scale: 1,
        },
    ] {
        content.sections[0].items[0]
            .attribution
            .as_mut()
            .unwrap()
            .icon = Some(icon);
        assert!(content.validate().is_err());
    }
}

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
