//! Renderer-independent window intent. It grants no filesystem execution authority.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, os::unix::ffi::OsStrExt, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserTarget {
    #[serde(with = "crate::path_serde")]
    pub directory: PathBuf,
    #[serde(with = "crate::path_serde::sequence")]
    pub selected: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum View {
    QuickLook(#[serde(with = "crate::path_serde")] PathBuf),
    Browsers(Vec<BrowserTarget>),
    Properties(#[serde(with = "crate::path_serde::sequence")] Vec<PathBuf>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub view: View,
    pub startup_id: String,
}

#[derive(Clone, Copy, Debug)]
pub enum FileManagerRequest {
    Folders,
    Items,
    Properties,
}

impl Presentation {
    pub fn quick_look(path: PathBuf) -> io::Result<Self> {
        let path = if path.as_os_str().as_bytes().starts_with(b"file://") {
            crate::file_clipboard::local_path(
                path.to_str()
                    .ok_or_else(|| io::Error::other("File URI must be UTF-8"))?,
            )
            .map_err(|_| io::Error::other("Quick Look requires a local file URI"))?
        } else {
            std::path::absolute(path)?
        };
        let request = Self {
            view: View::QuickLook(path),
            startup_id: String::new(),
        };
        request.validate()?;
        Ok(request)
    }
    pub fn open(paths: Vec<PathBuf>) -> Self {
        Self {
            view: View::Browsers(
                paths
                    .into_iter()
                    .map(|directory| BrowserTarget {
                        directory,
                        selected: Vec::new(),
                    })
                    .collect(),
            ),
            startup_id: String::new(),
        }
    }

    /// Validate the complete URI request before admitting any of its windows.
    /// No filesystem reads or existence checks occur on this path.
    pub fn from_file_manager(
        kind: FileManagerRequest,
        uris: &[String],
        startup_id: &str,
    ) -> io::Result<Option<Self>> {
        if uris.len() > 64 || uris.iter().map(String::len).sum::<usize>() > 65_536 {
            return Err(io::Error::other(
                "A Files request supports at most 64 local locations within 64 KiB.",
            ));
        }
        let mut paths = Vec::new();
        for uri in uris {
            let path = crate::file_clipboard::local_path(uri)
                .map_err(|_| io::Error::other("FileManager1 requires valid local file URIs."))?;
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        let view = match kind {
            FileManagerRequest::Folders => Self::open(paths).view,
            FileManagerRequest::Properties => View::Properties(paths),
            FileManagerRequest::Items => {
                let mut groups = BTreeMap::new();
                let mut targets: Vec<BrowserTarget> = Vec::new();
                for path in paths {
                    let parent = path.parent().unwrap_or(&path).to_owned();
                    let index = *groups.entry(parent.clone()).or_insert_with(|| {
                        targets.push(BrowserTarget {
                            directory: parent,
                            selected: Vec::new(),
                        });
                        targets.len() - 1
                    });
                    if path != targets[index].directory {
                        targets[index].selected.push(path);
                    }
                }
                View::Browsers(targets)
            }
        };
        let request = Self {
            view,
            startup_id: startup_id.to_owned(),
        };
        request.validate()?;
        Ok((!uris.is_empty()).then_some(request))
    }

    pub fn validate(&self) -> io::Result<()> {
        if self.startup_id.len() > 4096 || self.startup_id.as_bytes().contains(&0) {
            return Err(io::Error::other(
                "The Files startup identifier is invalid or too large.",
            ));
        }
        let valid_path = |path: &PathBuf| !path.as_os_str().as_bytes().contains(&0);
        let valid = match &self.view {
            View::QuickLook(path) => {
                path.is_absolute() && valid_path(path) && path.parent().is_some()
            }
            View::Browsers(targets) => {
                targets.len() <= 64
                    && targets.iter().map(|t| t.selected.len()).sum::<usize>() <= 64
                    && targets.iter().all(|t| {
                        valid_path(&t.directory)
                            && t.selected
                                .iter()
                                .all(|p| valid_path(p) && p.parent() == Some(t.directory.as_path()))
                    })
            }
            View::Properties(paths) => {
                paths.len() <= 64 && paths.iter().all(|p| p.is_absolute() && valid_path(p))
            }
        };
        if !valid {
            return Err(io::Error::other(
                "The Files presentation contains invalid or too many locations.",
            ));
        }
        Ok(())
    }
}
