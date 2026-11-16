//! The login screen's display layout, `/etc/canopa/greeter-displays.kdl`.
//!
//! Settings sends the session's current outputs as JSON to
//! `canopa-greeter-displays apply`, which runs as root through `pkexec`. The
//! helper never copies text it receives: it validates every field and writes
//! only `output` nodes it generates itself, because the login compositor
//! includes the file and a Niri configuration can start programs.
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    collections::HashSet,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

pub const MAX_INPUT: u64 = 64 * 1024;
const MAX_OUTPUTS: usize = 16;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub outputs: Vec<Output>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub name: String,
    #[serde(default)]
    pub off: bool,
    pub mode: Option<Mode>,
    #[serde(default = "one")]
    pub scale: f64,
    #[serde(default = "normal")]
    pub transform: String,
    pub position: Option<Position>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    /// Hz, as Niri reports it divided by 1000.
    pub refresh: f64,
    #[serde(default)]
    pub custom: bool,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}
fn one() -> f64 {
    1.
}
fn normal() -> String {
    "normal".into()
}

const TRANSFORMS: &[&str] = &[
    "normal",
    "90",
    "180",
    "270",
    "flipped",
    "flipped-90",
    "flipped-180",
    "flipped-270",
];

/// Connector names as Niri reports them (`eDP-1`, `HDMI-A-1`, `DP-3`).
fn valid_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Validate a request and produce the file's text.
pub fn render(request: &Request) -> Result<String> {
    ensure!(
        (1..=MAX_OUTPUTS).contains(&request.outputs.len()),
        "expected between 1 and {MAX_OUTPUTS} outputs"
    );
    ensure!(
        request.outputs.iter().any(|output| !output.off),
        "at least one output must stay on"
    );
    let mut names = HashSet::new();
    let mut text = String::from(
        "// Written by Canopa Settings for the login screen; replaced on the next change.\n",
    );
    for output in &request.outputs {
        ensure!(valid_name(&output.name), "invalid output name");
        ensure!(
            names.insert(&output.name),
            "duplicate output {}",
            output.name
        );
        text.push_str(&format!("output \"{}\" {{\n", output.name));
        if output.off {
            text.push_str("    off\n}\n");
            continue;
        }
        if let Some(mode) = &output.mode {
            ensure!(
                (1..=16384).contains(&mode.width) && (1..=16384).contains(&mode.height),
                "invalid mode size for {}",
                output.name
            );
            ensure!(
                mode.refresh.is_finite() && (1. ..=1000.).contains(&mode.refresh),
                "invalid refresh rate for {}",
                output.name
            );
            text.push_str(&format!(
                "    mode \"{}x{}@{:.3}\"{}\n",
                mode.width,
                mode.height,
                mode.refresh,
                if mode.custom { " custom=true" } else { "" }
            ));
        }
        ensure!(
            output.scale.is_finite() && (0.25..=10.).contains(&output.scale),
            "invalid scale for {}",
            output.name
        );
        text.push_str(&format!("    scale {}\n", output.scale));
        ensure!(
            TRANSFORMS.contains(&output.transform.as_str()),
            "invalid transform for {}",
            output.name
        );
        text.push_str(&format!("    transform \"{}\"\n", output.transform));
        if let Some(Position { x, y }) = output.position {
            ensure!(
                x.unsigned_abs() <= 1_000_000 && y.unsigned_abs() <= 1_000_000,
                "invalid position for {}",
                output.name
            );
            text.push_str(&format!("    position x={x} y={y}\n"));
        }
        text.push_str("}\n");
    }
    Ok(text)
}

pub fn parse(input: &[u8]) -> Result<Request> {
    ensure!(input.len() as u64 <= MAX_INPUT, "request too large");
    serde_json::from_slice(input).context("invalid display request")
}

/// Replace `target` atomically. The directory is created root-owned when
/// missing and must otherwise be root-owned and not group or world writable.
pub fn write(target: &Path, text: &str, require_root: bool) -> Result<()> {
    let directory = target.parent().context("display file has no directory")?;
    if !directory.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o755)
            .create(directory)?;
    }
    if require_root {
        crate::config::check_system_source(directory)?;
    }
    let name = target.file_name().context("display file has no name")?;
    let temporary = directory.join(format!(".{}.new", name.to_string_lossy()));
    let _ = std::fs::remove_file(&temporary);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&temporary)?;
    let result = (|| -> Result<()> {
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, target)?;
        std::fs::File::open(directory)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn reset(target: &Path) -> Result<()> {
    match std::fs::remove_file(target) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// The helper's entry point. As root (the `pkexec` case) the target is always
/// the system file; `--target` exists for tests and is refused as root.
pub fn main(arguments: &[String], input: impl std::io::Read) -> Result<()> {
    let root = unsafe { libc::geteuid() } == 0;
    let (operation, target) = match arguments {
        [operation] => (operation.as_str(), None),
        [operation, flag, path] if flag == "--target" => {
            ensure!(!root, "--target is for unprivileged tests");
            (operation.as_str(), Some(Path::new(path)))
        }
        _ => bail!("usage: canopa-greeter-displays apply|reset [--target FILE]"),
    };
    let target = target.unwrap_or(Path::new(crate::compositor::LOGIN_DISPLAYS));
    match operation {
        "apply" => {
            let mut bytes = Vec::new();
            input.take(MAX_INPUT + 1).read_to_end(&mut bytes)?;
            let text = render(&parse(&bytes)?)?;
            write(target, &text, root)
        }
        "reset" => reset(target),
        other => bail!("unknown operation {other:?}"),
    }
}
