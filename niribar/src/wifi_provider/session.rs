//! A single in-flight operation, streamed progress, and responsive host input.
use super::transport::{Action, Transport};
use super::*;
use canopa_panel_protocol::ProtocolError;
#[cfg(test)]
use canopa_panel_protocol::v1::{HostMessage, ProviderMessage, read_frame, write_frame};
use canopa_panel_protocol::v5::Update;
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
    provider: WifiPanelProvider<B>,
    reader: R,
    writer: W,
    changes: mpsc::Receiver<()>,
) -> Result<(), ProtocolError>
where
    B: WifiControlBackend,
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin,
{
    run_with_transport(provider, reader, writer, changes, Transport::V1).await
}

pub async fn run_with_transport<B, R, W>(
    mut provider: WifiPanelProvider<B>,
    reader: R,
    mut writer: W,
    mut changes: mpsc::Receiver<()>,
    transport: Transport,
) -> Result<(), ProtocolError>
where
    B: WifiControlBackend,
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin,
{
    let mut reader = tokio::io::BufReader::new(reader);
    let input_transport = transport.clone();
    let (input_tx, mut input) = mpsc::channel(16);
    let reader_task = tokio::spawn(async move {
        while let Ok(message) = input_transport.read(&mut reader).await {
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
                message = input.recv() => match transport.action(message) {
                    Action::Close => return Ok(()),
                    Action::Event(event) => Operation::Event(event),
                    Action::Ignore => continue,
                },
                changed = changes.recv(), if changes_open => {
                    if changed.is_none() { changes_open = false; continue; }
                    Operation::Synchronize
                }
            }
        };
        let update = if matches!(
            &operation,
            Operation::Synchronize | Operation::Event(ProviderEvent::TextInputChanged { .. })
        ) {
            Update::PreserveFormEdits
        } else {
            Update::Replace
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
                        transport.write(&mut writer, content, false, Update::Replace).await?;
                    }
                    if busy {
                        while let Ok(message) = input.try_recv() {
                            if matches!(transport.action(Some(message)), Action::Close) { return Ok(()); }
                        }
                    }
                    let documents = result.unwrap_or_else(|err| vec![ExternalPanelDocument { elements: vec![ExternalPanelElement::Status {
                        title: "Wi-Fi problem".into(), message: user_message_for_error(err.category()).into(), severity: ExternalPanelSeverity::Error,
                    }] }]);
                    for content in documents { transport.write(&mut writer, content, false, update).await?; }
                    if dirty { next = Some(Operation::Synchronize); dirty = false; }
                    break;
                },
                Some(content) = progress.recv() => {
                    // Only scan/connect publish progress before awaiting a backend.
                    busy = true;
                    transport.write(&mut writer, content, false, Update::Replace).await?;
                },
                message = input.recv() => match transport.action(message) {
                    Action::Close => return Ok(()),
                    Action::Event(event) if !busy => {
                        // A background snapshot may yield to input, without
                        // discarding edits or submission from the current view.
                        dirty = true; next = Some(Operation::Event(event)); break;
                    }
                    Action::Event(_) | Action::Ignore => {}
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
                canopa_panel_protocol::v1::encode_frame(&HostMessage::Event(event)).unwrap(),
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
    #[tokio::test]
    async fn v5_buffers_credential_edits_and_submits_the_last_value_once() {
        use canopa_panel_protocol::v5;
        use tokio::io::AsyncWriteExt;
        struct Credentials {
            received: Arc<std::sync::Mutex<Vec<String>>>,
        }
        impl WifiControlBackend for Credentials {
            fn name(&self) -> BackendName {
                BackendName::Iwd
            }
            fn capabilities(&self) -> BackendCapabilities {
                BackendCapabilities {
                    can_scan_while_connecting: false,
                }
            }
            fn initial_snapshot(&mut self) -> WifiBackendFuture<'_, WifiSnapshot> {
                Box::pin(async {
                    let mut value = snapshot();
                    value.networks[0].security = WifiSecurity::Personal;
                    Ok(value)
                })
            }
            fn scan(&mut self) -> WifiBackendFuture<'_, WifiSnapshot> {
                self.initial_snapshot()
            }
            fn connect(
                &mut self,
                request: WifiConnectRequest,
            ) -> WifiBackendFuture<'_, WifiSnapshot> {
                self.received
                    .lock()
                    .unwrap()
                    .push(request.credential.unwrap());
                Box::pin(std::future::pending())
            }
        }
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let provider = WifiPanelProvider::new(Credentials {
            received: received.clone(),
        });
        let (host, provider_io) = tokio::io::duplex(65536);
        let (reader, writer) = tokio::io::split(provider_io);
        let (mut host_read, mut host_write) = tokio::io::split(host);
        let mut host_read = tokio::io::BufReader::new(&mut host_read);
        let (_updates, changes) = mpsc::channel(1);
        let task = tokio::spawn(run_with_transport(
            provider,
            reader,
            writer,
            changes,
            Transport::v5(),
        ));
        async fn next(reader: &mut (impl tokio::io::AsyncBufRead + Unpin)) -> v5::Response {
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                v5::Response::read_async(reader),
            )
            .await
            .unwrap()
            .unwrap()
            .unwrap()
        }
        next(&mut host_read).await;
        let networks = next(&mut host_read).await;
        host_write
            .write_all(
                &v5::HostMessage::event(
                    networks.revision,
                    v5::Interaction::Form(ProviderEvent::SelectableListEntryActivated {
                        element_id: "networks".into(),
                        entry_id: "home".into(),
                    }),
                )
                .encode()
                .unwrap(),
            )
            .await
            .unwrap();
        let prompt = next(&mut host_read).await;
        let mut bytes = Vec::new();
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
            bytes.extend(
                v5::HostMessage::event(prompt.revision, v5::Interaction::Form(event))
                    .encode()
                    .unwrap(),
            );
        }
        host_write.write_all(&bytes).await.unwrap();
        for expected in ["first", "second-edit"] {
            let response = next(&mut host_read).await;
            assert_eq!(response.revision, prompt.revision);
            let v5::Content::Form(form) = response.content else {
                panic!("form")
            };
            assert!(form.elements.iter().any(|element| matches!(element, v5::FormElement::TextInput { value, .. } if value == expected)));
        }
        assert!(next(&mut host_read).await.revision > prompt.revision);
        assert_eq!(*received.lock().unwrap(), vec!["second-edit"]);
        host_write
            .write_all(&v5::HostMessage::close().encode().unwrap())
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
