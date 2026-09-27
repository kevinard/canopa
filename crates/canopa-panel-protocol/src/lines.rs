//! Bounded JSON-line framing shared by the versioned contracts.
use crate::ProtocolError;
use std::io::{BufRead, Read, Write};

fn finish(bytes: Vec<u8>, limit: usize) -> Result<Option<Vec<u8>>, ProtocolError> {
    if bytes.len() > limit {
        return Err(ProtocolError::FrameTooLarge);
    }
    if bytes.is_empty() {
        return Ok(None);
    }
    if bytes.last() != Some(&b'\n') {
        return Err(ProtocolError::FrameTooShort);
    }
    Ok(Some(bytes))
}

pub(crate) fn read(
    reader: &mut impl BufRead,
    limit: usize,
) -> Result<Option<Vec<u8>>, ProtocolError> {
    let mut bytes = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|error| ProtocolError::Io(error.to_string()))?;
    finish(bytes, limit)
}

#[cfg(feature = "tokio")]
pub(crate) async fn read_async(
    reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
    limit: usize,
) -> Result<Option<Vec<u8>>, ProtocolError> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};
    let mut bytes = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|error| ProtocolError::Io(error.to_string()))?;
    finish(bytes, limit)
}

pub(crate) fn encode(
    value: &impl serde::Serialize,
    limit: usize,
) -> Result<Vec<u8>, ProtocolError> {
    struct Bounded {
        bytes: Vec<u8>,
        limit: usize,
        exceeded: bool,
    }
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                self.exceeded = true;
                return Err(std::io::Error::other("frame limit"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut out = Bounded {
        bytes: Vec::new(),
        limit: limit - 1,
        exceeded: false,
    };
    if serde_json::to_writer(&mut out, value).is_err() {
        return Err(if out.exceeded {
            ProtocolError::FrameTooLarge
        } else {
            ProtocolError::Json("invalid panel message".into())
        });
    }
    out.bytes.push(b'\n');
    Ok(out.bytes)
}
