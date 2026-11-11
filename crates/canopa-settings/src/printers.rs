//! The Printers page's model: CUPS queues and jobs, driverless printers found
//! on the network, and the changes the page asks for.
//!
//! CUPS keeps the queues. Reading, your default printer (`lpoptions`), your
//! jobs and test pages use the CUPS command-line tools, which every user may
//! run. Adding and removing printers and cancelling other people's jobs go
//! through cups-pk-helper and polkit when it is installed; without it they
//! fall back to `lpadmin`, which works only for members of CUPS's system
//! group. Discovery uses `ippfind` (DNS-SD through Avahi) and adds printers as
//! driverless IPP Everywhere queues. This page is independent of Canopa's
//! optional print portal backend.
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_services::cups_pk_helper as helper;
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

fn not_running() -> String {
    fl!("settings-printers-not-running")
}
const TEST_PAGES: [&str; 2] = [
    "/usr/share/cups/data/default-testpage.pdf",
    "/usr/share/cups/data/testprint",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueState {
    Idle,
    /// Printing this job.
    Printing(String),
    /// Paused, with CUPS's reason when it gives one.
    Stopped(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queue {
    pub name: String,
    pub description: String,
    pub location: String,
    pub uri: String,
    pub state: QueueState,
    pub default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// `Queue-12`, as `cancel` takes it.
    pub id: String,
    pub number: i32,
    pub queue: String,
    pub user: String,
    pub size: u64,
}

/// A printer on the network that can be added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub uri: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrintersState {
    pub queues: Vec<Queue>,
    pub jobs: Vec<Job>,
    /// `None` until searched.
    pub found: Option<Vec<Found>>,
    /// cups-pk-helper can add and remove printers after polkit asks.
    pub helper: bool,
    /// Who is using Settings, for "your jobs".
    pub user: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrinterAction {
    Search,
    Add(Found),
    Remove(String),
    /// Your default printer.
    Default(String),
    Cancel(Job),
    TestPage(String),
}

impl LiveAction for PrinterAction {
    fn control(&self) -> String {
        match self {
            Self::Search => "search".into(),
            Self::Add(found) => format!("add-{}", found.uri),
            Self::Remove(name) | Self::TestPage(name) => format!("queue-{name}"),
            Self::Default(_) => "default".into(),
            Self::Cancel(job) => format!("job-{}", job.id),
        }
    }
}

/// `lpstat -l -p` output.
pub fn parse_queues(text: &str) -> Vec<Queue> {
    let mut queues: Vec<Queue> = Vec::new();
    let mut disabled = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("printer ") {
            let mut words = rest.split_whitespace();
            let Some(name) = words.next() else { continue };
            let rest = rest[name.len()..].trim_start();
            disabled = rest.starts_with("disabled");
            let state = if let Some(printing) = rest.strip_prefix("now printing ") {
                QueueState::Printing(printing.split(['.', ' ']).next().unwrap_or("").to_owned())
            } else if disabled {
                QueueState::Stopped(String::new())
            } else {
                QueueState::Idle
            };
            queues.push(Queue {
                name: name.to_owned(),
                description: String::new(),
                location: String::new(),
                uri: String::new(),
                state,
                default: false,
            });
        } else if let Some(queue) = queues.last_mut()
            && line.starts_with(['\t', ' '])
        {
            let line = line.trim();
            if let Some(description) = line.strip_prefix("Description:") {
                queue.description = description.trim().to_owned();
            } else if let Some(location) = line.strip_prefix("Location:") {
                queue.location = location.trim().to_owned();
            } else if disabled
                && !line.contains(':')
                && let QueueState::Stopped(reason) = &mut queue.state
                && reason.is_empty()
            {
                *reason = line.chars().take(120).collect();
            }
        }
    }
    queues.truncate(128);
    queues
}

/// `lpstat -v` output: queue and device URI.
fn parse_devices(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("device for ")?;
            let (name, uri) = rest.split_once(": ")?;
            Some((name.to_owned(), uri.trim().to_owned()))
        })
        .collect()
}

/// `lpstat -d` output.
fn parse_default(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("system default destination: "))
        .map(|name| name.trim().to_owned())
}

/// `lpstat -o` output.
pub fn parse_jobs(text: &str) -> Vec<Job> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let id = words.next()?;
            let user = words.next()?;
            let size = words.next()?.parse().ok()?;
            let (queue, number) = id.rsplit_once('-')?;
            Some(Job {
                id: id.to_owned(),
                number: number.parse().ok()?,
                queue: queue.to_owned(),
                user: user.to_owned(),
                size,
            })
        })
        .take(256)
        .collect()
}

/// `ippfind … -x printf '%s\t%s\n' {} {service_name} ;` output, one printer
/// per name (secure IPP first).
pub fn parse_found(text: &str) -> Vec<Found> {
    let mut found: Vec<Found> = Vec::new();
    for line in text.lines().take(256) {
        let Some((uri, name)) = line.split_once('\t') else {
            continue;
        };
        let (uri, name) = (uri.trim(), name.trim());
        if !(uri.starts_with("ipp://") || uri.starts_with("ipps://"))
            || uri.chars().any(char::is_control)
            || name.is_empty()
        {
            continue;
        }
        match found.iter_mut().find(|known| known.name == name) {
            Some(known) if uri.starts_with("ipps://") => known.uri = uri.to_owned(),
            Some(_) => {}
            None => found.push(Found {
                name: name.chars().take(127).collect(),
                uri: uri.to_owned(),
            }),
        }
    }
    found.sort_by_key(|printer| printer.name.to_lowercase());
    found
}

/// A queue name for a printer's network name: `HP LaserJet (Office)` →
/// `HP_LaserJet_Office`.
pub fn queue_name(name: &str) -> String {
    let mut queue = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
            queue.push(c);
        } else if !queue.ends_with('_') && !queue.is_empty() {
            queue.push('_');
        }
    }
    let queue = queue.trim_end_matches('_');
    let queue: String = queue.chars().take(127).collect();
    if queue.is_empty() {
        "Printer".into()
    } else {
        queue
    }
}

/// The CUPS programs this page runs (tests use their own).
#[derive(Clone, Debug)]
pub struct Tools {
    pub lpstat: PathBuf,
    pub lpadmin: PathBuf,
    pub lpoptions: PathBuf,
    pub lp: PathBuf,
    pub cancel: PathBuf,
    pub ippfind: PathBuf,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            lpstat: "lpstat".into(),
            lpadmin: "lpadmin".into(),
            lpoptions: "lpoptions".into(),
            lp: "lp".into(),
            cancel: "cancel".into(),
            ippfind: "ippfind".into(),
        }
    }
}

/// Run a CUPS tool in the C locale; stdout and stderr, and whether it succeeded.
async fn run(
    program: &Path,
    args: &[&str],
    deadline: Duration,
) -> Result<(bool, String, String), ServiceError> {
    let output = tokio::time::timeout(
        deadline,
        tokio::process::Command::new(program)
            .args(args)
            .env("LC_ALL", "C")
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| ServiceError::Failed(fl!("settings-printers-timed-out")))?
    .map_err(|_| {
        ServiceError::Unavailable(fl!(
            "settings-printers-tool-missing",
            program = program.display().to_string()
        ))
    })?;
    let text = |bytes: Vec<u8>| {
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        text.truncate(512 * 1024);
        text
    };
    Ok((
        output.status.success(),
        text(output.stdout),
        text(output.stderr),
    ))
}

/// A CUPS tool's refusal. `task` names what was tried: `add`, `remove`,
/// `default`, `cancel` or `test`.
fn refusal(stderr: &str, task: &str) -> ServiceError {
    let line = stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    let lower = line.to_lowercase();
    ServiceError::Failed(
        if lower.contains("forbidden")
            || lower.contains("not authorized")
            || lower.contains("unauthorized")
        {
            fl!("settings-printers-needs-permission", task = task)
        } else if line.is_empty() {
            fl!("settings-printers-task-failed", task = task)
        } else {
            // CUPS tools run in the C locale: `line` is English.
            fl!(
                "settings-printers-task-failed-because",
                task = task,
                error = line
            )
        },
    )
}

/// CUPS and cups-pk-helper for the page.
#[derive(Clone)]
pub struct PrintersService {
    tools: Tools,
    system: Arc<tokio::sync::OnceCell<Option<zbus::Connection>>>,
    found: Arc<Mutex<Option<Vec<Found>>>>,
    user: String,
}

impl Default for PrintersService {
    fn default() -> Self {
        Self {
            tools: Tools::default(),
            system: Arc::default(),
            found: Arc::default(),
            user: std::env::var("USER").unwrap_or_default(),
        }
    }
}

fn helper_failure(error: canopa_services::Error) -> ServiceError {
    use canopa_services::Error;
    ServiceError::Failed(match error {
        Error::NotAuthorized => fl!("settings-error-not-authorized"),
        Error::Unavailable => fl!("settings-printers-no-helper"),
        Error::TimedOut => fl!("settings-printers-timed-out"),
        Error::Invalid(what) => crate::service_text::invalid(what),
        Error::Changed => fl!("settings-printers-changed"),
        Error::Refused(message) => fl!("settings-printers-refused", reason = message),
    })
}

impl PrintersService {
    /// A service with its own tools, cups-pk-helper on `system`, as `user` (tests).
    pub fn with(tools: Tools, system: Option<zbus::Connection>, user: &str) -> Self {
        Self {
            tools,
            system: Arc::new(tokio::sync::OnceCell::new_with(Some(system))),
            found: Arc::default(),
            user: user.into(),
        }
    }

    async fn helper(&self) -> Option<zbus::Connection> {
        let system = self
            .system
            .get_or_init(|| async { canopa_services::bus::system().await.ok() })
            .await
            .clone()?;
        helper::available(&system).await.then_some(system)
    }

    async fn read(&self) -> Result<PrintersState, ServiceError> {
        let tools = &self.tools;
        let deadline = Duration::from_secs(5);
        let (_, scheduler, _) = run(&tools.lpstat, &["-r"], deadline).await?;
        if !scheduler.contains("scheduler is running") {
            return Err(ServiceError::Unavailable(not_running()));
        }
        let (queues, devices, default, jobs) = tokio::join!(
            run(&tools.lpstat, &["-l", "-p"], deadline),
            run(&tools.lpstat, &["-v"], deadline),
            run(&tools.lpstat, &["-d"], deadline),
            run(&tools.lpstat, &["-o"], deadline),
        );
        let mut queues = parse_queues(&queues?.1);
        let devices = parse_devices(&devices?.1);
        let default = parse_default(&default?.1);
        for queue in &mut queues {
            queue.uri = devices
                .iter()
                .find(|(name, _)| *name == queue.name)
                .map(|(_, uri)| uri.clone())
                .unwrap_or_default();
            queue.default = default.as_deref() == Some(&queue.name);
        }
        let found = self
            .found
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .map(|found| {
                found
                    .into_iter()
                    .filter(|printer| !queues.iter().any(|queue| queue.uri == printer.uri))
                    .collect()
            });
        Ok(PrintersState {
            queues,
            jobs: parse_jobs(&jobs?.1),
            found,
            helper: self.helper().await.is_some(),
            user: self.user.clone(),
        })
    }

    async fn run(&self, action: PrinterAction) -> Result<(), ServiceError> {
        let tools = &self.tools;
        let deadline = Duration::from_secs(30);
        match action {
            PrinterAction::Search => {
                let (_, out, err) = run(
                    &tools.ippfind,
                    &[
                        "-T",
                        "4",
                        "_ipp._tcp",
                        "_ipps._tcp",
                        "-x",
                        "printf",
                        "%s\\t%s\\n",
                        "{}",
                        "{service_name}",
                        ";",
                    ],
                    Duration::from_secs(15),
                )
                .await
                .map_err(|_| ServiceError::Failed(fl!("settings-printers-needs-ippfind")))?;
                // ippfind's own (C locale) messages.
                if err.contains("Daemon not running") || err.contains("Unable to use Bonjour") {
                    return Err(ServiceError::Failed(fl!("settings-printers-needs-avahi")));
                }
                // printf in ippfind's -x gets a literal \t; accept both.
                let text = out.replace("\\t", "\t").replace("\\n", "\n");
                *self.found.lock().unwrap_or_else(|error| error.into_inner()) =
                    Some(parse_found(&text));
                Ok(())
            }
            PrinterAction::Add(found) => {
                let name = queue_name(&found.name);
                if let Some(system) = self.helper().await {
                    helper::printer_add(&system, &name, &found.uri, "everywhere", &found.name, "")
                        .await
                        .map_err(helper_failure)?;
                    return helper::printer_enable(&system, &name)
                        .await
                        .map_err(helper_failure);
                }
                let (ok, _, err) = run(
                    &tools.lpadmin,
                    &[
                        "-p",
                        &name,
                        "-v",
                        &found.uri,
                        "-m",
                        "everywhere",
                        "-D",
                        &found.name,
                        "-E",
                    ],
                    deadline,
                )
                .await?;
                if ok {
                    Ok(())
                } else {
                    Err(refusal(&err, "add"))
                }
            }
            PrinterAction::Remove(name) => {
                if let Some(system) = self.helper().await {
                    return helper::printer_delete(&system, &name)
                        .await
                        .map_err(helper_failure);
                }
                let (ok, _, err) = run(&tools.lpadmin, &["-x", &name], deadline).await?;
                if ok {
                    Ok(())
                } else {
                    Err(refusal(&err, "remove"))
                }
            }
            PrinterAction::Default(name) => {
                let (ok, _, err) = run(&tools.lpoptions, &["-d", &name], deadline).await?;
                if ok {
                    Ok(())
                } else {
                    Err(refusal(&err, "default"))
                }
            }
            PrinterAction::Cancel(job) => {
                let (ok, _, err) = run(&tools.cancel, &[&job.id], deadline).await?;
                if ok {
                    return Ok(());
                }
                match self.helper().await {
                    Some(system) => helper::job_cancel(&system, job.number)
                        .await
                        .map_err(helper_failure),
                    None => Err(refusal(&err, "cancel")),
                }
            }
            PrinterAction::TestPage(name) => {
                let page = TEST_PAGES
                    .iter()
                    .find(|path| Path::new(path).is_file())
                    .ok_or_else(|| ServiceError::Failed(fl!("settings-printers-no-test-page")))?;
                // The job's title in print queues.
                let title = fl!("settings-printers-test-job");
                let (ok, _, err) =
                    run(&tools.lp, &["-d", &name, "-t", &title, page], deadline).await?;
                if ok {
                    Ok(())
                } else {
                    Err(refusal(&err, "test"))
                }
            }
        }
    }
}

impl LiveService for PrintersService {
    type State = PrintersState;
    type Action = PrinterAction;

    fn name(&self) -> String {
        "CUPS".into()
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<PrintersState, ServiceError>> {
        let service = self.clone();
        async move { service.read().await }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<PrintersState, ServiceError>> {
        // CUPS has no change signal for unprivileged clients; queues and jobs
        // are read again every few seconds while the page is shown.
        let service = self.clone();
        futures_util::stream::unfold(None, move |last| {
            let service = service.clone();
            async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    let state = service.read().await;
                    match state {
                        Ok(state) if last.as_ref() == Some(&state) => {}
                        Ok(state) => return Some((Ok(state.clone()), Some(state))),
                        Err(error) => return Some((Err(error), None)),
                    }
                }
            }
        })
        .boxed()
    }

    fn apply(
        &self,
        action: PrinterAction,
    ) -> BoxFuture<'static, Result<Option<PrintersState>, ServiceError>> {
        let service = self.clone();
        async move {
            service.run(action).await?;
            service.read().await.map(Some)
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{TestBus, cups_pk_helper::FakeCupsPkHelper};

    const QUEUES: &str = "printer Office is idle.  enabled since Tue Nov 11 10:00:00 2026\n\tForm mounted:\n\tDescription: Office printer\n\tAlerts: none\n\tLocation: 2nd floor\nprinter Laser now printing Laser-12.  enabled since Tue Nov 11 10:00:00 2026\n\tDescription: Laser\nprinter Old disabled since Tue Nov 11 10:00:00 2026 -\n\tPaused by the administrator\n\tDescription: Old one\n";

    #[test]
    fn lpstat_and_ippfind_output_is_read() {
        let queues = parse_queues(QUEUES);
        assert_eq!(queues.len(), 3);
        assert_eq!(queues[0].description, "Office printer");
        assert_eq!(queues[0].location, "2nd floor");
        assert_eq!(queues[0].state, QueueState::Idle);
        assert_eq!(queues[1].state, QueueState::Printing("Laser-12".into()));
        assert_eq!(
            queues[2].state,
            QueueState::Stopped("Paused by the administrator".into())
        );
        assert_eq!(
            parse_jobs("Laser-12 kevin 1024 Tue Nov 11 10:00:00 2026\nbad line\n"),
            [Job {
                id: "Laser-12".into(),
                number: 12,
                queue: "Laser".into(),
                user: "kevin".into(),
                size: 1024
            }]
        );
        assert_eq!(
            parse_default("system default destination: Office\n").as_deref(),
            Some("Office")
        );
        assert_eq!(parse_default("no system default destination\n"), None);
        let found = parse_found(
            "ipp://hp.local:631/ipp/print\tHP LaserJet (Office)\nipps://hp.local:443/ipp/print\tHP LaserJet (Office)\nhttp://x\tBad\nipp://epson.local/ipp/print\tEpson\n",
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].uri, "ipps://hp.local:443/ipp/print");
        assert_eq!(queue_name("HP LaserJet (Office)"), "HP_LaserJet_Office");
        assert_eq!(queue_name("///"), "Printer");
    }

    fn tools(dir: &Path) -> Tools {
        use std::os::unix::fs::PermissionsExt;
        let log = dir.join("log");
        let script = |name: &str, body: &str| {
            let path = dir.join(name);
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\necho \"{name} $*\" >> {}\n{body}\n",
                    log.display()
                ),
            )
            .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        let queues = QUEUES.replace('\t', "\\t").replace('\n', "\\n");
        Tools {
            lpstat: script(
                "lpstat",
                &format!(
                    r#"case "$1" in
  -r) [ -e {stopped} ] && echo 'scheduler is not running' || echo 'scheduler is running';;
  -l) printf '{queues}';;
  -v) printf 'device for Office: ipp://office.local/ipp/print\n';;
  -d) printf 'system default destination: Office\n';;
  -o) printf 'Laser-12 kevin 1024 Tue Nov 11 10:00:00 2026\nLaser-13 ada 2048 Tue Nov 11 10:00:00 2026\n';;
esac"#,
                    stopped = dir.join("stopped").display()
                ),
            ),
            lpadmin: script("lpadmin", "echo 'lpadmin: Forbidden' >&2; exit 1"),
            lpoptions: script("lpoptions", ""),
            lp: script("lp", ""),
            cancel: script(
                "cancel",
                r#"[ "$1" = Laser-12 ] || { echo 'cancel: Forbidden' >&2; exit 1; }"#,
            ),
            ippfind: script(
                "ippfind",
                r#"printf 'ipp://office.local/ipp/print\tOffice\nipp://hp.local/ipp/print\tHP LaserJet\n'"#,
            ),
        }
    }

    #[tokio::test]
    async fn the_page_reads_cups_and_asks_cups_pk_helper_for_changes() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tools(dir.path());
        let service = PrintersService::with(tools.clone(), None, "kevin");
        let state = service.snapshot().await.unwrap();
        assert_eq!(state.queues.len(), 3);
        assert!(state.queues[0].default && state.queues[0].uri.starts_with("ipp://office"));
        assert_eq!(state.jobs.len(), 2);
        assert!(!state.helper && state.found.is_none());

        // Discovery leaves out printers that already have a queue.
        let state = service.apply(PrinterAction::Search).await.unwrap().unwrap();
        let found = state.found.unwrap();
        assert_eq!(
            found,
            [Found {
                name: "HP LaserJet".into(),
                uri: "ipp://hp.local/ipp/print".into()
            }]
        );
        // Without cups-pk-helper, lpadmin's refusal says what to do.
        assert!(matches!(
            service.apply(PrinterAction::Add(found[0].clone())).await,
            Err(ServiceError::Failed(message)) if message.contains("cups-pk-helper")
        ));
        service
            .apply(PrinterAction::Default("Laser".into()))
            .await
            .unwrap();
        service
            .apply(PrinterAction::Cancel(state.jobs[0].clone()))
            .await
            .unwrap();
        assert!(
            service
                .apply(PrinterAction::Cancel(state.jobs[1].clone()))
                .await
                .is_err()
        );
        let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
        assert!(log.contains("lpoptions -d Laser") && log.contains("cancel Laser-12"));

        let bus = TestBus::new();
        let fake = FakeCupsPkHelper::serve(&bus).await;
        let service = PrintersService::with(tools, Some(bus.connect().await), "kevin");
        assert!(service.snapshot().await.unwrap().helper);
        service.apply(PrinterAction::Search).await.unwrap();
        service
            .apply(PrinterAction::Add(found[0].clone()))
            .await
            .unwrap();
        service
            .apply(PrinterAction::Cancel(state.jobs[1].clone()))
            .await
            .unwrap();
        service
            .apply(PrinterAction::Remove("HP_LaserJet".into()))
            .await
            .unwrap();
        assert_eq!(
            fake.calls.lock().unwrap().as_slice(),
            [
                "PrinterAdd HP_LaserJet ipp://hp.local/ipp/print everywhere",
                "PrinterSetEnabled HP_LaserJet",
                "PrinterSetAcceptJobs HP_LaserJet",
                "JobCancelPurge 13",
                "PrinterDelete HP_LaserJet"
            ]
        );

        std::fs::write(dir.path().join("stopped"), "").unwrap();
        assert!(
            matches!(service.snapshot().await, Err(ServiceError::Unavailable(m)) if m == not_running())
        );
    }
}
