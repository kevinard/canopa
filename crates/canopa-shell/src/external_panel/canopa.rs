//! Canopa JSON-lines transport and generation-scoped host interactions.
//! The provider's domain keys remain opaque to Canopa Shell.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use canopa_panel_protocol::{InteractionEvent, InteractionTarget, PanelAction, PanelContent};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};

use canopa_panel_protocol::ProtocolError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanopaSnapshot {
    pub content: PanelContent,
    generation: u64,
    images: HashMap<String, iced::widget::image::Handle>,
    icons: HashMap<canopa_panel_protocol::PanelIcon, iced::widget::image::Handle>,
    affordances: Vec<(String, String)>,
    focused: Option<usize>,
}

impl CanopaSnapshot {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn focus_action(&mut self, id: &str) -> bool {
        let Some(index) = self
            .affordances
            .iter()
            .position(|(candidate, _)| candidate == id)
        else {
            return false;
        };
        self.focused = Some(index);
        true
    }

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
    /// Keep keyboard focus on the same declared action after a live update,
    /// while retaining the new generation's protection against stale events.
    pub fn preserve_focus_from(&mut self, previous: &Self) {
        let Some(previous) = previous.focused_action_id() else {
            return;
        };
        let Ok((_, target)) = serde_json::from_str::<(u64, InteractionEvent)>(previous) else {
            return;
        };
        self.focused = self.affordances.iter().position(|(id, _)| {
            serde_json::from_str::<(u64, InteractionEvent)>(id)
                .is_ok_and(|(_, event)| event == target)
        });
    }
    pub fn image(&self, path: &str) -> Option<&iced::widget::image::Handle> {
        self.images.get(path)
    }
    pub fn icon(
        &self,
        icon: &canopa_panel_protocol::PanelIcon,
    ) -> Option<&iced::widget::image::Handle> {
        self.icons.get(icon)
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

    pub fn slider_id(&self, item: &str, slider: &canopa_panel_protocol::PanelSlider) -> String {
        self.action_id(
            InteractionTarget::Item { id: item.into() },
            &PanelAction {
                request_activation_token: false,
                destructive: false,
                pressed: None,
                secondary: false,
                primary: false,
                symbol: None,
                key: slider.key.clone(),
                label: slider.label.clone(),
                value: None,
            },
        )
    }

    pub fn slider(&self, id: &str) -> Option<&canopa_panel_protocol::PanelSlider> {
        let (
            generation,
            InteractionEvent::Invoke {
                target: InteractionTarget::Item { id },
                action_key,
                value,
            },
        ): (u64, InteractionEvent) = serde_json::from_str(id).ok()?
        else {
            return None;
        };
        if generation != self.generation || value.is_some() {
            return None;
        }
        self.content
            .sections
            .iter()
            .flat_map(|section| &section.items)
            .find(|item| item.id == id)?
            .slider
            .as_ref()
            .filter(|slider| slider.key == action_key)
    }
}

/// Add a numeric value to a captured control identity without rebasing its generation.
pub fn slider_value_action(id: &str, value: u16) -> Option<String> {
    let (generation, mut event): (u64, InteractionEvent) = serde_json::from_str(id).ok()?;
    let InteractionEvent::Invoke { value: current, .. } = &mut event;
    if current.is_some() {
        return None;
    }
    *current = Some(value.to_string());
    serde_json::to_string(&(generation, event)).ok()
}

impl CanopaSnapshot {
    /// The generation Shell-made content uses; provider sessions start at 1,
    /// so its action ids never reach a provider.
    pub const LOCAL_GENERATION: u64 = 0;

    /// Content the Shell itself shows in a provider panel (a failure), with the
    /// same focus, accessibility and action ids as a provider's.
    pub fn local(content: PanelContent) -> Self {
        Self::assemble(
            content,
            Self::LOCAL_GENERATION,
            HashMap::new(),
            HashMap::new(),
        )
    }

    fn assemble(
        content: PanelContent,
        generation: u64,
        images: HashMap<String, iced::widget::image::Handle>,
        icons: HashMap<canopa_panel_protocol::PanelIcon, iced::widget::image::Handle>,
    ) -> Self {
        let mut snapshot = CanopaSnapshot {
            content,
            generation,
            images,
            icons,
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
                                request_activation_token: false,
                                destructive: false,
                                pressed: None,
                                secondary: false,
                                primary: false,
                                symbol: None,
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
                if let Some(slider) = &item.slider {
                    affordances.push((
                        snapshot.slider_id(&item.id, slider),
                        format!("{} · {}", slider.label, item.title),
                    ));
                }
                let links = item
                    .rich_body
                    .iter()
                    .flat_map(|body| &body.segments)
                    .filter_map(|segment| match segment {
                        canopa_panel_protocol::RichBodySegment::Link { action, .. } => Some(action),
                        _ => None,
                    });
                for action in links.chain(&item.actions) {
                    affordances.push((
                        snapshot.action_id(
                            InteractionTarget::Item {
                                id: item.id.clone(),
                            },
                            action,
                        ),
                        format!(
                            "{} · {} · {}",
                            action.label,
                            item.attribution
                                .as_ref()
                                .map_or(section.title.as_str(), |source| source.label.as_str()),
                            item.title
                        ),
                    ));
                }
            }
        }
        snapshot.affordances = affordances;
        snapshot
    }

    /// The action key a [`Self::local`] action id invokes, if it is one.
    pub fn local_action_key(action_id: &str) -> Option<String> {
        match serde_json::from_str::<(u64, InteractionEvent)>(action_id).ok()? {
            (Self::LOCAL_GENERATION, InteractionEvent::Invoke { action_key, .. }) => {
                Some(action_key)
            }
            _ => None,
        }
    }
}

#[derive(Default)]
struct SessionState {
    actions: HashSet<String>,
    generation: u64,
    sliders: HashMap<(String, String), canopa_panel_protocol::PanelSlider>,
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
        let content = canopa_panel_protocol::v4::Frame::read_async(reader)
            .await?
            .ok_or(ProtocolError::FrameTooShort)?
            .into_content();
        self.prepare(content).await
    }

    pub async fn prepare(&self, content: PanelContent) -> Result<CanopaSnapshot, ProtocolError> {
        content
            .validate()
            .map_err(|_| ProtocolError::InvalidContent("invalid Canopa content".into()))?;
        let assets = super::canopa_images::prepare(&content).await;
        let mut state = self.state.lock().unwrap();
        // Unique across sessions as well as replacements: late clicks from a closed panel stay stale.
        static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
        let generation = NEXT_GENERATION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ProtocolError::UnexpectedMessage)?;
        let snapshot = CanopaSnapshot::assemble(content, generation, assets.images, assets.icons);
        state.actions = snapshot
            .affordances
            .iter()
            .map(|(id, _)| id.clone())
            .collect();
        state.generation = generation;
        state.sliders = snapshot
            .content
            .sections
            .iter()
            .flat_map(|section| &section.items)
            .filter_map(|item| {
                item.slider
                    .as_ref()
                    .map(|slider| ((item.id.clone(), slider.key.clone()), slider.clone()))
            })
            .collect();
        Ok(snapshot)
    }

    /// Ignore stale/unknown affordances instead of dispatching them against new content.
    pub async fn write_interaction<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        action_id: &str,
    ) -> Result<bool, ProtocolError> {
        let Some(event) = self.interaction(action_id) else {
            return Ok(false);
        };
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
    pub fn interaction(&self, action_id: &str) -> Option<InteractionEvent> {
        let (generation, event): (u64, InteractionEvent) = serde_json::from_str(action_id).ok()?;
        let state = self.state.lock().unwrap();
        if generation != state.generation {
            return None;
        }
        if let InteractionEvent::Invoke {
            target: InteractionTarget::Item { id },
            action_key,
            value,
        } = &event
            && let Some(slider) = state.sliders.get(&(id.clone(), action_key.clone()))
        {
            slider.parse_value(value.as_deref())?;
            return Some(event);
        }
        state.actions.contains(action_id).then_some(event)
    }
}
