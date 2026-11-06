//! Validate the original compositor's finite stream before it becomes app signals.
use super::{Commit, Notice, Snapshot};
use anyhow::{Result, bail, ensure};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};
use tokio::net::UnixStream;

pub const MAX_FRAME: usize = 32 * 1024;
#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Reply {
        sequence: u64,
        generation: u64,
        error: Option<String>,
        bindings: Vec<(u32, String)>,
        failed: Vec<u32>,
    },
    Changed {
        generation: u64,
        bindings: Vec<(u32, String)>,
        failed: Vec<u32>,
    },
    Activated {
        generation: u64,
        binding: u32,
        timestamp_ms: u64,
    },
    Deactivated {
        generation: u64,
        binding: u32,
        timestamp_ms: u64,
    },
}
#[derive(Default)]
pub struct Reader {
    header: [u8; 4],
    header_bytes: usize,
    payload: Vec<u8>,
    payload_bytes: usize,
}
impl Reader {
    pub fn next(&mut self, stream: &UnixStream) -> Result<Option<Event>> {
        loop {
            let target = if self.header_bytes < 4 {
                &mut self.header[self.header_bytes..]
            } else {
                &mut self.payload[self.payload_bytes..]
            };
            match stream.try_read(target) {
                Ok(0) => bail!("Shortcut compositor closed"),
                Ok(n) if self.header_bytes < 4 => {
                    self.header_bytes += n;
                    if self.header_bytes == 4 {
                        let length = u32::from_le_bytes(self.header) as usize;
                        ensure!(
                            length > 0 && length <= MAX_FRAME,
                            "Invalid shortcut frame length"
                        );
                        self.payload.resize(length, 0);
                    }
                }
                Ok(n) => {
                    self.payload_bytes += n;
                    if self.payload_bytes == self.payload.len() {
                        let event = serde_json::from_slice(&self.payload)?;
                        self.header_bytes = 0;
                        self.payload_bytes = 0;
                        self.payload.clear();
                        return Ok(Some(event));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}
pub fn frame(sequence: u64, command: serde_json::Value) -> Result<Vec<u8>> {
    ensure!(sequence != 0, "Invalid shortcut sequence");
    let payload = serde_json::to_vec(&serde_json::json!({"sequence":sequence,"command":command}))?;
    ensure!(
        !payload.is_empty() && payload.len() <= MAX_FRAME,
        "Shortcut command exceeds limits"
    );
    let mut bytes = (payload.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}
pub fn bindings(values: Vec<(u32, String)>) -> Result<BTreeMap<u32, String>> {
    ensure!(
        values.len() <= super::super::MAX_ACTIONS,
        "Too many compositor shortcuts"
    );
    let mut parsed = BTreeMap::new();
    for (id, trigger) in values {
        ensure!(
            id != 0
                && !trigger.is_empty()
                && trigger.len() <= 256
                && !trigger.chars().any(char::is_control),
            "Invalid compositor shortcut"
        );
        ensure!(
            parsed.insert(id, trigger).is_none(),
            "Repeated compositor shortcut"
        );
    }
    Ok(parsed)
}
fn failures(values: Vec<u32>, proposed: &BTreeMap<u32, String>) -> Result<BTreeSet<u32>> {
    ensure!(
        values.len() <= super::super::MAX_ACTIONS,
        "Too many shortcut refusals"
    );
    let mut ids = BTreeSet::new();
    for id in values {
        ensure!(
            proposed.contains_key(&id) && ids.insert(id),
            "Invalid shortcut refusal"
        );
    }
    Ok(ids)
}
pub struct Ledger {
    pub snapshot: Snapshot,
    approved: BTreeMap<u32, String>,
    active: BTreeSet<u32>,
}
impl Ledger {
    pub fn initial(snapshot: Snapshot) -> Result<Self> {
        ensure!(
            snapshot.bindings.is_empty(),
            "A fresh shortcut lease has authority"
        );
        Ok(Self {
            snapshot,
            approved: BTreeMap::new(),
            active: BTreeSet::new(),
        })
    }
    pub fn event(&mut self, event: Event) -> Result<Notice> {
        match event {
            Event::Changed {
                generation,
                bindings: values,
                failed,
            } => {
                let bindings = bindings(values)?;
                let failed = failures(failed, &self.approved)?;
                ensure!(
                    generation > self.snapshot.generation && self.active.is_empty(),
                    "Invalid shortcut generation retirement"
                );
                ensure!(
                    bindings
                        .iter()
                        .all(|(id, trigger)| self.approved.get(id) == Some(trigger)),
                    "Unapproved compositor shortcut"
                );
                let absent: BTreeSet<_> = self
                    .approved
                    .keys()
                    .filter(|id| !bindings.contains_key(id))
                    .copied()
                    .collect();
                ensure!(
                    failed == absent,
                    "Shortcut refusal set differs from active bindings"
                );
                self.snapshot = Snapshot {
                    generation,
                    bindings,
                };
                Ok(Notice::Changed(self.snapshot.clone()))
            }
            Event::Activated {
                generation,
                binding,
                timestamp_ms,
            } => {
                ensure!(
                    generation == self.snapshot.generation
                        && self.snapshot.bindings.contains_key(&binding)
                        && self.active.insert(binding),
                    "Invalid shortcut activation"
                );
                Ok(Notice::Activated {
                    generation,
                    binding,
                    timestamp_ms,
                })
            }
            Event::Deactivated {
                generation,
                binding,
                timestamp_ms,
            } => {
                ensure!(
                    generation == self.snapshot.generation && self.active.remove(&binding),
                    "Unpaired shortcut deactivation"
                );
                Ok(Notice::Deactivated {
                    generation,
                    binding,
                    timestamp_ms,
                })
            }
            Event::Reply { .. } => bail!("Unsolicited shortcut reply"),
        }
    }
    pub fn reply(
        &mut self,
        event: Event,
        sequence: u64,
        expected: u64,
        proposed: &BTreeMap<u32, String>,
    ) -> Result<Commit> {
        let Event::Reply {
            sequence: actual,
            generation,
            error,
            bindings: values,
            failed,
        } = event
        else {
            bail!("Missing shortcut reply");
        };
        ensure!(actual == sequence, "Wrong shortcut reply sequence");
        let bindings = bindings(values)?;
        let failed = failures(failed, proposed)?;
        if let Some(error) = &error {
            ensure!(
                !error.is_empty()
                    && error.len() <= 128
                    && generation == self.snapshot.generation
                    && bindings == self.snapshot.bindings,
                "Refused shortcut mutation changed authority"
            );
        } else {
            ensure!(
                generation > self.snapshot.generation
                    && generation > expected
                    && expected == self.snapshot.generation
                    && bindings == *proposed
                    && failed.is_empty()
                    && self.active.is_empty(),
                "Invalid shortcut commit"
            );
            self.approved = bindings.clone();
            self.snapshot = Snapshot {
                generation,
                bindings,
            };
        }
        Ok(Commit {
            snapshot: self.snapshot.clone(),
            error,
            failed: failed.into_iter().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ready() -> Ledger {
        Ledger::initial(Snapshot {
            generation: 0,
            bindings: BTreeMap::new(),
        })
        .unwrap()
    }
    fn commit(ledger: &mut Ledger) {
        ledger
            .reply(
                Event::Reply {
                    sequence: 2,
                    generation: 1,
                    error: None,
                    bindings: vec![(7, "CTRL+a".into())],
                    failed: vec![],
                },
                2,
                0,
                &BTreeMap::from([(7, "CTRL+a".into())]),
            )
            .unwrap();
    }
    #[test]
    fn rebind_cannot_retire_a_pressed_generation_without_a_real_deactivation() {
        let mut ledger = ready();
        commit(&mut ledger);
        ledger
            .event(Event::Activated {
                generation: 1,
                binding: 7,
                timestamp_ms: 5_000_000_123,
            })
            .unwrap();
        assert!(
            ledger
                .event(Event::Changed {
                    generation: 3,
                    bindings: vec![],
                    failed: vec![7]
                })
                .is_err()
        );
        let notice = ledger
            .event(Event::Deactivated {
                generation: 1,
                binding: 7,
                timestamp_ms: 5_000_000_124,
            })
            .unwrap();
        assert!(matches!(
            notice,
            Notice::Deactivated {
                timestamp_ms: 5_000_000_124,
                ..
            }
        ));
        ledger
            .event(Event::Changed {
                generation: 3,
                bindings: vec![],
                failed: vec![7],
            })
            .unwrap();
        ledger
            .event(Event::Changed {
                generation: 5,
                bindings: vec![(7, "CTRL+a".into())],
                failed: vec![],
            })
            .unwrap();
    }
    #[test]
    fn stale_actions_unknown_ids_duplicate_activation_and_unpaired_releases_are_refused() {
        let mut ledger = ready();
        commit(&mut ledger);
        assert!(
            ledger
                .event(Event::Activated {
                    generation: 0,
                    binding: 7,
                    timestamp_ms: 1
                })
                .is_err()
        );
        assert!(
            ledger
                .event(Event::Activated {
                    generation: 1,
                    binding: 8,
                    timestamp_ms: 1
                })
                .is_err()
        );
        ledger
            .event(Event::Activated {
                generation: 1,
                binding: 7,
                timestamp_ms: 1,
            })
            .unwrap();
        assert!(
            ledger
                .event(Event::Activated {
                    generation: 1,
                    binding: 7,
                    timestamp_ms: 2
                })
                .is_err()
        );
        assert!(
            ledger
                .event(Event::Deactivated {
                    generation: 1,
                    binding: 8,
                    timestamp_ms: 3
                })
                .is_err()
        );
    }
    #[test]
    fn error_replies_preserve_live_authority_and_changed_cannot_invent_an_approved_action() {
        let mut ledger = ready();
        commit(&mut ledger);
        let proposed = BTreeMap::from([(8, "F9".into())]);
        let reply = ledger
            .reply(
                Event::Reply {
                    sequence: 3,
                    generation: 1,
                    error: Some("conflict".into()),
                    bindings: vec![(7, "CTRL+a".into())],
                    failed: vec![8],
                },
                3,
                1,
                &proposed,
            )
            .unwrap();
        assert_eq!(reply.snapshot.bindings, ledger.snapshot.bindings);
        assert!(
            ledger
                .event(Event::Changed {
                    generation: 2,
                    bindings: vec![(8, "F9".into())],
                    failed: vec![7]
                })
                .is_err()
        );
        assert!(
            ledger
                .reply(
                    Event::Reply {
                        sequence: 4,
                        generation: 2,
                        error: Some("conflict".into()),
                        bindings: vec![],
                        failed: vec![8]
                    },
                    4,
                    1,
                    &proposed
                )
                .is_err()
        );
    }
}
