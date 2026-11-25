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
    /// Focus the window this process mapped with `app_id`.
    pub fn focus_own_window(&self, app_id: &str) -> Result<()> {
        let Response::Windows(windows) = self.request(Request::Windows)? else {
            bail!("unexpected compositor window reply");
        };
        let pid = i32::try_from(std::process::id())?;
        let window = windows
            .iter()
            .find(|window| window.pid == Some(pid) && window.app_id.as_deref() == Some(app_id))
            .context("the window is not mapped yet")?;
        ensure!(
            matches!(
                self.request(Request::Action(niri_ipc::Action::FocusWindow {
                    id: window.id
                }))?,
                Response::Handled
            ),
            "compositor focus was not acknowledged"
        );
        Ok(())
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
const MOTION_MARKER: &str = "// Canopa Settings: reduce window and workspace motion.";

/// The text without Settings' reduced-motion block, and whether it was there.
/// The block is recognized wherever it sits, so text appended after it does not
/// lock the control; only a marker without its exact block (or two blocks) is
/// an outside edit.
#[cfg(feature = "desktop")]
fn split_motion(text: &str) -> Result<(String, bool)> {
    let blocks = text.matches(MOTION_OVERRIDE).count();
    ensure!(
        blocks <= 1 && text.matches(MOTION_MARKER).count() == blocks,
        "{}",
        fl!("settings-error-motion-edited")
    );
    Ok((text.replacen(MOTION_OVERRIDE, "", 1), blocks == 1))
}

#[cfg(feature = "desktop")]
pub(crate) fn reduced_motion(source: &crate::storage::Snapshot) -> Result<bool> {
    split_motion(&source.text).map(|(_, enabled)| enabled)
}

#[cfg(feature = "desktop")]
pub(crate) fn set_reduced_motion(paths: &crate::session::Paths, enabled: bool) -> Result<()> {
    ensure!(paths.active(), "{}", fl!("settings-error-motion-session"));
    let source = crate::storage::Snapshot::load(paths.niri())?;
    let (without, current) = split_motion(&source.text)?;
    if current == enabled {
        return Ok(());
    }
    let text = if enabled {
        format!("{without}{MOTION_OVERRIDE}")
    } else {
        without
    };
    save_main(&source, &text, || Ok(()))?;
    Ok(())
}

/// Window layout choices Settings writes over the shipped Niri defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowLayout {
    /// Space around and between windows, in logical pixels.
    pub gaps: u8,
    pub center: CenterColumn,
    pub width: ColumnWidth,
}

/// Niri's `center-focused-column`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CenterColumn {
    Never,
    OnOverflow,
    Always,
}

/// The width new columns open at, as a share of the working area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnWidth {
    Third,
    Half,
    TwoThirds,
    Full,
}

impl CenterColumn {
    pub const ALL: [Self; 3] = [Self::Never, Self::OnOverflow, Self::Always];
    fn kdl(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::OnOverflow => "on-overflow",
            Self::Always => "always",
        }
    }
}

impl ColumnWidth {
    pub const ALL: [Self; 4] = [Self::Third, Self::Half, Self::TwoThirds, Self::Full];
    fn kdl(self) -> &'static str {
        match self {
            Self::Third => "0.33333",
            Self::Half => "0.5",
            Self::TwoThirds => "0.66667",
            Self::Full => "1.0",
        }
    }
}

impl WindowLayout {
    /// Gap sizes offered by Settings.
    pub const GAPS: [u8; 5] = [0, 6, 12, 16, 20];
    /// The values in `packaging/niri/defaults.kdl`.
    pub const SHIPPED: Self = Self {
        gaps: 16,
        center: CenterColumn::OnOverflow,
        width: ColumnWidth::Half,
    };

    fn kdl(self) -> String {
        format!(
            "{LAYOUT_MARKER}\nlayout {{\n    gaps {}\n    center-focused-column \"{}\"\n    default-column-width {{ proportion {}; }}\n}}\n",
            self.gaps,
            self.center.kdl(),
            self.width.kdl()
        )
    }

    /// Every layout Settings can write; the block is recognised only when it
    /// is exactly one of them.
    fn all() -> impl Iterator<Item = Self> {
        Self::GAPS.into_iter().flat_map(|gaps| {
            CenterColumn::ALL.into_iter().flat_map(move |center| {
                ColumnWidth::ALL.into_iter().map(move |width| Self {
                    gaps,
                    center,
                    width,
                })
            })
        })
    }
}

const LAYOUT_MARKER: &str = "\n// Canopa Settings: window layout.";

/// `niri.kdl` as Settings sees its tail: the person's text, then an optional
/// window layout block, then the optional reduced-motion block.
#[cfg(feature = "desktop")]
fn split_layout(text: &str) -> Result<(String, Option<WindowLayout>, &'static str)> {
    let (mut rest, enabled) = split_motion(text)?;
    let motion = if enabled { MOTION_OVERRIDE } else { "" };
    let Some(start) = rest.rfind(LAYOUT_MARKER) else {
        return Ok((rest, None, motion));
    };
    let layout = WindowLayout::all()
        .find(|layout| layout.kdl() == rest[start..])
        .with_context(|| fl!("settings-error-window-layout-edited"))?;
    rest.truncate(start);
    Ok((rest, Some(layout), motion))
}

/// The window layout Settings wrote, or `None` for the shipped defaults.
#[cfg(feature = "desktop")]
pub(crate) fn window_layout(source: &crate::storage::Snapshot) -> Result<Option<WindowLayout>> {
    split_layout(&source.text).map(|(_, layout, _)| layout)
}

/// Write `layout` over the shipped defaults, or remove Settings' block for
/// `None`. The block stays before the reduced-motion block, which must end
/// the file.
#[cfg(feature = "desktop")]
pub(crate) fn set_window_layout(
    paths: &crate::session::Paths,
    layout: Option<WindowLayout>,
) -> Result<()> {
    ensure!(paths.active(), "{}", fl!("settings-error-motion-session"));
    let source = crate::storage::Snapshot::load(paths.niri())?;
    let text = with_window_layout(&source.text, layout)?;
    if text == source.text {
        return Ok(());
    }
    save_main(&source, &text, || Ok(()))?;
    Ok(())
}

#[cfg(feature = "desktop")]
fn with_window_layout(text: &str, layout: Option<WindowLayout>) -> Result<String> {
    let (base, _, motion) = split_layout(text)?;
    let block = layout.map(WindowLayout::kdl).unwrap_or_default();
    Ok(format!("{base}{block}{motion}"))
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
        return Err(error.context(fl!("settings-error-compositor-reload")));
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

#[cfg(all(test, feature = "desktop"))]
mod window_layout_tests {
    use super::*;

    const BASE: &str = "include \"/usr/share/canopa/niri-defaults.kdl\"\n";

    #[test]
    fn the_layout_block_sits_before_reduced_motion_and_round_trips() {
        let layout = WindowLayout {
            gaps: 6,
            center: CenterColumn::Always,
            width: ColumnWidth::TwoThirds,
        };
        let with_motion = format!("{BASE}{MOTION_OVERRIDE}");
        let written = with_window_layout(&with_motion, Some(layout)).unwrap();
        assert!(written.starts_with(BASE));
        assert!(written.ends_with(MOTION_OVERRIDE));
        assert!(written.contains("default-column-width { proportion 0.66667; }"));
        assert_eq!(split_layout(&written).unwrap().1, Some(layout));
        assert_eq!(with_window_layout(&written, None).unwrap(), with_motion);

        let without_motion = with_window_layout(BASE, Some(layout)).unwrap();
        assert_eq!(
            split_layout(&without_motion).unwrap(),
            (BASE.to_owned(), Some(layout), "")
        );
    }

    #[test]
    fn reduced_motion_is_recognized_with_text_after_it() {
        let appended = format!("{BASE}{MOTION_OVERRIDE}// a later note\n");
        let (without, enabled) = split_motion(&appended).unwrap();
        assert!(enabled);
        assert_eq!(without, format!("{BASE}// a later note\n"));
        assert_eq!(split_motion(BASE).unwrap(), (BASE.to_owned(), false));
        // The marker without its exact block, or the block twice, is an outside edit.
        let edited = format!("{BASE}{}", MOTION_OVERRIDE.replace("off;", "slowdown 2.0;"));
        assert!(split_motion(&edited).is_err());
        let twice = format!("{BASE}{MOTION_OVERRIDE}{MOTION_OVERRIDE}");
        assert!(split_motion(&twice).is_err());
    }

    #[test]
    fn an_edited_block_is_refused() {
        let written = with_window_layout(BASE, Some(WindowLayout::SHIPPED)).unwrap();
        let edited = written.replace("gaps 16", "gaps 13");
        assert!(split_layout(&edited).is_err());
    }

    #[test]
    fn every_layout_is_valid_niri_configuration() {
        for layout in WindowLayout::all() {
            let document = kdl::KdlDocument::parse_v1(&layout.kdl()).unwrap();
            assert_eq!(document.nodes().len(), 1);
        }
    }

    #[test]
    fn the_shipped_values_match_the_packaged_defaults() {
        let defaults = include_str!("../../../packaging/niri/defaults.kdl");
        let layout = WindowLayout::SHIPPED;
        assert!(defaults.contains(&format!("    gaps {}\n", layout.gaps)));
        assert!(defaults.contains(&format!(
            "    center-focused-column \"{}\"\n",
            layout.center.kdl()
        )));
        assert!(defaults.contains(&format!(
            "    default-column-width {{ proportion {}; }}\n",
            layout.width.kdl()
        )));
    }
}
