use std::path::PathBuf;

use iced::futures::stream;
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::app::Message;

pub(crate) mod protocol;

#[derive(Hash)]
struct IpcSubscriptionKey;

/// Path of the control socket: `$XDG_RUNTIME_DIR/niribar.sock`.
pub fn socket_path() -> std::io::Result<PathBuf> {
    let dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "XDG_RUNTIME_DIR is required for niribar IPC",
        )
    })?;
    Ok(PathBuf::from(dir).join("niribar.sock"))
}

fn remove_stale_socket_if_safe(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::FileTypeExt;

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => std::fs::remove_file(path),
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

/// Iced subscription that listens on the Unix socket for IPC commands.
///
/// Emits wrapped `SurfaceEvent::TogglePanel` when it receives `"toggle-panel"` on any connection.
/// Removes a stale socket file on first startup. Runs indefinitely.
///
/// Pattern mirrors `niri_subscription`: a `tokio::spawn` task handles blocking/async I/O
/// and sends events via an `mpsc` channel; the stream yields from the channel.
pub fn ipc_subscription() -> iced::Subscription<Message> {
    iced::Subscription::run_with(IpcSubscriptionKey, |_: &IpcSubscriptionKey| {
        stream::unfold(None::<mpsc::UnboundedReceiver<Message>>, |rx| async move {
            let mut rx = match rx {
                Some(r) => r,
                None => {
                    // First call: bind socket and spawn the listener task.
                    let path = match socket_path() {
                        Ok(path) => path,
                        Err(_) => {
                            std::future::pending::<()>().await;
                            unreachable!()
                        }
                    };
                    if remove_stale_socket_if_safe(&path).is_err() {
                        std::future::pending::<()>().await;
                        unreachable!()
                    }
                    let listener = match UnixListener::bind(&path) {
                        Ok(l) => l,
                        Err(_) => {
                            // Cannot bind — keep stream alive but emit nothing.
                            std::future::pending::<()>().await;
                            unreachable!()
                        }
                    };
                    let _ = set_owner_only_permissions(&path);
                    let (tx, rx) = mpsc::unbounded_channel();
                    tokio::spawn(async move {
                        loop {
                            let Ok((stream, _)) = listener.accept().await else {
                                continue;
                            };
                            let mut stream = stream;
                            let mut bytes = Vec::new();
                            if tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut bytes)
                                .await
                                .is_err()
                            {
                                continue;
                            }
                            if bytes.starts_with(protocol::FRAMED_MAGIC.as_bytes()) {
                                if let Ok(protocol::IpcRequest::DmenuSelection(request)) =
                                    protocol::decode_frame_payload(&bytes)
                                {
                                    let (responder, response_rx) = tokio::sync::oneshot::channel();
                                    if tx
                                        .send(Message::Ipc(
                                            crate::app::messages::IpcEvent::DmenuSelection {
                                                request,
                                                responder: crate::app::messages::ipc_responder(
                                                    responder,
                                                ),
                                            },
                                        ))
                                        .is_ok()
                                        && let Ok(response) = response_rx.await
                                        && let Ok(frame) = protocol::encode_frame(&response)
                                    {
                                        let _ = tokio::io::AsyncWriteExt::write_all(
                                            &mut stream,
                                            &frame,
                                        )
                                        .await;
                                    }
                                }
                                continue;
                            }
                            if let Some(message) = ipc_message_for_bytes(&bytes)
                                && tx.send(message).is_err()
                            {
                                break;
                            }
                        }
                    });
                    rx
                }
            };
            // Wait for the next command from the listener task.
            match rx.recv().await {
                Some(msg) => Some((msg, Some(rx))),
                None => {
                    // Channel closed — keep stream alive but emit nothing.
                    std::future::pending::<()>().await;
                    unreachable!()
                }
            }
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
        return Some(ipc_message_for_request(request));
    }
    let line = std::str::from_utf8(bytes).ok()?;
    ipc_message_for_line(line)
}

fn ipc_message_for_request(request: protocol::IpcRequest) -> Message {
    match request {
        protocol::IpcRequest::DmenuSelection(request) => {
            let (responder, _rx) = tokio::sync::oneshot::channel();
            Message::Ipc(crate::app::messages::IpcEvent::DmenuSelection {
                request,
                responder: crate::app::messages::ipc_responder(responder),
            })
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

    #[test]
    fn socket_path_ends_with_niribar_sock() {
        let original = std::env::var("XDG_RUNTIME_DIR").ok();
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        let path = socket_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "niribar.sock");
        unsafe {
            match original {
                Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
                None => std::env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
    }

    #[test]
    fn socket_path_uses_xdg_runtime_dir_when_set() {
        let original = std::env::var("XDG_RUNTIME_DIR").ok();
        // SAFETY: single-threaded test; no concurrent env reads.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        let path = socket_path().unwrap();
        assert_eq!(
            path,
            std::path::PathBuf::from("/run/user/1000/niribar.sock")
        );
        unsafe {
            match original {
                Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
                None => std::env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
    }

    #[test]
    fn socket_path_errors_when_xdg_unset() {
        let original = std::env::var("XDG_RUNTIME_DIR").ok();
        // SAFETY: single-threaded test; no concurrent env reads.
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        let err = socket_path().unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        unsafe {
            if let Some(v) = original {
                std::env::set_var("XDG_RUNTIME_DIR", v);
            }
        }
    }

    #[test]
    fn stale_cleanup_refuses_regular_file() {
        let path =
            std::env::temp_dir().join(format!("niribar-test-{}-regular.sock", std::process::id()));
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
    fn legacy_toggle_panel_bytes_still_work() {
        let decoded = ipc_message_for_bytes(b"toggle-panel\n").expect("message");
        assert!(matches!(
            decoded,
            Message::Surface(crate::app::messages::SurfaceEvent::TogglePanel)
        ));
    }
}
