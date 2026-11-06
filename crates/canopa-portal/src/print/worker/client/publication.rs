use super::*;
use crate::print::file_target::{Preparation, Prepared};
use std::os::unix::{ffi::OsStrExt, net::UnixStream};

fn retained(disposition: &Disposition) {
    if let Some(path) = &disposition.recovery {
        eprintln!(
            "level=warn component=print event=file_recovery_retained published={} uncertain={} directory={:?}",
            disposition.published,
            disposition.uncertain,
            std::ffi::OsStr::from_bytes(path)
        );
    }
}

impl Client {
    /// Metadata checks run in an independently owned helper, including when an
    /// approval is held between PreparePrint and its one-use delivery.
    pub async fn verify_file(
        &self,
        pin: &Pinned,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<()>> {
        let descriptors = pin
            .files()
            .into_iter()
            .map(duplicate)
            .collect::<std::io::Result<Vec<_>>>()?;
        let operation = Operation::VerifyFile {
            selection: pin.selection().clone(),
            descriptors: descriptors.iter().map(AsRawFd::as_raw_fd).collect(),
        };
        match self
            .execute(operation, descriptors, cancelled, None)
            .await?
        {
            None => Ok(None),
            Some(Event::VerifiedFile) => Ok(Some(())),
            _ => anyhow::bail!("Print file verification was not confirmed"),
        }
    }

    /// The task retains consent, frozen storage and all helper lifetimes. Setup
    /// and settlement perform no filesystem I/O in the owning backend process.
    pub async fn publish_file(
        &self,
        document: &Frozen,
        pin: Pinned,
        format: Format,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Disposition>> {
        self.publish_body(document.storage.clone(), pin, Some(format), None, cancelled)
            .await
    }

    pub async fn publish_recovered(
        &self,
        document: &Recovered,
        pin: Pinned,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Disposition>> {
        anyhow::ensure!(
            self.recovery.is_some(),
            "Recovery export needs a configured recovery store"
        );
        anyhow::ensure!(
            document.allows_destination(pin.selection()),
            "Choose a destination outside the retained folder"
        );
        self.publish_body(
            document.storage.clone(),
            pin,
            None,
            Some(document.protection),
            cancelled,
        )
        .await
    }

    async fn publish_body(
        &self,
        storage: Arc<Storage>,
        pin: Pinned,
        format: Option<Format>,
        protection: Option<crate::print::file_target::RecoveryProtection>,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Disposition>> {
        let permit = self.operations.clone().try_acquire_owned()?;
        let client = self.clone();
        let recovery = self.recovery.clone().map(|mut registration| {
            registration.recovering = format.is_none();
            registration.protection = protection;
            registration
        });
        let (reporter, notices) = self.reporter();
        let (alive, mut lifetime) = oneshot::channel();
        let (reply, result) = oneshot::channel();
        tokio::spawn(async move {
            let _permit = permit;
            let response = async {
                if *cancelled.borrow() || cancelled.has_changed().is_err() { return Ok(None); }
                let preparation = Preparation::new(&pin)?;
                let fallback = preparation.retained(false);
                let file_reporter = reporter.for_file(&preparation, false);
                let hint = std::ffi::OsStr::from_bytes(fallback.recovery.as_deref().unwrap_or_default());
                let (channel, child) = UnixStream::pair()?;
                // The setup helper is reaped before receiving. A malformed or
                // leaked writer fails immediately instead of blocking the owner.
                channel.set_nonblocking(true)?;
                let mut descriptors = pin.files().into_iter().map(duplicate).collect::<std::io::Result<Vec<_>>>()?;
                let operation = Operation::PrepareFile { recovery: recovery.clone(), preparation: preparation.clone(), channel: child.as_raw_fd(),
                    descriptors: descriptors.iter().map(AsRawFd::as_raw_fd).collect() };
                descriptors.push(child.into());
                let setup = task::run(&client, operation, descriptors, cancelled.clone(), &mut lifetime, &file_reporter, None).await;
                let Some(info) = setup.prepared.clone() else {
                    retained(&fallback);
                    return match setup.result {
                        Ok(None) => Ok(None),
                        Err(error) => Err(anyhow::anyhow!("{error}. File setup is unconfirmed; recovery may be retained at {hint:?}")),
                        _ => anyhow::bail!("Print file setup was not confirmed"),
                    };
                };
                let publication = Prepared::receive(&channel, info, &preparation).map_err(|error| {
                    retained(&fallback);
                    anyhow::anyhow!("{error}. Print staging handoff failed; recovery may be retained at {hint:?}")
                })?;
                let execution: anyhow::Result<task::Execution> = async { if matches!(&setup.result, Ok(Some(Event::PreparedFile { .. }))) {
                    let document = duplicate(&storage.file)?;
                    let mut descriptors = publication.files().iter().map(duplicate).collect::<std::io::Result<Vec<_>>>()?;
                    let files = descriptors.iter().map(AsRawFd::as_raw_fd).collect();
                    let operation = match format {
                        Some(format) => Operation::PublishFile { recovery: recovery.clone(), document: document.as_raw_fd(), format,
                            publication: publication.info().clone(), descriptors: files },
                        None => Operation::PublishRecoveredFile { recovery: recovery.clone().expect("configured recovery export"),
                            document: document.as_raw_fd(), publication: publication.info().clone(), descriptors: files },
                    };
                    descriptors.push(document);
                    Ok(task::run(&client, operation, descriptors, cancelled, &mut lifetime, &file_reporter, None).await)
                } else { Ok(setup) } }.await;
                let acknowledged = execution.as_ref().is_ok_and(|execution| execution.accepted);
                let cleanup: anyhow::Result<Disposition> = async {
                    let descriptors = publication.files().iter().map(duplicate).collect::<std::io::Result<Vec<_>>>()?;
                    let operation = Operation::SettleFile { recovery, publication: publication.info().clone(),
                        descriptors: descriptors.iter().map(AsRawFd::as_raw_fd).collect(), acknowledged };
                    let (_cancel, cancelled) = watch::channel(false);
                    let (_alive, mut lifetime) = oneshot::channel();
                    let cleanup = task::run(&client, operation, descriptors, cancelled, &mut lifetime,
                        &reporter.for_file(&preparation, acknowledged).cleanup(), None).await;
                    match cleanup.result? {
                        Some(Event::SettledFile { disposition }) => Ok(disposition),
                        _ => anyhow::bail!("Print file settlement was not confirmed"),
                    }
                }.await;
                // Never inspect the filesystem again when a filesystem helper
                // could not finish. Only a positive receipt establishes commit.
                let disposition = cleanup.unwrap_or_else(|_| preparation.retained(acknowledged));
                retained(&disposition);
                anyhow::ensure!(!acknowledged || disposition.published, "Print file receipt contradicts settlement");
                if disposition.published { return Ok(Some(disposition)); }
                if let Some(recovery) = disposition.recovery {
                    anyhow::bail!("Print file output could not be confirmed; do not retry automatically. Recovery may be retained at {:?}", std::ffi::OsStr::from_bytes(&recovery));
                }
                anyhow::ensure!(!disposition.uncertain, "Print file publication is uncertain; do not retry automatically");
                match execution?.result? {
                    None => Ok(None),
                    _ => anyhow::bail!("Print file was not published"),
                }
            }.await;
            let _ = reply.send(response);
        });
        let response = retirement::receive(result, notices, |pending| {
            let disposition = pending.file.ok_or_else(|| anyhow::anyhow!("Print file worker retirement is pending"))?;
            retained(&disposition);
            if disposition.published { return Ok(Some(disposition)); }
            anyhow::bail!("Print file output is unconfirmed while its worker retires; do not retry automatically. Recovery may be retained at {:?}",
                std::ffi::OsStr::from_bytes(disposition.recovery.as_deref().unwrap_or_default()))
        }).await;
        drop(alive);
        response
    }
}
