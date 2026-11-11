//! Private helper traffic. Secrets travel only in answers, never in observations.
use super::*;

pub(super) const INPUT_LIMIT: u64 = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub version: u8,
    pub id: u64,
    pub command: Command,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Command {
    Mount {
        instance: u64,
        generation: u64,
    },
    Remove {
        instance: u64,
        generation: u64,
        removal: Removal,
    },
    Cancel,
    Answer {
        question: u64,
        answer: Answer,
    },
    /// Mount the location enclosing a validated server address.
    Connect {
        uri: String,
    },
    /// List a GVFS location's children without a local folder: network
    /// discovery (`network:///`) and the listing of a mount without FUSE.
    /// Read-only and independent of the single active device request.
    Enumerate {
        uri: String,
    },
}

/// What GIO may do with a password the user entered. Only
/// `G_ASK_PASSWORD_SAVING_SUPPORTED` questions accept more than `Never`;
/// GVFS then stores it in the Secret Service keyring. Files never does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Remember {
    #[default]
    Never,
    ForSession,
    Permanently,
}

impl Remember {
    pub fn label(self) -> &'static str {
        match self {
            Self::Never => "Forget the password immediately",
            Self::ForSession => "Remember it until you log out",
            Self::Permanently => "Remember it in the keyring",
        }
    }

    pub(super) fn platform(self) -> gio::PasswordSave {
        match self {
            Self::Never => gio::PasswordSave::Never,
            Self::ForSession => gio::PasswordSave::ForSession,
            Self::Permanently => gio::PasswordSave::Permanently,
        }
    }
}

/// One child of an enumerated GVFS location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteEntry {
    pub name: String,
    /// The child's location, or its shortcut target (`standard::target-uri`).
    pub uri: String,
    pub directory: bool,
    pub size: Option<u64>,
}

pub(super) const ENUMERATION_LIMIT: usize = 2000;

pub(super) fn valid_entry(entry: &RemoteEntry) -> bool {
    !entry.name.is_empty()
        && entry.name.len() <= 4096
        && entry.uri.len() <= 4096
        && !entry.uri.chars().any(char::is_control)
}

// Serialization is only for the private pipe. Deliberately no Debug or Clone:
// credential answers must never enter logs, snapshots or saved records.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Answer {
    Password {
        username: String,
        domain: String,
        password: String,
        anonymous: bool,
        #[serde(default)]
        pim: u32,
        #[serde(default)]
        hidden_volume: bool,
        #[serde(default)]
        system_volume: bool,
        #[serde(default)]
        save: Remember,
    },
    Choice {
        choice: usize,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Question {
    Password {
        message: String,
        username: String,
        domain: String,
        flags: u32,
    },
    Choice {
        message: String,
        choices: Vec<String>,
        blocked: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PasswordFields {
    pub username: bool,
    pub domain: bool,
    pub password: bool,
    pub anonymous: bool,
    pub tcrypt: bool,
    /// The backend can keep the password (Remember choices are offered).
    pub save: bool,
}

impl Question {
    pub fn password_fields(&self) -> Option<PasswordFields> {
        let Self::Password { flags, .. } = self else {
            return None;
        };
        let has = |flag: gio::AskPasswordFlags| flags & flag.bits() != 0;
        Some(PasswordFields {
            username: has(gio::AskPasswordFlags::NEED_USERNAME),
            domain: has(gio::AskPasswordFlags::NEED_DOMAIN),
            password: has(gio::AskPasswordFlags::NEED_PASSWORD),
            anonymous: has(gio::AskPasswordFlags::ANONYMOUS_SUPPORTED),
            tcrypt: has(gio::AskPasswordFlags::TCRYPT),
            save: has(gio::AskPasswordFlags::SAVING_SUPPORTED),
        })
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct Event {
    pub version: u8,
    pub id: u64,
    #[serde(flatten)]
    pub event: Update,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub(super) enum Update {
    Question {
        question: u64,
        prompt: Option<Question>,
    },
    Finished {
        outcome: Outcome,
    },
    Refused {
        message: String,
        terminal: bool,
    },
    Enumerated {
        entries: Vec<RemoteEntry>,
        error: Option<String>,
        truncated: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Outcome {
    Mounted {
        #[serde(with = "crate::path_serde")]
        root: PathBuf,
    },
    /// A server address was mounted. `location` is the requested folder
    /// inside `root`, both GVFS FUSE paths.
    Connected {
        #[serde(with = "crate::path_serde")]
        root: PathBuf,
        #[serde(with = "crate::path_serde")]
        location: PathBuf,
    },
    /// Mounted, but GVFS offers no local folder (its FUSE daemon is not running).
    ConnectedRemote {
        uri: String,
    },
    Removed,
    Cancelled,
    Failed {
        message: String,
    },
    /// Only the parent can report loss after the exact helper has been reaped.
    /// This cannot be received from or sent to the helper.
    #[serde(skip)]
    Uncertain {
        message: String,
    },
}

pub(super) fn emit(id: u64, event: Update) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(&Event {
        version: 1,
        id,
        event,
    })
    .map_err(io::Error::other)?;
    if bytes.len() as u64 >= FRAME_LIMIT {
        return Err(io::Error::other("Device response exceeds its size limit."));
    }
    bytes.push(b'\n');
    let mut output = io::stdout().lock();
    output.write_all(&bytes)?;
    output.flush()
}
