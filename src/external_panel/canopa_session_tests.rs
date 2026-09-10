use super::*;

#[tokio::test]
async fn canopa_process_receives_typed_actions_and_streams_replacements() {
    let initial = r#"{"type":"replace_content","protocol_version":3,"content":{"title":"Initial","sections":[],"actions":[{"key":"refresh","label":"Refresh","value":null}],"error":null}}"#;
    let updated = r#"{"type":"replace_content","protocol_version":3,"content":{"title":"Updated","sections":[],"actions":[],"error":null}}"#;
    let expected_event =
        r#"{"type":"invoke","target":{"target":"content"},"action_key":"refresh","value":null}"#;
    // A host-first handshake would fail this fixture's exact event comparison.
    let script = format!(
        "printf '%s\\n' '{initial}'; IFS= read -r event; [ \"$event\" = '{expected_event}' ] || exit 9; printf '%s\\n' '{updated}'; while IFS= read -r event; do :; done"
    );
    let command = crate::config::ConfiguredCommand::argv_for_test("sh", vec!["-c".into(), script]);
    let process = spawn_provider_process(&command).unwrap();
    let session = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        process.start_session_for_protocol(
            protocol::HostHandshake {
                protocol_version: protocol::PROTOCOL_VERSION,
                session_id: "canopa-process-test".into(),
                provider_name: "notifications".into(),
                origin: protocol::ProviderOrigin {
                    bar_name: "main".into(),
                    output_name: "test-output".into(),
                    origin_kind: "module".into(),
                },
            },
            ProviderProtocol::CanopaV3,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let protocol::ExternalPanelElement::Canopa { snapshot } = &session.content.elements[0] else {
        panic!("expected Canopa content")
    };
    assert_eq!(snapshot.content.title, "Initial");
    let element_id = snapshot.action_id(
        canopa_panel_protocol::InteractionTarget::Content,
        &snapshot.content.actions[0],
    );
    let (sent, received) = tokio::join!(
        session
            .handle
            .send_event(protocol::ProviderEvent::ButtonActivated { element_id }),
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            session.handle.read_next_content()
        )
    );
    sent.unwrap();
    let document = received.unwrap().unwrap();
    let protocol::ExternalPanelElement::Canopa { snapshot } = &document.elements[0] else {
        panic!("expected replacement")
    };
    assert_eq!(snapshot.content.title, "Updated");
    session.handle.close();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        session.handle.wait_for_exit(),
    )
    .await
    .unwrap()
    .unwrap();
}
