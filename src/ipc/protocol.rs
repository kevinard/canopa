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
    pub entries: Vec<DmenuEntry>,
    pub allow_custom: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DmenuEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl DmenuEntry {
    pub(crate) fn plain(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            label: Some(value.clone()),
            value: Some(value),
            icon: None,
            keywords: Vec::new(),
        }
    }

    pub(crate) fn display_label(&self) -> &str {
        self.label
            .as_deref()
            .or(self.value.as_deref())
            .unwrap_or("")
    }

    pub(crate) fn selected_value(&self) -> &str {
        self.value
            .as_deref()
            .or(self.label.as_deref())
            .unwrap_or("")
    }
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

pub(crate) fn split_dmenu_entries_plain(
    input: &str,
    delimiter: &str,
) -> Result<Vec<DmenuEntry>, ProtocolError> {
    split_entries(input, delimiter).map(|entries| {
        entries
            .into_iter()
            .map(DmenuEntry::plain)
            .collect::<Vec<_>>()
    })
}

pub(crate) fn validate_dmenu_request(request: &DmenuRequest) -> Result<(), ProtocolError> {
    if request.entries.len() > MAX_DMENU_ENTRIES {
        return Err(ProtocolError::TooManyEntries);
    }
    if request
        .entries
        .iter()
        .any(|entry| entry.display_label().len() > MAX_DMENU_ENTRY_BYTES)
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
    fn split_plain_entries_converts_to_dmenu_entries() {
        let entries = split_dmenu_entries_plain("firefox\nkitty\nfirefox", "\n").unwrap();

        assert_eq!(
            entries,
            vec![
                DmenuEntry::plain("firefox"),
                DmenuEntry::plain("kitty"),
                DmenuEntry::plain("firefox"),
            ]
        );
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
            entries: vec![DmenuEntry::plain("x"); MAX_DMENU_ENTRIES + 1],
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
            entries: vec![DmenuEntry::plain("firefox"), DmenuEntry::plain("kitty")],
            allow_custom: false,
        });
        let frame = encode_frame(&request).unwrap();
        assert!(frame.starts_with(FRAMED_MAGIC.as_bytes()));
        assert_eq!(decode_frame_payload(&frame).unwrap(), request);
    }

    #[test]
    fn frame_roundtrip_preserves_rich_dmenu_entries() {
        let request = IpcRequest::DmenuSelection(DmenuRequest {
            prompt: Some("Run".to_string()),
            entries: vec![
                DmenuEntry {
                    label: Some("Firefox".to_string()),
                    value: Some("org.mozilla.firefox".to_string()),
                    icon: Some("firefox".to_string()),
                    keywords: vec!["browser".to_string(), "web".to_string()],
                },
                DmenuEntry {
                    label: None,
                    value: Some("kitty".to_string()),
                    icon: None,
                    keywords: Vec::new(),
                },
            ],
            allow_custom: false,
        });

        let frame = encode_frame(&request).unwrap();
        assert_eq!(decode_frame_payload(&frame).unwrap(), request);
    }
}
