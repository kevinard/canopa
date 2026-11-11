//! Independent implementation of greetd-ipc(7). Response buffers and requests are bounded.
use crate::config::Session;
use anyhow::{Context, Result, bail, ensure};
use canopa_session_ui::secret::Secret;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    sync::mpsc,
    time::Instant,
};
use zeroize::Zeroizing;
const MAX_FRAME: usize = 65536;
const IO_TIMEOUT: Duration = Duration::from_secs(3);
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request<'a> {
    CreateSession {
        username: &'a str,
    },
    PostAuthMessageResponse {
        response: Option<&'a str>,
    },
    StartSession {
        cmd: &'a [String],
        env: &'a [String],
    },
    CancelSession,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    Success,
    Error {
        error_type: String,
        description: String,
    },
    AuthMessage {
        auth_message_type: String,
        auth_message: String,
    },
}
pub enum Command {
    Begin { generation: u64, user: String },
    Answer { generation: u64, answer: Secret },
    Commit { generation: u64, session: Session },
    Cancel { generation: u64 },
}
pub enum Kind {
    Prompt { text: String, echo: bool },
    Message { text: String, error: bool },
    Authenticated,
    Started,
    Cancelled(String),
    Failed(String),
    Fatal,
}
pub struct Event {
    pub generation: u64,
    pub kind: Kind,
}
pub struct Worker {
    sender: Option<mpsc::UnboundedSender<Command>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(
        path: PathBuf,
        timeout: u64,
        mut notify: impl FnMut(Event) + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let thread = std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| runtime.block_on(run(path, timeout, receiver, &mut notify)));
            if let Err(error) = result {
                eprintln!("greeter login transport failed: {error:#}");
                notify(Event {
                    generation: 0,
                    kind: Kind::Fatal,
                });
            }
        });
        Self {
            sender: Some(sender),
            thread: Some(thread),
        }
    }
    pub fn send(&self, command: Command) -> Result<()> {
        self.sender
            .as_ref()
            .context("login worker unavailable")?
            .send(command)
            .map_err(|_| anyhow::anyhow!("login worker unavailable"))
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
async fn connect(path: &std::path::Path) -> Result<UnixStream> {
    ensure!(path.is_absolute(), "GREETD_SOCK must be absolute");
    let stream = tokio::time::timeout(IO_TIMEOUT, UnixStream::connect(path)).await??;
    let uid = stream.peer_cred()?.uid();
    ensure!(
        uid == 0 || uid == unsafe { libc::geteuid() },
        "login socket belongs to another user"
    );
    Ok(stream)
}
async fn write(stream: &mut UnixStream, request: &Request<'_>) -> Result<()> {
    // A fixed backing allocation avoids plaintext copies in reallocated Vec arenas.
    let mut bytes = Zeroizing::new(vec![0; MAX_FRAME]);
    let mut cursor = std::io::Cursor::new(bytes.as_mut_slice());
    serde_json::to_writer(&mut cursor, request)?;
    let length = cursor.position() as usize;
    tokio::time::timeout(IO_TIMEOUT, async {
        stream.write_all(&(length as u32).to_ne_bytes()).await?;
        stream.write_all(&bytes[..length]).await
    })
    .await??;
    Ok(())
}
async fn read(stream: &mut UnixStream) -> Result<Response> {
    let mut header = [0; 4];
    stream.read_exact(&mut header).await?;
    let length = u32::from_ne_bytes(header) as usize;
    ensure!(
        (1..=MAX_FRAME).contains(&length),
        "login response exceeds budget"
    );
    let mut bytes = Zeroizing::new(vec![0; length]);
    stream.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
async fn reset(stream: &mut UnixStream) -> Result<()> {
    write(stream, &Request::CancelSession).await?;
    ensure!(
        matches!(
            tokio::time::timeout(IO_TIMEOUT, read(stream)).await??,
            Response::Success
        ),
        "login cancellation was refused"
    );
    Ok(())
}
async fn reset_after_failure(stream: &mut UnixStream) -> Result<()> {
    write(stream, &Request::CancelSession).await?;
    match tokio::time::timeout(IO_TIMEOUT, read(stream)).await?? {
        Response::Success => Ok(()),
        // greetd may already have lost its PAM worker after authentication
        // failure. Its first cancel removes the session but reports that worker
        // error. Require a second, successful reset before accepting a retry.
        Response::Error { .. } => reset(stream).await,
        _ => bail!("unexpected failure cleanup response"),
    }
}
enum Reply {
    Response(Response),
    Cancelled { generation: u64, message: String },
    Closed,
}
async fn exchange(
    stream: &mut UnixStream,
    path: &std::path::Path,
    request: &Request<'_>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    generation: u64,
    deadline: Instant,
) -> Result<Reply> {
    write(stream, request).await?;
    pending_response(stream, path, commands, generation, deadline).await
}
async fn pending_response(
    stream: &mut UnixStream,
    path: &std::path::Path,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    generation: u64,
    deadline: Instant,
) -> Result<Reply> {
    let response = read(stream);
    tokio::pin!(response);
    let cancelled = tokio::select! {biased;
        command=commands.recv()=>match command {Some(Command::Cancel{generation})=>Some((generation,fl!("greeter-authentication-cancelled"))),None=>None,_=>bail!("unexpected login command while awaiting service")},
        _=tokio::time::sleep_until(deadline)=>Some((generation,fl!("greeter-authentication-timed-out"))),
        reply=&mut response=>return Ok(Reply::Response(reply?)),
    };
    // greetd accepts concurrent clients. Cancel on a separate connection, then
    // drain the original outstanding reply before reusing its stream. Dropping an
    // unfinished request could make its later EPIPE cancel a NEW transaction.
    let mut cancellation = connect(path).await?;
    reset(&mut cancellation).await?;
    tokio::time::timeout(IO_TIMEOUT, &mut response).await??;
    Ok(match cancelled {
        Some((generation, message)) => Reply::Cancelled {
            generation,
            message,
        },
        None => Reply::Closed,
    })
}
fn clean(text: String) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(2048)
        .collect()
}
async fn run(
    path: PathBuf,
    timeout: u64,
    mut commands: mpsc::UnboundedReceiver<Command>,
    notify: &mut impl FnMut(Event),
) -> Result<()> {
    let mut stream = connect(&path).await?;
    reset(&mut stream).await?;
    while let Some(command) = commands.recv().await {
        let (generation, user) = match command {
            Command::Begin { generation, user } => (generation, user),
            Command::Cancel { generation } => {
                reset(&mut stream).await?;
                notify(Event {
                    generation,
                    kind: Kind::Cancelled(fl!("greeter-authentication-cancelled")),
                });
                continue;
            }
            _ => continue,
        };
        ensure!(crate::config::valid_user(&user), "invalid login user");
        let deadline = Instant::now() + Duration::from_secs(timeout);
        let mut reply = exchange(
            &mut stream,
            &path,
            &Request::CreateSession { username: &user },
            &mut commands,
            generation,
            deadline,
        )
        .await?;
        loop {
            let response = match reply {
                Reply::Cancelled {
                    generation,
                    message,
                } => {
                    notify(Event {
                        generation,
                        kind: Kind::Cancelled(message),
                    });
                    break;
                }
                Reply::Closed => return Ok(()),
                Reply::Response(response) => response,
            };
            match response {
                Response::Error {
                    error_type,
                    description,
                } => {
                    // Even auth_error may leave an allocated configuring session.
                    reset_after_failure(&mut stream).await?;
                    let text = if error_type == "auth_error" {
                        fl!("greeter-authentication-failed")
                    } else {
                        clean(description)
                    };
                    notify(Event {
                        generation,
                        kind: Kind::Failed(text),
                    });
                    break;
                }
                Response::AuthMessage {
                    auth_message_type,
                    auth_message,
                } => {
                    let text = clean(auth_message);
                    match auth_message_type.as_str() {
                        "info" | "error" => {
                            notify(Event {
                                generation,
                                kind: Kind::Message {
                                    text,
                                    error: auth_message_type == "error",
                                },
                            });
                            reply = exchange(
                                &mut stream,
                                &path,
                                &Request::PostAuthMessageResponse { response: None },
                                &mut commands,
                                generation,
                                deadline,
                            )
                            .await?;
                        }
                        "visible" | "secret" => {
                            notify(Event {
                                generation,
                                kind: Kind::Prompt {
                                    text,
                                    echo: auth_message_type == "visible",
                                },
                            });
                            let action = tokio::select! {biased;command=commands.recv()=>command,_=tokio::time::sleep_until(deadline)=>Some(Command::Cancel{generation})};
                            match action {
                                Some(Command::Answer {
                                    generation: g,
                                    answer,
                                }) if g == generation => {
                                    write(
                                        &mut stream,
                                        &Request::PostAuthMessageResponse {
                                            response: Some(answer.text()),
                                        },
                                    )
                                    .await?;
                                    // No response remains in our memory while PAM waits on a device.
                                    drop(answer);
                                    reply = pending_response(
                                        &mut stream,
                                        &path,
                                        &mut commands,
                                        generation,
                                        deadline,
                                    )
                                    .await?;
                                }
                                Some(Command::Cancel { generation: g }) => {
                                    reset(&mut stream).await?;
                                    notify(Event {
                                        generation: g,
                                        kind: Kind::Cancelled(fl!(
                                            "greeter-authentication-cancelled-or-timed-out"
                                        )),
                                    });
                                    break;
                                }
                                None => {
                                    reset(&mut stream).await?;
                                    return Ok(());
                                }
                                _ => bail!("unexpected prompt response"),
                            }
                        }
                        _ => {
                            reset(&mut stream).await?;
                            bail!("unknown authentication message type");
                        }
                    }
                }
                Response::Success => {
                    notify(Event {
                        generation,
                        kind: Kind::Authenticated,
                    });
                    // Only the UI's still-current generation can authorize session
                    // handoff. A late PAM success by itself never starts a desktop.
                    let action = tokio::select! {biased;command=commands.recv()=>command,_=tokio::time::sleep_until(deadline)=>Some(Command::Cancel{generation})};
                    if Instant::now() >= deadline {
                        reset(&mut stream).await?;
                        let generation = match &action {
                            Some(Command::Cancel { generation }) => *generation,
                            _ => generation,
                        };
                        notify(Event {
                            generation,
                            kind: Kind::Cancelled(fl!("greeter-authentication-timed-out")),
                        });
                        break;
                    }
                    match action {
                        Some(Command::Commit {
                            generation: g,
                            session,
                        }) if g == generation => {
                            let cmd = vec![session.shell_command()?];
                            write(
                                &mut stream,
                                &Request::StartSession {
                                    cmd: &cmd,
                                    env: &session.env,
                                },
                            )
                            .await?;
                            match tokio::time::timeout(IO_TIMEOUT, read(&mut stream)).await?? {
                                Response::Success => {
                                    notify(Event {
                                        generation,
                                        kind: Kind::Started,
                                    });
                                    return Ok(());
                                }
                                Response::Error { description, .. } => {
                                    reset_after_failure(&mut stream).await?;
                                    notify(Event {
                                        generation,
                                        kind: Kind::Failed(clean(description)),
                                    });
                                    break;
                                }
                                _ => bail!("unexpected session-start response"),
                            }
                        }
                        Some(Command::Cancel { generation: g }) => {
                            reset(&mut stream).await?;
                            notify(Event {
                                generation: g,
                                kind: Kind::Cancelled(fl!("greeter-authentication-cancelled")),
                            });
                            break;
                        }
                        None => {
                            reset(&mut stream).await?;
                            return Ok(());
                        }
                        _ => bail!("unexpected session authorization"),
                    }
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancellation_at_the_ready_deadline_acknowledges_the_new_ui_generation() {
        async fn request(stream: &mut UnixStream) -> String {
            let mut header = [0; 4];
            stream.read_exact(&mut header).await.unwrap();
            let mut body = vec![0; u32::from_ne_bytes(header) as usize];
            stream.read_exact(&mut body).await.unwrap();
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned()
        }
        async fn success(stream: &mut UnixStream) {
            let body = br#"{"type":"success"}"#;
            stream
                .write_all(&(body.len() as u32).to_ne_bytes())
                .await
                .unwrap();
            stream.write_all(body).await.unwrap();
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("greetd.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            for expected in ["cancel_session", "create_session", "cancel_session"] {
                assert_eq!(request(&mut stream).await, expected);
                success(&mut stream).await;
            }
        });
        let (sender, receiver) = mpsc::unbounded_channel();
        sender
            .send(Command::Begin {
                generation: 1,
                user: "alice".into(),
            })
            .unwrap();
        let mut sender = Some(sender);
        let mut events = Vec::new();
        run(path, 1, receiver, &mut |event| {
            if matches!(event.kind, Kind::Authenticated) {
                sender
                    .take()
                    .unwrap()
                    .send(Command::Cancel { generation: 2 })
                    .unwrap();
                // Model scheduling delay after success while the UI has cancelled.
                std::thread::sleep(Duration::from_millis(1100));
            }
            events.push(event);
        })
        .await
        .unwrap();
        server.await.unwrap();
        assert!(
            events
                .iter()
                .any(|event| event.generation == 2 && matches!(event.kind, Kind::Cancelled(_)))
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event.kind, Kind::Started))
        );
    }
    #[tokio::test]
    async fn an_oversized_request_is_rejected_before_any_frame_is_written() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let cmd = vec!["x".repeat(MAX_FRAME)];
        assert!(
            write(
                &mut client,
                &Request::StartSession {
                    cmd: &cmd,
                    env: &[]
                }
            )
            .await
            .is_err()
        );
        assert_eq!(
            server.try_read(&mut [0; 1]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[tokio::test]
    async fn frames_are_native_endian_bounded_and_reject_unknown_responses() {
        for bytes in [
            0u32.to_ne_bytes().to_vec(),
            (MAX_FRAME as u32 + 1).to_ne_bytes().to_vec(),
            {
                let mut v = 2u32.to_ne_bytes().to_vec();
                v.extend(b"{}");
                v
            },
        ] {
            let (mut client, mut server) = UnixStream::pair().unwrap();
            server.write_all(&bytes).await.unwrap();
            assert!(read(&mut client).await.is_err());
        }
        let (mut client, mut server) = UnixStream::pair().unwrap();
        write(&mut client, &Request::CreateSession { username: "alice" })
            .await
            .unwrap();
        let mut header = [0; 4];
        server.read_exact(&mut header).await.unwrap();
        let mut bytes = vec![0; u32::from_ne_bytes(header) as usize];
        server.read_exact(&mut bytes).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["username"],
            "alice"
        );
    }
}
