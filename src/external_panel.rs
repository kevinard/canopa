use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::{Child, ChildStderr};

pub(crate) mod protocol {
    pub(crate) use niribar::external_panel::protocol::*;
}
pub(crate) struct ExternalPanelStartedSession {
    pub(crate) content: crate::surface::panel::ExternalPanelDocument,
    pub(crate) handle: ExternalPanelSessionHandle,
}

#[derive(Clone)]
pub(crate) struct ExternalPanelSessionHandle {
    session_id: String,
    stdin: Arc<tokio::sync::Mutex<ProviderStdin>>,
    stdout: Arc<tokio::sync::Mutex<ProviderStdout>>,
    child: Arc<tokio::sync::Mutex<Option<Child>>>,
    closed: Arc<AtomicBool>,
    stderr_tail: Arc<tokio::sync::Mutex<ProviderStderrTail>>,
}

impl std::fmt::Debug for ExternalPanelSessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalPanelSessionHandle")
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

type ProviderStdin = Box<dyn AsyncWrite + Unpin + Send>;
type ProviderStdout = Box<dyn AsyncRead + Unpin + Send>;
const PROVIDER_CLOSE_GRACE: std::time::Duration = std::time::Duration::from_millis(250);
const PROVIDER_STDERR_TAIL_BYTES: usize = 4096;

#[derive(Debug)]
struct ProviderStderrTail {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl ProviderStderrTail {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        self.bytes.extend_from_slice(chunk);
        if self.bytes.len() > self.max_bytes {
            let excess = self.bytes.len() - self.max_bytes;
            self.bytes.drain(0..excess);
        }
    }

    fn snapshot(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
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
                self.send_close_event_best_effort().await;
                return self.wait_for_close_or_terminate().await;
            }

            {
                let mut child_slot = self.child.lock().await;
                let Some(child) = child_slot.as_mut() else {
                    return Ok(None);
                };
                match child.try_wait() {
                    Ok(Some(status)) => {
                        *child_slot = None;
                        self.trace_stderr_tail("external panel provider exited unexpectedly")
                            .await;
                        return Ok(Some(format!("provider process exited with {status}")));
                    }
                    Ok(None) => {}
                    Err(err) => return Err(protocol::ProtocolError::Io(err.to_string())),
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    async fn send_close_event_best_effort(&self) {
        let mut stdin = self.stdin.lock().await;
        let _ = protocol::write_frame(
            &mut *stdin,
            &protocol::HostMessage::Event(protocol::ProviderEvent::Close),
        )
        .await;
    }

    async fn wait_for_close_or_terminate(&self) -> Result<Option<String>, protocol::ProtocolError> {
        let deadline = tokio::time::Instant::now() + PROVIDER_CLOSE_GRACE;
        loop {
            {
                let mut child_slot = self.child.lock().await;
                let Some(child) = child_slot.as_mut() else {
                    return Ok(None);
                };
                match child.try_wait() {
                    Ok(Some(_status)) => {
                        *child_slot = None;
                        return Ok(None);
                    }
                    Ok(None) => {}
                    Err(err) => return Err(protocol::ProtocolError::Io(err.to_string())),
                }
            }

            if tokio::time::Instant::now() >= deadline {
                let child = {
                    let mut child_slot = self.child.lock().await;
                    child_slot.take()
                };
                if let Some(mut child) = child {
                    let _ = child.start_kill();
                    let _ =
                        tokio::time::timeout(std::time::Duration::from_secs(1), child.wait()).await;
                }
                self.trace_stderr_tail("external panel provider terminated after close grace")
                    .await;
                return Ok(None);
            }

            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }

    async fn trace_stderr_tail(&self, message: &'static str) {
        let stderr_tail = self.stderr_tail.lock().await.snapshot();
        if stderr_tail.is_empty() {
            return;
        }
        tracing::debug!(
            target: "niribar::external_panel",
            session = self.session_id.as_str(),
            provider_stderr_tail = stderr_tail.as_str(),
            "{message}"
        );
    }

    pub(crate) async fn send_event(
        &self,
        event: protocol::ProviderEvent,
    ) -> Result<(), protocol::ProtocolError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(protocol::ProtocolError::Io(
                "external panel session is closed".into(),
            ));
        }
        let mut stdin = self.stdin.lock().await;
        protocol::write_frame(&mut *stdin, &protocol::HostMessage::Event(event)).await
    }

    pub(crate) async fn read_next_content(
        &self,
    ) -> Result<crate::surface::panel::ExternalPanelDocument, protocol::ProtocolError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(protocol::ProtocolError::Io(
                "external panel session is closed".into(),
            ));
        }
        let mut stdout = self.stdout.lock().await;
        match protocol::read_provider_frame(&mut *stdout).await? {
            protocol::ProviderMessage::Content { content } => {
                content
                    .validate()
                    .map_err(|err| protocol::ProtocolError::InvalidContent(err.to_string()))?;
                Ok(content)
            }
            _ => Err(protocol::ProtocolError::UnexpectedMessage),
        }
    }

    #[cfg(test)]
    fn for_test(
        session_id: impl Into<String>,
        stdin: impl AsyncWrite + Unpin + Send + 'static,
        stdout: impl AsyncRead + Unpin + Send + 'static,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            stdin: Arc::new(tokio::sync::Mutex::new(Box::new(stdin))),
            stdout: Arc::new(tokio::sync::Mutex::new(Box::new(stdout))),
            child: Arc::new(tokio::sync::Mutex::new(None)),
            closed: Arc::new(AtomicBool::new(false)),
            stderr_tail: Arc::new(tokio::sync::Mutex::new(ProviderStderrTail::new(
                PROVIDER_STDERR_TAIL_BYTES,
            ))),
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
        let stderr_tail = Arc::new(tokio::sync::Mutex::new(ProviderStderrTail::new(
            PROVIDER_STDERR_TAIL_BYTES,
        )));
        if let Some(stderr) = self.child.stderr.take() {
            capture_provider_stderr(stderr, stderr_tail);
        }
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
        let stderr_tail = Arc::new(tokio::sync::Mutex::new(ProviderStderrTail::new(
            PROVIDER_STDERR_TAIL_BYTES,
        )));
        if let Some(stderr) = self.child.stderr.take() {
            capture_provider_stderr(stderr, stderr_tail.clone());
        }
        let content = protocol::perform_handshake(&mut stdout, &mut stdin, handshake).await?;
        let handle = ExternalPanelSessionHandle {
            session_id,
            stdin: Arc::new(tokio::sync::Mutex::new(Box::new(stdin))),
            stdout: Arc::new(tokio::sync::Mutex::new(Box::new(stdout))),
            child: Arc::new(tokio::sync::Mutex::new(Some(self.child))),
            closed: Arc::new(AtomicBool::new(false)),
            stderr_tail,
        };
        Ok(ExternalPanelStartedSession { content, handle })
    }
}

fn capture_provider_stderr(
    mut stderr: ChildStderr,
    tail: Arc<tokio::sync::Mutex<ProviderStderrTail>>,
) {
    tokio::spawn(async move {
        let mut buffer = [0_u8; 1024];
        loop {
            match tokio::io::AsyncReadExt::read(&mut stderr, &mut buffer).await {
                Ok(0) => return,
                Ok(read) => tail.lock().await.push(&buffer[..read]),
                Err(_) => return,
            }
        }
    });
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
    command.stderr(std::process::Stdio::piped());
    command.kill_on_drop(true);
    let child = command.spawn()?;
    Ok(ExternalPanelProcess { child })
}

pub(crate) fn is_builtin_provider_name(name: &str) -> bool {
    matches!(name, "wifi")
}

pub(crate) fn builtin_provider_command(name: &str) -> Option<crate::config::ConfiguredCommand> {
    let binary_name = match name {
        "wifi" => "niribar-wifi-provider",
        _ => return None,
    };
    let program = match std::env::current_exe() {
        Ok(current_exe) => {
            resolve_builtin_provider_binary(binary_name, &current_exe, |path| path.is_file())
        }
        Err(_) => binary_name.to_string(),
    };
    Some(crate::config::ConfiguredCommand::argv(
        program,
        Vec::new(),
        &crate::config::CommandPolicy::default(),
    ))
}

fn resolve_builtin_provider_binary(
    binary_name: &str,
    current_exe: &std::path::Path,
    exists: impl Fn(&std::path::Path) -> bool,
) -> String {
    if let Some(parent) = current_exe.parent() {
        let sibling = parent.join(binary_name);
        if exists(&sibling) {
            return sibling.to_string_lossy().into_owned();
        }
    }
    binary_name.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn sending_provider_event_does_not_wait_for_provider_content() {
        let (host_stdin, mut provider_reader) = tokio::io::duplex(4096);
        let (_provider_writer, host_stdout) = tokio::io::duplex(4096);
        let handle = ExternalPanelSessionHandle::for_test("session-1", host_stdin, host_stdout);

        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            handle.send_event(protocol::ProviderEvent::ButtonActivated {
                element_id: "scan".into(),
            }),
        )
        .await
        .expect("event write should not wait for provider content")
        .unwrap();

        let mut header = vec![0_u8; protocol::FRAMED_MAGIC.len() + 8];
        provider_reader.read_exact(&mut header).await.unwrap();
        let mut len_bytes = [0_u8; 8];
        len_bytes.copy_from_slice(&header[protocol::FRAMED_MAGIC.len()..]);
        let payload_len = u64::from_be_bytes(len_bytes) as usize;
        let mut payload = vec![0_u8; payload_len];
        provider_reader.read_exact(&mut payload).await.unwrap();

        assert_eq!(
            serde_json::from_slice::<protocol::HostMessage>(&payload).unwrap(),
            protocol::HostMessage::Event(protocol::ProviderEvent::ButtonActivated {
                element_id: "scan".into(),
            })
        );
    }

    #[tokio::test]
    async fn reads_unsolicited_provider_content_replacement() {
        let (host_stdin, _provider_reader) = tokio::io::duplex(4096);
        let (mut provider_writer, host_stdout) = tokio::io::duplex(4096);
        let handle = ExternalPanelSessionHandle::for_test("session-1", host_stdin, host_stdout);
        let content = crate::surface::panel::ExternalPanelDocument {
            elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                text: "Scan complete".into(),
            }],
        };
        let frame = protocol::encode_frame(&protocol::ProviderMessage::Content {
            content: content.clone(),
        })
        .unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut provider_writer, &frame)
            .await
            .unwrap();

        assert_eq!(handle.read_next_content().await.unwrap(), content);
    }

    #[tokio::test]
    async fn invalid_streamed_provider_content_fails_before_rendering() {
        let (host_stdin, _provider_reader) = tokio::io::duplex(4096);
        let (mut provider_writer, host_stdout) = tokio::io::duplex(4096);
        let handle = ExternalPanelSessionHandle::for_test("session-1", host_stdin, host_stdout);
        let frame = protocol::encode_frame(&protocol::ProviderMessage::Content {
            content: crate::surface::panel::ExternalPanelDocument {
                elements: vec![
                    crate::surface::panel::ExternalPanelElement::Button {
                        id: "dup".into(),
                        label: "One".into(),
                    },
                    crate::surface::panel::ExternalPanelElement::Button {
                        id: "dup".into(),
                        label: "Two".into(),
                    },
                ],
            },
        })
        .unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut provider_writer, &frame)
            .await
            .unwrap();

        let err = handle.read_next_content().await.unwrap_err();

        assert!(matches!(err, protocol::ProtocolError::InvalidContent(_)));
    }

    #[tokio::test]
    async fn closing_session_sends_best_effort_close_event() {
        let (host_stdin, mut provider_reader) = tokio::io::duplex(4096);
        let (_provider_writer, host_stdout) = tokio::io::duplex(4096);
        let handle = ExternalPanelSessionHandle::for_test("session-1", host_stdin, host_stdout);

        handle.close();
        handle.wait_for_exit().await.unwrap();

        let mut header = vec![0_u8; protocol::FRAMED_MAGIC.len() + 8];
        provider_reader.read_exact(&mut header).await.unwrap();
        let mut len_bytes = [0_u8; 8];
        len_bytes.copy_from_slice(&header[protocol::FRAMED_MAGIC.len()..]);
        let payload_len = u64::from_be_bytes(len_bytes) as usize;
        let mut payload = vec![0_u8; payload_len];
        provider_reader.read_exact(&mut payload).await.unwrap();

        assert_eq!(
            serde_json::from_slice::<protocol::HostMessage>(&payload).unwrap(),
            protocol::HostMessage::Event(protocol::ProviderEvent::Close)
        );
    }

    #[tokio::test]
    async fn closing_session_terminates_child_after_grace_period() {
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 60")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr_tail = Arc::new(tokio::sync::Mutex::new(ProviderStderrTail::new(
            PROVIDER_STDERR_TAIL_BYTES,
        )));
        if let Some(stderr) = child.stderr.take() {
            capture_provider_stderr(stderr, stderr_tail.clone());
        }
        let child = Arc::new(tokio::sync::Mutex::new(Some(child)));
        let handle = ExternalPanelSessionHandle {
            session_id: "session-1".into(),
            stdin: Arc::new(tokio::sync::Mutex::new(Box::new(stdin))),
            stdout: Arc::new(tokio::sync::Mutex::new(Box::new(stdout))),
            child: child.clone(),
            closed: Arc::new(AtomicBool::new(false)),
            stderr_tail,
        };

        handle.close();
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_for_exit())
            .await
            .expect("provider should be terminated after close grace")
            .unwrap();

        assert!(child.lock().await.is_none());
    }

    #[test]
    fn provider_stderr_tail_keeps_bounded_suffix() {
        let mut tail = ProviderStderrTail::new(8);

        tail.push(b"first-line\n");
        tail.push(b"second-line\n");

        assert_eq!(tail.snapshot(), "nd-line\n");
    }

    #[test]
    fn builtin_provider_binary_prefers_sibling_of_current_executable() {
        let current_exe = std::path::Path::new("/opt/niribar/niribar");

        let resolved =
            resolve_builtin_provider_binary("niribar-wifi-provider", current_exe, |path| {
                path == std::path::Path::new("/opt/niribar/niribar-wifi-provider")
            });

        assert_eq!(resolved, "/opt/niribar/niribar-wifi-provider");
    }

    #[test]
    fn builtin_provider_binary_falls_back_to_path_resolution() {
        let current_exe = std::path::Path::new("/opt/niribar/niribar");

        let resolved =
            resolve_builtin_provider_binary("niribar-wifi-provider", current_exe, |_path| false);

        assert_eq!(resolved, "niribar-wifi-provider");
    }
}
