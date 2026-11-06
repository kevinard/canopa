use anyhow::{Context, Result, bail, ensure};
use niri_ipc::{Output, Request, Response};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct Client {
    pub path: PathBuf,
    identity: (u64, u64),
}
impl Client {
    pub fn discover() -> Result<Self> {
        Self::connect(
            std::env::var_os("NIRI_SOCKET")
                .context("Niri is unavailable in this session")?
                .into(),
        )
    }
    pub fn connect(path: PathBuf) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(&path)?;
        ensure!(
            metadata.file_type().is_socket() && metadata.uid() == unsafe { libc::geteuid() },
            "invalid compositor socket"
        );
        Ok(Self {
            path,
            identity: (metadata.dev(), metadata.ino()),
        })
    }
    pub fn stream(&self) -> Result<UnixStream> {
        let metadata = std::fs::symlink_metadata(&self.path)?;
        ensure!(
            (metadata.dev(), metadata.ino()) == self.identity,
            "the compositor restarted; reload settings"
        );
        let stream = UnixStream::connect(&self.path)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        Ok(stream)
    }
    pub fn request(&self, request: Request) -> Result<Response> {
        let mut stream = self.stream()?;
        serde_json::to_writer(&mut stream, &request)?;
        stream.write_all(b"\n")?;
        let mut bytes = Vec::new();
        BufReader::new(stream)
            .take(256 * 1024 + 1)
            .read_until(b'\n', &mut bytes)?;
        ensure!(bytes.len() <= 256 * 1024, "compositor reply exceeds budget");
        let reply: niri_ipc::Reply = serde_json::from_slice(&bytes)?;
        reply.map_err(anyhow::Error::msg)
    }
    pub fn outputs(&self) -> Result<HashMap<String, Output>> {
        match self.request(Request::Outputs)? {
            Response::Outputs(outputs) => {
                ensure!(outputs.len() <= 32, "too many outputs");
                Ok(outputs)
            }
            _ => bail!("unexpected compositor output reply"),
        }
    }
    pub fn reload(&self) -> Result<()> {
        ensure!(
            matches!(
                self.request(Request::Action(niri_ipc::Action::LoadConfigFile {
                    path: None
                }))?,
                Response::Handled
            ),
            "compositor reload was not acknowledged"
        );
        Ok(())
    }
}

pub fn program() -> PathBuf {
    std::env::var_os("CANOPA_NIRI_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let packaged = PathBuf::from("/usr/lib/canopa/niri");
            if packaged.is_file() {
                packaged
            } else {
                PathBuf::from("niri")
            }
        })
}
pub fn validate(path: &Path) -> Result<()> {
    crate::storage::command(
        program(),
        &[
            "validate".into(),
            "--config".into(),
            path.to_string_lossy().into_owned(),
        ],
    )?;
    Ok(())
}

#[cfg(feature = "desktop")]
const MOTION_OVERRIDE: &str =
    "\n// Canopa Settings: reduce window and workspace motion.\nanimations { off; }\n";

#[cfg(feature = "desktop")]
pub(crate) fn reduced_motion(source: &crate::storage::Snapshot) -> Result<bool> {
    let enabled = source.text.ends_with(MOTION_OVERRIDE);
    ensure!(
        enabled
            || !source
                .text
                .contains("// Canopa Settings: reduce window and workspace motion."),
        "The compositor motion override was edited outside Settings."
    );
    Ok(enabled)
}

#[cfg(feature = "desktop")]
pub(crate) fn set_reduced_motion(paths: &crate::session::Paths, enabled: bool) -> Result<()> {
    ensure!(
        paths.active(),
        "Log into the Canopa session to change compositor motion."
    );
    let source = crate::storage::Snapshot::load(paths.niri())?;
    if reduced_motion(&source)? == enabled {
        return Ok(());
    }
    let text = if enabled {
        format!("{}{}", source.text, MOTION_OVERRIDE)
    } else {
        source
            .text
            .strip_suffix(MOTION_OVERRIDE)
            .unwrap()
            .to_owned()
    };
    save_main(&source, &text, || Ok(()))?;
    Ok(())
}

pub(crate) fn save_main(
    source: &crate::storage::Snapshot,
    text: &str,
    check_includes: impl Fn() -> Result<()>,
) -> Result<crate::storage::Snapshot> {
    let client = Client::discover()?;
    let saved = source.save(text, |candidate| {
        check_includes()?;
        validate(candidate)?;
        check_includes()
    })?;
    if let Err(error) = client.reload() {
        saved.save(&source.text, validate)?;
        let _ = client.reload();
        return Err(error.context("Compositor reload failed. Previous settings restored."));
    }
    Ok(saved)
}

pub fn validate_fragment(
    paths: &crate::session::Paths,
    name: &str,
    candidate: &Path,
) -> Result<()> {
    let snapshot = crate::storage::Snapshot::load(paths.niri())?;
    let mut document = kdl::KdlDocument::parse_v1(&snapshot.text)?;
    let mut matches = 0;
    for node in document.nodes_mut() {
        if node.name().value() == "include"
            && node.get(0).and_then(|entry| entry.as_string()) == Some(name)
        {
            let entry = node
                .entries_mut()
                .iter_mut()
                .find(|entry| entry.name().is_none())
                .context("include needs a filename")?;
            // Replacing only KdlValue would retain its old source representation.
            // Replace the entry so validation reads the candidate, not the old file.
            *entry = kdl::KdlEntry::new(
                candidate
                    .to_str()
                    .context("settings path must be UTF-8")?
                    .to_owned(),
            );
            matches += 1;
        }
    }
    ensure!(
        matches == 1,
        "the Canopa session configuration no longer includes these settings"
    );
    document.ensure_v1();
    let mut wrapper = tempfile::NamedTempFile::new_in(&paths.directory)?;
    wrapper.write_all(document.to_string().as_bytes())?;
    validate(wrapper.path())?;
    snapshot.unchanged()
}
