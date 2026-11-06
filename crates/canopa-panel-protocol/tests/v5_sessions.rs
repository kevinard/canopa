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
        request_activation_token: false,
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
        request_activation_token: false,
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
            slider: None,
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
fn advertised_actions_must_fit_the_event_limit_including_json_escapes() {
    let large = "\u{1}".repeat(MAX_EVENT_BYTES / 6);
    for content in [
        Content::Form(FormContent {
            elements: vec![FormElement::Button {
                id: large.clone(),
                label: "Run".into(),
            }],
        }),
        Content::Form(FormContent {
            elements: vec![FormElement::TextInput {
                id: "input".into(),
                label: "Input".into(),
                value: large.clone(),
                secret: false,
                error: None,
            }],
        }),
        {
            let mut content = RichContent::new("Actions");
            content.actions.push(v4::PanelAction {
                request_activation_token: false,
                key: "run".into(),
                label: "Run".into(),
                value: Some(large),
            });
            Content::Rich(content)
        },
    ] {
        let mut session = Session::default();
        let original = session.replace(form(""), Update::Replace).unwrap();
        let response = Response {
            protocol_version: PROTOCOL_VERSION,
            revision: 1,
            content: content.clone(),
        };
        assert!(
            response.encode().is_err(),
            "must reject an unsendable advertised interaction"
        );
        assert!(session.replace(content, Update::Replace).is_err());
        assert!(session.accepts(&edit(original.revision, "valid")));
    }
}

#[test]
fn action_boundaries_round_trip_at_the_exact_encoded_event_limit() {
    let mut id = "é\"\\\u{1}".repeat(100);
    let event = |id: &str| {
        HostMessage::event(
            u64::MAX,
            Interaction::Form(FormEvent::ButtonActivated {
                element_id: id.into(),
            }),
        )
    };
    let spare = MAX_EVENT_BYTES - event(&id).encode().unwrap().len();
    id.push_str(&"x".repeat(spare));
    let frame = event(&id).encode().unwrap();
    assert_eq!(frame.len(), MAX_EVENT_BYTES);
    assert_eq!(HostMessage::decode(&frame).unwrap(), event(&id));
    let content = |id: &str| {
        Content::Form(FormContent {
            elements: vec![FormElement::Button {
                id: id.into(),
                label: "Run".into(),
            }],
        })
    };
    let mut host = Session::default();
    host.observe(Response {
        protocol_version: PROTOCOL_VERSION,
        revision: u64::MAX,
        content: content(&id),
    })
    .unwrap();
    assert!(host.accepts(&event(&id)));
    id.push('x');
    assert_eq!(
        event(&id).encode().unwrap_err(),
        ProtocolError::FrameTooLarge
    );
    assert_eq!(
        content(&id).validate().unwrap_err(),
        ProtocolError::FrameTooLarge
    );
}

#[test]
fn every_rich_interaction_field_is_checked_for_event_size() {
    let base = serde_json::json!({
        "title":"Controls", "actions":[{"key":"top", "label":"Top", "value":"value"}],
        "sections":[{"id":"section", "title":"Section",
            "controls":[{"key":"choice", "label":"Choice", "value":"one", "options":[{"value":"one", "label":"One"}]}],
            "items":[{"id":"item", "title":"Item", "metadata":[],
                "actions":[{"key":"run", "label":"Run", "value":"value"}],
                "rich_body":{"segments":[{"kind":"link", "uri":"https://example.org", "action":{"key":"link", "label":"Link", "value":"value"}, "style":{"bold":false,"italic":false,"underline":false}}]},
                "slider":{"key":"level", "label":"Level", "min":0, "max":65535, "step":1, "value":0, "unit":"%"}
            }]
        }]
    });
    for path in [
        "/actions/0/key",
        "/actions/0/value",
        "/sections/0/id",
        "/sections/0/controls/0/key",
        "/sections/0/controls/0/options/0/value",
        "/sections/0/items/0/id",
        "/sections/0/items/0/actions/0/key",
        "/sections/0/items/0/actions/0/value",
        "/sections/0/items/0/rich_body/segments/0/action/key",
        "/sections/0/items/0/rich_body/segments/0/action/value",
        "/sections/0/items/0/slider/key",
    ] {
        let mut value = base.clone();
        *value.pointer_mut(path).unwrap() =
            serde_json::Value::String("\u{1}".repeat(MAX_EVENT_BYTES / 6));
        let content: RichContent = serde_json::from_value(value).unwrap();
        content.validate().unwrap(); // Valid legacy rich semantics; only the v5 event is too large.
        assert_eq!(
            Content::Rich(content).validate().unwrap_err(),
            ProtocolError::FrameTooLarge,
            "{path}"
        );
    }
    for (list_id, entry_id) in [
        ("x".repeat(MAX_EVENT_BYTES), "entry".into()),
        ("list".into(), "x".repeat(MAX_EVENT_BYTES)),
    ] {
        let content = Content::Form(FormContent {
            elements: vec![FormElement::SelectableList {
                id: list_id,
                allow_custom: true,
                entries: vec![v1::ExternalSelectableEntry {
                    id: entry_id,
                    label: "Entry".into(),
                    description: None,
                    disabled: false,
                    active: false,
                    status: None,
                }],
            }],
        });
        assert_eq!(
            content.validate().unwrap_err(),
            ProtocolError::FrameTooLarge
        );
    }
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

#[test]
fn rich_sliders_validate_ranges_values_and_revisions_without_changing_legacy_items() {
    use canopa_panel_protocol::{v4, v5};
    let content: v4::PanelContent = serde_json::from_value(serde_json::json!({
        "title":"Levels", "error":null, "actions":[], "sections":[{
            "id":"levels", "title":"Levels", "controls":[], "items":[{
                "id":"device", "title":"Device", "body":null, "styled_body":null,
                "rich_body":null, "metadata":[], "actions":[],
                "slider":{"key":"level", "label":"Level", "min":10, "max":90,
                          "step":5, "value":40, "unit":"%"}
            }]
        }]
    }))
    .unwrap();
    let mut session = v5::Session::default();
    let response = session
        .replace(v5::Content::Rich(content.clone()), v5::Update::Replace)
        .unwrap();
    assert_eq!(
        v5::Response::decode(&response.encode().unwrap()).unwrap(),
        response
    );
    let event = |revision, key: &str, value: Option<&str>| {
        v5::HostMessage::event(
            revision,
            v5::Interaction::Rich(v4::InteractionEvent::Invoke {
                target: v4::InteractionTarget::Item {
                    id: "device".into(),
                },
                action_key: key.into(),
                value: value.map(str::to_owned),
            }),
        )
    };
    for value in ["10", "45", "90"] {
        assert!(session.accepts(&event(response.revision, "level", Some(value))));
    }
    for value in [
        None,
        Some("9"),
        Some("91"),
        Some("41"),
        Some("045"),
        Some("45.0"),
        Some("NaN"),
        Some("-1"),
        Some("65536"),
    ] {
        assert!(
            !session.accepts(&event(response.revision, "level", value)),
            "{value:?}"
        );
    }
    assert!(!session.accepts(&event(response.revision, "unknown", Some("45"))));
    session
        .replace(v5::Content::Rich(content.clone()), v5::Update::Replace)
        .unwrap();
    assert!(!session.accepts(&event(response.revision, "level", Some("45"))));
    for (min, max, step, value) in [
        (10, 90, 0, 40),
        (90, 10, 5, 40),
        (10, 10, 5, 10),
        (10, 91, 5, 40),
        (10, 90, 5, 41),
        (10, 90, 5, 95),
    ] {
        let mut invalid = content.clone();
        let slider = invalid.sections[0].items[0].slider.as_mut().unwrap();
        (slider.min, slider.max, slider.step, slider.value) = (min, max, step, value);
        assert!(invalid.validate().is_err());
    }
    let mut collision = content.clone();
    collision.sections[0].items[0]
        .actions
        .push(v4::PanelAction::new("level", "Conflict"));
    assert!(collision.validate().is_err());
    let mut legacy = content;
    legacy.sections[0].items[0].slider = None;
    let wire = serde_json::to_value(&legacy).unwrap();
    assert!(wire["sections"][0]["items"][0].get("slider").is_none());
}

#[test]
fn activation_is_opted_in_revision_bound_bounded_and_absent_from_legacy_events() {
    let content = |activation| {
        Content::Rich(v4::PanelContent {
            title: "Actions".into(),
            sections: vec![],
            actions: vec![v4::PanelAction {
                request_activation_token: activation,
                ..v4::PanelAction::new("open", "Open")
            }],
            error: None,
        })
    };
    let event = |revision| {
        HostMessage::event(
            revision,
            Interaction::Rich(v4::InteractionEvent::Invoke {
                target: v4::InteractionTarget::Content,
                action_key: "open".into(),
                value: None,
            }),
        )
    };
    let mut session = Session::default();
    let old = session.replace(content(false), Update::Replace).unwrap();
    let plain = event(old.revision);
    assert!(!session.requests_activation(&plain));
    assert!(session.accepts(&plain));
    assert!(!session.accepts(&plain.clone().with_activation_token(Some("token".into()))));
    let expected = b"{\"type\":\"event\",\"protocol_version\":5,\"revision\":1,\"interaction\":{\"kind\":\"rich\",\"event\":{\"type\":\"invoke\",\"target\":{\"target\":\"content\"},\"action_key\":\"open\",\"value\":null}}}\n";
    assert_eq!(plain.encode().unwrap(), expected);
    assert_eq!(
        serde_json::to_string(&v4::PanelAction::new("open", "Open")).unwrap(),
        r#"{"key":"open","label":"Open","value":null}"#
    );
    let current = session.replace(content(true), Update::Replace).unwrap();
    assert!(!session.requests_activation(&plain));
    let plain = event(current.revision);
    assert!(session.requests_activation(&plain));
    assert!(session.accepts(&plain));
    assert!(session.accepts(&plain.clone().with_activation_token(Some("fresh".into()))));
    for token in [
        String::new(),
        "nul\0token".into(),
        "x".repeat(MAX_ACTIVATION_TOKEN_BYTES + 1),
    ] {
        assert!(!session.accepts(&plain.clone().with_activation_token(Some(token))));
    }
    let mut old_host = Session::default();
    old_host.observe(current).unwrap();
    assert!(
        old_host.accepts(&plain),
        "a missing context remains a valid fallback"
    );
}

#[test]
fn opted_in_actions_reserve_the_complete_escaped_token_envelope() {
    let mut content = v4::PanelContent {
        title: "Actions".into(),
        sections: vec![],
        actions: vec![v4::PanelAction::new("open", "Open")],
        error: None,
    };
    content.actions[0].value = Some("x".repeat(46 * 1024));
    assert!(Content::Rich(content.clone()).validate().is_ok());
    content.actions[0].request_activation_token = true;
    assert_eq!(
        Content::Rich(content).validate(),
        Err(ProtocolError::FrameTooLarge)
    );
}

#[test]
fn activation_opt_in_keeps_old_strict_host_events_and_permissive_actions_compatible() {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OldEvent {
        #[serde(rename = "type")]
        kind: String,
        protocol_version: u16,
        revision: u64,
        interaction: Interaction,
    }
    #[derive(serde::Deserialize)]
    struct OldAction {
        key: String,
        label: String,
        value: Option<String>,
    }
    let action = v4::PanelAction::new("open", "Open").with_activation_token();
    let old: OldAction = serde_json::from_value(serde_json::to_value(&action).unwrap()).unwrap();
    assert_eq!(
        (old.key.as_str(), old.label.as_str(), old.value),
        ("open", "Open", None)
    );
    let absent: v4::PanelAction =
        serde_json::from_str(r#"{"key":"open","label":"Open","value":null}"#).unwrap();
    assert!(!absent.request_activation_token);
    let message = HostMessage::event(
        9,
        Interaction::Rich(v4::InteractionEvent::Invoke {
            target: v4::InteractionTarget::Content,
            action_key: "open".into(),
            value: None,
        }),
    );
    let old: OldEvent = serde_json::from_slice(&message.encode().unwrap()).unwrap();
    assert_eq!(
        (old.kind.as_str(), old.protocol_version, old.revision),
        ("event", 5, 9)
    );
    assert!(matches!(old.interaction, Interaction::Rich(_)));
    let new = message
        .with_activation_token(Some("fresh".into()))
        .encode()
        .unwrap();
    assert!(
        serde_json::from_slice::<OldEvent>(&new).is_err(),
        "context must never be sent unless the provider opts in"
    );
}
