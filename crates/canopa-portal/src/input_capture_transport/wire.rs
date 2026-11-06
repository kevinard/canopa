//! Private worker/control frames. Input data goes directly to the EI connection.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(super) const MAX_FRAME: usize = 16 * 1024;
pub type Zones = Vec<(u32, u32, i32, i32)>;
pub type Barriers = Vec<(u32, (i32, i32, i32, i32))>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    GetZones,
    SetBarriers { zone_set: u32, barriers: Barriers },
    Enable,
    Disable,
    Release { activation: u32 },
}
impl Command {
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        if let Self::SetBarriers { barriers, .. } = self {
            anyhow::ensure!(barriers.len() <= 128, "Too many capture barriers");
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub sequence: u64,
    pub command: Command,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum SourceRequest {
    Existing(Request),
    KeyboardEnable {
        sequence: u64,
        command: KeyboardEnable,
    },
}
#[derive(Serialize)]
pub(super) struct KeyboardEnable {
    op: &'static str,
    keymap: u32,
}
impl SourceRequest {
    pub(super) fn new(sequence: u64, command: Command, keymap: Option<u32>) -> Self {
        if matches!(command, Command::Enable)
            && let Some(keymap) = keymap
        {
            Self::KeyboardEnable {
                sequence,
                command: KeyboardEnable {
                    op: "enable",
                    keymap,
                },
            }
        } else {
            Self::Existing(Request { sequence, command })
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Unit,
    Barriers { failed_barriers: Vec<u32> },
    Zones { zone_set: u32, zones: Zones },
    Failed { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Notice {
    ReceiverReady {
        ready: bool,
    },
    Activated {
        activation: u32,
        barrier: u32,
        position: (f64, f64),
    },
    Deactivated {
        activation: u32,
    },
    Disabled,
    ZonesChanged {
        zone_set: u32,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Frame {
    Ready { zone_set: u32, zones: Zones },
    Reply { sequence: u64, reply: Reply },
    Notice { notice: Notice },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ParentCommand {
    Connect,
    Capture { command: Command },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ParentRequest {
    pub sequence: u64,
    pub command: ParentCommand,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CaptureEvent {
    Reply {
        sequence: u64,
        error: Option<String>,
        failed_barriers: Vec<u32>,
    },
    Zones {
        sequence: u64,
        zone_set: u32,
        zones: Zones,
    },
    ZonesChanged {
        zone_set: u32,
    },
    Activated {
        activation: u32,
        barrier: u32,
        position: (f64, f64),
    },
    Deactivated {
        activation: u32,
    },
    Disabled,
    KeymapChanged {
        keymap: u32,
    },
    Modifiers {
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    },
    Key {
        time: u64,
        code: u32,
        pressed: bool,
    },
    Motion {
        time: u64,
        delta: (f64, f64),
    },
    Button {
        time: u64,
        button: u32,
        pressed: bool,
    },
    Axis {
        time: u64,
        source: u32,
        amount: (Option<f64>, Option<f64>),
        v120: (Option<f64>, Option<f64>),
    },
}

pub(super) fn validate_zones(zones: &Zones) -> anyhow::Result<()> {
    anyhow::ensure!(
        zones.len() <= 64 && zones.iter().all(|(w, h, _, _)| *w > 0 && *h > 0),
        "Invalid capture zones"
    );
    Ok(())
}
pub(super) fn encode<T: Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    let data = serde_json::to_vec(value)?;
    anyhow::ensure!(
        !data.is_empty() && data.len() <= MAX_FRAME,
        "Capture control frame too large"
    );
    let mut frame = Vec::with_capacity(4 + data.len());
    frame.extend_from_slice(&(data.len() as u32).to_le_bytes());
    frame.extend_from_slice(&data);
    Ok(frame)
}
pub(super) async fn read<T: DeserializeOwned>(
    input: &mut (impl AsyncRead + Unpin),
) -> anyhow::Result<T> {
    Ok(read_counted(input).await?.1)
}
async fn read_counted<T: DeserializeOwned>(
    input: &mut (impl AsyncRead + Unpin),
) -> anyhow::Result<(usize, T)> {
    let size = input.read_u32_le().await? as usize;
    anyhow::ensure!(
        (1..=MAX_FRAME).contains(&size),
        "Invalid capture control frame size"
    );
    let mut bytes = vec![0; size];
    input.read_exact(&mut bytes).await?;
    Ok((4 + size, serde_json::from_slice(&bytes)?))
}
pub(super) async fn write<T: Serialize>(
    output: &mut (impl AsyncWrite + Unpin),
    value: &T,
) -> anyhow::Result<()> {
    output.write_all(&encode(value)?).await?;
    Ok(())
}

pub(super) enum ReadMessage<T> {
    Data { bytes: usize, value: T },
    Eof,
    Error(String),
}
pub(super) struct Reader<T> {
    receiver: tokio::sync::mpsc::Receiver<ReadMessage<T>>,
    task: tokio::task::JoinHandle<()>,
}
impl<T> Drop for Reader<T> {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl<T: DeserializeOwned + Send + 'static> Reader<T> {
    pub fn new(mut input: impl AsyncRead + Unpin + Send + 'static, capacity: usize) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        let task = tokio::spawn(async move {
            loop {
                let (message, terminal) = match read_counted::<T>(&mut input).await {
                    Ok((bytes, value)) => (ReadMessage::Data { bytes, value }, false),
                    Err(error)
                        if error
                            .downcast_ref::<std::io::Error>()
                            .is_some_and(|e| e.kind() == std::io::ErrorKind::UnexpectedEof) =>
                    {
                        (ReadMessage::Eof, true)
                    }
                    Err(error) => (ReadMessage::Error(error.to_string()), true),
                };
                if sender.send(message).await.is_err() || terminal {
                    break;
                }
            }
        });
        Self { receiver, task }
    }
    pub async fn recv(&mut self) -> Option<ReadMessage<T>> {
        self.receiver.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enable_revision_is_only_added_to_keyboard_enable_and_v1_frames_stay_compatible() {
        assert_eq!(
            serde_json::to_value(SourceRequest::new(1, Command::Enable, None)).unwrap(),
            serde_json::json!({"sequence":1,"command":{"op":"enable"}})
        );
        assert_eq!(
            serde_json::to_value(SourceRequest::new(2, Command::Enable, Some(9))).unwrap(),
            serde_json::json!({"sequence":2,"command":{"op":"enable","keymap":9}})
        );
        assert_eq!(
            serde_json::to_value(SourceRequest::new(3, Command::Disable, Some(9))).unwrap(),
            serde_json::json!({"sequence":3,"command":{"op":"disable"}})
        );
    }
    #[tokio::test]
    async fn fragmented_frames_preserve_the_following_message_and_refuse_truncated_or_oversized_input()
     {
        let (mut tx, mut rx) = tokio::io::duplex(1);
        let sending = tokio::spawn(async move {
            write(
                &mut tx,
                &Request {
                    sequence: 1,
                    command: Command::GetZones,
                },
            )
            .await
            .unwrap();
            write(
                &mut tx,
                &Request {
                    sequence: 2,
                    command: Command::Enable,
                },
            )
            .await
            .unwrap();
        });
        assert_eq!(read::<Request>(&mut rx).await.unwrap().sequence, 1);
        assert_eq!(read::<Request>(&mut rx).await.unwrap().sequence, 2);
        sending.await.unwrap();
        assert!(read::<Request>(&mut rx).await.is_err());
        let mut oversized = &((MAX_FRAME as u32 + 1).to_le_bytes())[..];
        assert!(read::<Request>(&mut oversized).await.is_err());
        let mut truncated = &b"\x06\0\0\0{}"[..];
        assert!(read::<Request>(&mut truncated).await.is_err());
    }
}
