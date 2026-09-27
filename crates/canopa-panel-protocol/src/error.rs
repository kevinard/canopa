#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    FrameTooLarge,
    FrameTooShort,
    MissingMagic,
    Json(String),
    Io(String),
    IncompatibleVersion(u32),
    InvalidContent(String),
    UnexpectedMessage,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FrameTooLarge => write!(f, "external panel provider frame is too large"),
            Self::FrameTooShort => write!(f, "external panel provider frame is incomplete"),
            Self::MissingMagic => write!(f, "missing external panel provider frame magic"),
            Self::Json(err) => write!(f, "invalid external panel provider JSON: {err}"),
            Self::Io(err) => write!(f, "external panel provider I/O failed: {err}"),
            Self::IncompatibleVersion(version) => {
                write!(
                    f,
                    "incompatible external panel provider protocol version {version}"
                )
            }
            Self::InvalidContent(err) => write!(f, "invalid external panel content: {err}"),
            Self::UnexpectedMessage => write!(f, "unexpected external panel provider message"),
        }
    }
}

impl std::error::Error for ProtocolError {}
