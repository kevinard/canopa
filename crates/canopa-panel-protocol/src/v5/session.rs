use super::*;

#[derive(Clone, Copy)]
pub enum Update {
    Replace,
    /// The provider guarantees that the form still addresses the same operation.
    /// Only input values/errors may differ; any other difference advances revision.
    PreserveFormEdits,
}

/// One bounded current interaction contract, shared by hosts and providers.
/// Domain revisions, execution, queues and process lifetimes remain caller-owned.
#[derive(Default)]
pub struct Session {
    current: Option<Response>,
}

impl Session {
    pub fn replace(&mut self, content: Content, update: Update) -> Result<Response, ProtocolError> {
        content.validate()?;
        let previous = self.current.as_ref();
        let revision = match previous {
            Some(old)
                if matches!(update, Update::PreserveFormEdits)
                    && compatible_form(&old.content, &content) =>
            {
                old.revision
            }
            _ => previous
                .map_or(0, |old| old.revision)
                .checked_add(1)
                .ok_or(ProtocolError::UnexpectedMessage)?,
        };
        let response = Response {
            protocol_version: PROTOCOL_VERSION,
            revision,
            content,
        };
        // Do not install content that cannot be sent within the wire limit.
        response.encode()?;
        self.current = Some(response.clone());
        Ok(response)
    }

    pub fn observe(&mut self, response: Response) -> Result<(), ProtocolError> {
        response.encode()?;
        if let Some(old) = &self.current
            && (response.revision < old.revision
                || (response.revision == old.revision
                    && !compatible_form(&old.content, &response.content)))
        {
            return Err(ProtocolError::UnexpectedMessage);
        }
        self.current = Some(response);
        Ok(())
    }

    pub fn accepts(&self, message: &HostMessage) -> bool {
        if message.encode().is_err() {
            return false;
        }
        match message {
            HostMessage::Close { .. } => true,
            HostMessage::Event {
                revision,
                interaction,
                ..
            } => self.current.as_ref().is_some_and(|current| {
                current.revision == *revision && allows(&current.content, interaction)
            }),
        }
    }
}

fn compatible_form(old: &Content, new: &Content) -> bool {
    let (Content::Form(old), Content::Form(new)) = (old, new) else {
        return false;
    };
    old.elements.len() == new.elements.len()
        && old
            .elements
            .iter()
            .zip(&new.elements)
            .all(|(a, b)| match (a, b) {
                (
                    FormElement::TextInput {
                        id: a,
                        label: al,
                        secret: asec,
                        ..
                    },
                    FormElement::TextInput {
                        id: b,
                        label: bl,
                        secret: bsec,
                        ..
                    },
                ) => a == b && al == bl && asec == bsec,
                _ => a == b,
            })
}

fn allows(content: &Content, interaction: &Interaction) -> bool {
    match (content, interaction) {
        (Content::Form(form), Interaction::Form(event)) => {
            form.elements.iter().any(|element| match (element, event) {
                (FormElement::Button { id, .. }, FormEvent::ButtonActivated { element_id }) => {
                    id == element_id
                }
                (
                    FormElement::SelectableList { id, entries, .. },
                    FormEvent::SelectableListEntryActivated {
                        element_id,
                        entry_id,
                    },
                ) => {
                    id == element_id
                        && entries
                            .iter()
                            .any(|entry| entry.id == *entry_id && !entry.disabled)
                }
                (
                    FormElement::SelectableList {
                        id, allow_custom, ..
                    },
                    FormEvent::SelectableListCustomValueSubmitted { element_id, .. },
                ) => id == element_id && *allow_custom,
                (
                    FormElement::TextInput { id, .. },
                    FormEvent::TextInputChanged { element_id, .. }
                    | FormEvent::TextInputSubmitted { element_id, .. },
                ) => id == element_id,
                _ => false,
            })
        }
        (
            Content::Rich(content),
            Interaction::Rich(v4::InteractionEvent::Invoke {
                target,
                action_key,
                value,
            }),
        ) => {
            let matches =
                |action: &v4::PanelAction| action.key == *action_key && action.value == *value;
            match target {
                v4::InteractionTarget::Content => content.actions.iter().any(matches),
                v4::InteractionTarget::Section { id } => content
                    .sections
                    .iter()
                    .filter(|section| section.id == *id)
                    .flat_map(|section| &section.controls)
                    .any(|control| {
                        control.key == *action_key
                            && control.options.iter().any(|option| {
                                Some(&option.value) == value.as_ref()
                            })
                    }),
                v4::InteractionTarget::Item { id } => content
                    .sections
                    .iter()
                    .flat_map(|section| &section.items)
                    .filter(|item| item.id == *id)
                    .any(|item| {
                        item.actions.iter().any(matches)
                            || item.rich_body.iter().flat_map(|body| &body.segments).any(
                                |segment| matches!(segment, v4::RichBodySegment::Link { action, .. } if matches(action)),
                            )
                    }),
            }
        }
        _ => false,
    }
}
