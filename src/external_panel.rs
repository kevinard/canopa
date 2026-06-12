use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::process::{Child, ChildStdin, ChildStdout};

pub(crate) mod protocol {
    pub(crate) const FRAMED_MAGIC: &str = "NIRIBAR_EXTERNAL_PANEL_JSON_V1\n";
    pub(crate) const PROTOCOL_VERSION: u32 = 1;
    pub(crate) const MAX_FRAME_BYTES: usize = 1_048_576;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ProtocolError {
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
    pub(crate) enum HostMessage {
        Handshake(HostHandshake),
        Event(ProviderEvent),
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct HostHandshake {
        pub protocol_version: u32,
        pub session_id: String,
        pub provider_name: String,
        pub origin: ProviderOrigin,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct ProviderOrigin {
        pub bar_name: String,
        pub output_name: String,
        pub origin_kind: String,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    #[serde(tag = "type", rename_all = "kebab-case")]
    pub(crate) enum ProviderMessage {
        Handshake {
            protocol_version: u32,
            content: crate::surface::panel::ExternalPanelDocument,
        },
        Content {
            content: crate::surface::panel::ExternalPanelDocument,
        },
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    #[serde(tag = "type", rename_all = "kebab-case")]
    pub(crate) enum ProviderEvent {
        ButtonActivated {
            element_id: String,
        },
        SelectableListEntryActivated {
            element_id: String,
            entry_id: String,
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

    pub(crate) fn encode_frame<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
        let payload =
            serde_json::to_vec(value).map_err(|err| ProtocolError::Json(err.to_string()))?;
        if payload.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        let mut frame = Vec::with_capacity(FRAMED_MAGIC.len() + 8 + payload.len());
        frame.extend_from_slice(FRAMED_MAGIC.as_bytes());
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame)
    }

    pub(crate) fn decode_provider_frame(frame: &[u8]) -> Result<ProviderMessage, ProtocolError> {
        decode_frame(frame)
    }

    pub(crate) fn decode_frame<T: serde::de::DeserializeOwned>(
        frame: &[u8],
    ) -> Result<T, ProtocolError> {
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

    pub(crate) async fn perform_handshake<R, W>(
        reader: R,
        writer: W,
        handshake: HostHandshake,
    ) -> Result<crate::surface::panel::ExternalPanelDocument, ProtocolError>
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

    pub(crate) async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), ProtocolError>
    where
        W: tokio::io::AsyncWrite + Unpin,
        T: serde::Serialize,
    {
        let frame = encode_frame(value)?;
        tokio::io::AsyncWriteExt::write_all(writer, &frame)
            .await
            .map_err(|err| ProtocolError::Io(err.to_string()))
    }

    pub(crate) async fn read_provider_frame<R>(
        reader: &mut R,
    ) -> Result<ProviderMessage, ProtocolError>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        read_frame(reader).await
    }

    pub(crate) async fn read_frame<R, T>(reader: &mut R) -> Result<T, ProtocolError>
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
                content: crate::surface::panel::ExternalPanelDocument {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
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
                content: crate::surface::panel::ExternalPanelDocument {
                    elements: vec![
                        crate::surface::panel::ExternalPanelElement::Button {
                            id: "dup".into(),
                            label: "One".into(),
                        },
                        crate::surface::panel::ExternalPanelElement::TextInput {
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
                content: crate::surface::panel::ExternalPanelDocument {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
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
                    content: crate::surface::panel::ExternalPanelDocument {
                        elements: vec![crate::surface::panel::ExternalPanelElement::Text {
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
                crate::surface::panel::ExternalPanelDocument {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "Ready".into(),
                    }],
                }
            );
        }
    }
}

pub(crate) struct ExternalPanelStartedSession {
    pub(crate) content: crate::surface::panel::ExternalPanelDocument,
    pub(crate) handle: ExternalPanelSessionHandle,
}

#[derive(Clone)]
pub(crate) struct ExternalPanelSessionHandle {
    session_id: String,
    io: Arc<tokio::sync::Mutex<ExternalPanelSessionIo>>,
    child: Arc<tokio::sync::Mutex<Option<Child>>>,
    closed: Arc<AtomicBool>,
}

impl std::fmt::Debug for ExternalPanelSessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalPanelSessionHandle")
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

struct ExternalPanelSessionIo {
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl ExternalPanelSessionHandle {
    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub(crate) async fn wait_for_exit(&self) -> Result<Option<String>, protocol::ProtocolError> {
        loop {
            if self.closed.load(Ordering::Acquire) {
                let mut child = self.child.lock().await;
                if let Some(child) = child.as_mut() {
                    let _ = child.start_kill();
                }
                *child = None;
                return Ok(None);
            }

            {
                let mut child_slot = self.child.lock().await;
                let Some(child) = child_slot.as_mut() else {
                    return Ok(None);
                };
                match child.try_wait() {
                    Ok(Some(status)) => {
                        *child_slot = None;
                        return Ok(Some(format!("provider process exited with {status}")));
                    }
                    Ok(None) => {}
                    Err(err) => return Err(protocol::ProtocolError::Io(err.to_string())),
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    pub(crate) async fn send_event(
        &self,
        event: protocol::ProviderEvent,
    ) -> Result<crate::surface::panel::ExternalPanelDocument, protocol::ProtocolError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(protocol::ProtocolError::Io(
                "external panel session is closed".into(),
            ));
        }
        let mut io = self.io.lock().await;
        protocol::write_frame(&mut io.stdin, &protocol::HostMessage::Event(event)).await?;
        match protocol::read_provider_frame(&mut io.stdout).await? {
            protocol::ProviderMessage::Content { content } => {
                content
                    .validate()
                    .map_err(|err| protocol::ProtocolError::InvalidContent(err.to_string()))?;
                Ok(content)
            }
            _ => Err(protocol::ProtocolError::UnexpectedMessage),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExternalPanelProcess {
    child: Child,
}

impl ExternalPanelProcess {
    pub(crate) async fn perform_handshake(
        mut self,
        handshake: protocol::HostHandshake,
    ) -> Result<crate::surface::panel::ExternalPanelDocument, protocol::ProtocolError> {
        let stdout = self.child.stdout.take().ok_or_else(|| {
            protocol::ProtocolError::Io("provider stdout pipe unavailable".into())
        })?;
        let stdin =
            self.child.stdin.take().ok_or_else(|| {
                protocol::ProtocolError::Io("provider stdin pipe unavailable".into())
            })?;
        protocol::perform_handshake(stdout, stdin, handshake).await
    }

    pub(crate) async fn start_session(
        mut self,
        handshake: protocol::HostHandshake,
    ) -> Result<ExternalPanelStartedSession, protocol::ProtocolError> {
        let session_id = handshake.session_id.clone();
        let mut stdout = self.child.stdout.take().ok_or_else(|| {
            protocol::ProtocolError::Io("provider stdout pipe unavailable".into())
        })?;
        let mut stdin =
            self.child.stdin.take().ok_or_else(|| {
                protocol::ProtocolError::Io("provider stdin pipe unavailable".into())
            })?;
        let content = protocol::perform_handshake(&mut stdout, &mut stdin, handshake).await?;
        let handle = ExternalPanelSessionHandle {
            session_id,
            io: Arc::new(tokio::sync::Mutex::new(ExternalPanelSessionIo {
                stdin,
                stdout,
            })),
            child: Arc::new(tokio::sync::Mutex::new(Some(self.child))),
            closed: Arc::new(AtomicBool::new(false)),
        };
        Ok(ExternalPanelStartedSession { content, handle })
    }
}

pub(crate) fn next_session_id() -> String {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("external-panel-{id}")
}

pub(crate) fn spawn_provider_process(
    command: &crate::config::ConfiguredCommand,
) -> std::io::Result<ExternalPanelProcess> {
    let mut command = command_for_provider(command);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::null());
    command.kill_on_drop(true);
    let child = command.spawn()?;
    Ok(ExternalPanelProcess { child })
}

fn command_for_provider(command: &crate::config::ConfiguredCommand) -> tokio::process::Command {
    match command.spec() {
        crate::config::ConfiguredCommandSpec::Shell(command) => {
            let mut process = tokio::process::Command::new("sh");
            process.arg("-c").arg(command);
            process
        }
        crate::config::ConfiguredCommandSpec::Argv { program, args } => {
            let mut process = tokio::process::Command::new(program);
            process.args(args);
            process
        }
    }
}
