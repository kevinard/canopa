use std::path::PathBuf;

use iced::futures::stream;
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::app::Message;

pub(crate) mod protocol;

#[derive(Hash)]
struct IpcSubscriptionKey;

/// Path of the control socket: `$XDG_RUNTIME_DIR/canopa-shell.sock`.
pub fn socket_path() -> std::io::Result<PathBuf> {
    let dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "XDG_RUNTIME_DIR is required for canopa-shell IPC",
        )
    })?;
    Ok(PathBuf::from(dir).join("canopa-shell.sock"))
}

fn remove_stale_socket_if_safe(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::FileTypeExt;

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            match std::os::unix::net::UnixStream::connect(path) {
                Ok(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "IPC socket is still active",
                )),
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                    std::fs::remove_file(path)
                }
                Err(error) => Err(error),
            }
        }
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("refusing to remove non-socket IPC path {}", path.display()),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn set_owner_only_permissions(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)
}

/// Own the socket name for exactly the lifetime of the listener. The lock also
/// serializes stale-socket recovery by simultaneous starting instances.
struct IpcServer {
    listener: UnixListener,
    _lock: std::fs::File,
    path: PathBuf,
    identity: (u64, u64),
}

impl IpcServer {
    fn bind(path: PathBuf) -> std::io::Result<Self> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(path.with_extension("lock"))?;
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).map_err(
            |_| {
                std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "another Canopa Shell instance owns the IPC socket",
                )
            },
        )?;
        remove_stale_socket_if_safe(&path)?;
        let listener = UnixListener::bind(&path)?;
        set_owner_only_permissions(&path)?;
        let metadata = std::fs::metadata(&path)?;
        Ok(Self {
            listener,
            _lock: lock,
            path,
            identity: (metadata.dev(), metadata.ino()),
        })
    }

    async fn run(self, tx: mpsc::Sender<Message>) {
        let mut clients = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _ = tx.closed() => break,
                _ = clients.join_next(), if !clients.is_empty() => {},
                accepted = self.listener.accept() => match accepted {
                    Ok((socket, _)) if clients.len() < 64 => {
                        let tx = tx.clone();
                        clients.spawn(async move {
                            if let Err(error) = serve_client(socket, tx).await {
                                tracing::debug!(target: "canopa_shell::ipc", %error, "IPC request ended");
                            }
                        });
                    }
                    Ok(_) => {}, // Bound concurrent stalled clients.
                    Err(error) => {
                        tracing::warn!(target: "canopa_shell::ipc", %error, "IPC accept failed");
                        break;
                    }
                }
            }
        }
        // JoinSet aborts pending clients when the subscription is cancelled.
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path)
            && (metadata.dev(), metadata.ino()) == self.identity
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

async fn read_request(socket: &mut tokio::net::UnixStream) -> anyhow::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    for expected in protocol::FRAMED_MAGIC.bytes() {
        let byte = socket.read_u8().await?;
        bytes.push(byte);
        if byte != expected {
            socket.take(256).read_to_end(&mut bytes).await?;
            anyhow::ensure!(bytes.len() <= 256, "legacy IPC command is too large");
            return Ok(bytes);
        }
    }
    let len = socket.read_u64().await?;
    anyhow::ensure!(
        len <= protocol::MAX_FRAME_BYTES as u64,
        "IPC frame is too large"
    );
    bytes.extend_from_slice(&len.to_be_bytes());
    let header_len = bytes.len();
    bytes.resize(header_len + len as usize, 0);
    socket.read_exact(&mut bytes[header_len..]).await?;
    Ok(bytes)
}

async fn serve_client(
    mut socket: tokio::net::UnixStream,
    tx: mpsc::Sender<Message>,
) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(2), read_request(&mut socket))
        .await??;
    if bytes.starts_with(protocol::FRAMED_MAGIC.as_bytes()) {
        let request = protocol::decode_frame_payload(&bytes)?;
        if let protocol::IpcRequest::DmenuSelection(request) = &request {
            protocol::validate_dmenu_request(request)?;
        }
        let (responder, response_rx) = tokio::sync::oneshot::channel();
        tx.send(ipc_message_for_request(
            request,
            crate::app::messages::ipc_responder(responder),
        ))
        .await?;
        // No selection deadline: the UI owns the user's interactive wait.
        let response = response_rx.await?;
        let frame = protocol::encode_frame(&response)?;
        tokio::time::timeout(std::time::Duration::from_secs(2), socket.write_all(&frame)).await??;
    } else if let Some(message) = ipc_message_for_bytes(&bytes) {
        tx.send(message).await?;
    }
    Ok(())
}

/// Concurrent, bounded IPC service. Dropping its receiver stops the listener.
pub fn ipc_subscription() -> iced::Subscription<Message> {
    iced::Subscription::run_with(IpcSubscriptionKey, |_: &IpcSubscriptionKey| {
        stream::unfold(None::<mpsc::Receiver<Message>>, |rx| async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => {
                    let (tx, rx) = mpsc::channel(32);
                    match socket_path().and_then(IpcServer::bind) {
                        Ok(server) => {
                            tokio::spawn(server.run(tx));
                        }
                        Err(error) => {
                            tracing::error!(target: "canopa_shell::ipc", %error, "IPC listener unavailable");
                        }
                    }
                    rx
                }
            };
            rx.recv().await.map(|message| (message, Some(rx)))
        })
    })
}

pub(crate) fn decode_response_frame(frame: &[u8]) -> anyhow::Result<protocol::IpcResponse> {
    if !frame.starts_with(protocol::FRAMED_MAGIC.as_bytes()) {
        anyhow::bail!("missing response frame magic");
    }
    let len_start = protocol::FRAMED_MAGIC.len();
    let len_end = len_start + 8;
    if frame.len() < len_end {
        anyhow::bail!("response frame is incomplete");
    }
    let mut len_bytes = [0_u8; 8];
    len_bytes.copy_from_slice(&frame[len_start..len_end]);
    let payload_len = u64::from_be_bytes(len_bytes) as usize;
    let payload_end = len_end + payload_len;
    if payload_len > protocol::MAX_FRAME_BYTES || frame.len() < payload_end {
        anyhow::bail!("invalid response frame length");
    }
    Ok(serde_json::from_slice(&frame[len_end..payload_end])?)
}

fn ipc_message_for_bytes(bytes: &[u8]) -> Option<Message> {
    if bytes.starts_with(protocol::FRAMED_MAGIC.as_bytes()) {
        let request = protocol::decode_frame_payload(bytes).ok()?;
        let (responder, _rx) = tokio::sync::oneshot::channel();
        return Some(ipc_message_for_request(
            request,
            crate::app::messages::ipc_responder(responder),
        ));
    }
    let line = std::str::from_utf8(bytes).ok()?;
    ipc_message_for_line(line)
}

fn ipc_message_for_request(
    request: protocol::IpcRequest,
    responder: crate::app::messages::IpcResponder,
) -> Message {
    match request {
        protocol::IpcRequest::Reload => {
            Message::Ipc(crate::app::messages::IpcEvent::Reload { responder })
        }
        protocol::IpcRequest::OpenMenu { name } => {
            Message::Ipc(crate::app::messages::IpcEvent::OpenMenu { name, responder })
        }
        protocol::IpcRequest::ToggleMenu { name } => {
            Message::Ipc(crate::app::messages::IpcEvent::ToggleMenu { name, responder })
        }
        protocol::IpcRequest::DmenuSelection(request) => {
            Message::Ipc(crate::app::messages::IpcEvent::DmenuSelection { request, responder })
        }
    }
}

fn ipc_message_for_line(line: &str) -> Option<Message> {
    match line.trim() {
        "toggle-panel" => Some(Message::Surface(
            crate::app::messages::SurfaceEvent::TogglePanel,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static XDG_RUNTIME_DIR_LOCK: Mutex<()> = Mutex::new(());

    struct XdgRuntimeDirGuard {
        original: Option<String>,
        _lock: MutexGuard<'static, ()>,
    }

    impl XdgRuntimeDirGuard {
        fn set(value: Option<&str>) -> Self {
            let lock = XDG_RUNTIME_DIR_LOCK.lock().unwrap();
            let original = std::env::var("XDG_RUNTIME_DIR").ok();
            unsafe {
                match value {
                    Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
                    None => std::env::remove_var("XDG_RUNTIME_DIR"),
                }
            }
            Self {
                original,
                _lock: lock,
            }
        }
    }

    impl Drop for XdgRuntimeDirGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
                    None => std::env::remove_var("XDG_RUNTIME_DIR"),
                }
            }
        }
    }

    #[test]
    fn socket_path_ends_with_canopa_shell_sock() {
        let _guard = XdgRuntimeDirGuard::set(Some("/run/user/1000"));
        let path = socket_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "canopa-shell.sock");
    }

    #[test]
    fn socket_path_uses_xdg_runtime_dir_when_set() {
        let _guard = XdgRuntimeDirGuard::set(Some("/run/user/1000"));
        let path = socket_path().unwrap();
        assert_eq!(
            path,
            std::path::PathBuf::from("/run/user/1000/canopa-shell.sock")
        );
    }

    #[test]
    fn socket_path_errors_when_xdg_unset() {
        let _guard = XdgRuntimeDirGuard::set(None);
        let err = socket_path().unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn stale_cleanup_refuses_regular_file() {
        let path = std::env::temp_dir().join(format!(
            "canopa-shell-test-{}-regular.sock",
            std::process::id()
        ));
        std::fs::write(&path, b"not a socket").unwrap();

        let err = remove_stale_socket_if_safe(&path).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(path.exists(), "regular files must not be unlinked");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn toggle_panel_line_emits_surface_toggle_panel() {
        let message = ipc_message_for_line("toggle-panel\n").expect("message");

        assert!(matches!(
            message,
            Message::Surface(crate::app::messages::SurfaceEvent::TogglePanel)
        ));
    }

    #[test]
    fn framed_dmenu_request_decodes_to_ipc_message() {
        let request = protocol::IpcRequest::DmenuSelection(protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
            allow_custom: true,
        });
        let frame = protocol::encode_frame(&request).unwrap();
        let decoded = ipc_message_for_bytes(&frame).expect("message");

        assert!(matches!(
            decoded,
            Message::Ipc(crate::app::messages::IpcEvent::DmenuSelection { .. })
        ));
    }

    #[test]
    fn framed_toggle_menu_request_decodes_to_ipc_message() {
        let frame = protocol::encode_frame(&protocol::IpcRequest::ToggleMenu {
            name: "Tools ☰".into(),
        })
        .unwrap();
        assert!(matches!(
            ipc_message_for_bytes(&frame),
            Some(Message::Ipc(crate::app::messages::IpcEvent::ToggleMenu { name, .. }))
                if name == "Tools ☰"
        ));
    }

    #[test]
    fn legacy_toggle_panel_bytes_still_work() {
        let decoded = ipc_message_for_bytes(b"toggle-panel\n").expect("message");
        assert!(matches!(
            decoded,
            Message::Surface(crate::app::messages::SurfaceEvent::TogglePanel)
        ));
    }
    #[tokio::test]
    async fn open_menu_request_returns_an_acknowledgement_over_the_socket() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut client, server) = tokio::net::UnixStream::pair().unwrap();
        let (tx, mut rx) = mpsc::channel(1);
        let task = tokio::spawn(serve_client(server, tx));
        client
            .write_all(
                &protocol::encode_frame(&protocol::IpcRequest::OpenMenu {
                    name: "My menu\n☰".into(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        client.shutdown().await.unwrap();
        let Message::Ipc(crate::app::messages::IpcEvent::OpenMenu { name, responder }) =
            rx.recv().await.unwrap()
        else {
            panic!("expected open-menu request")
        };
        assert_eq!(name, "My menu\n☰");
        responder
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(protocol::IpcResponse::Accepted)
            .unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            client.read_to_end(&mut response),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            decode_response_frame(&response).unwrap(),
            protocol::IpcResponse::Accepted
        );
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn pending_selection_does_not_block_other_clients() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canopa-shell.sock");
        let server = IpcServer::bind(path.clone()).unwrap();
        let (tx, mut rx) = mpsc::channel(8);
        let task = tokio::spawn(server.run(tx));
        let request = protocol::IpcRequest::DmenuSelection(protocol::DmenuRequest {
            prompt: None,
            entries: vec![protocol::DmenuEntry::plain("one")],
            allow_custom: false,
        });
        let mut first = tokio::net::UnixStream::connect(&path).await.unwrap();
        first
            .write_all(&protocol::encode_frame(&request).unwrap())
            .await
            .unwrap();
        let Message::Ipc(crate::app::messages::IpcEvent::DmenuSelection { responder, .. }) =
            rx.recv().await.unwrap()
        else {
            panic!("expected dmenu")
        };
        let mut second = tokio::net::UnixStream::connect(&path).await.unwrap();
        second.write_all(b"toggle-panel\n").await.unwrap();
        second.shutdown().await.unwrap();
        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .unwrap(),
            Some(Message::Surface(
                crate::app::messages::SurfaceEvent::TogglePanel
            ))
        ));
        // A second selection must reach the app while the first responder lives.
        let mut third = tokio::net::UnixStream::connect(&path).await.unwrap();
        third
            .write_all(&protocol::encode_frame(&request).unwrap())
            .await
            .unwrap();
        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .unwrap(),
            Some(Message::Ipc(_))
        ));
        drop(responder);
        drop(rx);
        task.await.unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn oversized_header_is_rejected_without_waiting_for_payload() {
        use tokio::io::AsyncWriteExt;
        let (mut client, mut server) = tokio::net::UnixStream::pair().unwrap();
        client
            .write_all(protocol::FRAMED_MAGIC.as_bytes())
            .await
            .unwrap();
        client
            .write_u64(protocol::MAX_FRAME_BYTES as u64 + 1)
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(200),
                read_request(&mut server)
            )
            .await
            .unwrap()
            .is_err()
        );
    }

    #[tokio::test]
    async fn second_instance_preserves_first_listener_and_stale_socket_can_recover() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canopa-shell.sock");
        let first = IpcServer::bind(path.clone()).unwrap();
        assert!(IpcServer::bind(path.clone()).is_err());
        assert!(tokio::net::UnixStream::connect(&path).await.is_ok());
        drop(first);
        let stale = UnixListener::bind(&path).unwrap();
        drop(stale);
        let recovered = IpcServer::bind(path.clone()).unwrap();
        drop(recovered);
        assert!(!path.exists());
    }
}
