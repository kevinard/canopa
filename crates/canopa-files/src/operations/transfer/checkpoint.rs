//! Stable, byte-preserving representation of an exact engine continuation.
//! The native queue does not interpret or reconstruct directory ownership.
use super::{DirectoryFinish, TransferContinuation, TransferKind, saved_stage};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Checkpoint {
    version: u8,
    kind: TransferKind,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    pending: Vec<Pending>,
    directories: Vec<Directory>,
    saved: Vec<Saved>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    copied: Vec<super::copied_leaf::Cleanup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restart: Option<super::RestartObservation>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    #[serde(with = "crate::path_serde")]
    source: PathBuf,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    finish: Option<DirectoryFinish>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Directory {
    #[serde(with = "crate::path_serde")]
    source: PathBuf,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    finish: DirectoryFinish,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    #[serde(with = "crate::path_serde")]
    source: PathBuf,
    receipt: saved_stage::Receipt,
}

impl From<TransferContinuation> for Checkpoint {
    fn from(plan: TransferContinuation) -> Self {
        Self {
            version: if plan
                .pending
                .iter()
                .any(|(_, _, finish)| finish.is_some_and(|finish| finish.was_empty))
                || plan
                    .directories
                    .values()
                    .any(|(_, finish)| finish.was_empty)
            {
                5
            } else if plan.copied.is_empty() {
                3
            } else {
                4
            },
            copied: plan.copied.into_values().collect(),
            restart: plan.restart,
            kind: plan.kind,
            destination: plan.destination,
            pending: plan
                .pending
                .into_iter()
                .map(|(source, destination, finish)| Pending {
                    source,
                    destination,
                    finish,
                })
                .collect(),
            directories: plan
                .directories
                .into_iter()
                .map(|(source, (destination, finish))| Directory {
                    source,
                    destination,
                    finish,
                })
                .collect(),
            saved: plan
                .saved
                .into_iter()
                .map(|(source, receipt)| Saved { source, receipt })
                .collect(),
        }
    }
}

impl TryFrom<Checkpoint> for TransferContinuation {
    type Error = &'static str;

    fn try_from(checkpoint: Checkpoint) -> Result<Self, Self::Error> {
        if !matches!(checkpoint.version, 1..=5)
            || (checkpoint.version == 1 && checkpoint.restart.is_some())
        {
            return Err("unsupported transfer checkpoint version");
        }
        if checkpoint.version < 3
            && (checkpoint
                .pending
                .iter()
                .any(|entry| entry.finish.is_some_and(|finish| finish.original.is_some()))
                || checkpoint
                    .directories
                    .iter()
                    .any(|entry| entry.finish.original.is_some()))
        {
            return Err("original traversal facts require checkpoint version 3");
        }
        if checkpoint.version < 5
            && (checkpoint
                .pending
                .iter()
                .any(|entry| entry.finish.is_some_and(|finish| finish.was_empty))
                || checkpoint
                    .directories
                    .iter()
                    .any(|entry| entry.finish.was_empty))
        {
            return Err("empty directory facts require checkpoint version 5");
        }
        if checkpoint.version < 4 && !checkpoint.copied.is_empty() {
            return Err("copied-source cleanup requires checkpoint version 4");
        }
        if !checkpoint.destination.is_absolute() {
            return Err("transfer checkpoint locations must be absolute");
        }
        if let Some(original) = &checkpoint.restart {
            let request = super::TransferRequest {
                kind: checkpoint.kind,
                destination: checkpoint.destination.clone(),
                sources: checkpoint
                    .pending
                    .iter()
                    .map(|item| item.source.clone())
                    .collect(),
            };
            if !original.covers(&request) {
                return Err("transfer checkpoint original selection does not cover its work");
            }
        }
        let mut pending = Vec::with_capacity(checkpoint.pending.len());
        for item in checkpoint.pending {
            if !item.source.is_absolute()
                || !item.destination.is_absolute()
                || item.destination.file_name().is_none()
            {
                return Err("transfer checkpoint targets must be absolute entries");
            }
            if let Some(finish) = item.finish {
                validate_finish(finish)?;
            }
            pending.push((item.source, item.destination, item.finish));
        }
        let mut directories = BTreeMap::new();
        for item in checkpoint.directories {
            if !item.source.is_absolute() || !item.destination.is_absolute() {
                return Err("transfer checkpoint folders must be absolute");
            }
            validate_finish(item.finish)?;
            if directories
                .insert(item.source, (item.destination, item.finish))
                .is_some()
            {
                return Err("duplicate transfer checkpoint folder");
            }
        }
        let mut saved = BTreeMap::new();
        for item in checkpoint.saved {
            if !item.source.is_absolute() {
                return Err("transfer checkpoint saved locations must be absolute");
            }
            if saved.insert(item.source, item.receipt).is_some() {
                return Err("duplicate transfer checkpoint saved location");
            }
        }
        let mut copied = BTreeMap::new();
        for proof in checkpoint.copied {
            if checkpoint.kind != TransferKind::Move
                || !pending.iter().any(|(source, destination, finish)| {
                    finish.is_none() && proof.valid_for(source, destination)
                })
                || copied.insert(proof.source.path.clone(), proof).is_some()
            {
                return Err("invalid or duplicate copied-source cleanup task");
            }
        }
        Ok(Self {
            kind: checkpoint.kind,
            destination: checkpoint.destination,
            pending,
            directories,
            saved,
            copied,
            restart: checkpoint.restart,
            observed: None,
            destination_observation: None,
        })
    }
}

fn validate_finish(finish: DirectoryFinish) -> Result<(), &'static str> {
    if finish.mode & !0o777 != 0 {
        return Err("invalid transfer checkpoint folder mode");
    }
    Ok(())
}
