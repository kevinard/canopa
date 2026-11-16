use crate::{
    niri::Client,
    session::Paths,
    storage::{Snapshot, lock, quote},
};
use anyhow::{Context, Result, ensure};
use niri_ipc::{
    ConfiguredMode, ConfiguredPosition, ModeToSet, Output, OutputAction, PositionToSet, Request,
    Response, ScaleToSet, Transform,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Read, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub const CONFIRM_SECONDS: u64 = 20;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Monitor {
    pub name: String,
    pub make: String,
    pub model: String,
    pub serial: Option<String>,
    pub enabled: bool,
    pub mode: Option<usize>,
    pub modes: Vec<niri_ipc::Mode>,
    pub custom_mode: bool,
    pub scale: f64,
    pub transform: Transform,
    pub x: i32,
    pub y: i32,
}
impl Monitor {
    pub fn from_output(value: &Output) -> Self {
        Self {
            name: value.name.clone(),
            make: value.make.clone(),
            model: value.model.clone(),
            serial: value.serial.clone(),
            enabled: value.current_mode.is_some(),
            mode: value
                .current_mode
                .or_else(|| value.modes.iter().position(|m| m.is_preferred))
                .or((!value.modes.is_empty()).then_some(0)),
            modes: value.modes.clone(),
            custom_mode: value.is_custom_mode,
            scale: value.logical.map_or(1., |v| v.scale),
            transform: value.logical.map_or(Transform::Normal, |v| v.transform),
            x: value.logical.map_or(0, |v| v.x),
            y: value.logical.map_or(0, |v| v.y),
        }
    }
    fn identity(&self, output: &Output) -> bool {
        self.name == output.name
            && self.make == output.make
            && self.model == output.model
            && self.serial == output.serial
    }
    pub(crate) fn matches_live(&self, output: &Output) -> bool {
        if !self.identity(output) {
            return false;
        }
        if !self.enabled {
            return output.current_mode.is_none();
        }
        let Some(mode) = self.mode.and_then(|i| self.modes.get(i)) else {
            return false;
        };
        let Some(current) = output.current_mode.and_then(|i| output.modes.get(i)) else {
            return false;
        };
        let Some(logical) = output.logical else {
            return false;
        };
        mode.width == current.width
            && mode.height == current.height
            && mode.refresh_rate.abs_diff(current.refresh_rate) <= 2
            && (self.scale - logical.scale).abs() < 0.0001
            && self.transform == logical.transform
            && self.x == logical.x
            && self.y == logical.y
    }
    fn actions(&self, output: &Output) -> Result<Vec<OutputAction>> {
        if !self.enabled {
            return Ok(vec![OutputAction::Off]);
        }
        let mode = self
            .mode
            .and_then(|index| self.modes.get(index))
            .with_context(|| fl!("settings-error-display-select-mode"))?;
        ensure!(
            output.modes.contains(mode),
            "{}",
            fl!("settings-error-display-mode-gone")
        );
        let configured = ConfiguredMode {
            width: mode.width,
            height: mode.height,
            refresh: Some(f64::from(mode.refresh_rate) / 1000.),
        };
        let action = if self.custom_mode {
            OutputAction::CustomMode { mode: configured }
        } else {
            OutputAction::Mode {
                mode: ModeToSet::Specific(configured),
            }
        };
        Ok(vec![
            OutputAction::On,
            action,
            OutputAction::Scale {
                scale: ScaleToSet::Specific(self.scale),
            },
            OutputAction::Transform {
                transform: self.transform,
            },
            OutputAction::Position {
                position: PositionToSet::Specific(ConfiguredPosition {
                    x: self.x,
                    y: self.y,
                }),
            },
        ])
    }
}

fn settled(client: &Client, monitors: &[Monitor]) -> Result<()> {
    wait_for_outputs(
        client,
        |outputs| {
            monitors.iter().all(|monitor| {
                outputs
                    .get(&monitor.name)
                    .is_some_and(|output| monitor.matches_live(output))
            })
        },
        "the compositor could not apply the requested display configuration",
    )
}

fn wait_for_outputs(
    client: &Client,
    ready: impl Fn(&HashMap<String, Output>) -> bool,
    failure: &str,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if ready(&client.outputs()?) {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "{failure}");
        std::thread::sleep(Duration::from_millis(30));
    }
}

pub fn validate(monitors: &[Monitor], outputs: &HashMap<String, Output>) -> Result<()> {
    ensure!(
        !monitors.is_empty() && monitors.len() <= 32,
        "{}",
        fl!("settings-error-display-select")
    );
    ensure!(
        monitors.iter().any(|m| m.enabled),
        "{}",
        fl!("settings-error-display-one-enabled")
    );
    let mut names = HashSet::new();
    for monitor in monitors {
        let output = outputs
            .get(&monitor.name)
            .with_context(|| fl!("settings-error-display-disconnected"))?;
        ensure!(
            names.insert(&monitor.name) && monitor.identity(output),
            "{}",
            fl!("settings-error-display-identity")
        );
        ensure!(
            (0.5..=4.).contains(&monitor.scale)
                && (-32768..=32768).contains(&monitor.x)
                && (-32768..=32768).contains(&monitor.y),
            "display scale or position is outside its supported range"
        );
        monitor.actions(output)?;
    }
    Ok(())
}

pub(crate) fn validate_profile(
    monitors: &[Monitor],
    outputs: &HashMap<String, Output>,
) -> Result<()> {
    ensure!(
        monitors.len() == outputs.len(),
        "{}",
        fl!("settings-error-display-profile-mismatch")
    );
    validate(monitors, outputs)
}

fn lease() -> Result<std::fs::File> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .context("session runtime directory is unavailable")?;
    lock(&runtime.join("canopa-settings-displays.lock"))
}

pub(crate) fn recover_after_hotplug(client: &Client) -> Result<()> {
    let _lease = lease()?;
    enable_visible_output(client)
}

fn enable_visible_output(client: &Client) -> Result<()> {
    let outputs = client.outputs()?;
    if outputs.values().any(|output| output.current_mode.is_some()) {
        return Ok(());
    }
    let Some(output) = outputs
        .values()
        .filter(|output| !output.modes.is_empty())
        .min_by_key(|output| {
            let internal = ["eDP-", "LVDS-", "DSI-"]
                .iter()
                .any(|prefix| output.name.starts_with(prefix));
            (!internal, &output.name)
        })
    else {
        return Ok(());
    };
    ensure!(
        matches!(
            client.request(Request::Output {
                output: output.name.clone(),
                action: OutputAction::On,
            })?,
            Response::OutputConfigChanged(niri_ipc::OutputConfigChanged::Applied)
        ),
        "could not turn on a connected display"
    );
    wait_for_outputs(
        client,
        |outputs| outputs.values().any(|output| output.current_mode.is_some()),
        "no connected display turned on",
    )
}

fn apply(client: &Client, monitor: &Monitor, output: &Output) -> Result<()> {
    ensure!(monitor.identity(output), "monitor identity changed");
    for action in monitor.actions(output)? {
        ensure!(
            matches!(
                client.request(Request::Output {
                    output: monitor.name.clone(),
                    action
                })?,
                Response::OutputConfigChanged(niri_ipc::OutputConfigChanged::Applied)
            ),
            "monitor disappeared while applying settings"
        );
    }
    Ok(())
}

pub fn text(
    snapshot: &Snapshot,
    monitors: &[Monitor],
    outputs: &HashMap<String, Output>,
) -> Result<String> {
    let mut document = kdl::KdlDocument::parse_v1(&snapshot.text)?;
    let names: HashSet<_> = monitors.iter().map(|m| m.name.as_str()).collect();
    let mut previous = HashMap::new();
    for node in document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "output")
    {
        if let Some(name) = node
            .get(0)
            .and_then(|v| v.as_string())
            .filter(|name| names.contains(name))
        {
            ensure!(
                previous.insert(name.to_owned(), node.clone()).is_none(),
                "duplicate managed output definitions are read-only"
            );
        }
    }
    document.nodes_mut().retain(|node| {
        node.name().value() != "output"
            || !names.contains(node.get(0).and_then(|v| v.as_string()).unwrap_or(""))
    });
    for monitor in monitors {
        let output = outputs.get(&monitor.name).context("monitor disconnected")?;
        let mut body = format!("output {} {{\n", quote(&monitor.name));
        if monitor.enabled {
            let mode = monitor
                .mode
                .and_then(|index| monitor.modes.get(index))
                .context("missing output mode")?;
            ensure!(output.modes.contains(mode), "monitor modes changed");
            body.push_str(&format!(
                "    mode {}{}\n    scale {}\n    transform {}\n    position x={} y={}\n",
                quote(&format!(
                    "{}x{}@{:.3}",
                    mode.width,
                    mode.height,
                    f64::from(mode.refresh_rate) / 1000.
                )),
                if monitor.custom_mode {
                    " custom=true"
                } else {
                    ""
                },
                monitor.scale,
                quote(transform_name(monitor.transform)),
                monitor.x,
                monitor.y
            ));
        } else {
            body.push_str("    off\n");
        }
        body.push_str("}\n");
        let mut generated = kdl::KdlDocument::parse_v1(&body)?.nodes()[0].clone();
        if let Some(mut old) = previous.remove(&monitor.name) {
            let children = old.ensure_children();
            children.nodes_mut().retain(|node| {
                !matches!(
                    node.name().value(),
                    "mode" | "scale" | "transform" | "position" | "off"
                )
            });
            children
                .nodes_mut()
                .extend(generated.children().unwrap().nodes().iter().cloned());
            generated = old;
        }
        document.nodes_mut().push(generated);
    }
    document.ensure_v1();
    Ok(document.to_string())
}
pub fn transform_name(value: Transform) -> &'static str {
    match value {
        Transform::Normal => "normal",
        Transform::_90 => "90",
        Transform::_180 => "180",
        Transform::_270 => "270",
        Transform::Flipped => "flipped",
        Transform::Flipped90 => "flipped-90",
        Transform::Flipped180 => "flipped-180",
        Transform::Flipped270 => "flipped-270",
    }
}

/// Parent and helper exchange only bounded frames on inherited private pipes.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialRequest {
    pub directory: PathBuf,
    pub monitors: Vec<Monitor>,
}
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
extern "C" fn interrupt(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}

pub fn guard(profile: Option<&str>, automatic: bool) -> Result<()> {
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .take(128 * 1024 + 1)
        .read_line(&mut line)?;
    ensure!(line.len() <= 128 * 1024, "display request exceeds budget");
    let request: TrialRequest = serde_json::from_str(&line)?;
    let paths = Paths {
        directory: request.directory,
    };
    ensure!(
        paths.directory.is_absolute() && paths.active(),
        "{}",
        fl!("settings-error-display-session")
    );
    let _lease = lease()?;
    let client = Client::discover()?;
    let outputs = client.outputs()?;
    let profile_source = if let Some(name) = profile {
        use crate::profiles::{Book, Switching};
        let book = Book::load(&paths)?;
        let expected = if automatic {
            ensure!(
                book.switching == Switching::Automatic,
                "automatic switching was disabled"
            );
            let (profile, layout) = book
                .matching(&outputs)?
                .with_context(|| fl!("settings-error-display-no-profile"))?;
            ensure!(profile.name == name, "the matching profile changed");
            layout
        } else {
            book.get(name)?.resolve(&outputs)?
        };
        ensure!(
            expected == request.monitors,
            "the profile or connected displays changed"
        );
        validate_profile(&request.monitors, &outputs)?;
        Some(book.source)
    } else {
        validate(&request.monitors, &outputs)?;
        None
    };
    let persistence = if profile.is_none() {
        let snapshot = Snapshot::load(paths.displays())?;
        let candidate = text(&snapshot, &request.monitors, &outputs)?;
        let mut prepared = tempfile::NamedTempFile::new_in(&paths.directory)?;
        prepared.write_all(candidate.as_bytes())?;
        crate::niri::validate_fragment(&paths, "niri-displays.kdl", prepared.path())?;
        Some((snapshot, candidate))
    } else {
        None
    };
    for signum in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        unsafe {
            libc::signal(signum, interrupt as *const () as libc::sighandler_t);
        }
    }
    let before: Vec<_> = request
        .monitors
        .iter()
        .filter_map(|m| outputs.get(&m.name).map(Monitor::from_output))
        .collect();
    let mut fully_applied = false;
    let outcome = (|| -> Result<bool> {
        if let Some(source) = &profile_source {
            source.unchanged()?;
        }
        for enabled in [true, false] {
            if !enabled && profile.is_some() && request.monitors.iter().any(|m| !m.enabled) {
                let current = client.outputs()?;
                validate_profile(&request.monitors, &current)?;
                ensure!(
                    request
                        .monitors
                        .iter()
                        .any(|m| m.enabled && current[&m.name].current_mode.is_some()),
                    "the replacement display did not turn on"
                );
            }
            for monitor in request.monitors.iter().filter(|m| m.enabled == enabled) {
                let current;
                let available = if profile.is_some() {
                    ensure!(
                        !INTERRUPTED.load(Ordering::Relaxed),
                        "display switch interrupted"
                    );
                    current = client.outputs()?;
                    validate_profile(&request.monitors, &current)?;
                    &current
                } else {
                    &outputs
                };
                let output = available.get(&monitor.name).context("missing output")?;
                if profile.is_none() || !monitor.matches_live(output) {
                    apply(&client, monitor, output)?;
                }
            }
        }
        settled(&client, &request.monitors)?;
        fully_applied = true;
        if profile.is_some() {
            validate_profile(&request.monitors, &client.outputs()?)?;
        }
        // A vanished GUI is an ordinary I/O failure: it must take the rollback
        // path, including when it disappears before this readiness frame.
        writeln!(
            std::io::stdout(),
            "{{\"state\":\"ready\",\"seconds\":{CONFIRM_SECONDS}}}"
        )?;
        std::io::stdout().flush()?;
        let deadline = Instant::now() + Duration::from_secs(CONFIRM_SECONDS);
        let stdin = std::io::stdin();
        let fd = stdin.as_raw_fd();
        unsafe {
            libc::fcntl(
                fd,
                libc::F_SETFL,
                libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK,
            );
        }
        let mut command = Vec::new();
        while Instant::now() < deadline && !INTERRUPTED.load(Ordering::Relaxed) {
            let mut byte = [0];
            match stdin.lock().read(&mut byte) {
                Ok(0) => return Ok(false),
                Ok(_) if byte[0] == b'\n' => {
                    if command == b"keep" {
                        let current = client.outputs()?;
                        if let Some(source) = &profile_source {
                            source.unchanged()?;
                            validate_profile(&request.monitors, &current)?;
                        } else {
                            validate(&request.monitors, &current)?;
                        }
                        ensure!(
                            request.monitors.iter().all(|monitor| current
                                .get(&monitor.name)
                                .is_some_and(|output| monitor.matches_live(output))),
                            "display settings changed outside the trial; reload before saving"
                        );
                        let Some((snapshot, candidate)) = &persistence else {
                            return Ok(true);
                        };
                        let saved = snapshot.save(candidate, |path| {
                            crate::niri::validate_fragment(&paths, "niri-displays.kdl", path)
                        })?;
                        if let Err(error) = client
                            .reload()
                            .and_then(|_| settled(&client, &request.monitors))
                        {
                            saved.save(&snapshot.text, |_| Ok(()))?;
                            let _ = client.reload();
                            return Err(error.context(fl!("settings-error-display-not-applied")));
                        }
                        return Ok(true);
                    }
                    return Ok(false);
                }
                Ok(_) => {
                    command.push(byte[0]);
                    ensure!(command.len() <= 16, "invalid display decision");
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(25))
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(false)
    })();
    if !matches!(outcome, Ok(true)) {
        let current = client
            .outputs()
            .context("cannot inspect displays for restoration")?;
        let mut failures = Vec::new();
        for monitor in before
            .iter()
            .filter(|m| m.enabled)
            .chain(before.iter().filter(|m| !m.enabled))
        {
            if let Some(output) = current
                .get(&monitor.name)
                .filter(|output| monitor.identity(output))
            {
                if profile.is_some() && monitor.matches_live(output) {
                    continue;
                }
                if fully_applied
                    && !request
                        .monitors
                        .iter()
                        .any(|trial| trial.name == monitor.name && trial.matches_live(output))
                {
                    if !monitor.matches_live(output) {
                        failures.push(format!(
                            "{} changed outside the trial and was preserved",
                            monitor.name
                        ));
                    }
                    continue;
                }
                if profile.is_some() && !monitor.enabled {
                    let live = client.outputs()?;
                    if !live
                        .values()
                        .any(|other| other.name != monitor.name && other.current_mode.is_some())
                    {
                        // A hot-unplug may have removed the original active display.
                        continue;
                    }
                }
                if let Err(error) = apply(&client, monitor, output) {
                    failures.push(error.to_string());
                }
            }
        }
        if profile.is_some() && before.iter().any(|monitor| monitor.enabled) {
            enable_visible_output(&client)?;
        }
        ensure!(
            failures.is_empty(),
            "display restoration failed: {}",
            failures.join("; ")
        );
    }
    let kept = outcome?;
    writeln!(
        std::io::stdout(),
        "{{\"state\":\"{}\"}}",
        if kept { "kept" } else { "reverted" }
    )?;
    Ok(())
}

#[derive(Debug)]
struct Trial {
    child: Child,
    profile: bool,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Drop for Trial {
    fn drop(&mut self) {
        let _ = self.input.write_all(b"revert\n");
        let _ = self.input.flush();
        let _ = self.child.wait();
    }
}
#[derive(Clone, Debug)]
pub(crate) struct TrialHandle(Arc<Mutex<Trial>>);
impl TrialHandle {
    pub(crate) fn start(
        paths: &Paths,
        monitors: Vec<Monitor>,
        profile: Option<&str>,
        automatic: bool,
    ) -> Result<Self> {
        let mut command = Command::new(canopa_file::executable::current()?);
        command.arg("display-guard");
        if let Some(name) = profile {
            command.arg(format!("--profile={name}"));
        }
        if automatic {
            command.arg("--automatic");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .process_group(0)
            .spawn()?;
        let input = child.stdin.take().context("missing display helper input")?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .context("missing display helper output")?,
        );
        let mut trial = Trial {
            child,
            profile: profile.is_some(),
            input,
            output,
        };
        serde_json::to_writer(
            &mut trial.input,
            &TrialRequest {
                directory: paths.directory.clone(),
                monitors,
            },
        )?;
        trial.input.write_all(b"\n")?;
        trial.input.flush()?;
        let reply = frame(&mut trial.output)?;
        ensure!(
            reply["state"] == "ready",
            "{}",
            reply["message"]
                .as_str()
                .unwrap_or("display helper did not become ready")
        );
        Ok(Self(Arc::new(Mutex::new(trial))))
    }
    pub(crate) fn finish(self, keep: bool) -> Result<String> {
        let mut trial = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("display helper lock failed"))?;
        // The independent helper may already have reverted on its own deadline.
        let _ = trial
            .input
            .write_all(if keep { b"keep\n" } else { b"revert\n" });
        let reply = frame(&mut trial.output)?;
        let _ = trial.child.wait();
        match reply["state"].as_str() {
            Some("kept") if trial.profile => Ok(fl!("settings-displays-profile-applied")),
            Some("kept") => Ok(fl!("settings-displays-saved")),
            Some("reverted") => Ok(fl!("settings-displays-reverted")),
            _ => anyhow::bail!(
                "{}",
                reply["message"].as_str().unwrap_or("display change failed")
            ),
        }
    }
}
fn frame(reader: &mut BufReader<ChildStdout>) -> Result<serde_json::Value> {
    let mut line = String::new();
    reader.take(16385).read_line(&mut line)?;
    ensure!(
        !line.is_empty() && line.len() <= 16384,
        "display helper stopped unexpectedly"
    );
    Ok(serde_json::from_str(&line)?)
}

/// Installed with the login screen package; runs as root through `pkexec`.
pub const LOGIN_DISPLAYS_HELPER: &str = "/usr/lib/canopa/greeter-displays";

/// Whether the login screen's display layout can be set from here.
pub fn login_screen_available() -> bool {
    std::path::Path::new(LOGIN_DISPLAYS_HELPER).is_file()
}

/// The applied outputs (not the editor's draft) as the helper's request.
pub fn login_request(outputs: &HashMap<String, Output>) -> serde_json::Value {
    let mut names: Vec<_> = outputs.keys().collect();
    names.sort();
    let outputs: Vec<_> = names
        .into_iter()
        .map(|name| {
            let output = &outputs[name];
            match (
                output.current_mode.and_then(|i| output.modes.get(i)),
                output.logical,
            ) {
                (Some(mode), Some(logical)) => serde_json::json!({
                    "name": output.name,
                    "mode": {
                        "width": mode.width,
                        "height": mode.height,
                        "refresh": f64::from(mode.refresh_rate) / 1000.,
                        "custom": output.is_custom_mode,
                    },
                    "scale": logical.scale,
                    "transform": transform_name(logical.transform),
                    "position": {"x": logical.x, "y": logical.y},
                }),
                _ => serde_json::json!({"name": output.name, "off": true}),
            }
        })
        .collect();
    serde_json::json!({ "outputs": outputs })
}

/// Ask polkit for administrator authorization and hand the layout to the helper.
pub fn use_for_login_screen(outputs: &HashMap<String, Output>) -> Result<String> {
    let request = serde_json::to_vec(&login_request(outputs))?;
    let mut child = Command::new("pkexec")
        .arg(LOGIN_DISPLAYS_HELPER)
        .arg("apply")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| fl!("settings-displays-login-screen-failed", error = "pkexec"))?;
    // The helper reads at most 64 KiB; a closed pipe surfaces as its error below.
    let _ = child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(&request));
    let mut diagnostics = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let _ = stderr.take(4096).read_to_end(&mut diagnostics);
    }
    let status = child.wait()?;
    match status.code() {
        Some(0) => Ok(fl!("settings-displays-login-screen-saved")),
        // pkexec: the dialog was dismissed, or authorization was refused.
        Some(126 | 127) => Err(anyhow::anyhow!(
            "{}",
            fl!("settings-displays-login-screen-denied")
        )),
        _ => {
            let detail = String::from_utf8_lossy(&diagnostics);
            let detail = detail
                .trim()
                .trim_start_matches("canopa-greeter-displays: ")
                .to_owned();
            Err(anyhow::anyhow!(
                "{}",
                fl!("settings-displays-login-screen-failed", error = detail)
            ))
        }
    }
}
