//! Private descriptor-bearing worker protocol. This is not a portal interface.
mod catalog;
mod client;
mod cups;
pub(crate) mod document;
mod preferences;
pub mod runtime;
mod settings;
pub use crate::print::printer_jobs::{
    Report as PrinterRecoveryReport, ReviewItem as PrinterRecoveryItem,
    ReviewStatus as PrinterRecoveryStatus,
};
pub use catalog::{Capabilities, CustomSize, Media, Printer, PrinterState};
#[cfg(test)]
pub(crate) use client::fixture_document;
pub use client::{Client, Frozen, Recovered};
pub use cups::{Destination, JobOptions, Sides};
pub use document::MAX_DOCUMENT_BYTES;
pub use preferences::{PreferenceSelection, Preferences};
pub use settings::{Control, Controls, Field as SettingField, SettingChoice, Settings};

use super::Format;
use super::file_target::{
    Disposition, Pinned, Preparation, Publication, PublicationInfo, RecoveryBody, RecoveryItem,
    RecoveryProtection, RecoveryStore, Selection,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Write},
    os::fd::FromRawFd,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    ListLiveJobs {
        destination: Destination,
    },
    AuthenticateHeldJob {
        job: super::live_jobs::Job,
    },
    RecoverPrinterJobs {
        store: RecoveryStore,
        #[serde(default)]
        interactive: bool,
    },
    ListPrinterRecovery {
        store: RecoveryStore,
    },
    DismissPrinterRecovery {
        store: RecoveryStore,
        item: PrinterRecoveryItem,
    },
    ListRecovery {
        store: RecoveryStore,
    },
    SnapshotRecovery {
        store: RecoveryStore,
        id: String,
        body: RecoveryBody,
        target: i32,
    },
    DismissRecovery {
        store: RecoveryStore,
        id: String,
    },
    Printers,
    Capabilities {
        destination: Destination,
        #[serde(default)]
        preferences: Option<Box<Preferences>>,
    },
    CustomMedia {
        destination: Destination,
        size: CustomSize,
        sides: Option<Sides>,
    },
    CheckPrinter {
        destination: Destination,
        options: JobOptions,
        custom_size: Option<CustomSize>,
    },
    Snapshot {
        source: i32,
        target: i32,
    },
    Render {
        source: i32,
        target: i32,
        request: super::render::Request,
    },
    Submit {
        document: i32,
        destination: Destination,
        title: String,
        options: JobOptions,
        #[serde(default)]
        recovery: Option<RecoveryRegistration>,
    },
    CancelJob {
        destination: Destination,
        job: i32,
    },
    VerifyFile {
        selection: Selection,
        descriptors: Vec<i32>,
    },
    PrepareFile {
        preparation: Preparation,
        channel: i32,
        descriptors: Vec<i32>,
        #[serde(default)]
        recovery: Option<RecoveryRegistration>,
    },
    PublishFile {
        document: i32,
        format: Format,
        publication: PublicationInfo,
        descriptors: Vec<i32>,
        #[serde(default)]
        recovery: Option<RecoveryRegistration>,
    },
    PublishRecoveredFile {
        document: i32,
        publication: PublicationInfo,
        descriptors: Vec<i32>,
        recovery: RecoveryRegistration,
    },
    SettleFile {
        publication: PublicationInfo,
        descriptors: Vec<i32>,
        acknowledged: bool,
        #[serde(default)]
        recovery: Option<RecoveryRegistration>,
    },
}

impl Operation {
    pub(crate) fn uses_cups(&self) -> bool {
        matches!(
            self,
            Operation::Printers
                | Operation::ListLiveJobs { .. }
                | Operation::AuthenticateHeldJob { .. }
                | Operation::Capabilities { .. }
                | Operation::CustomMedia { .. }
                | Operation::CheckPrinter { .. }
                | Operation::Submit { .. }
                | Operation::RecoverPrinterJobs { .. }
                | Operation::CancelJob { .. }
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRegistration {
    pub store: RecoveryStore,
    pub owner: u32,
    #[serde(default)]
    pub recovering: bool,
    #[serde(default)]
    pub protection: Option<RecoveryProtection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub cancel: i32,
    #[serde(default)]
    pub authentication: Option<i32>,
    #[serde(default, skip_serializing_if = "runtime::Selection::is_system")]
    pub cups_runtime: runtime::Selection,
    pub operation: Operation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub bytes: u64,
    pub format: Option<Format>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    LiveJobs {
        jobs: Vec<super::live_jobs::Job>,
    },
    AuthenticatedJob {
        job: i32,
    },
    AuthenticationCancelled,
    RetiredPrinterJobs {
        report: PrinterRecoveryReport,
    },
    PrinterRecovery {
        items: Vec<PrinterRecoveryItem>,
    },
    DismissedPrinterRecovery {
        id: String,
    },
    Recovery {
        items: Vec<RecoveryItem>,
    },
    Recovered {
        document: Document,
        protection: RecoveryProtection,
    },
    DismissedRecovery {
        id: String,
    },
    Printers {
        printers: Vec<Printer>,
    },
    Capabilities {
        capabilities: Capabilities,
    },
    CustomMedia {
        media: Media,
    },
    CheckedPrinter {
        options: JobOptions,
    },
    Snapshot {
        document: Document,
    },
    Rendered {
        document: Document,
    },
    Created {
        job: i32,
    },
    Accepted {
        job: i32,
    },
    RetiredJob {
        job: i32,
    },
    VerifiedFile,
    PreparedFile {
        publication: PublicationInfo,
    },
    PublishedFile,
    SettledFile {
        disposition: Disposition,
    },
    Failed {
        message: String,
    },
}

pub(crate) struct Cancellation(File);
impl Cancellation {
    #[cfg(test)]
    pub(crate) fn fixture(file: File) -> Self {
        Self(file)
    }
    pub(crate) fn check(&self) -> anyhow::Result<()> {
        use rustix::event::{PollFd, PollFlags, poll};
        let mut descriptors = [PollFd::new(&self.0, PollFlags::IN)];
        let timeout = rustix::event::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let ready = poll(&mut descriptors, Some(&timeout))?;
        anyhow::ensure!(ready == 0, "Print operation cancelled");
        Ok(())
    }

    pub(crate) fn withdrawn(&self) -> bool {
        use rustix::event::{PollFd, PollFlags, poll};
        let mut descriptors = [PollFd::new(&self.0, PollFlags::IN)];
        let timeout = rustix::event::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // Only an actual cancellation signal or parent-channel loss grants
        // withdrawal authority. An inspection error cannot authorize it.
        poll(&mut descriptors, Some(&timeout)).is_ok_and(|ready| {
            ready > 0
                && descriptors[0]
                    .revents()
                    .intersects(PollFlags::IN | PollFlags::HUP)
        })
    }
}

fn event(event: &Event) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, event)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

fn inherited(descriptors: &[i32]) -> anyhow::Result<Vec<File>> {
    anyhow::ensure!(
        descriptors.iter().all(|fd| *fd >= 3)
            && descriptors
                .iter()
                .enumerate()
                .all(|(index, fd)| !descriptors[..index].contains(fd)),
        "Invalid print descriptor set"
    );
    // Validate all supplied numbers before allocating duplicates, so a missing
    // supplied descriptor cannot alias a duplicate allocated for an earlier one.
    for descriptor in descriptors {
        // SAFETY: fcntl accepts arbitrary integer descriptors and reports EBADF.
        anyhow::ensure!(
            unsafe { libc::fcntl(*descriptor, libc::F_GETFD) } >= 0,
            "Missing print descriptor"
        );
    }
    descriptors
        .iter()
        .map(|descriptor| {
            // SAFETY: fcntl validates the integer source and atomically returns
            // a new descriptor. We never adopt a caller-supplied integer.
            let duplicate = unsafe { libc::fcntl(*descriptor, libc::F_DUPFD_CLOEXEC, 3) };
            anyhow::ensure!(duplicate >= 0, "Print descriptor duplication failed");
            // SAFETY: F_DUPFD_CLOEXEC returned a new uniquely owned descriptor.
            let file = unsafe { File::from_raw_fd(duplicate) };
            // SAFETY: only this child's descriptor flags change; restore
            // CLOEXEC so a CUPS-internal exec cannot inherit the original FD.
            anyhow::ensure!(
                unsafe { libc::fcntl(*descriptor, libc::F_SETFD, libc::FD_CLOEXEC) } >= 0,
                "Print descriptor isolation failed"
            );
            Ok(file)
        })
        .collect()
}

/// Entrypoint for the standalone helper, before starting any application runtime.
pub fn run_worker() -> anyhow::Result<()> {
    let input: Input = crate::process::read_frame(&mut BufReader::new(std::io::stdin()))?;
    if let runtime::Selection::Image { descriptor } = input.cups_runtime {
        anyhow::ensure!(
            descriptor != input.cancel && Some(descriptor) != input.authentication,
            "CUPS client descriptor aliases a control channel"
        );
    }
    anyhow::ensure!(
        input.operation.uses_cups() || input.cups_runtime.is_system(),
        "Unexpected CUPS runtime for file operation"
    );
    if input.operation.uses_cups()
        && let Err(error) = runtime::initialize(input.cups_runtime)
    {
        return event(&Event::Failed {
            message: error.to_string(),
        });
    }
    let _authentication = if let Some(descriptor) = input.authentication {
        anyhow::ensure!(
            descriptor != input.cancel,
            "Authentication channel aliases cancellation"
        );
        let mut files = inherited(&[descriptor, input.cancel])?.into_iter();
        let channel = std::os::unix::net::UnixStream::from(std::os::fd::OwnedFd::from(
            files.next().expect("authentication channel"),
        ));
        Some(super::authentication::Scope::enter(
            channel,
            files.next().expect("authentication cancellation"),
        )?)
    } else {
        None
    };
    let result = match input.operation {
        Operation::ListLiveJobs { destination } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            cups::live_jobs(destination, &cancel).map(|jobs| Event::LiveJobs { jobs })
        }
        Operation::AuthenticateHeldJob { job } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            let id = job.job;
            cups::authenticate_held(job, &cancel).map(|()| Event::AuthenticatedJob { job: id })
        }
        Operation::RecoverPrinterJobs { store, interactive } => {
            let mut files = inherited(&[input.cancel])?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            cups::recover(&store, &cancel, interactive)
                .map(|report| Event::RetiredPrinterJobs { report })
        }
        Operation::ListPrinterRecovery { store } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            (|| {
                cancel.check()?;
                Ok(Event::PrinterRecovery {
                    items: store.printer_list()?,
                })
            })()
        }
        Operation::DismissPrinterRecovery { store, item } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            (|| {
                cancel.check()?;
                store.printer_dismiss(&item)?;
                Ok(Event::DismissedPrinterRecovery { id: item.id })
            })()
        }
        Operation::SnapshotRecovery {
            store,
            id,
            body,
            target,
        } => {
            let mut files = inherited(&[input.cancel, target])?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            store
                .snapshot(
                    &id,
                    body,
                    files.next().expect("snapshot descriptor"),
                    &cancel,
                )
                .map(|(document, protection)| Event::Recovered {
                    document,
                    protection,
                })
        }
        Operation::DismissRecovery { store, id } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            (|| {
                cancel.check()?;
                store.dismiss(&id)?;
                Ok(Event::DismissedRecovery { id })
            })()
        }
        Operation::ListRecovery { store } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            (|| {
                cancel.check()?;
                Ok(Event::Recovery {
                    items: store.list()?,
                })
            })()
        }
        Operation::Printers => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            cups::printers(&cancel).map(|printers| Event::Printers { printers })
        }
        Operation::Capabilities {
            destination,
            preferences,
        } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            cups::capabilities(destination, preferences.as_deref(), &cancel)
                .map(|capabilities| Event::Capabilities { capabilities })
        }
        Operation::CustomMedia {
            destination,
            size,
            sides,
        } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            cups::custom_media(destination, size, sides, &cancel)
                .map(|media| Event::CustomMedia { media })
        }
        Operation::CheckPrinter {
            destination,
            options,
            custom_size,
        } => {
            let cancel = Cancellation(inherited(&[input.cancel])?.remove(0));
            cups::check_printer(destination, options, custom_size, &cancel)
                .map(|options| Event::CheckedPrinter { options })
        }
        Operation::Snapshot { source, target } => {
            let mut files = inherited(&[input.cancel, source, target])?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            let source = files.next().expect("source descriptor");
            let target = files.next().expect("target descriptor");
            document::snapshot(source, target, &cancel).map(|document| Event::Snapshot { document })
        }
        Operation::Render {
            source,
            target,
            request,
        } => {
            let mut files = inherited(&[input.cancel, source, target])?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            let source = files.next().expect("source descriptor");
            let target = files.next().expect("target descriptor");
            super::render::runner::transform(source, target, request, &cancel)
                .map(|document| Event::Rendered { document })
        }
        Operation::Submit {
            document,
            destination,
            title,
            options,
            recovery,
        } => {
            let mut files = inherited(&[input.cancel, document])?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            let document = files.next().expect("document descriptor");
            cups::submit(document, destination, title, options, &cancel, recovery)
        }
        Operation::CancelJob { destination, job } => {
            let mut files = inherited(&[input.cancel])?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            cancel.check()?;
            cups::cancel(&destination, job).map(|()| Event::RetiredJob { job })
        }
        Operation::VerifyFile {
            selection,
            mut descriptors,
        } => {
            descriptors.insert(0, input.cancel);
            let mut files = inherited(&descriptors)?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            cancel.check()?;
            Pinned::inherited(selection, files.collect())
                .map(|_| Event::VerifiedFile)
                .map_err(Into::into)
        }
        Operation::PrepareFile {
            preparation,
            channel,
            mut descriptors,
            recovery,
        } => {
            descriptors.insert(0, channel);
            descriptors.insert(0, input.cancel);
            let mut files = inherited(&descriptors)?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            let channel = std::os::unix::net::UnixStream::from(std::os::fd::OwnedFd::from(
                files.next().expect("channel descriptor"),
            ));
            (|| {
                cancel.check()?;
                if let Some(recovery) = &recovery {
                    if recovery.recovering {
                        anyhow::ensure!(
                            recovery.protection.is_some_and(
                                |protection| protection.allows_preparation(&preparation)
                            ),
                            "Choose a destination outside the retained folder"
                        );
                        recovery
                            .store
                            .begin_recovery(&preparation, recovery.owner)?;
                    } else {
                        recovery.store.begin(&preparation, recovery.owner)?;
                    }
                }
                let publication = preparation.prepare(files.collect())?;
                if let Some(recovery) = &recovery {
                    recovery.store.active(publication.info(), recovery.owner)?;
                }
                publication.send(&channel)?;
                // Complete the one-shot stream even if a concurrent fork still
                // holds a CLOEXEC copy until its exec. The receiver requires EOF.
                channel.shutdown(std::net::Shutdown::Write)?;
                Ok(Event::PreparedFile {
                    publication: publication.info().clone(),
                })
            })()
        }
        operation @ (Operation::PublishFile { .. } | Operation::PublishRecoveredFile { .. }) => {
            let (document, format, publication, mut descriptors, recovery) = match operation {
                Operation::PublishFile {
                    document,
                    format,
                    publication,
                    descriptors,
                    recovery,
                } => (document, Some(format), publication, descriptors, recovery),
                Operation::PublishRecoveredFile {
                    document,
                    publication,
                    descriptors,
                    recovery,
                } => {
                    anyhow::ensure!(
                        recovery.recovering,
                        "Recovery export requires its own admission"
                    );
                    anyhow::ensure!(
                        recovery
                            .protection
                            .is_some_and(|protection| protection.allows_publication(&publication)),
                        "Choose a destination outside the retained folder"
                    );
                    (document, None, publication, descriptors, Some(recovery))
                }
                _ => unreachable!(),
            };
            descriptors.insert(0, document);
            descriptors.insert(0, input.cancel);
            let mut files = inherited(&descriptors)?.into_iter();
            let cancel = Cancellation(files.next().expect("cancel descriptor"));
            let document = files.next().expect("document descriptor");
            if let Some(recovery) = &recovery {
                recovery.store.active(&publication, recovery.owner)?;
            }
            let publication = Publication::inherited(publication, files.collect())?;
            (|| {
                if let Some(format) = format {
                    document::stage(&document, publication.document(), format, &cancel)?;
                    publication.commit(|| cancel.check().map_err(std::io::Error::other))?;
                } else {
                    document::stage_recovery(&document, publication.document(), &cancel)?;
                    publication
                        .commit_recovery(|| cancel.check().map_err(std::io::Error::other))?;
                }
                Ok(Event::PublishedFile)
            })()
        }
        Operation::SettleFile {
            publication,
            mut descriptors,
            acknowledged,
            recovery,
        } => {
            descriptors.insert(0, input.cancel);
            let mut files = inherited(&descriptors)?.into_iter();
            let _cancel = Cancellation(files.next().expect("cancel descriptor"));
            if let Some(recovery) = &recovery {
                recovery.store.active(&publication, recovery.owner)?;
            }
            let publication = Publication::inherited(publication, files.collect())?;
            (|| {
                let disposition = publication.settle(acknowledged)?;
                if let Some(recovery) = &recovery {
                    recovery
                        .store
                        .settled(publication.info(), recovery.owner, &disposition)?;
                }
                Ok(Event::SettledFile { disposition })
            })()
        }
    };
    event(&result.unwrap_or_else(|error| {
        if super::authentication::current().is_some_and(|channel| channel.declined()) {
            Event::AuthenticationCancelled
        } else {
            Event::Failed {
                message: error.to_string(),
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;

    #[test]
    fn inherited_numbers_are_duplicated_without_adopting_or_closing_the_original() {
        let original = tempfile::tempfile().unwrap();
        let copies = inherited(&[original.as_raw_fd()]).unwrap();
        assert_ne!(copies[0].as_raw_fd(), original.as_raw_fd());
        drop(copies);
        assert!(original.metadata().is_ok());
        assert!(inherited(&[original.as_raw_fd(), original.as_raw_fd()]).is_err());
        assert!(inherited(&[i32::MAX]).is_err());
        assert!(inherited(&[0]).is_err());
    }
}
