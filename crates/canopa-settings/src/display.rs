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
    io::{BufRead, Read, Write},
    os::fd::AsRawFd,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const CONFIRM_SECONDS: u64 = 20;
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    fn matches_live(&self, output: &Output) -> bool {
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
            .context("select a monitor mode")?;
        ensure!(
            output.modes.contains(mode),
            "the selected monitor mode is no longer available"
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
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let outputs = client.outputs()?;
        if monitors.iter().all(|monitor| {
            outputs
                .get(&monitor.name)
                .is_some_and(|output| monitor.matches_live(output))
        }) {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "the compositor could not apply the requested display configuration"
        );
        std::thread::sleep(Duration::from_millis(30));
    }
}

pub fn validate(monitors: &[Monitor], outputs: &HashMap<String, Output>) -> Result<()> {
    ensure!(
        !monitors.is_empty() && monitors.len() <= 32,
        "select at least one connected display"
    );
    ensure!(
        monitors.iter().any(|m| m.enabled),
        "at least one display must remain enabled"
    );
    let mut names = HashSet::new();
    for monitor in monitors {
        let output = outputs
            .get(&monitor.name)
            .context("a monitor was disconnected; reload displays")?;
        ensure!(
            names.insert(&monitor.name) && monitor.identity(output),
            "monitor identity changed; reload displays"
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

pub fn guard() -> Result<()> {
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
        "display persistence requires the Canopa session"
    );
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .context("session runtime directory is unavailable")?;
    let _lease = lock(&runtime.join("canopa-settings-displays.lock"))?;
    let client = Client::discover()?;
    let outputs = client.outputs()?;
    validate(&request.monitors, &outputs)?;
    let snapshot = Snapshot::load(paths.displays())?;
    let candidate = text(&snapshot, &request.monitors, &outputs)?;
    let mut prepared = tempfile::NamedTempFile::new_in(&paths.directory)?;
    prepared.write_all(candidate.as_bytes())?;
    crate::niri::validate_fragment(&paths, "niri-displays.kdl", prepared.path())?;
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
        for monitor in request
            .monitors
            .iter()
            .filter(|m| m.enabled)
            .chain(request.monitors.iter().filter(|m| !m.enabled))
        {
            apply(
                &client,
                monitor,
                outputs.get(&monitor.name).context("missing output")?,
            )?;
        }
        settled(&client, &request.monitors)?;
        fully_applied = true;
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
                        validate(&request.monitors, &current)?;
                        ensure!(
                            request.monitors.iter().all(|monitor| current
                                .get(&monitor.name)
                                .is_some_and(|output| monitor.matches_live(output))),
                            "display settings changed outside the trial; reload before saving"
                        );
                        let saved = snapshot.save(&candidate, |path| {
                            crate::niri::validate_fragment(&paths, "niri-displays.kdl", path)
                        })?;
                        if let Err(error) = client
                            .reload()
                            .and_then(|_| settled(&client, &request.monitors))
                        {
                            saved.save(&snapshot.text, |_| Ok(()))?;
                            let _ = client.reload();
                            return Err(error.context(
                                "saved display settings were not applied; previous file restored",
                            ));
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
        let current = client.outputs().unwrap_or_default();
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
                if let Err(error) = apply(&client, monitor, output) {
                    failures.push(error.to_string());
                }
            }
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
