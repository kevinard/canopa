use super::{
    CtlRequest, SocketControlRequest, control_socket_response, invalid_control_reply,
    panel_content_from_projection,
};
use anyhow::Context;
use canopa_notify::NotifyControl;
use canopa_panel_protocol::{
    InteractionEvent, MAX_FRAME_BYTES as MAX_PANEL_FRAME_BYTES, PanelContent, ProviderResponse,
};
use std::io::{BufRead, Read, Write};
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

const MAX_CONTROL_FRAME_BYTES: usize = 64 * 1024;
const MAX_CONTROL_REPLY_BYTES: usize = 8 * 1024 * 1024;
pub(super) const CONTROL_REPLY_TOO_LARGE: &str = "error control reply exceeds the 8 MiB limit\n";
const MAX_PANEL_INTERACTION_BYTES: usize = MAX_CONTROL_FRAME_BYTES - "panel-interaction ".len();
const MAX_PANEL_REPLY_BYTES: usize = 4096;
const CONTROL_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const CONTROL_WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const CONTROL_EXCHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const PANEL_INTERACTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub(crate) async fn run_ctl(socket: &Path, request: CtlRequest) -> anyhow::Result<()> {
    let line = request.line();
    anyhow::ensure!(
        line.len() <= MAX_CONTROL_FRAME_BYTES,
        "Canopa Notify control request exceeds 64 KiB; command was not sent"
    );
    let bytes = tokio::time::timeout(CONTROL_EXCHANGE_TIMEOUT, async {
        let mut stream = tokio::net::UnixStream::connect(socket).await.map_err(|source| {
            anyhow::anyhow!(
                "Canopa Notify daemon unavailable at {}: {source}",
                socket.display()
            )
        })?;
        tokio::time::timeout(CONTROL_WRITE_TIMEOUT, stream.write_all(line.as_bytes()))
            .await
            .context("Canopa Notify control write exceeded its five-second deadline; command outcome may be unknown")?
            .context("Canopa Notify control write failed; command outcome may be unknown")?;
        let mut bytes = Vec::with_capacity(MAX_CONTROL_REPLY_BYTES + 1);
        BufReader::new(stream)
            .take((MAX_CONTROL_REPLY_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .await
            .context("Canopa Notify control read failed; command outcome may be unknown")?;
        Ok::<_, anyhow::Error>(bytes)
    })
    .await
    .context("Canopa Notify control exchange exceeded its 30-second deadline; command outcome may be unknown")??;
    anyhow::ensure!(
        bytes.len() <= MAX_CONTROL_REPLY_BYTES,
        "Canopa Notify control reply exceeds 8 MiB; command outcome may be unknown"
    );
    anyhow::ensure!(bytes.last() == Some(&b'\n'), invalid_control_reply());
    let response = String::from_utf8(bytes).map_err(|_| invalid_control_reply())?;
    print!("{}", request.reply_text(&response)?);
    Ok(())
}

pub(crate) fn run_panel_provider(socket: &Path) -> anyhow::Result<()> {
    let mut stream = match StdUnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(source) => {
            let message = format!(
                "Canopa Notify daemon unavailable at {}: {source}",
                socket.display()
            );
            println!(
                "{}",
                serde_json::to_string(&ProviderResponse::replace_content(PanelContent::error(
                    "Canopa Notify",
                    message
                )))?
            );
            return Ok(());
        }
    };
    stream.set_write_timeout(Some(CONTROL_WRITE_TIMEOUT))?;
    stream.write_all(SocketControlRequest::PanelSubscribe.line().as_bytes())?;
    spawn_panel_interaction_forwarder(socket.to_path_buf())?;
    let mut reader = std::io::BufReader::new(stream);
    let mut stdout = std::io::stdout().lock();
    loop {
        match read_panel_frame(&mut reader) {
            Ok(Some(frame)) => stdout.write_all(&frame)?,
            Ok(None) => break,
            Err(_) => {
                stdout.write_all(panel_response_line(PanelContent::error(
                    "Canopa Notify",
                    "Notification Center stream ended with an invalid or incomplete frame; reopen the panel.",
                ))?.as_bytes())?;
                stdout.flush()?;
                break;
            }
        }
        stdout.flush()?;
    }
    Ok(())
}

fn read_panel_frame(reader: &mut impl BufRead) -> anyhow::Result<Option<Vec<u8>>> {
    let mut frame = Vec::with_capacity(MAX_PANEL_FRAME_BYTES + 1);
    reader
        .take((MAX_PANEL_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut frame)?;
    if frame.is_empty() {
        return Ok(None);
    }
    anyhow::ensure!(
        frame.len() <= MAX_PANEL_FRAME_BYTES && frame.last() == Some(&b'\n'),
        "invalid panel frame length"
    );
    let ProviderResponse::ReplaceContent {
        protocol_version,
        content,
    } = serde_json::from_slice(&frame)?;
    anyhow::ensure!(
        protocol_version == canopa_panel_protocol::PROTOCOL_VERSION,
        "incompatible panel protocol"
    );
    content.validate()?;
    Ok(Some(frame))
}

fn spawn_panel_interaction_forwarder(socket: PathBuf) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    thread::Builder::new()
        .name("canopa-notify-panel-interactions".to_string())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut reader = stdin.lock();
            loop {
                let line = match read_panel_interaction(&mut reader) {
                    Ok(Some(line)) => line,
                    Ok(None) => break,
                    Err(_) => {
                        eprintln!("level=warn component=panel_provider event=input_failed");
                        break;
                    }
                };
                let result = runtime.block_on(async {
                    tokio::time::timeout(
                        PANEL_INTERACTION_TIMEOUT,
                        send_panel_interaction(&socket, &line),
                    )
                    .await
                });
                if !matches!(result, Ok(Ok(()))) {
                    eprintln!("level=warn component=panel_provider event=interaction_failed");
                }
            }
        })?;
    Ok(())
}

fn read_panel_interaction(reader: &mut impl BufRead) -> std::io::Result<Option<String>> {
    loop {
        let mut bytes = Vec::with_capacity(MAX_PANEL_INTERACTION_BYTES + 1);
        (&mut *reader)
            .take((MAX_PANEL_INTERACTION_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)?;
        if bytes.is_empty() {
            return Ok(None);
        }
        if bytes.len() > MAX_PANEL_INTERACTION_BYTES {
            if bytes.last() != Some(&b'\n') {
                loop {
                    let chunk = reader.fill_buf()?;
                    if chunk.is_empty() {
                        return Ok(None);
                    }
                    let end = chunk.iter().position(|byte| *byte == b'\n');
                    let consumed = end.map_or(chunk.len(), |index| index + 1);
                    reader.consume(consumed);
                    if end.is_some() {
                        break;
                    }
                }
            }
        } else if bytes.last() != Some(&b'\n') {
            return Ok(None);
        } else if let Ok(line) = String::from_utf8(bytes) {
            if line.trim().is_empty() {
                continue;
            }
            if serde_json::from_str::<InteractionEvent>(&line).is_ok() {
                return Ok(Some(line));
            }
        }
        eprintln!("level=warn component=panel_provider event=input_rejected");
    }
}

async fn send_panel_interaction(socket: &Path, event_json: &str) -> anyhow::Result<()> {
    let Some(request) = panel_interaction_request_line(event_json) else {
        return Ok(());
    };
    let mut stream = tokio::net::UnixStream::connect(socket).await?;
    stream.write_all(request.as_bytes()).await?;
    let mut response = Vec::with_capacity(MAX_PANEL_REPLY_BYTES + 1);
    BufReader::new(stream)
        .take((MAX_PANEL_REPLY_BYTES + 1) as u64)
        .read_until(b'\n', &mut response)
        .await?;
    if response == b"ok\n" {
        Ok(())
    } else {
        anyhow::bail!("panel interaction was rejected");
    }
}

fn panel_interaction_request_line(event_json: &str) -> Option<String> {
    let event_json = event_json.trim_end_matches(['\r', '\n']);
    (!event_json.trim().is_empty()).then(|| format!("panel-interaction {event_json}\n"))
}

#[derive(Clone)]
pub(crate) struct ControlSocket {
    control: NotifyControl,
    connections: Arc<tokio::sync::Semaphore>,
    panel_subscriptions: Arc<tokio::sync::Semaphore>,
}

impl ControlSocket {
    pub(crate) fn new(control: NotifyControl) -> Self {
        Self {
            control,
            connections: Arc::new(tokio::sync::Semaphore::new(64)),
            panel_subscriptions: Arc::new(tokio::sync::Semaphore::new(8)),
        }
    }

    pub(crate) fn accept(&self, stream: tokio::net::UnixStream) {
        let Ok(permit) = self.connections.clone().try_acquire_owned() else {
            return;
        };
        let server = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = server.handle_connection(stream).await {
                eprintln!("level=warn component=control event=request_failed error={error:#}");
            }
        });
    }

    async fn handle_connection(self, stream: tokio::net::UnixStream) -> anyhow::Result<()> {
        let control = self.control;
        let mut stream = BufReader::new(stream);
        let mut request = String::new();
        {
            let mut limited = (&mut stream).take((MAX_CONTROL_FRAME_BYTES + 1) as u64);
            tokio::time::timeout(CONTROL_READ_TIMEOUT, limited.read_line(&mut request)).await??;
        }
        if request.len() > MAX_CONTROL_FRAME_BYTES || !request.ends_with('\n') {
            write_socket_frame(
                stream.get_mut(),
                b"error control frame too large or incomplete\n",
            )
            .await?;
            stream.get_mut().shutdown().await?;
            return Ok(());
        }
        let request = request.trim_end_matches(['\r', '\n']);
        let request = SocketControlRequest::parse(request);
        eprintln!(
            "level=info component=control event=request command={}",
            request.kind()
        );

        if matches!(request, SocketControlRequest::PanelSubscribe) {
            let Ok(_permit) = self.panel_subscriptions.try_acquire() else {
                let response = panel_response_line(PanelContent::error(
                    "Canopa Notify",
                    "Too many Notification Center subscriptions; close a panel and retry.",
                ))?;
                write_socket_frame(stream.get_mut(), response.as_bytes()).await?;
                stream.get_mut().shutdown().await?;
                return Ok(());
            };
            stream_projection_subscription(&mut stream, control).await?;
            return Ok(());
        }

        let response = control_socket_response(request, &control)
            .await
            .unwrap_or_else(|_| {
                "error request rejected; inspect daemon status for configuration or health errors\n"
                    .to_string()
            });

        let stream = stream.get_mut();
        write_socket_frame(stream, response.as_bytes()).await?;
        stream.shutdown().await?;
        Ok(())
    }
}

async fn write_socket_frame(
    stream: &mut tokio::net::UnixStream,
    frame: &[u8],
) -> anyhow::Result<()> {
    tokio::time::timeout(CONTROL_WRITE_TIMEOUT, stream.write_all(frame)).await??;
    Ok(())
}

async fn stream_projection_subscription(
    stream: &mut BufReader<tokio::net::UnixStream>,
    control: NotifyControl,
) -> anyhow::Result<()> {
    if !stream.buffer().is_empty() {
        return Ok(());
    }
    let mut subscription = control.subscribe_notification_state();
    let mut unexpected = [0];
    loop {
        let projection = tokio::select! {
            biased;
            result = stream.read(&mut unexpected) => {
                result?;
                break;
            }
            projection = subscription.next_projection() => projection?,
        };
        let Some(projection) = projection else {
            break;
        };
        let content = panel_content_from_projection(projection);
        let response = panel_response_line(content)?;
        write_socket_frame(stream.get_mut(), response.as_bytes()).await?;
    }
    stream.get_mut().shutdown().await?;
    Ok(())
}

pub(super) fn panel_response_line(content: PanelContent) -> anyhow::Result<String> {
    if let Some(line) = json_line(
        &ProviderResponse::replace_content(content),
        MAX_PANEL_FRAME_BYTES,
    )? {
        Ok(line)
    } else {
        Ok(
            serde_json::to_string(&ProviderResponse::replace_content(PanelContent::error(
                "Canopa Notify",
                "Notification Center exceeds the 1 MiB display limit; use notification history controls.",
            )))? + "\n",
        )
    }
}

pub(super) fn control_reply_line(value: &impl serde::Serialize) -> anyhow::Result<String> {
    Ok(
        json_line(value, MAX_CONTROL_REPLY_BYTES)?
            .unwrap_or_else(|| CONTROL_REPLY_TOO_LARGE.into()),
    )
}

fn json_line(value: &impl serde::Serialize, maximum: usize) -> anyhow::Result<Option<String>> {
    let mut buffer = JsonLineBuffer {
        bytes: Vec::with_capacity(4096.min(maximum)),
        maximum,
        exceeded: false,
    };
    if let Err(error) = serde_json::to_writer(&mut buffer, value) {
        if buffer.exceeded {
            return Ok(None);
        }
        return Err(error.into());
    }
    buffer.bytes.push(b'\n');
    Ok(Some(String::from_utf8(buffer.bytes)?))
}

struct JsonLineBuffer {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl Write for JsonLineBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.maximum - 1 - self.bytes.len() {
            self.exceeded = true;
            return Err(std::io::Error::other("JSON reply limit exceeded"));
        }
        // Grow geometrically within the limit, retaining room for the final newline.
        let required = self.bytes.len() + bytes.len() + 1;
        if required > self.bytes.capacity() {
            let capacity = required.max(self.bytes.capacity() * 2).min(self.maximum);
            self.bytes.reserve_exact(capacity - self.bytes.len());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
