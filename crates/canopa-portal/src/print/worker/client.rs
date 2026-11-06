//! Bounded child lifetime survives cancellation of the initiating D-Bus future.
mod authentication;
mod live_jobs;
mod publication;
#[cfg(test)]
mod publication_tests;
mod recovery;
mod retirement;
mod task;
use super::{
    Capabilities, CustomSize, Destination, Document, Event, JobOptions, Media, Operation, Printer,
    RecoveryRegistration, Sides, catalog, document,
};
use crate::print::{
    Format,
    file_target::{Disposition, Pinned},
};
use crate::process::DialogProcess;
use std::{
    fs::File,
    os::fd::{AsFd, AsRawFd, OwnedFd},
    sync::Arc,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot, watch};

struct Storage {
    file: File,
    budget: Arc<Budget>,
    _source: Option<Arc<Storage>>,
    _derived: Option<OwnedSemaphorePermit>,
}
struct Budget {
    _permit: OwnedSemaphorePermit,
    derived: Arc<Semaphore>,
}

#[derive(Clone)]
pub struct Frozen {
    storage: Arc<Storage>,
    pub document: Document,
}

/// A selected retained body, verified in a worker and held as immutable bytes.
/// Its separate type cannot be submitted as an ordinary application print job.
#[derive(Clone)]
pub struct Recovered {
    storage: Arc<Storage>,
    pub bytes: u64,
    protection: crate::print::file_target::RecoveryProtection,
}
impl Recovered {
    pub fn allows_destination(&self, selection: &crate::print::file_target::Selection) -> bool {
        self.protection.allows(selection)
    }
}
impl std::fmt::Debug for Recovered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recovered")
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

/// Two live documents and two active helpers; every helper is reaped even when
/// the caller drops its future. Snapshot storage uses sealed memfds, not disk.
#[derive(Clone)]
pub struct Client {
    process: DialogProcess,
    documents: Arc<Semaphore>,
    operations: Arc<Semaphore>,
    shutdown: watch::Sender<bool>,
    recovery: Option<RecoveryRegistration>,
    authentication: Option<Arc<crate::print::authentication::Broker>>,
    runtime: super::runtime::Runtime,
    #[cfg(test)]
    reap_gate: Option<Arc<retirement::ReapGate>>,
}
impl Client {
    pub(crate) fn with_authentication(
        mut self,
        authentication: Option<Arc<crate::print::authentication::Broker>>,
    ) -> Self {
        self.authentication = authentication;
        self
    }
    pub async fn check_printer(
        &self,
        destination: Destination,
        options: JobOptions,
        custom_size: Option<CustomSize>,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<JobOptions>> {
        destination.validate()?;
        options.validate_media()?;
        options.settings.validate()?;
        if let Some(size) = custom_size {
            size.validate()?;
            anyhow::ensure!(
                options.media.is_none() && options.custom_media.is_none(),
                "Conflicting custom paper query"
            );
        }
        match self
            .execute(
                Operation::CheckPrinter {
                    destination,
                    options: options.clone(),
                    custom_size,
                },
                vec![],
                cancelled,
                None,
            )
            .await?
        {
            None => Ok(None),
            Some(Event::CheckedPrinter { options: checked }) => {
                let mut expected = options;
                if let Some(size) = custom_size {
                    anyhow::ensure!(
                        checked
                            .custom_media
                            .as_ref()
                            .is_some_and(|m| size.matches(m)),
                        "Printer returned different custom paper geometry"
                    );
                    expected.custom_media = checked.custom_media.clone();
                }
                anyhow::ensure!(
                    checked == expected,
                    "Printer settings changed during validation"
                );
                Ok(Some(checked))
            }
            _ => anyhow::bail!("Unexpected printer settings response"),
        }
    }
    pub async fn recover_printer_jobs(
        &self,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<super::PrinterRecoveryReport>> {
        self.recover_printer_jobs_with(cancelled, false).await
    }
    /// Each explicit check owns a fresh credential broker. Startup and partial
    /// submission retirement use the separate unattended path above.
    pub async fn review_printer_jobs(
        &self,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<super::PrinterRecoveryReport>> {
        let broker = crate::print::authentication::Broker::new(
            self.process.clone(),
            crate::print::Context {
                app_id: "org.canopa.Portal".into(),
                parent_window: String::new(),
                title: "Printer recovery".into(),
                modal: false,
            },
        );
        self.clone()
            .with_authentication(Some(broker))
            .recover_printer_jobs_with(cancelled, true)
            .await
    }
    async fn recover_printer_jobs_with(
        &self,
        cancelled: watch::Receiver<bool>,
        interactive: bool,
    ) -> anyhow::Result<Option<super::PrinterRecoveryReport>> {
        let store = self
            .recovery
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Print recovery is not configured"))?
            .store
            .clone();
        match self
            .execute(
                Operation::RecoverPrinterJobs { store, interactive },
                vec![],
                cancelled,
                None,
            )
            .await?
        {
            None | Some(Event::AuthenticationCancelled) => Ok(None),
            Some(Event::RetiredPrinterJobs { report }) => {
                report.validate()?;
                Ok(Some(report))
            }
            _ => anyhow::bail!("Unexpected printer recovery response"),
        }
    }
    pub fn with_recovery(mut self, store: crate::print::file_target::RecoveryStore) -> Self {
        self.recovery = Some(RecoveryRegistration {
            store,
            owner: std::process::id(),
            recovering: false,
            protection: None,
        });
        self
    }

    pub async fn recovery(
        &self,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Vec<crate::print::file_target::RecoveryItem>>> {
        let store = self
            .recovery
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Print recovery is not configured"))?
            .store
            .clone();
        match self
            .execute(Operation::ListRecovery { store }, vec![], cancelled, None)
            .await?
        {
            None => Ok(None),
            Some(Event::Recovery { items }) => Ok(Some(items)),
            _ => anyhow::bail!("Unexpected Print recovery response"),
        }
    }
    pub async fn printers(
        &self,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Vec<Printer>>> {
        match self
            .execute(Operation::Printers, Vec::new(), cancelled, None)
            .await?
        {
            None => Ok(None),
            Some(Event::Printers { printers }) => {
                catalog::printers(&printers)?;
                Ok(Some(printers))
            }
            Some(_) => anyhow::bail!("Unexpected printer listing response"),
        }
    }

    pub async fn capabilities(
        &self,
        destination: Destination,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Capabilities>> {
        self.capabilities_with_preferences(destination, None, cancelled)
            .await
    }

    pub async fn capabilities_with_preferences(
        &self,
        destination: Destination,
        preferences: Option<Box<super::Preferences>>,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Capabilities>> {
        destination.validate()?;
        if let Some(preferences) = &preferences {
            preferences.validate()?;
        }
        let preferred = preferences.is_some();
        match self
            .execute(
                Operation::Capabilities {
                    destination: destination.clone(),
                    preferences,
                },
                Vec::new(),
                cancelled,
                None,
            )
            .await?
        {
            None => Ok(None),
            Some(Event::Capabilities { capabilities }) => {
                capabilities.validate()?;
                anyhow::ensure!(
                    capabilities.destination == destination
                        && capabilities.preferences.is_some() == preferred,
                    "Printer capability identity changed"
                );
                Ok(Some(capabilities))
            }
            Some(_) => anyhow::bail!("Unexpected printer capability response"),
        }
    }

    pub async fn custom_media(
        &self,
        destination: Destination,
        size: CustomSize,
        sides: Option<Sides>,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Media>> {
        destination.validate()?;
        size.validate()?;
        match self
            .execute(
                Operation::CustomMedia {
                    destination,
                    size,
                    sides,
                },
                Vec::new(),
                cancelled,
                None,
            )
            .await?
        {
            None => Ok(None),
            Some(Event::CustomMedia { media }) => {
                anyhow::ensure!(
                    size.matches(&media),
                    "Printer returned different custom paper geometry"
                );
                Ok(Some(media))
            }
            Some(_) => anyhow::bail!("Unexpected custom paper response"),
        }
    }

    async fn drain(&self) {
        while self.operations.available_permits() != 2 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }
    pub async fn idle(&self) -> anyhow::Result<()> {
        tokio::time::timeout(std::time::Duration::from_secs(15), self.drain())
            .await
            .map_err(|_| anyhow::anyhow!("Printer information is temporarily unavailable"))
    }

    /// Stop admission before draining. The caller first signals cancellation of
    /// its active requests. A queued UI future cannot start a helper after this
    /// barrier, even if its abort has not yet been observed by the executor.
    pub async fn shutdown(&self) -> usize {
        self.documents.close();
        self.operations.close();
        self.shutdown.send_replace(true);
        let _ = self.idle().await;
        let pending = 2 - self.operations.available_permits();
        if pending != 0 {
            eprintln!(
                "level=warn component=print event=worker_shutdown_pending operations={pending}"
            );
        }
        pending
    }
    pub fn new(process: DialogProcess) -> Self {
        Self::with_runtime(process, super::runtime::Runtime::configured())
    }
    pub(crate) fn with_runtime(process: DialogProcess, runtime: super::runtime::Runtime) -> Self {
        let (shutdown, _) = watch::channel(false);
        Self {
            process,
            documents: Arc::new(Semaphore::new(2)),
            operations: Arc::new(Semaphore::new(2)),
            shutdown,
            recovery: None,
            authentication: None,
            runtime,
            #[cfg(test)]
            reap_gate: None,
        }
    }
    fn reporter(
        &self,
    ) -> (
        retirement::Reporter,
        watch::Receiver<Option<retirement::Pending>>,
    ) {
        let (reporter, notices) = retirement::Reporter::new(self.shutdown.subscribe());
        #[cfg(test)]
        let reporter = reporter.with_gate(self.reap_gate.clone());
        (reporter, notices)
    }

    pub async fn snapshot(
        &self,
        source: OwnedFd,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Frozen>> {
        let permit = self.documents.clone().try_acquire_owned()?;
        let source = duplicate(&source)?;
        let storage = Arc::new(Storage {
            file: File::from(rustix::fs::memfd_create(
                c"canopa-print",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )?),
            budget: Arc::new(Budget {
                _permit: permit,
                derived: Arc::new(Semaphore::new(1)),
            }),
            _source: None,
            _derived: None,
        });
        let target = duplicate(&storage.file)?;
        let operation = Operation::Snapshot {
            source: source.as_raw_fd(),
            target: target.as_raw_fd(),
        };
        let result = self
            .execute(
                operation,
                vec![source, target],
                cancelled,
                Some(storage.clone()),
            )
            .await?;
        match result {
            None => Ok(None),
            Some(Event::Snapshot { document: reported }) => {
                let document = document::verify(&storage.file)?;
                anyhow::ensure!(
                    document.bytes == reported.bytes && document.format == reported.format,
                    "Invalid print snapshot response"
                );
                Ok(Some(Frozen { storage, document }))
            }
            Some(_) => anyhow::bail!("Unexpected print snapshot response"),
        }
    }

    pub async fn submit(
        &self,
        document: &Frozen,
        destination: Destination,
        title: String,
        options: JobOptions,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<i32>> {
        let descriptor = duplicate(&document.storage.file)?;
        let operation = Operation::Submit {
            document: descriptor.as_raw_fd(),
            destination,
            title,
            options,
            recovery: self.recovery.clone(),
        };
        match self
            .execute(
                operation,
                vec![descriptor],
                cancelled,
                Some(document.storage.clone()),
            )
            .await?
        {
            None => Ok(None),
            Some(Event::Accepted { job }) => Ok(Some(job)),
            Some(_) => anyhow::bail!("Unexpected print submission response"),
        }
    }

    /// Each document admits one derived body. Both bodies and the shared
    /// document/derivation permits survive a dropped caller until worker reap.
    pub async fn render(
        &self,
        source: &Frozen,
        request: crate::print::render::Request,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Frozen>> {
        request.validate()?;
        anyhow::ensure!(
            source.storage._derived.is_none(),
            "A rendered body cannot be rendered again"
        );
        let derived = source.storage.budget.derived.clone().try_acquire_owned()?;
        let format = request.format;
        let storage = Arc::new(Storage {
            file: File::from(rustix::fs::memfd_create(
                c"canopa-print-rendered",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )?),
            budget: source.storage.budget.clone(),
            _source: Some(source.storage.clone()),
            _derived: Some(derived),
        });
        let input = duplicate(&source.storage.file)?;
        let output = duplicate(&storage.file)?;
        let operation = Operation::Render {
            source: input.as_raw_fd(),
            target: output.as_raw_fd(),
            request,
        };
        match self
            .execute(
                operation,
                vec![input, output],
                cancelled,
                Some(storage.clone()),
            )
            .await?
        {
            None => Ok(None),
            Some(Event::Rendered { document: reported }) => {
                let document = document::verify(&storage.file)?;
                anyhow::ensure!(
                    document.bytes == reported.bytes
                        && document.format == reported.format
                        && document.format == Some(format),
                    "Invalid print render response"
                );
                Ok(Some(Frozen { storage, document }))
            }
            _ => anyhow::bail!("Unexpected print render response"),
        }
    }

    async fn execute(
        &self,
        operation: Operation,
        descriptors: Vec<OwnedFd>,
        cancelled: watch::Receiver<bool>,
        keepalive: Option<Arc<Storage>>,
    ) -> anyhow::Result<Option<Event>> {
        let permit = self.operations.clone().try_acquire_owned()?;
        let client = self.clone();
        let recovery = self.recovery.clone();
        let authentication = self.authentication.clone();
        let (reporter, notices) = self.reporter();
        let (alive, mut lifetime) = oneshot::channel();
        let (reply, result) = oneshot::channel();
        tokio::spawn(async move {
            // These guards live in the task, including during child retirement
            // and partial-job cleanup after the outer method future disappears.
            let _permit = permit;
            let _storage = keepalive;
            let submission = matches!(&operation, Operation::Submit { .. });
            let execution = task::run(
                &client,
                operation,
                descriptors,
                cancelled,
                &mut lifetime,
                &reporter,
                authentication,
            )
            .await;
            if submission && !execution.accepted {
                // A Created frame is only a job number. After the original
                // worker exits it cannot prove identity, or that the scheduler
                // did not receive a document whose acknowledgement was lost.
                // Parent cleanup therefore requires the durable recovery proof.
                let cleanup_operation = recovery.map(|recovery| Operation::RecoverPrinterJobs {
                    store: recovery.store,
                    interactive: false,
                });
                if let Some(operation) = cleanup_operation {
                    let (_not_cancelled, cancelled) = watch::channel(false);
                    let (_alive, mut lifetime) = oneshot::channel();
                    let cleanup = task::run(
                        &client,
                        operation,
                        Vec::new(),
                        cancelled,
                        &mut lifetime,
                        &reporter.cleanup(),
                        None,
                    )
                    .await;
                    if !matches!(
                        cleanup.result,
                        Ok(Some(Event::RetiredPrinterJobs {
                            report: super::PrinterRecoveryReport { unresolved: 0, .. }
                        }))
                    ) {
                        eprintln!(
                            "level=warn component=print event=partial_job_cleanup_unconfirmed"
                        );
                    }
                } else if execution.created.is_some() {
                    eprintln!("level=warn component=print event=partial_job_cleanup_unconfirmed");
                }
            }
            let _ = reply.send(execution.result);
        });
        let response = retirement::receive(result, notices, |pending| {
            match pending.accepted_job {
                Some(job) => Ok(Some(Event::Accepted { job })),
                None => anyhow::bail!("Print worker retirement is pending; result is unconfirmed; do not retry automatically"),
            }
        }).await;
        // Dropping this sender is the task's independent lifetime cancellation.
        // Retain it through the entire wait, including all error paths.
        drop(alive);
        response
    }
}

fn duplicate(file: &impl AsFd) -> std::io::Result<OwnedFd> {
    Ok(rustix::io::fcntl_dupfd_cloexec(file, 3)?)
}

#[cfg(test)]
pub(crate) fn fixture_document(bytes: &[u8]) -> Frozen {
    use std::os::unix::fs::FileExt;
    let file = File::from(
        rustix::fs::memfd_create(
            c"print-wire-fixture",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    file.write_all_at(bytes, 0).unwrap();
    rustix::fs::fcntl_add_seals(
        &file,
        rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::SEAL,
    )
    .unwrap();
    let document = document::verify(&file).unwrap();
    Frozen {
        storage: Arc::new(Storage {
            file,
            budget: Arc::new(Budget {
                _permit: Arc::new(Semaphore::new(2)).try_acquire_owned().unwrap(),
                derived: Arc::new(Semaphore::new(1)),
            }),
            _source: None,
            _derived: None,
        }),
        document,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_refuses_late_metadata_without_executing_another_helper() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("unexpected-helper");
        let client = Client::new(DialogProcess {
            executable: "/bin/sh".into(),
            arguments: vec![
                "-c".into(),
                "printf started > \"$0\"; exit 1".into(),
                marker.clone().into_os_string(),
            ],
        });
        client.shutdown().await;
        let (_cancel, cancelled) = watch::channel(false);
        assert!(client.printers(cancelled).await.is_err());
        assert!(
            !marker.exists(),
            "retired client executed a late metadata helper"
        );
    }

    #[tokio::test]
    #[ignore = "requires the normal backend executable and native PDF input"]
    async fn native_rendered_bodies_retain_both_storage_limits() {
        let client = Client::new(DialogProcess {
            executable: std::env::var_os("CANOPA_PRINT_BINARY").unwrap().into(),
            arguments: vec![],
        });
        let input = std::env::var_os("CANOPA_PRINT_RENDER_INPUT").unwrap();
        let (_cancel, cancelled) = watch::channel(false);
        let first = client
            .snapshot(File::open(&input).unwrap().into(), cancelled.clone())
            .await
            .unwrap()
            .unwrap();
        let second = client
            .snapshot(File::open(&input).unwrap().into(), cancelled.clone())
            .await
            .unwrap()
            .unwrap();
        let request = crate::print::render::Request {
            layout: Default::default(),
            page_setup: Default::default(),
            format: super::super::Format::Pdf,
            mode: crate::print::render::Mode::Layout,
            duplex: false,
        };
        let rendered = client
            .render(&first, request.clone(), cancelled.clone())
            .await
            .unwrap()
            .unwrap();
        assert!(
            client
                .render(&rendered, request.clone(), cancelled.clone())
                .await
                .is_err()
        );
        assert!(
            client
                .render(&first, request.clone(), cancelled.clone())
                .await
                .is_err()
        );
        drop(first);
        assert!(
            client
                .snapshot(File::open(&input).unwrap().into(), cancelled.clone())
                .await
                .is_err()
        );
        assert_eq!(client.documents.available_permits(), 0);
        drop(rendered);
        assert_eq!(client.documents.available_permits(), 1);
        let third = client
            .snapshot(File::open(&input).unwrap().into(), cancelled.clone())
            .await
            .unwrap()
            .unwrap();
        let rendered = client
            .render(&second, request, cancelled)
            .await
            .unwrap()
            .unwrap();
        drop((second, third, rendered));
        assert_eq!(client.documents.available_permits(), 2);
        client.shutdown().await;
    }
    #[tokio::test]
    async fn unattended_recovery_cannot_open_a_dialog_even_when_the_worker_requests_one() {
        for explicit in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let script = directory.path().join("helper.py");
            let marker = directory.path().join("prompt");
            std::fs::write(&script, r#"import json,socket,sys
from pathlib import Path
request=json.loads(sys.stdin.readline())
if sys.argv[-1] != '--print-worker':
    assert request['context']['title']=='Printer recovery'
    assert request['context']['app_id']=='org.canopa.Portal'
    Path(sys.argv[1]).write_text('prompted')
    print('false',flush=True)
    raise SystemExit(0)
c=socket.socket(fileno=request['authentication'])
challenge=dict(version=1,interactive=True,id=1,endpoint=dict(server='localhost',port=631,transport='loopback'),kind=dict(kind='server',realm='Fixture',resource='/'),attempt=1,suggested_user='desktop')
data=json.dumps(challenge).encode();c.sendall(len(data).to_bytes(4,'big')+data)
def receive(n):
    data=b''
    while len(data)<n:
        part=c.recv(n-len(data));assert part;data+=part
    return data
length=int.from_bytes(receive(4),'big');assert receive(length)==bytes([1,0,0,0,1,0])
print(json.dumps(dict(event='retired_printer_jobs',report=dict(retired=0,unresolved=1,busy=0))),flush=True)
"#).unwrap();
            let process = DialogProcess {
                executable: "/usr/bin/python3".into(),
                arguments: vec![script.into_os_string(), marker.clone().into_os_string()],
            };
            let broker = crate::print::authentication::Broker::new(
                process.clone(),
                crate::print::Context {
                    app_id: "org.canopa.Portal".into(),
                    parent_window: String::new(),
                    title: "Printer recovery".into(),
                    modal: false,
                },
            );
            let client = Client::new(process)
                .with_recovery(
                    crate::print::file_target::RecoveryStore::new(
                        &directory.path().join("recovery"),
                    )
                    .unwrap(),
                )
                .with_authentication(Some(broker));
            let (_cancel, cancelled) = watch::channel(false);
            let report = if explicit {
                client.review_printer_jobs(cancelled).await
            } else {
                client.recover_printer_jobs(cancelled).await
            }
            .unwrap()
            .unwrap();
            assert_eq!(report.unresolved, 1);
            assert_eq!(marker.exists(), explicit);
            client.shutdown().await;
        }
    }
}
