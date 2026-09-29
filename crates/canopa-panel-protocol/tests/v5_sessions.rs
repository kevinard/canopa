use canopa_panel_protocol::{ProtocolError, v1, v4, v5::*};
fn form(value: &str) -> Content {
    Content::Form(FormContent {
        elements: vec![
            FormElement::Status {
                title: "Credential".into(),
                message: "Home".into(),
                severity: v1::ExternalPanelSeverity::Info,
            },
            FormElement::TextInput {
                id: "password".into(),
                label: "Password".into(),
                value: value.into(),
                secret: true,
                error: None,
            },
            FormElement::Button {
                id: "connect".into(),
                label: "Connect".into(),
            },
        ],
    })
}
fn edit(revision: u64, value: &str) -> HostMessage {
    HostMessage::event(
        revision,
        Interaction::Form(FormEvent::TextInputChanged {
            element_id: "password".into(),
            value: value.into(),
        }),
    )
}
#[test]
fn buffered_edits_keep_revision_but_replacing_the_operation_retires_them() {
    let mut provider = Session::default();
    let initial = provider.replace(form(""), Update::Replace).unwrap();
    let mut host = Session::default();
    host.observe(initial.clone()).unwrap();
    for value in ["first", "second", "third"] {
        assert!(provider.accepts(&edit(initial.revision, value)));
        let echo = provider
            .replace(form(value), Update::PreserveFormEdits)
            .unwrap();
        assert_eq!(echo.revision, initial.revision);
        host.observe(echo).unwrap();
    }
    let submit = HostMessage::event(
        initial.revision,
        Interaction::Form(FormEvent::ButtonActivated {
            element_id: "connect".into(),
        }),
    );
    assert!(provider.accepts(&submit));
    let next = provider.replace(form(""), Update::Replace).unwrap();
    assert!(next.revision > initial.revision);
    assert!(!provider.accepts(&submit));
    assert!(!provider.accepts(&edit(initial.revision, "late")));
    host.observe(next).unwrap();
    assert!(host.observe(initial).is_err());
    assert!(provider.accepts(&HostMessage::close()));
}
#[test]
fn changed_form_context_cannot_reuse_a_revision_even_with_reused_ids() {
    let mut provider = Session::default();
    let old = provider.replace(form(""), Update::Replace).unwrap();
    let mut changed = form("");
    let Content::Form(document) = &mut changed else {
        unreachable!()
    };
    let FormElement::Status { message, .. } = &mut document.elements[0] else {
        unreachable!()
    };
    *message = "Different network".into();
    let next = provider
        .replace(changed, Update::PreserveFormEdits)
        .unwrap();
    assert!(next.revision > old.revision);
    let mut host = Session::default();
    host.observe(old.clone()).unwrap();
    let mut forged = next;
    forged.revision = old.revision;
    assert!(host.observe(forged).is_err());
    assert!(host.accepts(&edit(old.revision, "still current")));
}
#[test]
fn validation_checks_control_kind_disabled_entries_and_custom_permission() {
    let mut session = Session::default();
    let response = session
        .replace(
            Content::Form(FormContent {
                elements: vec![FormElement::SelectableList {
                    id: "networks".into(),
                    allow_custom: false,
                    entries: vec![v1::ExternalSelectableEntry {
                        id: "home".into(),
                        label: "Home".into(),
                        description: None,
                        disabled: true,
                        active: false,
                        status: None,
                    }],
                }],
            }),
            Update::Replace,
        )
        .unwrap();
    for event in [
        FormEvent::ButtonActivated {
            element_id: "networks".into(),
        },
        FormEvent::SelectableListEntryActivated {
            element_id: "networks".into(),
            entry_id: "home".into(),
        },
        FormEvent::SelectableListCustomValueSubmitted {
            element_id: "networks".into(),
            value: "hidden".into(),
        },
        FormEvent::TextInputChanged {
            element_id: "networks".into(),
            value: "password".into(),
        },
        FormEvent::Close,
    ] {
        assert!(!session.accepts(&HostMessage::event(
            response.revision,
            Interaction::Form(event)
        )));
    }
}
#[test]
fn rich_actions_retain_opaque_values_and_require_the_current_revision() {
    let mut content = RichContent::new("Notifications");
    content.actions.push(v4::PanelAction {
        key: "opaque".into(),
        label: "Clear".into(),
        value: Some("domain-revision".into()),
    });
    let mut session = Session::default();
    let first = session
        .replace(Content::Rich(content.clone()), Update::Replace)
        .unwrap();
    let action = HostMessage::event(
        first.revision,
        Interaction::Rich(RichEvent::Invoke {
            target: v4::InteractionTarget::Content,
            action_key: "opaque".into(),
            value: Some("domain-revision".into()),
        }),
    );
    assert!(session.accepts(&action));
    let next = session
        .replace(Content::Rich(content), Update::PreserveFormEdits)
        .unwrap();
    assert!(
        next.revision > first.revision,
        "rich actions always retire on replacement"
    );
    assert!(!session.accepts(&action));
    assert_eq!(
        HostMessage::decode(&action.encode().unwrap()).unwrap(),
        action
    );
}
#[test]
fn rich_membership_checks_section_options_and_item_actions_including_links() {
    let mut content = RichContent::new("Center");
    let action = |key: &str| v4::PanelAction {
        key: key.into(),
        label: key.into(),
        value: Some("domain-token".into()),
    };
    content.sections.push(v4::PanelSection {
        icon: None,
        id: "mail".into(),
        title: "Mail".into(),
        controls: vec![v4::PanelControl {
            key: "permission".into(),
            label: "Permission".into(),
            value: "allow".into(),
            options: vec![v4::PanelControlOption {
                value: "deny".into(),
                label: "Deny".into(),
            }],
        }],
        items: vec![v4::PanelItem {
            attribution: None,
            id: "message".into(),
            title: "Message".into(),
            body: None,
            styled_body: None,
            rich_body: Some(v4::RichBody {
                segments: vec![v4::RichBodySegment::Link {
                    uri: "https://example.org".into(),
                    action: action("link"),
                    style: Default::default(),
                }],
            }),
            metadata: vec![],
            actions: vec![action("open")],
        }],
    });
    let mut session = Session::default();
    let response = session
        .replace(Content::Rich(content), Update::Replace)
        .unwrap();
    let section = v4::InteractionTarget::Section { id: "mail".into() };
    let item = v4::InteractionTarget::Item {
        id: "message".into(),
    };
    let accepts = |target, key: &str, value: &str| {
        session.accepts(&HostMessage::event(
            response.revision,
            Interaction::Rich(RichEvent::Invoke {
                target,
                action_key: key.into(),
                value: Some(value.into()),
            }),
        ))
    };
    assert!(accepts(section.clone(), "permission", "deny"));
    assert!(!accepts(section.clone(), "permission", "unpublished"));
    assert!(!accepts(section, "open", "domain-token"));
    for key in ["open", "link"] {
        assert!(accepts(item.clone(), key, "domain-token"));
        assert!(!accepts(item.clone(), key, "other-token"));
        assert!(!accepts(
            v4::InteractionTarget::Content,
            key,
            "domain-token"
        ));
        assert!(!accepts(
            v4::InteractionTarget::Item { id: "other".into() },
            key,
            "domain-token"
        ));
    }
    assert!(!accepts(item, "unknown", "domain-token"));
}
#[test]
fn codecs_bound_both_directions_and_do_not_install_unsendable_content() {
    let mut session = Session::default();
    let response = session.replace(form(""), Update::Replace).unwrap();
    let encoded = response.encode().unwrap();
    assert_eq!(Response::decode(&encoded).unwrap(), response);
    let mut stream = [encoded.as_slice(), encoded.as_slice()].concat();
    let mut reader = stream.as_slice();
    assert_eq!(Response::read(&mut reader).unwrap(), Some(response.clone()));
    assert_eq!(Response::read(&mut reader).unwrap(), Some(response.clone()));
    assert!(Response::read(&mut reader).unwrap().is_none());
    stream.pop();
    assert!(Response::decode(&stream).is_err());
    let mut bad = response.clone();
    bad.protocol_version = 4;
    assert_eq!(
        Response::decode(
            &serde_json::to_vec(&bad)
                .unwrap()
                .into_iter()
                .chain([b'\n'])
                .collect::<Vec<_>>()
        )
        .unwrap_err(),
        ProtocolError::IncompatibleVersion(4)
    );
    assert!(
        session
            .replace(form(&"x".repeat(MAX_FRAME_BYTES)), Update::Replace)
            .is_err()
    );
    assert!(session.accepts(&edit(response.revision, "valid")));
    assert_eq!(
        edit(response.revision, &"x".repeat(MAX_EVENT_BYTES))
            .encode()
            .unwrap_err(),
        ProtocolError::FrameTooLarge
    );
    let overflow = vec![b' '; MAX_EVENT_BYTES + 12];
    let mut reader = overflow.as_slice();
    assert_eq!(
        HostMessage::read(&mut reader).unwrap_err(),
        ProtocolError::FrameTooLarge
    );
    assert_eq!(reader.len(), 11);
}
#[cfg(feature = "tokio")]
#[tokio::test]
async fn async_and_sync_codecs_share_the_same_frames() {
    let response = Session::default()
        .replace(form(""), Update::Replace)
        .unwrap();
    let bytes = response.encode().unwrap();
    assert_eq!(
        Response::read_async(&mut bytes.as_slice()).await.unwrap(),
        Some(response)
    );
    let close = HostMessage::close().encode().unwrap();
    assert_eq!(
        HostMessage::read_async(&mut close.as_slice())
            .await
            .unwrap(),
        Some(HostMessage::close())
    );
}
