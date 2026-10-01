//! Translate the unified wire contract into host-owned, revision-scoped controls.
use crate::surface::panel::ExternalPanelDocument;
use canopa_panel_protocol::{ProtocolError, v1, v5};
use canopa_shell::external_panel::canopa::CanopaSession;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};

#[derive(Default)]
struct State {
    contract: v5::Session,
    revision: u64,
    rich_generation: Option<u64>,
}

#[derive(Clone)]
pub(super) struct Session {
    id: String,
    state: Arc<Mutex<State>>,
    rich: CanopaSession,
}

impl Session {
    pub fn new(id: String) -> Self {
        Self {
            id,
            state: Default::default(),
            rich: Default::default(),
        }
    }

    pub async fn read_content(
        &self,
        reader: &mut (impl AsyncBufRead + Unpin),
    ) -> Result<ExternalPanelDocument, ProtocolError> {
        let response = v5::Response::read_async(reader)
            .await?
            .ok_or(ProtocolError::FrameTooShort)?;
        let (document, generation) = match response.content.clone() {
            v5::Content::Form(mut form) => {
                for element in &mut form.elements {
                    let id = match element {
                        v1::ExternalPanelElement::Button { id, .. }
                        | v1::ExternalPanelElement::TextInput { id, .. }
                        | v1::ExternalPanelElement::SelectableList { id, .. } => id,
                        _ => continue,
                    };
                    *id = serde_json::to_string(&(&self.id, response.revision, &*id))
                        .expect("opaque ID");
                }
                (form.into(), None)
            }
            v5::Content::Rich(content) => {
                let snapshot = self.rich.prepare(content).await?;
                let generation = snapshot.generation();
                (
                    ExternalPanelDocument::Canopa(Box::new(snapshot)),
                    Some(generation),
                )
            }
        };
        let mut state = self.state.lock().unwrap();
        let revision = response.revision;
        state.contract.observe(response)?;
        state.revision = revision;
        state.rich_generation = generation;
        Ok(document)
    }

    fn candidate(&self, mut event: v1::ProviderEvent) -> Option<v5::HostMessage> {
        if event == v1::ProviderEvent::Close {
            return Some(v5::HostMessage::close());
        }
        let state = self.state.lock().unwrap();
        let interaction = if let Some(generation) = state.rich_generation {
            let v1::ProviderEvent::ButtonActivated { element_id } = event else {
                return None;
            };
            let (captured, _): (u64, canopa_panel_protocol::InteractionEvent) =
                serde_json::from_str(&element_id).ok()?;
            if captured != generation {
                return None;
            }
            v5::Interaction::Rich(self.rich.interaction(&element_id)?)
        } else {
            let element_id = match &mut event {
                v1::ProviderEvent::ButtonActivated { element_id }
                | v1::ProviderEvent::SelectableListEntryActivated { element_id, .. }
                | v1::ProviderEvent::SelectableListCustomValueSubmitted { element_id, .. }
                | v1::ProviderEvent::TextInputChanged { element_id, .. }
                | v1::ProviderEvent::TextInputSubmitted { element_id, .. } => element_id,
                v1::ProviderEvent::Close => unreachable!(),
            };
            let (session, revision, original): (String, u64, String) =
                serde_json::from_str(element_id).ok()?;
            if session != self.id || revision != state.revision {
                return None;
            }
            *element_id = original;
            v5::Interaction::Form(event)
        };
        Some(v5::HostMessage::event(state.revision, interaction))
    }

    fn message(&self, event: v1::ProviderEvent) -> Option<v5::HostMessage> {
        let message = self.candidate(event)?;
        self.state
            .lock()
            .unwrap()
            .contract
            .accepts(&message)
            .then_some(message)
    }

    pub(super) fn input_fits(&self, event: &v1::ProviderEvent) -> bool {
        // Check the longer submit envelope before accepting a local edit.
        // The value is never truncated, especially for secret inputs.
        let event = match event {
            v1::ProviderEvent::TextInputChanged { element_id, value } => {
                v1::ProviderEvent::TextInputSubmitted {
                    element_id: element_id.clone(),
                    value: value.clone(),
                }
            }
            _ => event.clone(),
        };
        let Some(mut message) = self.candidate(event) else {
            return true;
        };
        if let v5::HostMessage::Event { revision, .. } = &mut message {
            *revision = u64::MAX;
        }
        message.encode().is_ok()
    }

    pub async fn write(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        event: v1::ProviderEvent,
    ) -> Result<(), ProtocolError> {
        if !self.input_fits(&event) {
            return Err(ProtocolError::FrameTooLarge);
        }
        let Some(message) = self.message(event) else {
            return Ok(());
        };
        writer
            .write_all(&message.encode()?)
            .await
            .map_err(|error| ProtocolError::Io(error.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|error| ProtocolError::Io(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form(value: &str) -> v5::Content {
        v5::Content::Form(v5::FormContent {
            elements: vec![
                v5::FormElement::TextInput {
                    id: "password".into(),
                    label: "Password".into(),
                    value: value.into(),
                    secret: true,
                    error: None,
                },
                v5::FormElement::Button {
                    id: "connect".into(),
                    label: "Connect".into(),
                },
            ],
        })
    }
    fn button(document: &ExternalPanelDocument) -> String {
        let v1::ExternalPanelElement::Button { id, .. } = &document.elements()[1] else {
            panic!("button")
        };
        id.clone()
    }
    #[tokio::test]
    async fn form_ids_preserve_edits_and_reject_replacements_and_other_sessions() {
        let mut provider = v5::Session::default();
        let host = Session::new("first".into());
        let initial = provider.replace(form(""), v5::Update::Replace).unwrap();
        let document = host
            .read_content(&mut initial.encode().unwrap().as_slice())
            .await
            .unwrap();
        let old = button(&document);
        let message = host
            .message(v1::ProviderEvent::ButtonActivated {
                element_id: old.clone(),
            })
            .unwrap();
        assert!(provider.accepts(&message));
        let echo = provider
            .replace(form("typed"), v5::Update::PreserveFormEdits)
            .unwrap();
        let document = host
            .read_content(&mut echo.encode().unwrap().as_slice())
            .await
            .unwrap();
        assert_eq!(button(&document), old);
        let second = Session::new("second".into());
        second
            .read_content(&mut initial.encode().unwrap().as_slice())
            .await
            .unwrap();
        assert!(
            second
                .message(v1::ProviderEvent::ButtonActivated {
                    element_id: old.clone()
                })
                .is_none()
        );
        let replacement = provider.replace(form(""), v5::Update::Replace).unwrap();
        host.read_content(&mut replacement.encode().unwrap().as_slice())
            .await
            .unwrap();
        assert!(
            host.message(v1::ProviderEvent::ButtonActivated { element_id: old })
                .is_none()
        );
        assert!(host.message(v1::ProviderEvent::Close).is_some());
    }
    #[tokio::test]
    async fn rich_ids_keep_domain_values_and_reject_replaced_actions() {
        let host = Session::new("center".into());
        let mut provider = v5::Session::default();
        let mut content = v5::RichContent::new("Center");
        content.actions.push(canopa_panel_protocol::PanelAction {
            key: "opaque".into(),
            label: "Open".into(),
            value: Some("daemon/revision".into()),
        });
        let response = provider
            .replace(v5::Content::Rich(content.clone()), v5::Update::Replace)
            .unwrap();
        let document = host
            .read_content(&mut response.encode().unwrap().as_slice())
            .await
            .unwrap();
        let snapshot = document.canopa().unwrap();
        let id = snapshot.action_id(
            canopa_panel_protocol::InteractionTarget::Content,
            &content.actions[0],
        );
        let message = host
            .message(v1::ProviderEvent::ButtonActivated {
                element_id: id.clone(),
            })
            .unwrap();
        assert!(provider.accepts(&message));
        let bytes = message.encode().unwrap();
        assert!(
            String::from_utf8(bytes)
                .unwrap()
                .contains("daemon/revision")
        );
        let replacement = provider
            .replace(v5::Content::Rich(content), v5::Update::Replace)
            .unwrap();
        host.read_content(&mut replacement.encode().unwrap().as_slice())
            .await
            .unwrap();
        assert!(
            host.message(v1::ProviderEvent::ButtonActivated { element_id: id })
                .is_none()
        );
    }
}
