use canopa_shell::external_panel::canopa::CanopaSession;

#[tokio::test]
async fn live_controls_preserve_keyboard_target_without_accepting_stale_actions() {
    use canopa_panel_protocol::{PanelAction, PanelContent};
    let session = CanopaSession::default();
    let mut content = PanelContent::new("Sound");
    content
        .actions
        .push(PanelAction::new("volume-up", "Increase volume"));
    let mut first = session.prepare(content.clone()).await.unwrap();
    first.move_focus(true);
    let old_action = first.focused_action_id().unwrap().to_owned();
    content.title = "Sound: 55%".into();
    let mut updated = session.prepare(content.clone()).await.unwrap();
    updated.preserve_focus_from(&first);
    assert_eq!(updated.focused_label(), Some("Increase volume"));
    assert_ne!(updated.focused_action_id(), Some(old_action.as_str()));
    assert!(session.interaction(&old_action).is_none());
    assert!(
        session
            .interaction(updated.focused_action_id().unwrap())
            .is_some()
    );
    content.actions.clear();
    let mut removed = session.prepare(content).await.unwrap();
    removed.preserve_focus_from(&updated);
    assert!(removed.focused_action_id().is_none());
}

#[tokio::test]
async fn keyboard_selection_names_action_before_context_and_preserves_wire_target() {
    let mut input = serde_json::json!({
        "type": "replace_content", "protocol_version": 4,
        "content": {"title": "Notifications", "actions": [], "error": null,
            "sections": [{"id": "mail", "title": "Mail", "controls": [{
                "key": "permission", "label": "App permission", "value": "allow",
                "options": [{"label": "Allow", "value": "allow"}, {"label": "Silence", "value": "silence"}]
            }], "items": [{"id": "message", "title": "An important message", "metadata": [],
                "actions": [{"key": "opaque/key", "label": "Reply", "value": "captured-revision"}]
            }]}]
        }
    }).to_string();
    input.push('\n');
    let session = CanopaSession::default();
    let mut snapshot = session
        .read_content(&mut tokio::io::BufReader::new(input.as_bytes()))
        .await
        .unwrap();
    snapshot.move_focus(true);
    assert_eq!(
        snapshot.focused_label(),
        Some("App permission: Allow · Mail")
    );
    snapshot.move_focus(true);
    assert_eq!(
        snapshot.focused_label(),
        Some("App permission: Silence · Mail")
    );
    snapshot.move_focus(true);
    assert_eq!(
        snapshot.focused_label(),
        Some("Reply · Mail · An important message")
    );
    let mut output = Vec::new();
    assert!(
        session
            .write_interaction(&mut output, snapshot.focused_action_id().unwrap())
            .await
            .unwrap()
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
        serde_json::json!({
            "type": "invoke", "target": {"target": "item", "id": "message"},
            "action_key": "opaque/key", "value": "captured-revision"
        })
    );
}

#[tokio::test]
async fn canopa_provider_rejects_bad_versions_truncated_and_oversized_frames() {
    let valid = "{\"type\":\"replace_content\",\"protocol_version\":4,\"content\":{\"title\":\"Ready\",\"sections\":[],\"actions\":[],\"error\":null}}\n";
    for input in [
        valid.replace("version\":4", "version\":3"),
        valid.replace("version\":4", "version\":5"),
        valid.trim_end().to_string(),
        " ".repeat(canopa_panel_protocol::MAX_FRAME_BYTES + 1),
    ] {
        assert!(
            CanopaSession::default()
                .read_content(&mut tokio::io::BufReader::new(input.as_bytes()))
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn canopa_provider_accepts_v4_content_and_sends_typed_actions() {
    let input = br#"{"type":"replace_content","protocol_version":4,"content":{"title":"Notifications","sections":[],"actions":[{"key":"clear","label":"Clear","value":null}],"error":null}}
"#;
    let mut reader = tokio::io::BufReader::new(&input[..]);
    let session = CanopaSession::default();
    let snapshot = session.read_content(&mut reader).await.unwrap();
    assert_eq!(snapshot.content.title, "Notifications");
    let action_id = snapshot.action_id(
        canopa_panel_protocol::InteractionTarget::Content,
        &snapshot.content.actions[0],
    );
    let mut output = Vec::new();
    session
        .write_interaction(&mut output, &action_id)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "{\"type\":\"invoke\",\"target\":{\"target\":\"content\"},\"action_key\":\"clear\",\"value\":null}\n"
    );
    let mut replacement = tokio::io::BufReader::new(&input[..]);
    session.read_content(&mut replacement).await.unwrap();
    let mut stale_output = Vec::new();
    assert!(
        !session
            .write_interaction(&mut stale_output, &action_id)
            .await
            .unwrap()
    );
    assert!(stale_output.is_empty());
}

#[tokio::test]
async fn canopa_provider_prepares_only_bounded_local_images() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("body.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
        .save(&path)
        .unwrap();
    let mut input = serde_json::json!({"type":"replace_content","protocol_version":4,"content":{"title":"Images","actions":[],"error":null,"sections":[{"id":"mail","title":"Mail","controls":[],"items":[{"id":"1","title":"Photo","body":"fallback","styled_body":null,"rich_body":{"segments":[{"kind":"image","image":{"source":{"kind":"local_path","path":path},"alt":"Red square"}}]},"metadata":[],"actions":[]}]}]}}).to_string();
    input.push('\n');
    let snapshot = CanopaSession::default()
        .read_content(&mut tokio::io::BufReader::new(input.as_bytes()))
        .await
        .unwrap();
    assert!(
        matches!(
            snapshot.image(path.to_str().unwrap()),
            Some(iced::widget::image::Handle::Rgba {
                width: 2,
                height: 2,
                ..
            })
        ),
        "decoded image: {:?}",
        snapshot.image(path.to_str().unwrap())
    );
    assert!(
        snapshot
            .image("https://example.invalid/image.png")
            .is_none()
    );
}

#[tokio::test]
async fn canopa_image_policy_keeps_smaller_limits_thumbnailing_and_codec_support() {
    let temp = tempfile::tempdir().unwrap();
    let small = temp.path().join("small.gif");
    let large = temp.path().join("thumbnail.png");
    let wide = temp.path().join("wide.png");
    let oversized = temp.path().join("oversized.png");
    image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 255]))
        .save(&small)
        .unwrap();
    image::RgbaImage::from_pixel(1024, 512, image::Rgba([255, 0, 0, 255]))
        .save(&large)
        .unwrap();
    image::RgbaImage::from_pixel(2049, 1, image::Rgba([255, 0, 0, 255]))
        .save(&wide)
        .unwrap();
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    let paths = [&small, &large, &wide, &oversized];
    let segments: Vec<_> = paths.iter().map(|path| serde_json::json!({
        "kind": "image", "image": {"source": {"kind": "local_path", "path": path}, "alt": "Unavailable"}
    })).collect();
    let mut input = serde_json::json!({
        "type": "replace_content", "protocol_version": 4,
        "content": {"title": "Images", "actions": [], "error": null, "sections": [{
            "id": "images", "title": "Images", "controls": [], "items": [{
                "id": "images", "title": "Images", "rich_body": {"segments": segments},
                "metadata": [], "actions": []
            }]
        }]}
    })
    .to_string();
    input.push('\n');
    let snapshot = CanopaSession::default()
        .read_content(&mut tokio::io::BufReader::new(input.as_bytes()))
        .await
        .unwrap();
    assert!(matches!(
        snapshot.image(small.to_str().unwrap()),
        Some(iced::widget::image::Handle::Rgba {
            width: 4,
            height: 2,
            ..
        })
    ));
    assert!(matches!(
        snapshot.image(large.to_str().unwrap()),
        Some(iced::widget::image::Handle::Rgba {
            width: 512,
            height: 256,
            ..
        })
    ));
    assert!(snapshot.image(wide.to_str().unwrap()).is_none());
    assert!(snapshot.image(oversized.to_str().unwrap()).is_none());
}

#[tokio::test]
async fn canopa_provider_rejects_ambiguous_item_ids() {
    let input = br#"{"type":"replace_content","protocol_version":4,"content":{"title":"Notifications","actions":[],"error":null,"sections":[{"id":"mail","title":"Mail","controls":[],"items":[{"id":"1","title":"Old","body":null,"styled_body":null,"rich_body":null,"metadata":[],"actions":[]},{"id":"1","title":"New","body":null,"styled_body":null,"rich_body":null,"metadata":[],"actions":[]}]}]}}
"#;
    let mut reader = tokio::io::BufReader::new(&input[..]);
    assert!(
        CanopaSession::default()
            .read_content(&mut reader)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn canopa_panel_keyboard_focus_visits_each_action_and_resets_on_replacement() {
    let input = br#"{"type":"replace_content","protocol_version":4,"content":{"title":"Notifications","sections":[],"actions":[{"key":"clear","label":"Clear","value":null},{"key":"delete","label":"Delete history","value":null}],"error":null}}
"#;
    let session = CanopaSession::default();
    let mut snapshot = session
        .read_content(&mut tokio::io::BufReader::new(&input[..]))
        .await
        .unwrap();
    assert!(snapshot.focused_action_id().is_none());
    snapshot.move_focus(true);
    let mut output = Vec::new();
    session
        .write_interaction(&mut output, snapshot.focused_action_id().unwrap())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()["action_key"],
        "clear"
    );
    snapshot.move_focus(true);
    output.clear();
    session
        .write_interaction(&mut output, snapshot.focused_action_id().unwrap())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()["action_key"],
        "delete"
    );
    let next = session
        .read_content(&mut tokio::io::BufReader::new(&input[..]))
        .await
        .unwrap();
    assert!(next.focused_action_id().is_none());
}

#[tokio::test]
async fn numeric_controls_keep_opaque_focus_but_only_accept_advertised_current_values() {
    use canopa_shell::external_panel::canopa::{CanopaSession, slider_value_action};
    let content: canopa_panel_protocol::PanelContent = serde_json::from_value(serde_json::json!({
        "title":"Level", "error":null, "actions":[], "sections":[{
            "id":"s", "title":"Levels", "controls":[], "items":[{
                "id":"i", "title":"Device", "body":null, "styled_body":null, "rich_body":null,
                "metadata":[], "actions":[], "slider":{"key":"level","label":"Level",
                "min":0,"max":100,"step":1,"value":40,"unit":"%"}
            }]
        }]
    }))
    .unwrap();
    let session = CanopaSession::default();
    let mut initial = session.prepare(content.clone()).await.unwrap();
    initial.move_focus(true);
    let id = initial.focused_action_id().unwrap().to_owned();
    assert!(initial.slider(&id).is_some());
    assert!(
        session.interaction(&id).is_none(),
        "focusing or pressing Enter must not change a level"
    );
    let selected = slider_value_action(&id, 63).unwrap();
    assert!(session.interaction(&selected).is_some());
    assert!(
        session
            .interaction(&slider_value_action(&id, 101).unwrap())
            .is_none()
    );
    let mut next = session.prepare(content).await.unwrap();
    next.preserve_focus_from(&initial);
    assert_eq!(next.focused_label(), initial.focused_label());
    assert_ne!(next.focused_action_id(), Some(id.as_str()));
    assert!(session.interaction(&selected).is_none());
}
