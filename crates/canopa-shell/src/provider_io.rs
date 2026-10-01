//! Bounded transport work shared by headless providers. Domain sessions own revisions
//! and operation ordering; this boundary owns partial frames, reader lifetime and output.
use canopa_panel_protocol::{ProtocolError, v5};
use std::{future::Future, pin::Pin, time::Duration};
use tokio::{
    io::{AsyncBufRead, AsyncWrite, AsyncWriteExt},
    sync::mpsc,
    task::JoinHandle,
};

const INPUT_CAPACITY: usize = 16;
const OUTPUT_TIMEOUT: Duration = Duration::from_secs(2);
type ReadFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<Option<T>, ProtocolError>> + Send + 'a>>;

pub(crate) struct Input<T> {
    frames: mpsc::Receiver<Result<T, ProtocolError>>,
    reader: JoinHandle<()>,
}

impl<T: Send + 'static> Input<T> {
    pub(crate) fn new<R, F>(mut reader: R, mut read: F, is_terminal: fn(&T) -> bool) -> Self
    where
        R: Send + 'static,
        F: for<'a> FnMut(&'a mut R) -> ReadFuture<'a, T> + Send + 'static,
    {
        let (tx, frames) = mpsc::channel(INPUT_CAPACITY);
        let reader = tokio::spawn(async move {
            loop {
                let result = match read(&mut reader).await {
                    Ok(Some(frame)) => Ok(frame),
                    Ok(None) => break,
                    Err(error) => Err(error),
                };
                // Do not read ahead beyond Close. Tokio stdin uses a blocking
                // read that cannot be aborted and would hold runtime shutdown
                // until the host also closes its pipe.
                let terminal = result.as_ref().map_or(true, is_terminal);
                if tx.send(result).await.is_err() || terminal {
                    break;
                }
            }
        });
        Self { frames, reader }
    }

    /// Receiving is cancellation safe: frame assembly belongs to the owned reader.
    pub(crate) async fn next(&mut self) -> Result<Option<T>, ProtocolError> {
        self.frames.recv().await.transpose()
    }

    pub(crate) fn try_next(&mut self) -> Result<Option<T>, ProtocolError> {
        self.frames.try_recv().ok().transpose()
    }
}

impl Input<v5::HostMessage> {
    pub(crate) fn v5(reader: impl AsyncBufRead + Unpin + Send + 'static) -> Self {
        Self::new(
            reader,
            |reader| Box::pin(v5::HostMessage::read_async(reader)),
            |message| matches!(message, v5::HostMessage::Close { .. }),
        )
    }
}

impl<T> Drop for Input<T> {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

pub(crate) async fn write_v5(
    output: &mut (impl AsyncWrite + Unpin),
    response: v5::Response,
) -> Result<(), ProtocolError> {
    write_frame(output, &response.encode()?).await
}

pub(crate) async fn write_frame(
    output: &mut (impl AsyncWrite + Unpin),
    bytes: &[u8],
) -> Result<(), ProtocolError> {
    tokio::time::timeout(OUTPUT_TIMEOUT, async {
        output.write_all(bytes).await?;
        output.flush().await
    })
    .await
    .map_err(|_| ProtocolError::Io("Provider output timed out".into()))?
    .map_err(|error| ProtocolError::Io(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_panel_protocol::v1;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tokio::io::{AsyncRead, ReadBuf};

    #[tokio::test]
    async fn cancelled_receives_keep_fragmented_input_and_close_in_both_versions() {
        for legacy in [false, true] {
            let (mut host, reader) = tokio::io::duplex(256);
            let reader = tokio::io::BufReader::new(reader);
            // Translate only for this wire fixture; providers retain their typed inputs.
            let mut input = Input::new(
                reader,
                move |reader| {
                    Box::pin(async move {
                        if legacy {
                            let message: v1::HostMessage = v1::read_frame(reader).await?;
                            Ok(Some(matches!(
                                message,
                                v1::HostMessage::Event(v1::ProviderEvent::Close)
                            )))
                        } else {
                            Ok(v5::HostMessage::read_async(reader)
                                .await?
                                .map(|message| matches!(message, v5::HostMessage::Close { .. })))
                        }
                    })
                },
                |closed| *closed,
            );
            let frame = if legacy {
                v1::encode_frame(&v1::HostMessage::Event(v1::ProviderEvent::Close)).unwrap()
            } else {
                b"{\"type\":\"close\",\"protocol_version\":5}\n".to_vec()
            };
            host.write_all(&frame[..5]).await.unwrap();
            for _ in 0..3 {
                // An unrelated update wins while this frame is incomplete.
                assert!(
                    tokio::time::timeout(Duration::from_millis(10), input.next())
                        .await
                        .is_err()
                );
            }
            host.write_all(&frame[5..]).await.unwrap();
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), input.next())
                    .await
                    .unwrap()
                    .unwrap(),
                Some(true)
            );
        }
    }

    struct BlockedReader(Arc<AtomicBool>);
    impl AsyncRead for BlockedReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }
    }
    impl Drop for BlockedReader {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn cancelling_session_releases_its_blocked_reader() {
        let dropped = Arc::new(AtomicBool::new(false));
        let input = Input::v5(tokio::io::BufReader::new(BlockedReader(dropped.clone())));
        tokio::task::yield_now().await;
        drop(input);
        tokio::time::timeout(Duration::from_secs(1), async {
            while !dropped.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn output_publishes_complete_revisions_and_retires_old_interactions() {
        let (writer, reader) = tokio::io::duplex(8);
        let publisher = tokio::spawn(async move {
            let mut writer = writer;
            let mut session = v5::Session::default();
            let form = v1::ExternalPanelDocument {
                elements: vec![v1::ExternalPanelElement::Button {
                    id: "refresh".into(),
                    label: "Refresh".into(),
                }],
            };
            for _ in 0..2 {
                let response = session
                    .replace(v5::Content::Form(form.clone()), v5::Update::Replace)
                    .unwrap();
                write_v5(&mut writer, response).await.unwrap();
            }
            session
        });
        let mut reader = tokio::io::BufReader::new(reader);
        use tokio::io::AsyncBufReadExt;
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let first: v5::Response = serde_json::from_str(&line).unwrap();
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        let second: v5::Response = serde_json::from_str(&line).unwrap();
        assert!(second.revision > first.revision);
        let session = publisher.await.unwrap();
        let event = v5::HostMessage::Event {
            protocol_version: 5,
            revision: first.revision,
            interaction: v5::Interaction::Form(v1::ProviderEvent::ButtonActivated {
                element_id: "refresh".into(),
            }),
        };
        assert!(!session.accepts(&event));
        let v5::HostMessage::Event { interaction, .. } = event else {
            unreachable!()
        };
        assert!(session.accepts(&v5::HostMessage::event(second.revision, interaction)));
    }
}
