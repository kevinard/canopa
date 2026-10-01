//! Version selection is explicit; the Wi-Fi operation loop remains transport-neutral.
use canopa_panel_protocol::{ProtocolError, v1, v5};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufRead, AsyncWrite};

#[derive(Clone, Default)]
pub enum Transport {
    #[default]
    V1,
    V5(Arc<Mutex<v5::Session>>),
}
pub enum Input {
    V1(v1::HostMessage),
    V5(v5::HostMessage),
}
impl Input {
    pub(crate) fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::V1(
                v1::HostMessage::Handshake(_) | v1::HostMessage::Event(v1::ProviderEvent::Close)
            ) | Self::V5(v5::HostMessage::Close { .. })
        )
    }
}
pub enum Action {
    Event(v1::ProviderEvent),
    Close,
    Ignore,
}

impl Transport {
    pub fn v5() -> Self {
        Self::V5(Default::default())
    }

    pub async fn handshake(
        &self,
        reader: &mut (impl AsyncBufRead + Unpin),
    ) -> Result<(), ProtocolError> {
        if matches!(self, Self::V1) {
            let v1::HostMessage::Handshake(handshake) = v1::read_frame(reader).await? else {
                return Err(ProtocolError::UnexpectedMessage);
            };
            if handshake.protocol_version != v1::PROTOCOL_VERSION {
                return Err(ProtocolError::IncompatibleVersion(
                    handshake.protocol_version,
                ));
            }
        }
        Ok(())
    }

    pub async fn read(
        &self,
        reader: &mut (impl AsyncBufRead + Unpin),
    ) -> Result<Input, ProtocolError> {
        match self {
            Self::V1 => v1::read_frame(reader).await.map(Input::V1),
            Self::V5(_) => v5::HostMessage::read_async(reader)
                .await?
                .map(Input::V5)
                .ok_or(ProtocolError::FrameTooShort),
        }
    }

    /// Validate when dequeued, not when read: progress may retire a queued event.
    pub fn action(&self, input: Option<Input>) -> Action {
        match (self, input) {
            (_, None) | (Self::V1, Some(Input::V1(v1::HostMessage::Handshake(_)))) => Action::Close,
            (Self::V1, Some(Input::V1(v1::HostMessage::Event(v1::ProviderEvent::Close)))) => {
                Action::Close
            }
            (Self::V1, Some(Input::V1(v1::HostMessage::Event(event)))) => Action::Event(event),
            (Self::V5(state), Some(Input::V5(message)))
                if state.lock().unwrap().accepts(&message) =>
            {
                match message {
                    v5::HostMessage::Close { .. } => Action::Close,
                    v5::HostMessage::Event {
                        interaction: v5::Interaction::Form(event),
                        ..
                    } => Action::Event(event),
                    _ => Action::Ignore,
                }
            }
            _ => Action::Ignore,
        }
    }

    pub async fn write(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        content: v1::ExternalPanelDocument,
        initial: bool,
        update: v5::Update,
    ) -> Result<(), ProtocolError> {
        let bytes = match self {
            Self::V1 => v1::encode_frame(&if initial {
                v1::ProviderMessage::Handshake {
                    protocol_version: v1::PROTOCOL_VERSION,
                    content,
                }
            } else {
                v1::ProviderMessage::Content { content }
            })?,
            Self::V5(state) => state
                .lock()
                .unwrap()
                .replace(v5::Content::Form(content), update)?
                .encode()?,
        };
        crate::provider_io::write_frame(writer, &bytes).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn both_versions_bound_a_blocked_host_output() {
        for transport in [Transport::V1, Transport::v5()] {
            let (mut writer, _host_not_reading) = tokio::io::duplex(1);
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(2300),
                transport.write(
                    &mut writer,
                    super::super::no_control_document(),
                    true,
                    v5::Update::Replace,
                ),
            )
            .await;
            assert!(
                matches!(result, Ok(Err(ProtocolError::Io(_)))),
                "blocked host output did not terminate within its publication deadline"
            );
        }
    }
}
