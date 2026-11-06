//! Live scheduler evidence, independent of the interrupted-submission journal.
use super::{authentication::Requirement, printer_jobs::Server, worker::Destination};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_JOBS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Receiving,
    Queued,
    Held,
    NeedsAuthentication,
    Processing,
    Stopped,
    Cancelled,
    Aborted,
    Completed,
}

/// This proof is returned only by the isolated worker. The review host keeps it
/// privately; its Iced child receives display data and a selection key instead.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub server: Server,
    pub destination: Destination,
    pub job: i32,
    pub name: String,
    pub uuid: String,
    pub protection: i32,
    pub requirement: Requirement,
}
impl Binding {
    pub fn validate(&self) -> anyhow::Result<()> {
        let server = &self.server;
        anyhow::ensure!(
            self.job > 0
                && !self.name.is_empty()
                && self.name.len() <= 255
                && !self.name.chars().any(char::is_control)
                && super::printer_jobs::valid_uuid(&self.uuid)
                && (0..=3).contains(&self.protection)
                && !server.name.is_empty()
                && server.name.len() <= 4096
                && !server.name.contains(&0)
                && !server.user.is_empty()
                && server.user.len() <= 255
                && !server.user.contains(&0)
                && (1..=65535).contains(&server.port)
                && (0..=3).contains(&server.encryption),
            "Invalid live printer job identity"
        );
        self.destination.validate()?;
        self.requirement.validate()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub job: i32,
    pub title: String,
    pub status: Status,
    pub(crate) binding: Option<Binding>,
}
impl Job {
    pub fn can_authenticate(&self) -> bool {
        self.status == Status::NeedsAuthentication
            && self
                .binding
                .as_ref()
                .is_some_and(|binding| matches!(binding.requirement, Requirement::Fields { .. }))
    }
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.job > 0
                && !self.title.is_empty()
                && self.title.len() <= 255
                && !self.title.chars().any(char::is_control),
            "Invalid live printer job"
        );
        if let Some(binding) = &self.binding {
            binding.validate()?;
            anyhow::ensure!(binding.job == self.job, "Live job identity changed");
        }
        Ok(())
    }
    pub(crate) fn key(&self) -> anyhow::Result<String> {
        use sha2::{Digest, Sha256};
        Ok(Sha256::digest(serde_json::to_vec(self)?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

/// CUPS changes an initially unspecified requirement internally after backend
/// exit2, without necessarily publishing the new printer attribute. This default
/// applies only to a freshly confirmed held-for-authentication document.
pub(crate) fn held_requirement(requirement: Requirement) -> Requirement {
    if requirement == Requirement::None {
        Requirement::Fields {
            fields: vec![
                super::authentication::Field::Username,
                super::authentication::Field::Password,
            ],
        }
    } else {
        requirement
    }
}
