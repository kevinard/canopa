//! A response is usable only if each advertised interaction can cross the wire.
use super::{Content, FormElement, FormEvent, HostMessage, Interaction, ProtocolError};
use crate::v4::{InteractionEvent, InteractionTarget, RichBodySegment};

fn check(interaction: Interaction) -> Result<(), ProtocolError> {
    // Reserve the longest revision so a later replacement cannot invalidate
    // an otherwise identical interaction. Encoding accounts for JSON escaping.
    HostMessage::event(u64::MAX, interaction)
        .encode()
        .map(|_| ())
}

fn form(event: FormEvent) -> Result<(), ProtocolError> {
    check(Interaction::Form(event))
}

fn rich(target: InteractionTarget, key: &str, value: Option<&str>) -> Result<(), ProtocolError> {
    check(Interaction::Rich(InteractionEvent::Invoke {
        target,
        action_key: key.into(),
        value: value.map(str::to_owned),
    }))
}

pub(super) fn validate(content: &Content) -> Result<(), ProtocolError> {
    match content {
        Content::Form(content) => {
            for element in &content.elements {
                match element {
                    FormElement::Button { id, .. } => form(FormEvent::ButtonActivated {
                        element_id: id.clone(),
                    })?,
                    FormElement::TextInput { id, value, .. } => {
                        form(FormEvent::TextInputChanged {
                            element_id: id.clone(),
                            value: value.clone(),
                        })?;
                        form(FormEvent::TextInputSubmitted {
                            element_id: id.clone(),
                            value: value.clone(),
                        })?;
                    }
                    FormElement::SelectableList {
                        id,
                        entries,
                        allow_custom,
                    } => {
                        for entry in entries.iter().filter(|entry| !entry.disabled) {
                            form(FormEvent::SelectableListEntryActivated {
                                element_id: id.clone(),
                                entry_id: entry.id.clone(),
                            })?;
                        }
                        if *allow_custom {
                            form(FormEvent::SelectableListCustomValueSubmitted {
                                element_id: id.clone(),
                                value: String::new(),
                            })?;
                        }
                    }
                    _ => {}
                }
            }
        }
        Content::Rich(content) => {
            for action in &content.actions {
                rich(
                    InteractionTarget::Content,
                    &action.key,
                    action.value.as_deref(),
                )?;
            }
            for section in &content.sections {
                for control in &section.controls {
                    for option in &control.options {
                        rich(
                            InteractionTarget::Section {
                                id: section.id.clone(),
                            },
                            &control.key,
                            Some(&option.value),
                        )?;
                    }
                }
                for item in &section.items {
                    let target = InteractionTarget::Item {
                        id: item.id.clone(),
                    };
                    for action in
                        item.actions
                            .iter()
                            .chain(item.rich_body.iter().flat_map(|body| {
                                body.segments.iter().filter_map(|segment| match segment {
                                    RichBodySegment::Link { action, .. } => Some(action),
                                    _ => None,
                                })
                            }))
                    {
                        rich(target.clone(), &action.key, action.value.as_deref())?;
                    }
                    if let Some(slider) = &item.slider {
                        rich(target, &slider.key, Some(&slider.max.to_string()))?;
                    }
                }
            }
        }
    }
    Ok(())
}
