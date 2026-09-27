//! A bounded, validated replacement frame, usable by synchronous providers and
//! asynchronous hosts. Keep original bytes for forwarding without re-encoding.
use super::{MAX_FRAME_BYTES, PROTOCOL_VERSION, PanelContent, ProviderResponse};
use crate::ProtocolError;
use std::io::BufRead;

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
        crate::lines::read(reader, MAX_FRAME_BYTES)?
            .map(Self::decode)
            .transpose()
    }

    /// As with other stream reads, retain the read future across unrelated
    /// updates. Cancelling a partially consumed frame loses its prefix.
    #[cfg(feature = "tokio")]
    pub async fn read_async(
        reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
    ) -> Result<Option<Self>, ProtocolError> {
        crate::lines::read_async(reader, MAX_FRAME_BYTES)
            .await?
            .map(Self::decode)
            .transpose()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    pub fn into_content(self) -> PanelContent {
        self.content
    }
}
