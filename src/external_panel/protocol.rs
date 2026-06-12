pub const FRAMED_MAGIC: &str = "NIRIBAR_EXTERNAL_PANEL_JSON_V1\n";
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    FrameTooLarge,
    FrameTooShort,
    MissingMagic,
    Json(String),
    Io(String),
    IncompatibleVersion(u32),
    InvalidContent(String),
    UnexpectedMessage,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FrameTooLarge => write!(f, "external panel provider frame is too large"),
            Self::FrameTooShort => write!(f, "external panel provider frame is incomplete"),
            Self::MissingMagic => write!(f, "missing external panel provider frame magic"),
            Self::Json(err) => write!(f, "invalid external panel provider JSON: {err}"),
            Self::Io(err) => write!(f, "external panel provider I/O failed: {err}"),
            Self::IncompatibleVersion(version) => {
                write!(
                    f,
                    "incompatible external panel provider protocol version {version}"
                )
            }
            Self::InvalidContent(err) => write!(f, "invalid external panel content: {err}"),
            Self::UnexpectedMessage => write!(f, "unexpected external panel provider message"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload", rename_all = "kebab-case")]
pub enum HostMessage {
    Handshake(HostHandshake),
    Event(ProviderEvent),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostHandshake {
    pub protocol_version: u32,
    pub session_id: String,
    pub provider_name: String,
    pub origin: ProviderOrigin,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderOrigin {
    pub bar_name: String,
    pub output_name: String,
    pub origin_kind: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProviderMessage {
    Handshake {
        protocol_version: u32,
        content: ExternalPanelDocument,
    },
    Content {
        content: ExternalPanelDocument,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProviderEvent {
    ButtonActivated {
        element_id: String,
    },
    SelectableListEntryActivated {
        element_id: String,
        entry_id: String,
    },
    SelectableListCustomValueSubmitted {
        element_id: String,
        value: String,
    },
    TextInputChanged {
        element_id: String,
        value: String,
    },
    TextInputSubmitted {
        element_id: String,
        value: String,
    },
    Close,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExternalPanelDocument {
    pub elements: Vec<ExternalPanelElement>,
}

impl ExternalPanelDocument {
    pub fn validate(&self) -> Result<(), ExternalPanelValidationError> {
        let mut ids = std::collections::HashSet::new();
        for element in &self.elements {
            if let Some(id) = element.interactive_id() {
                if id.is_empty() {
                    return Err(ExternalPanelValidationError(
                        "interactive element id must not be empty".into(),
                    ));
                }
                if !ids.insert(id) {
                    return Err(ExternalPanelValidationError(format!(
                        "duplicate external panel element id {id:?}"
                    )));
                }
            }
            if let ExternalPanelElement::SelectableList { entries, .. } = element {
                validate_selectable_entries(entries)?;
            }
            validate_element_definition(element)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPanelValidationError(String);

impl std::fmt::Display for ExternalPanelValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExternalPanelValidationError {}

fn validate_selectable_entries(
    entries: &[ExternalSelectableEntry],
) -> Result<(), ExternalPanelValidationError> {
    let mut ids = std::collections::HashSet::new();
    for entry in entries {
        if entry.id.is_empty() {
            return Err(ExternalPanelValidationError(
                "external selectable entry id must not be empty".into(),
            ));
        }
        if entry.label.is_empty() {
            return Err(ExternalPanelValidationError(
                "external selectable entry label must not be empty".into(),
            ));
        }
        if !ids.insert(entry.id.as_str()) {
            return Err(ExternalPanelValidationError(format!(
                "duplicate external selectable entry id {:?}",
                entry.id
            )));
        }
    }
    Ok(())
}

fn validate_element_definition(
    element: &ExternalPanelElement,
) -> Result<(), ExternalPanelValidationError> {
    match element {
        ExternalPanelElement::Text { .. } | ExternalPanelElement::SelectableList { .. } => Ok(()),
        ExternalPanelElement::Button { label, .. } if label.is_empty() => Err(
            ExternalPanelValidationError("external button label must not be empty".into()),
        ),
        ExternalPanelElement::TextInput { label, .. } if label.is_empty() => Err(
            ExternalPanelValidationError("external text input label must not be empty".into()),
        ),
        ExternalPanelElement::Status { title, message, .. }
            if title.is_empty() || message.is_empty() =>
        {
            Err(ExternalPanelValidationError(
                "external status title and message must not be empty".into(),
            ))
        }
        ExternalPanelElement::Button { .. }
        | ExternalPanelElement::TextInput { .. }
        | ExternalPanelElement::Status { .. } => Ok(()),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ExternalPanelElement {
    Text {
        text: String,
    },
    Button {
        id: String,
        label: String,
    },
    SelectableList {
        id: String,
        entries: Vec<ExternalSelectableEntry>,
        #[serde(default)]
        allow_custom: bool,
        #[serde(default, skip)]
        search_query: String,
    },
    TextInput {
        id: String,
        label: String,
        value: String,
        #[serde(default)]
        secret: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Status {
        title: String,
        message: String,
        severity: ExternalPanelSeverity,
    },
}

impl ExternalPanelElement {
    fn interactive_id(&self) -> Option<&str> {
        match self {
            ExternalPanelElement::Button { id, .. }
            | ExternalPanelElement::SelectableList { id, .. }
            | ExternalPanelElement::TextInput { id, .. } => Some(id.as_str()),
            ExternalPanelElement::Text { .. } | ExternalPanelElement::Status { .. } => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExternalSelectableEntry {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExternalPanelSeverity {
    Info,
    Error,
}

pub fn encode_frame<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    let payload = serde_json::to_vec(value).map_err(|err| ProtocolError::Json(err.to_string()))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let mut frame = Vec::with_capacity(FRAMED_MAGIC.len() + 8 + payload.len());
    frame.extend_from_slice(FRAMED_MAGIC.as_bytes());
    frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_provider_frame(frame: &[u8]) -> Result<ProviderMessage, ProtocolError> {
    decode_frame(frame)
}

pub fn decode_frame<T: serde::de::DeserializeOwned>(frame: &[u8]) -> Result<T, ProtocolError> {
    if !frame.starts_with(FRAMED_MAGIC.as_bytes()) {
        return Err(ProtocolError::MissingMagic);
    }
    let len_start = FRAMED_MAGIC.len();
    let len_end = len_start + 8;
    if frame.len() < len_end {
        return Err(ProtocolError::FrameTooShort);
    }
    let mut len_bytes = [0_u8; 8];
    len_bytes.copy_from_slice(&frame[len_start..len_end]);
    let payload_len = u64::from_be_bytes(len_bytes) as usize;
    if payload_len > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let payload_end = len_end + payload_len;
    if frame.len() < payload_end {
        return Err(ProtocolError::FrameTooShort);
    }
    serde_json::from_slice(&frame[len_end..payload_end])
        .map_err(|err| ProtocolError::Json(err.to_string()))
}

pub async fn perform_handshake<R, W>(
    reader: R,
    writer: W,
    handshake: HostHandshake,
) -> Result<ExternalPanelDocument, ProtocolError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut reader = reader;
    let mut writer = writer;
    write_frame(&mut writer, &HostMessage::Handshake(handshake)).await?;
    match read_provider_frame(&mut reader).await? {
        ProviderMessage::Handshake {
            protocol_version,
            content,
        } if protocol_version == PROTOCOL_VERSION => {
            content
                .validate()
                .map_err(|err| ProtocolError::InvalidContent(err.to_string()))?;
            Ok(content)
        }
        ProviderMessage::Handshake {
            protocol_version, ..
        } => Err(ProtocolError::IncompatibleVersion(protocol_version)),
        _ => Err(ProtocolError::UnexpectedMessage),
    }
}

pub async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), ProtocolError>
where
    W: tokio::io::AsyncWrite + Unpin,
    T: serde::Serialize,
{
    let frame = encode_frame(value)?;
    tokio::io::AsyncWriteExt::write_all(writer, &frame)
        .await
        .map_err(|err| ProtocolError::Io(err.to_string()))?;
    tokio::io::AsyncWriteExt::flush(writer)
        .await
        .map_err(|err| ProtocolError::Io(err.to_string()))
}

pub async fn read_provider_frame<R>(reader: &mut R) -> Result<ProviderMessage, ProtocolError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    read_frame(reader).await
}

pub async fn read_frame<R, T>(reader: &mut R) -> Result<T, ProtocolError>
where
    R: tokio::io::AsyncRead + Unpin,
    T: serde::de::DeserializeOwned,
{
    let mut header = vec![0_u8; FRAMED_MAGIC.len() + 8];
    tokio::io::AsyncReadExt::read_exact(reader, &mut header)
        .await
        .map_err(|err| ProtocolError::Io(err.to_string()))?;
    if !header.starts_with(FRAMED_MAGIC.as_bytes()) {
        return Err(ProtocolError::MissingMagic);
    }
    let mut len_bytes = [0_u8; 8];
    len_bytes.copy_from_slice(&header[FRAMED_MAGIC.len()..]);
    let payload_len = u64::from_be_bytes(len_bytes) as usize;
    if payload_len > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let mut payload = vec![0_u8; payload_len];
    tokio::io::AsyncReadExt::read_exact(reader, &mut payload)
        .await
        .map_err(|err| ProtocolError::Io(err.to_string()))?;
    serde_json::from_slice(&payload).map_err(|err| ProtocolError::Json(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn framed_handshake_response_decodes_initial_content() {
        let message = ProviderMessage::Handshake {
            protocol_version: PROTOCOL_VERSION,
            content: ExternalPanelDocument {
                elements: vec![ExternalPanelElement::Text {
                    text: "Wi-Fi ready".into(),
                }],
            },
        };

        let frame = encode_frame(&message).unwrap();
        let decoded = decode_provider_frame(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn framed_button_event_encodes_opaque_element_id() {
        let message = HostMessage::Event(ProviderEvent::ButtonActivated {
            element_id: "scan-now".into(),
        });

        let frame = encode_frame(&message).unwrap();
        let decoded: HostMessage = decode_frame(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn framed_text_input_events_encode_value_and_element_id() {
        let changed = HostMessage::Event(ProviderEvent::TextInputChanged {
            element_id: "credential".into(),
            value: "secret".into(),
        });
        let submitted = HostMessage::Event(ProviderEvent::TextInputSubmitted {
            element_id: "credential".into(),
            value: "secret".into(),
        });

        assert_eq!(
            decode_frame::<HostMessage>(&encode_frame(&changed).unwrap()).unwrap(),
            changed
        );
        assert_eq!(
            decode_frame::<HostMessage>(&encode_frame(&submitted).unwrap()).unwrap(),
            submitted
        );
    }

    #[test]
    fn framed_selectable_list_event_encodes_list_and_entry_ids() {
        let message = HostMessage::Event(ProviderEvent::SelectableListEntryActivated {
            element_id: "networks".into(),
            entry_id: "network-1".into(),
        });

        let frame = encode_frame(&message).unwrap();
        let decoded: HostMessage = decode_frame(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn framed_selectable_list_custom_value_event_encodes_trimmed_value_and_list_id() {
        let message = HostMessage::Event(ProviderEvent::SelectableListCustomValueSubmitted {
            element_id: "networks".into(),
            value: "Hidden Wi-Fi".into(),
        });

        let frame = encode_frame(&message).unwrap();
        let decoded: HostMessage = decode_frame(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn provider_frame_rejects_invalid_json() {
        let payload = b"{";
        let mut frame = Vec::new();
        frame.extend_from_slice(FRAMED_MAGIC.as_bytes());
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        frame.extend_from_slice(payload);

        let err = decode_provider_frame(&frame).unwrap_err();

        assert!(matches!(err, ProtocolError::Json(_)));
    }

    #[test]
    fn provider_frame_rejects_oversized_payload_before_reading_body() {
        let mut frame = Vec::new();
        frame.extend_from_slice(FRAMED_MAGIC.as_bytes());
        frame.extend_from_slice(&((MAX_FRAME_BYTES as u64) + 1).to_be_bytes());

        let err = decode_provider_frame(&frame).unwrap_err();

        assert_eq!(err, ProtocolError::FrameTooLarge);
    }

    #[tokio::test]
    async fn handshake_rejects_duplicate_interactive_element_ids() {
        let (reader, mut writer) = tokio::io::duplex(4096);
        let (_host_reader, provider_writer) = tokio::io::duplex(4096);
        let response = encode_frame(&ProviderMessage::Handshake {
            protocol_version: PROTOCOL_VERSION,
            content: ExternalPanelDocument {
                elements: vec![
                    ExternalPanelElement::Button {
                        id: "dup".into(),
                        label: "One".into(),
                    },
                    ExternalPanelElement::TextInput {
                        id: "dup".into(),
                        label: "Two".into(),
                        value: String::new(),
                        secret: false,
                        error: None,
                    },
                ],
            },
        })
        .unwrap();
        writer.write_all(&response).await.unwrap();
        drop(writer);

        let err = perform_handshake(
            reader,
            provider_writer,
            HostHandshake {
                protocol_version: PROTOCOL_VERSION,
                session_id: "session-1".into(),
                provider_name: "wifi".into(),
                origin: ProviderOrigin {
                    bar_name: "main".into(),
                    output_name: "DP-1".into(),
                    origin_kind: "bar".into(),
                },
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("duplicate"));
    }

    #[tokio::test]
    async fn handshake_rejects_incompatible_protocol_version() {
        let (reader, mut writer) = tokio::io::duplex(4096);
        let (_host_reader, provider_writer) = tokio::io::duplex(4096);
        let response = encode_frame(&ProviderMessage::Handshake {
            protocol_version: PROTOCOL_VERSION + 1,
            content: ExternalPanelDocument {
                elements: vec![ExternalPanelElement::Text {
                    text: "Ready".into(),
                }],
            },
        })
        .unwrap();
        writer.write_all(&response).await.unwrap();
        drop(writer);

        let err = perform_handshake(
            reader,
            provider_writer,
            HostHandshake {
                protocol_version: PROTOCOL_VERSION,
                session_id: "session-1".into(),
                provider_name: "wifi".into(),
                origin: ProviderOrigin {
                    bar_name: "main".into(),
                    output_name: "DP-1".into(),
                    origin_kind: "bar".into(),
                },
            },
        )
        .await
        .unwrap_err();

        assert_eq!(
            err,
            ProtocolError::IncompatibleVersion(PROTOCOL_VERSION + 1)
        );
    }

    #[tokio::test]
    async fn handshake_sends_host_metadata_and_returns_initial_content() {
        let (mut host_reader, provider_writer) = tokio::io::duplex(4096);
        let (provider_reader, mut host_writer) = tokio::io::duplex(4096);

        let provider = tokio::spawn(async move {
            let mut header = vec![0_u8; FRAMED_MAGIC.len() + 8];
            host_reader.read_exact(&mut header).await.unwrap();
            let mut len_bytes = [0_u8; 8];
            len_bytes.copy_from_slice(&header[FRAMED_MAGIC.len()..]);
            let payload_len = u64::from_be_bytes(len_bytes) as usize;
            let mut payload = vec![0_u8; payload_len];
            host_reader.read_exact(&mut payload).await.unwrap();
            let request: HostMessage = serde_json::from_slice(&payload).unwrap();
            assert_eq!(
                request,
                HostMessage::Handshake(HostHandshake {
                    protocol_version: PROTOCOL_VERSION,
                    session_id: "session-1".into(),
                    provider_name: "wifi".into(),
                    origin: ProviderOrigin {
                        bar_name: "main".into(),
                        output_name: "DP-1".into(),
                        origin_kind: "bar".into(),
                    },
                })
            );

            let response = encode_frame(&ProviderMessage::Handshake {
                protocol_version: PROTOCOL_VERSION,
                content: ExternalPanelDocument {
                    elements: vec![ExternalPanelElement::Text {
                        text: "Ready".into(),
                    }],
                },
            })
            .unwrap();
            host_writer.write_all(&response).await.unwrap();
        });

        let content = perform_handshake(
            provider_reader,
            provider_writer,
            HostHandshake {
                protocol_version: PROTOCOL_VERSION,
                session_id: "session-1".into(),
                provider_name: "wifi".into(),
                origin: ProviderOrigin {
                    bar_name: "main".into(),
                    output_name: "DP-1".into(),
                    origin_kind: "bar".into(),
                },
            },
        )
        .await
        .unwrap();

        provider.await.unwrap();
        assert_eq!(
            content,
            ExternalPanelDocument {
                elements: vec![ExternalPanelElement::Text {
                    text: "Ready".into(),
                }],
            }
        );
    }
}
