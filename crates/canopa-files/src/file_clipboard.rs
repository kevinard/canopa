//! Local file selections exchanged at the desktop boundary. URI conversion
//! preserves Unix path bytes; a selection grants no filesystem authority.
use crate::transfers::TransferKind;
use std::{collections::BTreeSet, io, os::unix::ffi::OsStrExt, path::PathBuf, sync::Arc};
use url::Url;

#[cfg(feature = "wayland-clipboard")]
pub mod wayland;

pub const GNOME: &str = "x-special/gnome-copied-files";
pub const URIS: &str = "text/uri-list";
pub const KDE: &str = "application/x-kde-cutselection";
pub const LIMIT: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Selection {
    paths: Arc<[PathBuf]>,
    kind: TransferKind,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl Selection {
    pub fn new(paths: Vec<PathBuf>, kind: TransferKind) -> io::Result<Self> {
        let mut seen = BTreeSet::new();
        let mut bytes = 0usize;
        let mut unique = Vec::new();
        for path in paths {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|part| part == std::path::Component::ParentDir)
                || path.file_name().is_none()
                || path.as_os_str().as_bytes().contains(&0)
            {
                return Err(invalid("The file list contains an invalid local path"));
            }
            bytes = bytes.saturating_add(path.as_os_str().len());
            if bytes > LIMIT {
                return Err(invalid(fl!("files-clipboard-too-large")));
            }
            if seen.insert(path.clone()) {
                unique.push(path);
            }
        }
        if unique.is_empty() {
            return Err(invalid(fl!("files-clipboard-no-files")));
        }
        Ok(Self {
            paths: unique.into(),
            kind,
        })
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
    pub fn kind(&self) -> TransferKind {
        self.kind
    }

    pub(crate) fn retaining(&self, keep: &BTreeSet<PathBuf>) -> Option<Self> {
        let paths: Vec<_> = self
            .paths
            .iter()
            .filter(|path| keep.contains(*path))
            .cloned()
            .collect();
        (!paths.is_empty()).then(|| Self {
            paths: paths.into(),
            kind: self.kind,
        })
    }

    pub fn encode(&self, mime: &str) -> io::Result<Vec<u8>> {
        if mime == KDE {
            return Ok(if self.kind == TransferKind::Move {
                b"1"
            } else {
                b"0"
            }
            .to_vec());
        }
        if mime != GNOME && mime != URIS {
            return Err(invalid("Unsupported file clipboard format"));
        }
        let mut bytes = if mime == GNOME {
            if self.kind == TransferKind::Move {
                b"cut\n".to_vec()
            } else {
                b"copy\n".to_vec()
            }
        } else {
            Vec::new()
        };
        for path in self.paths.iter() {
            let uri =
                Url::from_file_path(path).map_err(|_| invalid("Invalid local file location"))?;
            let ending: &[u8] = if mime == GNOME { b"\n" } else { b"\r\n" };
            if bytes
                .len()
                .saturating_add(uri.as_str().len())
                .saturating_add(ending.len())
                > LIMIT
            {
                return Err(invalid(fl!("files-clipboard-too-large")));
            }
            bytes.extend_from_slice(uri.as_str().as_bytes());
            bytes.extend_from_slice(ending);
        }
        Ok(bytes)
    }

    pub fn decode(mime: &str, bytes: &[u8], kde: Option<&[u8]>) -> io::Result<Self> {
        if bytes.len() > LIMIT {
            return Err(invalid(fl!("files-clipboard-too-large")));
        }
        let value =
            std::str::from_utf8(bytes).map_err(|_| invalid("Invalid clipboard file list"))?;
        let mut lines = value.lines();
        let kind = match mime {
            GNOME => match lines.next() {
                Some("copy") => TransferKind::Copy,
                Some("cut") => TransferKind::Move,
                _ => return Err(invalid("Invalid clipboard file action")),
            },
            URIS => match kde {
                Some(b"1") => TransferKind::Move,
                Some(b"0") | None => TransferKind::Copy,
                _ => return Err(invalid("Invalid clipboard file action")),
            },
            _ => return Err(invalid("Unsupported file clipboard format")),
        };
        let mut paths = Vec::new();
        for line in lines.filter(|line| !line.is_empty() && !line.starts_with('#')) {
            paths.push(local_path(line)?);
        }
        Self::new(paths, kind)
    }
}

pub(crate) fn local_path(value: &str) -> io::Result<PathBuf> {
    if value
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte == b' ' || byte == b'\\')
    {
        return Err(invalid(fl!("files-clipboard-invalid-location")));
    }
    if let Some((_, rest)) = value.split_once(':')
        && let Some(authority) = rest
            .strip_prefix("//")
            .and_then(|rest| rest.split('/').next())
        && !authority.is_empty()
        && !authority.eq_ignore_ascii_case("localhost")
    {
        return Err(invalid(fl!("files-clipboard-non-local")));
    }
    // URL parsers remove dot segments before returning a path. For a directory
    // link followed by `..`, that can name a different file from Unix traversal.
    if value
        .split('/')
        .any(|part| part.replace("%2e", ".").replace("%2E", ".") == "..")
    {
        return Err(invalid(
            "Use a file location without parent-directory components",
        ));
    }
    let valid_escapes = value.as_bytes().iter().enumerate().all(|(index, byte)| {
        *byte != b'%'
            || value
                .as_bytes()
                .get(index + 1..index + 3)
                .is_some_and(|bytes| bytes.iter().all(u8::is_ascii_hexdigit))
    });
    if !valid_escapes
        || !value.split_once(':').is_some_and(|(scheme, path)| {
            scheme.eq_ignore_ascii_case("file") && path.starts_with('/')
        })
    {
        return Err(invalid(
            "The file list contains an invalid local file location",
        ));
    }
    let uri = Url::parse(value).map_err(|_| invalid("Invalid clipboard file location"))?;
    if uri.scheme() != "file"
        || uri
            .host_str()
            .is_some_and(|host| !host.is_empty() && !host.eq_ignore_ascii_case("localhost"))
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.port().is_some()
        || uri.query().is_some()
        || uri.fragment().is_some()
    {
        return Err(invalid(fl!("files-clipboard-non-local")));
    }
    uri.to_file_path()
        .map_err(|_| invalid("Invalid clipboard file location"))
}
