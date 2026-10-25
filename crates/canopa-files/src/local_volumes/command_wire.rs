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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Outcome {
    Mounted {
        #[serde(with = "crate::path_serde")]
        root: PathBuf,
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
