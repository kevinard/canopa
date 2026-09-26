//! A single in-flight operation, streamed progress, and responsive host input.
use super::*;
use crate::external_panel::protocol::{
    HostMessage, ProtocolError, ProviderMessage, read_frame, write_frame,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc,
};

enum Operation {
    Scan,
    Synchronize,
    Event(ProviderEvent),
}

/// Called after the handshake. The reader owns frame assembly so a backend
/// update cannot cancel a partially consumed input frame.
pub async fn run<B, R, W>(
    mut provider: WifiPanelProvider<B>,
    mut reader: R,
    mut writer: W,
    mut changes: mpsc::Receiver<()>,
) -> Result<(), ProtocolError>
where
    B: WifiControlBackend,
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin,
{
    let (input_tx, mut input) = mpsc::channel(16);
    let reader_task = tokio::spawn(async move {
        while let Ok(message) = read_frame::<_, HostMessage>(&mut reader).await {
            if input_tx.send(message).await.is_err() {
                break;
            }
        }
    });
    // Dropping this session must also cancel its blocked stdin reader.
    struct ReaderGuard(tokio::task::AbortHandle);
    impl Drop for ReaderGuard {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _reader = ReaderGuard(reader_task.abort_handle());
    let (progress_tx, mut progress) = mpsc::unbounded_channel();
    provider.stream_progress(progress_tx);
    let mut next = Some(Operation::Scan);
    let mut dirty = false;
    let mut changes_open = true;
    loop {
        let operation = if let Some(operation) = next.take() {
            operation
        } else {
            tokio::select! {
                biased;
                message = input.recv() => match message {
                    None | Some(HostMessage::Event(ProviderEvent::Close)) => return Ok(()),
                    Some(HostMessage::Event(event)) => Operation::Event(event),
                    Some(HostMessage::Handshake(_)) => return Ok(()),
                },
                changed = changes.recv(), if changes_open => {
                    if changed.is_none() { changes_open = false; continue; }
                    Operation::Synchronize
                }
            }
        };
        let work = async {
            match operation {
                Operation::Scan => provider.refresh().await.map(|document| vec![document]),
                Operation::Synchronize => {
                    provider.synchronize().await.map(|document| vec![document])
                }
                Operation::Event(event) => provider.handle_event(event).await,
            }
        };
        tokio::pin!(work);
        let mut busy = false;
        loop {
            tokio::select! {
                biased;
                // Immediate edits complete before the next input is consumed.
                result = &mut work => {
                    while let Ok(content) = progress.try_recv() {
                        busy = true;
                        write_frame(&mut writer, &ProviderMessage::Content { content }).await?;
                    }
                    if busy {
                        while let Ok(message) = input.try_recv() {
                            if matches!(message, HostMessage::Event(ProviderEvent::Close) | HostMessage::Handshake(_)) { return Ok(()); }
                        }
                    }
                    let documents = result.unwrap_or_else(|err| vec![ExternalPanelDocument { elements: vec![ExternalPanelElement::Status {
                        title: "Wi-Fi problem".into(), message: user_message_for_error(err.category()).into(), severity: ExternalPanelSeverity::Error,
                    }] }]);
                    for content in documents { write_frame(&mut writer, &ProviderMessage::Content { content }).await?; }
                    if dirty { next = Some(Operation::Synchronize); dirty = false; }
                    break;
                },
                Some(content) = progress.recv() => {
                    // Only scan/connect publish progress before awaiting a backend.
                    busy = true;
                    write_frame(&mut writer, &ProviderMessage::Content { content }).await?;
                },
                message = input.recv() => match message {
                    None | Some(HostMessage::Event(ProviderEvent::Close)) | Some(HostMessage::Handshake(_)) => return Ok(()),
                    Some(HostMessage::Event(event)) if !busy => {
                        // A background snapshot may yield to input, without
                        // discarding edits or submission from the current view.
                        dirty = true; next = Some(Operation::Event(event)); break;
                    }
                    Some(HostMessage::Event(_)) => {}
                },
                changed = changes.recv(), if changes_open => {
                    if changed.is_some() { dirty = true; } else { changes_open = false; }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Delayed {
        calls: Arc<AtomicUsize>,
    }
    fn snapshot() -> WifiSnapshot {
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "home".into(),
                label: "Home".into(),
                signal_strength: 80,
                security: WifiSecurity::Open,
                known: false,
                active: false,
            }],
            scanning: false,
        }
    }
    impl WifiControlBackend for Delayed {
        fn name(&self) -> BackendName {
            BackendName::Iwd
        }
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities {
                can_scan_while_connecting: false,
            }
        }
        fn initial_snapshot(&mut self) -> WifiBackendFuture<'_, WifiSnapshot> {
            Box::pin(async { Ok(snapshot()) })
        }
        fn scan(&mut self) -> WifiBackendFuture<'_, WifiSnapshot> {
            self.initial_snapshot()
        }
        fn connect(&mut self, _request: WifiConnectRequest) -> WifiBackendFuture<'_, WifiSnapshot> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(std::future::pending())
        }
    }

    #[tokio::test]
    async fn buffered_credential_edits_and_submit_are_processed_in_order() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut provider = WifiPanelProvider::new(Delayed {
            calls: calls.clone(),
        });
        provider.credential_prompt = Some(CredentialPrompt {
            network: snapshot().networks.remove(0),
            value: String::new(),
            error: None,
            hidden: false,
        });
        let (mut host, transport) = tokio::io::duplex(65536);
        let (reader, writer) = tokio::io::split(transport);
        let (_updates, changes) = mpsc::channel(1);
        let task = tokio::spawn(run(provider, reader, writer, changes));
        for _ in 0..2 {
            read_frame::<_, ProviderMessage>(&mut host).await.unwrap();
        }
        // Send all frames together before reading any replacements.
        let mut frames = Vec::new();
        for event in [
            ProviderEvent::TextInputChanged {
                element_id: "credential".into(),
                value: "first".into(),
            },
            ProviderEvent::TextInputChanged {
                element_id: "credential".into(),
                value: "second-edit".into(),
            },
            ProviderEvent::ButtonActivated {
                element_id: "connect".into(),
            },
        ] {
            frames.extend(
                crate::external_panel::protocol::encode_frame(&HostMessage::Event(event)).unwrap(),
            );
        }
        tokio::io::AsyncWriteExt::write_all(&mut host, &frames)
            .await
            .unwrap();
        for expected in ["first", "second-edit"] {
            let ProviderMessage::Content { content } = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                read_frame::<_, ProviderMessage>(&mut host),
            )
            .await
            .unwrap()
            .unwrap() else {
                panic!("content")
            };
            assert!(content.elements.iter().any(|element| matches!(element, ExternalPanelElement::TextInput { value, .. } if value == expected)));
        }
        read_frame::<_, ProviderMessage>(&mut host).await.unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the buffered submission must start the backend"
        );
        write_frame(&mut host, &HostMessage::Event(ProviderEvent::Close))
            .await
            .unwrap();
        task.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn progress_is_visible_while_pending_and_close_cancels_duplicate_input() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = WifiPanelProvider::new(Delayed {
            calls: calls.clone(),
        });
        let (mut host, transport) = tokio::io::duplex(65536);
        let (reader, writer) = tokio::io::split(transport);
        let (changes_tx, changes_rx) = mpsc::channel(1);
        let task = tokio::spawn(run(provider, reader, writer, changes_rx));
        // Initial scan progress and completion.
        for _ in 0..2 {
            read_frame::<_, ProviderMessage>(&mut host).await.unwrap();
        }
        changes_tx.send(()).await.unwrap();
        read_frame::<_, ProviderMessage>(&mut host).await.unwrap();
        let activate = HostMessage::Event(ProviderEvent::SelectableListEntryActivated {
            element_id: "networks".into(),
            entry_id: "home".into(),
        });
        write_frame(&mut host, &activate).await.unwrap();
        let progress = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_frame::<_, ProviderMessage>(&mut host),
        )
        .await
        .unwrap()
        .unwrap();
        let ProviderMessage::Content { content } = progress else {
            panic!("expected progress")
        };
        assert!(content.elements.iter().any(|element| matches!(element, ExternalPanelElement::Status { message, .. } if message == "Connecting to Home")));
        write_frame(&mut host, &activate).await.unwrap();
        write_frame(&mut host, &HostMessage::Event(ProviderEvent::Close))
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
