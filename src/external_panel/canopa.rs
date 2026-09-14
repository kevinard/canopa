//! Canopa JSON-lines transport and generation-scoped host interactions.
//! The provider's domain keys remain opaque to Niribar.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use canopa_panel_protocol::{
    InteractionEvent, InteractionTarget, PanelAction, PanelContent, ProviderResponse,
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::protocol::{MAX_FRAME_BYTES, ProtocolError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanopaSnapshot {
    pub content: PanelContent,
    generation: u64,
    images: HashMap<String, iced::widget::image::Handle>,
    affordances: Vec<(String, String)>,
    focused: Option<usize>,
}

impl CanopaSnapshot {
    pub fn move_focus(&mut self, forward: bool) {
        if self.affordances.is_empty() {
            self.focused = None;
            return;
        }
        let count = self.affordances.len();
        self.focused = Some(match (self.focused, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
        });
    }
    pub fn focused_action_id(&self) -> Option<&str> {
        self.focused
            .and_then(|index| self.affordances.get(index))
            .map(|(id, _)| id.as_str())
    }
    pub fn focused_label(&self) -> Option<&str> {
        self.focused
            .and_then(|index| self.affordances.get(index))
            .map(|(_, label)| label.as_str())
    }
    pub fn image(&self, path: &str) -> Option<&iced::widget::image::Handle> {
        self.images.get(path)
    }
    pub fn action_id(&self, target: InteractionTarget, action: &PanelAction) -> String {
        serde_json::to_string(&(
            self.generation,
            InteractionEvent::Invoke {
                target,
                action_key: action.key.clone(),
                value: action.value.clone(),
            },
        ))
        .expect("protocol values are JSON serializable")
    }
}

#[derive(Default)]
struct SessionState {
    actions: HashSet<String>,
}

#[derive(Clone, Default)]
pub struct CanopaSession {
    state: Arc<Mutex<SessionState>>,
}

impl CanopaSession {
    pub async fn read_content<R: AsyncBufRead + Unpin>(
        &self,
        reader: &mut R,
    ) -> Result<CanopaSnapshot, ProtocolError> {
        let mut bytes = Vec::new();
        reader
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|error| ProtocolError::Io(error.to_string()))?;
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        if bytes.last() != Some(&b'\n') {
            return Err(ProtocolError::FrameTooShort);
        }
        let ProviderResponse::ReplaceContent {
            protocol_version,
            content,
        } = serde_json::from_slice(&bytes)
            .map_err(|_| ProtocolError::Json("invalid Canopa provider message".into()))?;
        if protocol_version != canopa_panel_protocol::PROTOCOL_VERSION {
            return Err(ProtocolError::IncompatibleVersion(protocol_version.into()));
        }
        content
            .validate()
            .map_err(|_| ProtocolError::InvalidContent("invalid Canopa content".into()))?;
        let images = super::canopa_images::prepare(&content).await;
        let mut state = self.state.lock().unwrap();
        // Unique across sessions as well as replacements: late clicks from a closed panel stay stale.
        static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
        let generation = NEXT_GENERATION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ProtocolError::UnexpectedMessage)?;
        let mut snapshot = CanopaSnapshot {
            content,
            generation,
            images,
            affordances: Vec::new(),
            focused: None,
        };
        let mut affordances = Vec::new();
        for action in &snapshot.content.actions {
            affordances.push((
                snapshot.action_id(InteractionTarget::Content, action),
                action.label.clone(),
            ));
        }
        for section in &snapshot.content.sections {
            for control in &section.controls {
                for option in &control.options {
                    affordances.push((
                        snapshot.action_id(
                            InteractionTarget::Section {
                                id: section.id.clone(),
                            },
                            &PanelAction {
                                key: control.key.clone(),
                                label: option.label.clone(),
                                value: Some(option.value.clone()),
                            },
                        ),
                        format!("{}: {} · {}", control.label, option.label, section.title),
                    ));
                }
            }
            for item in &section.items {
                for action in &item.actions {
                    affordances.push((
                        snapshot.action_id(
                            InteractionTarget::Item {
                                id: item.id.clone(),
                            },
                            action,
                        ),
                        format!("{} · {} · {}", action.label, section.title, item.title),
                    ));
                }
            }
        }
        state.actions = affordances.iter().map(|(id, _)| id.clone()).collect();
        snapshot.affordances = affordances;
        Ok(snapshot)
    }

    /// Ignore stale/unknown affordances instead of dispatching them against new content.
    pub async fn write_interaction<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        action_id: &str,
    ) -> Result<bool, ProtocolError> {
        if !self.state.lock().unwrap().actions.contains(action_id) {
            return Ok(false);
        }
        let (_, event): (u64, InteractionEvent) =
            serde_json::from_str(action_id).map_err(|_| ProtocolError::UnexpectedMessage)?;
        let mut bytes = serde_json::to_vec(&event).map_err(|_| ProtocolError::UnexpectedMessage)?;
        bytes.push(b'\n');
        writer
            .write_all(&bytes)
            .await
            .map_err(|error| ProtocolError::Io(error.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|error| ProtocolError::Io(error.to_string()))?;
        Ok(true)
    }
}
