//! Internal acknowledged keyboard transport. The compositor resolves symbols.
use super::*;
use crate::screencast::remote::KeyboardInput;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const COMMAND_BYTES: usize = 16;
const REPLY_BYTES: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Accepted,
    Invalid,
    Unavailable,
    Conflict,
    Limit,
}
impl Status {
    fn parse(value: u32) -> anyhow::Result<Self> {
        Ok(match value {
            0 => Self::Accepted,
            1 => Self::Invalid,
            2 => Self::Unavailable,
            3 => Self::Conflict,
            4 => Self::Limit,
            _ => anyhow::bail!("Invalid keyboard acknowledgement"),
        })
    }
}

fn parts(input: KeyboardInput) -> anyhow::Result<(u32, u32, bool)> {
    match input {
        KeyboardInput::Keycode { code, pressed } if (1..=0x2ff).contains(&code) => {
            Ok((0, code, pressed))
        }
        KeyboardInput::Keysym { symbol, pressed } if (1..=0x1fff_ffff).contains(&symbol) => {
            Ok((1, symbol, pressed))
        }
        _ => anyhow::bail!("Invalid keyboard input"),
    }
}
fn command(sequence: u32, input: KeyboardInput) -> anyhow::Result<[u8; COMMAND_BYTES]> {
    let (kind, value, pressed) = parts(input)?;
    let mut bytes = [0; COMMAND_BYTES];
    for (chunk, value) in bytes
        .chunks_exact_mut(4)
        .zip([sequence, kind, value, u32::from(pressed)])
    {
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}
fn words<const N: usize>(bytes: &[u8]) -> [u32; N] {
    let mut words = [0; N];
    for (word, chunk) in words.iter_mut().zip(bytes.chunks_exact(4)) {
        *word = u32::from_le_bytes(chunk.try_into().expect("fixed keyboard frame"));
    }
    words
}
fn reply(bytes: &[u8; REPLY_BYTES]) -> anyhow::Result<(u32, Status)> {
    let [sequence, status, ..] = words::<6>(bytes);
    let status = Status::parse(status)?;
    anyhow::ensure!(
        sequence != 0 || status == Status::Accepted,
        "Invalid keyboard feedback"
    );
    Ok((sequence, status))
}

pub struct LegacyInput {
    stream: tokio::net::UnixStream,
    lease: InputLease,
    sequence: u32,
    failed: bool,
}
impl LegacyInput {
    pub(super) fn new(fd: OwnedFd, lease: InputLease) -> anyhow::Result<Self> {
        let stream = UnixStream::from(fd);
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream: tokio::net::UnixStream::from_std(stream)?,
            lease,
            sequence: 0,
            failed: false,
        })
    }
    pub fn closed(&self) -> watch::Receiver<bool> {
        if self.failed {
            return watch::channel(true).1;
        }
        self.lease.closed()
    }
    pub async fn send(&mut self, input: KeyboardInput) -> anyhow::Result<Status> {
        anyhow::ensure!(
            !self.failed && !*self.lease.closed().borrow(),
            "Keyboard input ended"
        );
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.failed = true;
            self.lease.revoke();
            anyhow::bail!("Keyboard sequence exhausted");
        };
        let bytes = command(sequence, input)?;
        self.sequence = sequence;
        let result = tokio::time::timeout(CALL_TIME, async {
            self.stream.write_all(&bytes).await?;
            for _ in 0..64 {
                let mut bytes = [0; REPLY_BYTES];
                self.stream.read_exact(&mut bytes).await?;
                let (received, status) = reply(&bytes)?;
                if received == 0 {
                    continue;
                }
                anyhow::ensure!(
                    received == sequence,
                    "Keyboard acknowledgement belongs to another command"
                );
                return Ok::<_, anyhow::Error>(status);
            }
            anyhow::bail!("Too much keyboard feedback before acknowledgement")
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|result| result);
        if result.is_err() {
            self.failed = true;
            self.lease.revoke();
        }
        result
    }
    pub async fn stop(self) -> anyhow::Result<()> {
        drop(self.stream);
        self.lease.stop().await
    }
}

async fn write(
    stream: &mut tokio::net::UnixStream,
    bytes: &[u8],
    stop: &mut watch::Receiver<bool>,
) -> anyhow::Result<bool> {
    if *stop.borrow() {
        return Ok(false);
    }
    tokio::select! {
        biased;
        _ = stop.changed() => Ok(false),
        result = tokio::time::timeout(Duration::from_secs(2), stream.write_all(bytes)) => {
            result??;
            Ok(true)
        }
    }
}

/// At most one pending command. Fixed buffers survive cancellation of read;
/// revocation is observed between reads and during blocked writes.
pub(super) async fn bridge(
    client: UnixStream,
    compositor: &mut tokio::net::UnixStream,
    stop: &mut watch::Receiver<bool>,
) -> anyhow::Result<()> {
    client.set_nonblocking(true)?;
    let mut client = tokio::net::UnixStream::from_std(client)?;
    let mut command = [0; COMMAND_BYTES];
    let mut command_filled = 0;
    let mut response = [0; REPLY_BYTES];
    let mut response_filled = 0;
    let mut sequence = 0_u32;
    let mut pending = None;
    loop {
        if *stop.borrow() {
            return Ok(());
        }
        tokio::select! {
            biased;
            _ = stop.changed() => return Ok(()),
            result = compositor.read(&mut response[response_filled..]) => {
                let count = result?;
                if count == 0 {
                    anyhow::ensure!(response_filled == 0, "Truncated compositor keyboard reply");
                    return Ok(());
                }
                response_filled += count;
                if response_filled == REPLY_BYTES {
                    let (received, _) = reply(&response)?;
                    if received != 0 {
                        anyhow::ensure!(pending == Some(received), "Unexpected keyboard acknowledgement");
                        pending = None;
                    }
                    if !write(&mut client, &response, stop).await? { return Ok(()); }
                    response_filled = 0;
                }
            },
            result = client.read(&mut command[command_filled..]), if pending.is_none() => {
                let count = result?;
                if count == 0 {
                    anyhow::ensure!(command_filled == 0, "Truncated keyboard command");
                    return Ok(());
                }
                command_filled += count;
                if command_filled == COMMAND_BYTES {
                    let [received, kind, value, state] = words::<4>(&command);
                    anyhow::ensure!(sequence.checked_add(1) == Some(received) && state <= 1, "Invalid keyboard command sequence/state");
                    parts(match kind {
                        0 => KeyboardInput::Keycode { code: value, pressed: state == 1 },
                        1 => KeyboardInput::Keysym { symbol: value, pressed: state == 1 },
                        _ => anyhow::bail!("Invalid keyboard input class"),
                    })?;
                    if !write(compositor, &command, stop).await? { return Ok(()); }
                    sequence = received;
                    pending = Some(received);
                    command_filled = 0;
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(sequence: u32, status: u32) -> [u8; 24] {
        let mut bytes = [0; 24];
        bytes[..4].copy_from_slice(&sequence.to_le_bytes());
        bytes[4..8].copy_from_slice(&status.to_le_bytes());
        bytes
    }
    async fn read(stream: &mut tokio::net::UnixStream, bytes: &mut [u8]) {
        tokio::time::timeout(Duration::from_secs(3), stream.read_exact(bytes))
            .await
            .unwrap()
            .unwrap();
    }
    #[test]
    fn input_and_acknowledgement_bounds_are_exact() {
        for input in [
            KeyboardInput::Keycode {
                code: 0,
                pressed: false,
            },
            KeyboardInput::Keycode {
                code: 0x300,
                pressed: true,
            },
            KeyboardInput::Keysym {
                symbol: 0,
                pressed: true,
            },
            KeyboardInput::Keysym {
                symbol: 0x2000_0000,
                pressed: false,
            },
        ] {
            assert!(command(1, input).is_err());
        }
        for status in 0..=4 {
            assert!(reply(&response(1, status)).is_ok());
        }
        assert!(reply(&response(1, 5)).is_err());
        assert!(reply(&response(0, 1)).is_err());
    }
    #[tokio::test]
    async fn bridge_preserves_partial_frames_and_observes_revocation_with_a_pending_ack() {
        let (worker, client) = UnixStream::pair().unwrap();
        client.set_nonblocking(true).unwrap();
        let mut client = tokio::net::UnixStream::from_std(client).unwrap();
        let (mut source, mut peer) = tokio::net::UnixStream::pair().unwrap();
        let (stop, mut stopped) = watch::channel(false);
        let worker = tokio::spawn(async move { bridge(worker, &mut source, &mut stopped).await });
        let input = command(
            1,
            KeyboardInput::Keysym {
                symbol: 0x41,
                pressed: true,
            },
        )
        .unwrap();
        client.write_all(&input[..3]).await.unwrap();
        peer.write_all(&response(0, 0)).await.unwrap();
        let mut packet = [0; 24];
        read(&mut client, &mut packet).await;
        client.write_all(&input[3..]).await.unwrap();
        let mut received = [0; 16];
        read(&mut peer, &mut received).await;
        assert_eq!(received, input);
        let ack = response(1, 2);
        peer.write_all(&ack[..5]).await.unwrap();
        peer.write_all(&ack[5..]).await.unwrap();
        read(&mut client, &mut packet).await;
        assert_eq!(packet, ack);
        client
            .write_all(
                &command(
                    2,
                    KeyboardInput::Keycode {
                        code: 42,
                        pressed: true,
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap();
        read(&mut peer, &mut received).await;
        stop.send_replace(true);
        tokio::time::timeout(Duration::from_secs(1), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
    #[tokio::test]
    async fn corrupt_or_truncated_compositor_replies_retire_the_bridge() {
        for bytes in [
            response(5, 0).to_vec(),
            response(0, 1).to_vec(),
            response(1, 5).to_vec(),
            vec![0; 7],
        ] {
            let (worker, _client) = UnixStream::pair().unwrap();
            let (mut source, mut peer) = tokio::net::UnixStream::pair().unwrap();
            let (_stop, mut stopped) = watch::channel(false);
            let worker =
                tokio::spawn(async move { bridge(worker, &mut source, &mut stopped).await });
            peer.write_all(&bytes).await.unwrap();
            peer.shutdown().await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_secs(1), worker)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_err()
            );
        }
    }
    #[tokio::test]
    async fn a_clean_source_eof_during_a_pending_command_is_orderly_retirement() {
        let (worker, client) = UnixStream::pair().unwrap();
        client.set_nonblocking(true).unwrap();
        let mut client = tokio::net::UnixStream::from_std(client).unwrap();
        let (mut source, mut peer) = tokio::net::UnixStream::pair().unwrap();
        let (_stop, mut stopped) = watch::channel(false);
        let worker = tokio::spawn(async move { bridge(worker, &mut source, &mut stopped).await });
        client
            .write_all(
                &command(
                    1,
                    KeyboardInput::Keycode {
                        code: 42,
                        pressed: true,
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap();
        read(&mut peer, &mut [0; 16]).await;
        peer.shutdown().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(client.read(&mut [0; 1]).await.unwrap(), 0);
    }
    #[tokio::test]
    async fn invalid_or_partial_client_commands_are_never_forwarded() {
        for values in [
            vec![0u32, 0, 42, 1],
            vec![1, 2, 42, 1],
            vec![1, 0, 0, 1],
            vec![1, 1, 0, 1],
            vec![1, 0, 42, 2],
            vec![1],
        ] {
            let (worker, client) = UnixStream::pair().unwrap();
            client.set_nonblocking(true).unwrap();
            let mut client = tokio::net::UnixStream::from_std(client).unwrap();
            let (mut source, mut peer) = tokio::net::UnixStream::pair().unwrap();
            let (_stop, mut stopped) = watch::channel(false);
            let worker =
                tokio::spawn(async move { bridge(worker, &mut source, &mut stopped).await });
            let bytes: Vec<_> = values.into_iter().flat_map(u32::to_le_bytes).collect();
            client.write_all(&bytes).await.unwrap();
            client.shutdown().await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_secs(1), worker)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_err()
            );
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), peer.read(&mut [0; 16]))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        }
    }
}
