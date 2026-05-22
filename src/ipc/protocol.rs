pub(crate) const FRAMED_MAGIC: &str = "NIRIBAR_IPC_JSON_V1\n";
pub(crate) const MAX_FRAME_BYTES: usize = 1_048_576;
pub(crate) const MAX_DMENU_ENTRIES: usize = 10_000;
pub(crate) const MAX_DMENU_ENTRY_BYTES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolError {
    EmptyDelimiter,
    UnknownEscape(char),
    TooManyEntries,
    EntryTooLarge,
    FrameTooLarge,
    FrameTooShort,
    MissingMagic,
    Json(String),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDelimiter => write!(f, "delimiter must not be empty"),
            Self::UnknownEscape(ch) => write!(f, "unknown delimiter escape \\{ch}"),
            Self::TooManyEntries => write!(f, "too many dmenu entries"),
            Self::EntryTooLarge => write!(f, "dmenu entry is too large"),
            Self::FrameTooLarge => write!(f, "IPC frame is too large"),
            Self::FrameTooShort => write!(f, "IPC frame is incomplete"),
            Self::MissingMagic => write!(f, "missing IPC frame magic"),
            Self::Json(err) => write!(f, "invalid IPC JSON: {err}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcRequest {
    DmenuSelection(DmenuRequest),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DmenuRequest {
    pub prompt: Option<String>,
    pub entries: Vec<String>,
    pub allow_custom: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcResponse {
    Selected { value: String },
    Cancelled,
    Rejected { reason: String },
}

pub(crate) fn decode_delimiter(input: &str) -> Result<String, ProtocolError> {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(escaped) = chars.next() else {
            return Err(ProtocolError::UnknownEscape('\\'));
        };
        match escaped {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            other => return Err(ProtocolError::UnknownEscape(other)),
        }
    }
    if out.is_empty() {
        return Err(ProtocolError::EmptyDelimiter);
    }
    Ok(out)
}

pub(crate) fn split_entries(input: &str, delimiter: &str) -> Result<Vec<String>, ProtocolError> {
    if delimiter.is_empty() {
        return Err(ProtocolError::EmptyDelimiter);
    }
    if input.is_empty() {
        return Ok(Vec::new());
    }
    Ok(input
        .split(delimiter)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect())
}

pub(crate) fn validate_dmenu_request(request: &DmenuRequest) -> Result<(), ProtocolError> {
    if request.entries.len() > MAX_DMENU_ENTRIES {
        return Err(ProtocolError::TooManyEntries);
    }
    if request
        .entries
        .iter()
        .any(|entry| entry.len() > MAX_DMENU_ENTRY_BYTES)
    {
        return Err(ProtocolError::EntryTooLarge);
    }
    Ok(())
}

pub(crate) fn encode_frame<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    let payload = serde_json::to_vec(value).map_err(|err| ProtocolError::Json(err.to_string()))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let mut frame = Vec::with_capacity(FRAMED_MAGIC.len() + 8 + payload.len());
    frame.extend_from_slice(FRAMED_MAGIC.as_bytes());
    frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub(crate) fn decode_frame_payload(frame: &[u8]) -> Result<IpcRequest, ProtocolError> {
    if !frame.starts_with(FRAMED_MAGIC.as_bytes()) {
        return Err(ProtocolError::MissingMagic);
    }
    let len_start = FRAMED_MAGIC.len();
    let len_end = len_start + 8;
    if frame.len() < len_end {
        return Err(ProtocolError::FrameTooShort);
    }
    let mut len_bytes = [0_u8; 8];
    len_bytes.copy_from_slice(&frame[len_start..len_end]);
    let payload_len = u64::from_be_bytes(len_bytes) as usize;
    if payload_len > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let payload_end = len_end + payload_len;
    if frame.len() < payload_end {
        return Err(ProtocolError::FrameTooShort);
    }
    serde_json::from_slice(&frame[len_end..payload_end])
        .map_err(|err| ProtocolError::Json(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_delimiter_escapes_common_sequences() {
        assert_eq!(decode_delimiter(r"\n").unwrap(), "\n");
        assert_eq!(decode_delimiter(r"\t").unwrap(), "\t");
        assert_eq!(decode_delimiter(r"\0").unwrap(), "\0");
        assert_eq!(decode_delimiter(r"\\").unwrap(), "\\");
        assert_eq!(decode_delimiter(r",").unwrap(), ",");
    }

    #[test]
    fn split_entries_preserves_duplicates_and_empty_input() {
        assert_eq!(
            split_entries("one\ntwo\none", "\n").unwrap(),
            vec!["one".to_string(), "two".to_string(), "one".to_string()]
        );
        assert_eq!(split_entries("", "\n").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn split_entries_rejects_empty_delimiter() {
        let err = split_entries("abc", "").unwrap_err();
        assert_eq!(err, ProtocolError::EmptyDelimiter);
    }

    #[test]
    fn validate_request_rejects_too_many_entries() {
        let req = DmenuRequest {
            prompt: None,
            entries: vec!["x".to_string(); MAX_DMENU_ENTRIES + 1],
            allow_custom: true,
        };
        assert_eq!(
            validate_dmenu_request(&req).unwrap_err(),
            ProtocolError::TooManyEntries
        );
    }

    #[test]
    fn frame_roundtrip_preserves_dmenu_request() {
        let request = IpcRequest::DmenuSelection(DmenuRequest {
            prompt: Some("Run".to_string()),
            entries: vec!["firefox".to_string(), "kitty".to_string()],
            allow_custom: false,
        });
        let frame = encode_frame(&request).unwrap();
        assert!(frame.starts_with(FRAMED_MAGIC.as_bytes()));
        assert_eq!(decode_frame_payload(&frame).unwrap(), request);
    }
}
