//! A bounded printer ledger shares the recovery directory's private IO rules,
//! but has its own schema and never stores or replays document bodies.
use super::*;
use crate::print::printer_jobs::{Intent, MAX_JOBS, Receipt, ReviewItem, ReviewStatus};

const LIMIT: u64 = 64 * 1024;
const NAME: &str = "printer-jobs.json";
const NEXT: &str = "printer-jobs.next";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    receipt: Receipt,
    owner: Process,
    worker: Process,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Jobs {
    entries: Vec<Entry>,
}

impl RecoveryStore {
    pub(crate) fn printer_list(&self) -> io::Result<Vec<ReviewItem>> {
        let Some(directory) = self.open(false)? else {
            return Ok(vec![]);
        };
        let _lock = match Self::lock(&directory, false) {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if any(&directory, OsStr::new(NAME))?.is_some() {
                    return Err(error);
                }
                return Ok(vec![]);
            }
            Err(error) => return Err(error),
        };
        self.jobs_load(&directory)?
            .entries
            .iter()
            .map(|entry| {
                let status = match entry.worker.alive() {
                    Ok(true) => ReviewStatus::Busy,
                    Ok(false) if entry.receipt.accepted => ReviewStatus::Accepted,
                    Ok(false) => ReviewStatus::Unconfirmed,
                    Err(_) => ReviewStatus::Unavailable,
                };
                ReviewItem::new(&entry.receipt, status)
            })
            .collect()
    }

    /// Removes only the reviewed metadata. Never contacts CUPS or touches a job.
    /// A fresh liveness and receipt check under the write lock rejects stale UI.
    pub(crate) fn printer_dismiss(&self, item: &ReviewItem) -> io::Result<()> {
        item.validate().map_err(|_| invalid())?;
        if !item.dismissible() {
            return Err(invalid());
        }
        self.jobs_change(false, |jobs| {
            let index = jobs
                .entries
                .iter()
                .position(|entry| entry.receipt.intent.key == item.id)
                .ok_or_else(invalid)?;
            let entry = &jobs.entries[index];
            if entry.worker.alive()? || entry.receipt.revision()? != item.revision {
                return Err(changed());
            }
            jobs.entries.remove(index);
            Ok(())
        })
    }

    fn jobs_load(&self, directory: &File) -> io::Result<Jobs> {
        let jobs = match read_only(directory, NAME) {
            Ok(file) => {
                private(&file, false)?;
                read_json::<Jobs>(&file, LIMIT)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Jobs::default(),
            Err(error) => return Err(error),
        };
        let mut keys = std::collections::HashSet::new();
        if jobs.entries.len() > MAX_JOBS {
            return Err(invalid());
        }
        for entry in &jobs.entries {
            entry.receipt.validate()?;
            if !keys.insert(&entry.receipt.intent.key) {
                return Err(invalid());
            }
        }
        Ok(jobs)
    }

    fn jobs_change<T>(
        &self,
        create: bool,
        update: impl FnOnce(&mut Jobs) -> io::Result<T>,
    ) -> io::Result<T> {
        let directory = self.open(create)?.ok_or_else(invalid)?;
        let _lock = Self::lock(&directory, true)?;
        let mut jobs = self.jobs_load(&directory)?;
        let result = update(&mut jobs)?;
        let bytes = serde_json::to_vec(&jobs)?;
        if bytes.len() as u64 > LIMIT || jobs.entries.len() > MAX_JOBS {
            return Err(invalid());
        }
        if let Some(stale) = any(&directory, OsStr::new(NEXT))? {
            private(&stale, false)?;
            rustix::fs::unlinkat(&directory, NEXT, AtFlags::empty())?;
        }
        let mut next = fresh(&directory, NEXT)?;
        next.write_all(&bytes)?;
        next.sync_all()?;
        rustix::fs::renameat(&directory, NEXT, &directory, NAME)?;
        directory.sync_all()?;
        let path = PathBuf::from(OsString::from_vec(self.path.clone()));
        let mut parent = path.parent();
        while let Some(path) = parent {
            File::open(path)?.sync_all()?;
            parent = path.parent();
        }
        Ok(result)
    }

    pub(crate) fn printer_begin(&self, intent: &Intent, owner: u32) -> io::Result<()> {
        intent.validate()?;
        let owner = Process::read(owner)?;
        let worker = Process::read(std::process::id())?;
        self.jobs_change(true, |jobs| {
            if !owner.alive()? || jobs.entries.len() >= MAX_JOBS {
                return Err(io::Error::other(
                    "Printer recovery list is full or owner exited",
                ));
            }
            if jobs
                .entries
                .iter()
                .any(|e| e.receipt.intent.key == intent.key)
            {
                return Err(invalid());
            }
            jobs.entries.push(Entry {
                receipt: Receipt {
                    intent: intent.clone(),
                    job: None,
                    accepted: false,
                    uuid: None,
                },
                owner,
                worker,
            });
            Ok(())
        })
    }

    /// Authentication can change the scheduler's owner before Create is retried.
    /// This must be durable before the password callback permits that request.
    pub(crate) fn printer_creation_user(&self, key: &str, user: &[u8]) -> io::Result<()> {
        self.printer_update(key, |receipt| {
            if receipt.job.is_some() || receipt.accepted {
                return Err(invalid());
            }
            receipt.intent.server.user = user.to_vec();
            receipt.validate()
        })
    }

    pub(crate) fn printer_created(&self, key: &str, job: i32) -> io::Result<()> {
        if job <= 0 {
            return Err(invalid());
        }
        self.printer_update(key, |receipt| {
            if receipt.job.is_some() || receipt.accepted {
                return Err(invalid());
            }
            receipt.job = Some(job);
            Ok(())
        })
    }

    pub(crate) fn printer_accepted(&self, key: &str, job: i32) -> io::Result<()> {
        self.printer_update(key, |receipt| {
            if receipt.job != Some(job) || receipt.accepted {
                return Err(invalid());
            }
            receipt.accepted = true;
            Ok(())
        })
    }

    pub(crate) fn printer_identified(&self, key: &str, job: i32, uuid: &str) -> io::Result<()> {
        self.printer_update(key, |receipt| {
            if receipt.job != Some(job)
                || receipt.accepted
                || receipt.uuid.as_ref().is_some_and(|old| old != uuid)
            {
                return Err(invalid());
            }
            receipt.uuid = Some(uuid.to_owned());
            receipt.validate()
        })
    }

    fn printer_update(
        &self,
        key: &str,
        update: impl FnOnce(&mut Receipt) -> io::Result<()>,
    ) -> io::Result<()> {
        let worker = Process::read(std::process::id())?;
        self.jobs_change(false, |jobs| {
            let entry = jobs
                .entries
                .iter_mut()
                .find(|entry| entry.receipt.intent.key == key)
                .ok_or_else(invalid)?;
            if entry.worker != worker {
                return Err(invalid());
            }
            update(&mut entry.receipt)
        })
    }

    /// Claim only dead workers. The claim itself is durable before any CUPS IO;
    /// another startup/cleanup worker sees this worker as busy, never a stale PID.
    pub(crate) fn printer_claim(&self) -> io::Result<(Vec<Receipt>, u32)> {
        let Some(directory) = self.open(false)? else {
            return Ok((vec![], 0));
        };
        if self.jobs_load(&directory)?.entries.is_empty() {
            return Ok((vec![], 0));
        }
        let worker = Process::read(std::process::id())?;
        self.jobs_change(false, |jobs| {
            let mut claimed = vec![];
            let mut busy = 0;
            for entry in &mut jobs.entries {
                if entry.worker.alive()? {
                    busy += 1;
                    continue;
                }
                entry.worker = worker.clone();
                claimed.push(entry.receipt.clone());
            }
            Ok((claimed, busy))
        })
    }

    pub(crate) fn printer_finish(&self, key: &str) -> io::Result<()> {
        let worker = Process::read(std::process::id())?;
        self.jobs_change(false, |jobs| {
            let index = jobs
                .entries
                .iter()
                .position(|entry| entry.receipt.intent.key == key && entry.worker == worker)
                .ok_or_else(invalid)?;
            jobs.entries.remove(index);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{printer_jobs::Server, worker::Destination};

    fn intent() -> Intent {
        Intent::new(
            Server {
                name: b"/private/cups.sock".to_vec(),
                port: 631,
                encryption: 0,
                user: b"owner".to_vec(),
            },
            Destination {
                queue: "Fixture".into(),
                instance: None,
            },
            &"📄 résumé ".repeat(100),
        )
        .unwrap()
    }

    #[test]
    fn printer_intent_survives_reopen_and_only_dead_workers_can_be_claimed() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let intent = intent();
        assert!(intent.name.len() <= 250);
        assert!(intent.name.contains("résumé"));
        store.printer_begin(&intent, std::process::id()).unwrap();
        store.printer_created(&intent.key, 7).unwrap();
        let reopened = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let (ready, busy) = reopened.printer_claim().unwrap();
        assert!(ready.is_empty());
        assert_eq!(busy, 1);
        store
            .jobs_change(false, |jobs| {
                jobs.entries[0].worker.fixture_retire();
                Ok(())
            })
            .unwrap();
        let (ready, busy) = reopened.printer_claim().unwrap();
        assert_eq!(busy, 0);
        assert_eq!(ready[0].intent, intent);
        assert_eq!(ready[0].job, Some(7));
        assert!(!ready[0].accepted);
        reopened.printer_accepted(&intent.key, 7).unwrap();
        assert!(
            reopened
                .jobs_load(&reopened.open(false).unwrap().unwrap())
                .unwrap()
                .entries[0]
                .receipt
                .accepted
        );
        reopened.printer_finish(&intent.key).unwrap();
        assert!(reopened.printer_claim().unwrap().0.is_empty());
    }

    #[test]
    fn authenticated_creation_owner_is_durable_before_id_and_frozen_after_creation() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let entry = intent();
        store.printer_begin(&entry, std::process::id()).unwrap();
        store
            .printer_creation_user(&entry.key, b"rejected-account")
            .unwrap();
        store
            .printer_creation_user(&entry.key, b"approved-account")
            .unwrap();
        let reopened = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let jobs = reopened
            .jobs_load(&reopened.open(false).unwrap().unwrap())
            .unwrap();
        let receipt = &jobs.entries[0].receipt;
        assert_eq!(receipt.intent.server.user, b"approved-account");
        let mut expected = entry.clone();
        expected.server.user = b"approved-account".to_vec();
        assert_eq!(receipt.intent, expected);
        assert_eq!(receipt.job, None);
        reopened.printer_created(&entry.key, 7).unwrap();
        let path = directory.path().join("state").join(NAME);
        let created = std::fs::read(&path).unwrap();
        assert!(
            store
                .printer_creation_user(&entry.key, b"send-account")
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), created);
        store.printer_accepted(&entry.key, 7).unwrap();
        assert!(
            store
                .printer_creation_user(&entry.key, b"late-account")
                .is_err()
        );
    }

    #[test]
    fn creation_owner_update_refuses_invalid_values_and_another_worker() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let entry = intent();
        store.printer_begin(&entry, std::process::id()).unwrap();
        let path = directory.path().join("state").join(NAME);
        let before = std::fs::read(&path).unwrap();
        for user in [b"".as_slice(), b"embedded\0user", &[b'x'; 256]] {
            assert!(store.printer_creation_user(&entry.key, user).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        store
            .jobs_change(false, |jobs| {
                jobs.entries[0].worker.fixture_retire();
                Ok(())
            })
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(
            store
                .printer_creation_user(&entry.key, b"foreign-worker")
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn printer_uuid_binding_is_durable_and_cannot_replace_another_job_or_legacy_owner() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let entry = intent();
        store.printer_begin(&entry, std::process::id()).unwrap();
        let path = directory.path().join("state").join(NAME);
        let original = std::fs::read(&path).unwrap();
        let uuid = "urn:uuid:01234567-89ab-cdef-0123-456789abcdef";
        assert!(
            !String::from_utf8(original.clone())
                .unwrap()
                .contains("uuid")
        );
        assert!(store.printer_identified(&entry.key, 7, uuid).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        store.printer_created(&entry.key, 7).unwrap();
        assert!(store.printer_identified(&entry.key, 8, uuid).is_err());
        assert!(
            store
                .printer_identified(&entry.key, 7, "not-a-uuid")
                .is_err()
        );
        store.printer_identified(&entry.key, 7, uuid).unwrap();
        let reopened = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let jobs = reopened
            .jobs_load(&reopened.open(false).unwrap().unwrap())
            .unwrap();
        assert_eq!(jobs.entries[0].receipt.uuid.as_deref(), Some(uuid));
        assert_eq!(jobs.entries[0].receipt.intent, entry);
        let identified = std::fs::read(&path).unwrap();
        assert!(
            store
                .printer_identified(
                    &entry.key,
                    7,
                    "urn:uuid:11234567-89ab-cdef-0123-456789abcdef"
                )
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), identified);
        store.printer_accepted(&entry.key, 7).unwrap();
        assert!(store.printer_identified(&entry.key, 7, uuid).is_err());
    }

    #[test]
    fn durable_printer_admission_stays_bounded_across_clients_and_reopens() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let first = intent();
        store.printer_begin(&first, std::process::id()).unwrap();
        assert!(store.printer_begin(&first, std::process::id()).is_err());
        for _ in 1..MAX_JOBS {
            store.printer_begin(&intent(), std::process::id()).unwrap();
        }
        let reopened = RecoveryStore::new(&directory.path().join("state")).unwrap();
        assert!(
            reopened
                .printer_begin(&intent(), std::process::id())
                .is_err()
        );
        reopened.printer_finish(&first.key).unwrap();
        reopened
            .printer_begin(&intent(), std::process::id())
            .unwrap();
    }

    #[test]
    fn foreign_links_and_corrupted_printer_receipts_are_preserved_and_refused() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let entry = intent();
        store.printer_begin(&entry, std::process::id()).unwrap();
        let path = directory.path().join("state").join(NAME);
        let saved = std::fs::read(&path).unwrap();
        let link = directory.path().join("foreign");
        std::fs::hard_link(&path, &link).unwrap();
        assert!(store.printer_created(&entry.key, 1).is_err());
        assert_eq!(std::fs::read(&link).unwrap(), saved);
        std::fs::remove_file(link).unwrap();
        let mut jobs: Jobs = serde_json::from_slice(&saved).unwrap();
        jobs.entries[0].receipt.accepted = true;
        std::fs::write(&path, serde_json::to_vec(&jobs).unwrap()).unwrap();
        assert!(store.printer_claim().is_err());
        assert!(store.printer_begin(&intent(), std::process::id()).is_err());
    }

    #[test]
    fn printer_review_is_read_only_and_busy_claims_cannot_be_dismissed() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        assert!(store.printer_list().unwrap().is_empty());
        assert!(!directory.path().join("state").exists());
        let entry = intent();
        store.printer_begin(&entry, std::process::id()).unwrap();
        let path = directory.path().join("state").join(NAME);
        let before = std::fs::read(&path).unwrap();
        let mut items = store.printer_list().unwrap();
        assert_eq!(items[0].status, ReviewStatus::Busy);
        assert!(items[0].title.starts_with("📄 résumé"));
        assert!(!items[0].title.contains("[Canopa:"));
        items[0].status = ReviewStatus::Unconfirmed;
        assert!(store.printer_dismiss(&items[0]).is_err());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn stale_printer_confirmation_cannot_remove_updated_or_reclaimed_work() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let entry = intent();
        store.printer_begin(&entry, std::process::id()).unwrap();
        store
            .jobs_change(false, |jobs| {
                jobs.entries[0].worker.fixture_retire();
                Ok(())
            })
            .unwrap();
        let old = store.printer_list().unwrap().remove(0);
        assert_eq!(old.status, ReviewStatus::Unconfirmed);
        store
            .jobs_change(false, |jobs| {
                jobs.entries[0].receipt.job = Some(7);
                jobs.entries[0].receipt.accepted = true;
                Ok(())
            })
            .unwrap();
        assert!(store.printer_dismiss(&old).is_err());
        let fresh = store.printer_list().unwrap().remove(0);
        assert_eq!(fresh.status, ReviewStatus::Accepted);
        store.printer_claim().unwrap();
        assert!(store.printer_dismiss(&fresh).is_err());
        store
            .jobs_change(false, |jobs| {
                jobs.entries[0].worker.fixture_retire();
                Ok(())
            })
            .unwrap();
        store.printer_dismiss(&fresh).unwrap();
        assert!(store.printer_list().unwrap().is_empty());
    }

    #[test]
    fn printer_metadata_removal_preserves_other_records_and_an_unreadable_file_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&directory.path().join("state")).unwrap();
        let first = intent();
        store.printer_begin(&first, std::process::id()).unwrap();
        store.printer_begin(&intent(), std::process::id()).unwrap();
        store
            .jobs_change(false, |jobs| {
                jobs.entries[0].worker.fixture_retire();
                Ok(())
            })
            .unwrap();
        let catalog = directory.path().join("state/catalog.json");
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&catalog, b"unreadable file catalog").unwrap();
        std::fs::set_permissions(&catalog, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(store.list().is_err());
        let item = store
            .printer_list()
            .unwrap()
            .into_iter()
            .find(|i| i.id == first.key)
            .unwrap();
        store.printer_dismiss(&item).unwrap();
        let remaining = store.printer_list().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].status, ReviewStatus::Busy);
        assert_eq!(std::fs::read(catalog).unwrap(), b"unreadable file catalog");
    }
}
