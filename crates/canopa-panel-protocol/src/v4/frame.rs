//! A bounded, validated replacement frame, usable by synchronous providers and
//! asynchronous hosts. Keep original bytes for forwarding without re-encoding.
use super::{MAX_FRAME_BYTES, PROTOCOL_VERSION, PanelContent, ProviderResponse};
use crate::ProtocolError;
use std::io::{BufRead, Read};

pub struct Frame {
    bytes: Vec<u8>,
    content: PanelContent,
}

impl Frame {
    fn decode(bytes: Vec<u8>) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        if bytes.last() != Some(&b'\n') {
            return Err(ProtocolError::FrameTooShort);
        }
        let ProviderResponse::ReplaceContent {
            protocol_version,
            content,
        } = serde_json::from_slice(&bytes)
            .map_err(|_| ProtocolError::Json("invalid Canopa provider message".into()))?;
        if protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::IncompatibleVersion(protocol_version.into()));
        }
        content
            .validate()
            .map_err(|_| ProtocolError::InvalidContent("invalid Canopa content".into()))?;
        Ok(Self { bytes, content })
    }

    /// Read exactly one JSON line. EOF is distinct from an incomplete frame.
    /// Callers own deadlines, backpressure and what to do on failure.
    pub fn read(reader: &mut impl BufRead) -> Result<Option<Self>, ProtocolError> {
        let mut bytes = Vec::new();
        reader
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|error| ProtocolError::Io(error.to_string()))?;
        Self::received(bytes)
    }

    /// As with other stream reads, retain the read future across unrelated
    /// updates. Cancelling a partially consumed frame loses its prefix.
    #[cfg(feature = "tokio")]
    pub async fn read_async(
        reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
    ) -> Result<Option<Self>, ProtocolError> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        let mut bytes = Vec::new();
        reader
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|error| ProtocolError::Io(error.to_string()))?;
        Self::received(bytes)
    }

    fn received(bytes: Vec<u8>) -> Result<Option<Self>, ProtocolError> {
        if bytes.is_empty() {
            Ok(None)
        } else {
            Self::decode(bytes).map(Some)
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    pub fn into_content(self) -> PanelContent {
        self.content
    }
}
