//! Unified provider-first JSON-lines contract. Presentation remains host-owned.
//! Form and rich content reuse the frozen semantic primitives of v1 and v4.
use crate::{ProtocolError, v1, v4};
use serde::{Deserialize, Serialize};

mod interaction_limits;
mod session;
pub use session::{Session, Update};
pub use v1::{
    ExternalPanelDocument as FormContent, ExternalPanelElement as FormElement,
    ProviderEvent as FormEvent,
};
pub use v4::{InteractionEvent as RichEvent, PanelContent as RichContent};

pub const PROTOCOL_VERSION: u16 = 5;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "body", rename_all = "snake_case")]
pub enum Content {
    Form(FormContent),
    Rich(RichContent),
}

impl Content {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let valid = match self {
            Self::Form(form) => {
                form.elements.len() <= 4096
                    && form.validate().is_ok()
                    && form
                        .elements
                        .iter()
                        .map(|element| match element {
                            FormElement::SelectableList { entries, .. } => {
                                entries.len().saturating_add(1)
                            }
                            _ => 1,
                        })
                        .try_fold(0_usize, |count, next| count.checked_add(next))
                        .is_some_and(|count| count <= v4::MAX_INTERACTIVE_ELEMENTS)
            }
            Self::Rich(content) => content.validate().is_ok(),
        };
        if valid {
            interaction_limits::validate(self)
        } else {
            Err(ProtocolError::InvalidContent("invalid v5 content".into()))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "event", rename_all = "snake_case")]
pub enum Interaction {
    Form(FormEvent),
    Rich(RichEvent),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub protocol_version: u16,
    /// Interaction revision. Repeated only for compatible form input echoes.
    pub revision: u64,
    pub content: Content,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostMessage {
    Event {
        protocol_version: u16,
        revision: u64,
        interaction: Interaction,
    },
    Close {
        protocol_version: u16,
    },
}

impl HostMessage {
    pub fn event(revision: u64, interaction: Interaction) -> Self {
        Self::Event {
            protocol_version: PROTOCOL_VERSION,
            revision,
            interaction,
        }
    }
    pub fn close() -> Self {
        Self::Close {
            protocol_version: PROTOCOL_VERSION,
        }
    }
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let version = match self {
            Self::Event {
                protocol_version, ..
            }
            | Self::Close { protocol_version } => *protocol_version,
        };
        version_check(version)?;
        if matches!(
            self,
            Self::Event { revision: 0, .. }
                | Self::Event {
                    interaction: Interaction::Form(FormEvent::Close),
                    ..
                }
        ) {
            return Err(ProtocolError::UnexpectedMessage);
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        crate::lines::encode(self, MAX_EVENT_BYTES)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let value: Self = decode(bytes, MAX_EVENT_BYTES)?;
        value.validate()?;
        Ok(value)
    }
    pub fn read(reader: &mut impl std::io::BufRead) -> Result<Option<Self>, ProtocolError> {
        crate::lines::read(reader, MAX_EVENT_BYTES)?
            .map(|bytes| Self::decode(&bytes))
            .transpose()
    }
    #[cfg(feature = "tokio")]
    pub async fn read_async(
        reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
    ) -> Result<Option<Self>, ProtocolError> {
        crate::lines::read_async(reader, MAX_EVENT_BYTES)
            .await?
            .map(|bytes| Self::decode(&bytes))
            .transpose()
    }
}

impl Response {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        version_check(self.protocol_version)?;
        if self.revision == 0 {
            return Err(ProtocolError::UnexpectedMessage);
        }
        self.content.validate()
    }
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        crate::lines::encode(self, MAX_FRAME_BYTES)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let value: Self = decode(bytes, MAX_FRAME_BYTES)?;
        value.validate()?;
        Ok(value)
    }
    pub fn read(reader: &mut impl std::io::BufRead) -> Result<Option<Self>, ProtocolError> {
        crate::lines::read(reader, MAX_FRAME_BYTES)?
            .map(|bytes| Self::decode(&bytes))
            .transpose()
    }
    #[cfg(feature = "tokio")]
    pub async fn read_async(
        reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
    ) -> Result<Option<Self>, ProtocolError> {
        crate::lines::read_async(reader, MAX_FRAME_BYTES)
            .await?
            .map(|bytes| Self::decode(&bytes))
            .transpose()
    }
}

fn version_check(version: u16) -> Result<(), ProtocolError> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(ProtocolError::IncompatibleVersion(version.into()))
    }
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, ProtocolError> {
    if bytes.len() > limit {
        return Err(ProtocolError::FrameTooLarge);
    }
    if bytes.last() != Some(&b'\n') {
        return Err(ProtocolError::FrameTooShort);
    }
    if bytes[..bytes.len() - 1].contains(&b'\n') {
        return Err(ProtocolError::Json("invalid v5 message".into()));
    }
    serde_json::from_slice(bytes).map_err(|_| ProtocolError::Json("invalid v5 message".into()))
}
