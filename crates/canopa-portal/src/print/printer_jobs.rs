//! Durable printer intent is distinct from a scheduler's reusable numeric ID.
use super::worker::Destination;
use serde::{Deserialize, Serialize};
use std::io;

pub(crate) const MAX_JOBS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Server {
    pub name: Vec<u8>,
    pub port: i32,
    pub encryption: i32,
    pub user: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Intent {
    pub key: String,
    pub server: Server,
    pub destination: Destination,
    pub name: String,
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid printer recovery record",
    )
}

impl Intent {
    pub fn new(server: Server, destination: Destination, title: &str) -> io::Result<Self> {
        let mut bytes = [0; 16];
        if rustix::rand::getrandom(&mut bytes, rustix::rand::GetRandomFlags::empty())?
            != bytes.len()
        {
            return Err(io::Error::other("Printer recovery randomness unavailable"));
        }
        let key = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let mut intent = Self {
            key,
            server,
            destination,
            name: String::new(),
        };
        intent.name = format!("[Canopa:{}] ", intent.key);
        // IPP names have a 255-byte limit. Keep the complete recovery tag and
        // truncate only the display title, at a UTF-8 boundary.
        for character in title.chars().map(|c| if c.is_control() { ' ' } else { c }) {
            if intent.name.len() + character.len_utf8() > 250 {
                break;
            }
            intent.name.push(character);
        }
        intent.validate()?;
        Ok(intent)
    }

    pub fn validate(&self) -> io::Result<()> {
        let server = &self.server;
        if self.key.len() != 32
            || !self
                .key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !self.name.starts_with(&format!("[Canopa:{}] ", self.key))
            || self.name.len() > 250
            || self.name.chars().any(char::is_control)
            || server.name.is_empty()
            || server.name.len() > 4096
            || server.name.contains(&0)
            || server.user.is_empty()
            || server.user.len() > 255
            || server.user.contains(&0)
            || !(1..=65535).contains(&server.port)
            || !(0..=3).contains(&server.encryption)
            || self.destination.validate().is_err()
        {
            return Err(invalid());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub intent: Intent,
    pub job: Option<i32>,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}
pub(crate) fn valid_uuid(value: &str) -> bool {
    let Some(value) = value.strip_prefix("urn:uuid:") else {
        return false;
    };
    value.len() == 36
        && value.bytes().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
impl Receipt {
    pub fn validate(&self) -> io::Result<()> {
        self.intent.validate()?;
        if self.job.is_some_and(|id| id <= 0)
            || (self.accepted && self.job.is_none())
            || self
                .uuid
                .as_ref()
                .is_some_and(|uuid| self.job.is_none() || !valid_uuid(uuid))
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub(crate) fn revision(&self) -> io::Result<[u8; 32]> {
        use sha2::{Digest, Sha256};
        Ok(Sha256::digest(serde_json::to_vec(self)?).into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Busy,
    Unconfirmed,
    Accepted,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewItem {
    pub id: String,
    pub title: String,
    pub destination: Destination,
    pub server: String,
    pub job: Option<i32>,
    pub status: ReviewStatus,
    pub(crate) revision: [u8; 32],
}
impl ReviewItem {
    pub(crate) fn new(receipt: &Receipt, status: ReviewStatus) -> io::Result<Self> {
        receipt.validate()?;
        Ok(Self {
            id: receipt.intent.key.clone(),
            title: receipt
                .intent
                .name
                .strip_prefix(&format!("[Canopa:{}] ", receipt.intent.key))
                .expect("validated printer intent prefix")
                .to_owned(),
            destination: receipt.intent.destination.clone(),
            server: String::from_utf8_lossy(&receipt.intent.server.name)
                .chars()
                .map(|c| if c.is_control() { '�' } else { c })
                .collect(),
            job: receipt.job,
            status,
            revision: receipt.revision()?,
        })
    }

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.id.len() == 32
                && self
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                && self.title.len() <= 250
                && !self.title.chars().any(char::is_control)
                && !self.server.is_empty()
                && self.server.len() <= 12288
                && !self.server.chars().any(char::is_control)
                && self.job.is_none_or(|id| id > 0)
                && (self.status != ReviewStatus::Accepted || self.job.is_some()),
            "Invalid printer review item"
        );
        self.destination.validate()
    }

    pub(crate) fn dismissible(&self) -> bool {
        matches!(
            self.status,
            ReviewStatus::Unconfirmed | ReviewStatus::Accepted
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub retired: u32,
    pub unresolved: u32,
    pub busy: u32,
}
impl Report {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            u64::from(self.retired) + u64::from(self.unresolved) + u64::from(self.busy)
                <= MAX_JOBS as u64,
            "Invalid printer recovery response"
        );
        Ok(())
    }
}
