use std::path::PathBuf;

use iced::futures::stream;
use tokio::io::AsyncBufReadExt;
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::app::Message;

/// Path of the control socket: `$XDG_RUNTIME_DIR/niribar.sock`.
pub fn socket_path() -> PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(dir).join("niribar.sock")
}

/// Iced subscription that listens on the Unix socket for IPC commands.
///
/// Emits `Message::ToggleExpand` when it receives `"toggle-expand"` on any connection.
/// Removes a stale socket file on first startup. Runs indefinitely.
///
/// Pattern mirrors `niri_subscription`: a `tokio::spawn` task handles blocking/async I/O
/// and sends events via an `mpsc` channel; the stream yields from the channel.
pub fn ipc_subscription() -> iced::Subscription<Message> {
    iced::Subscription::run_with((), |()| {
        stream::unfold(None::<mpsc::UnboundedReceiver<Message>>, |rx| async move {
            let mut rx = match rx {
                Some(r) => r,
                None => {
                    // First call: bind socket and spawn the listener task.
                    let path = socket_path();
                    let _ = tokio::fs::remove_file(&path).await;
                    let listener = match UnixListener::bind(&path) {
                        Ok(l) => l,
                        Err(_) => {
                            // Cannot bind — keep stream alive but emit nothing.
                            std::future::pending::<()>().await;
                            unreachable!()
                        }
                    };
                    let (tx, rx) = mpsc::unbounded_channel();
                    tokio::spawn(async move {
                        loop {
                            let Ok((stream, _)) = listener.accept().await else {
                                continue;
                            };
                            let mut reader = tokio::io::BufReader::new(stream);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok()
                                && line.trim() == "toggle-expand"
                                && tx.send(Message::ToggleExpand).is_err()
                            {
                                break; // Receiver dropped — app shutting down
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_ends_with_niribar_sock() {
        let path = socket_path();
        assert_eq!(path.file_name().unwrap(), "niribar.sock");
    }

    #[test]
    fn socket_path_uses_xdg_runtime_dir_when_set() {
        let original = std::env::var("XDG_RUNTIME_DIR").ok();
        // SAFETY: single-threaded test; no concurrent env reads.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        let path = socket_path();
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
    fn socket_path_falls_back_to_tmp_when_xdg_unset() {
        let original = std::env::var("XDG_RUNTIME_DIR").ok();
        // SAFETY: single-threaded test; no concurrent env reads.
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        let path = socket_path();
        assert_eq!(path, std::path::PathBuf::from("/tmp/niribar.sock"));
        unsafe {
            if let Some(v) = original {
                std::env::set_var("XDG_RUNTIME_DIR", v);
            }
        }
    }
}
