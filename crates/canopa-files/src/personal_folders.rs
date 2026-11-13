//! Prepared XDG personal-folder paths. Loading belongs on a filesystem worker.
use crate::interface::PlaceKind;
use std::{
    ffi::OsString,
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

const MAX_DEFINITION_BYTES: usize = 64 * 1024;
const NAMES: [&[u8]; 8] = [
    b"XDG_DESKTOP_DIR",
    b"XDG_DOCUMENTS_DIR",
    b"XDG_DOWNLOAD_DIR",
    b"XDG_PICTURES_DIR",
    b"XDG_MUSIC_DIR",
    b"XDG_VIDEOS_DIR",
    b"XDG_TEMPLATES_DIR",
    b"XDG_PUBLICSHARE_DIR",
];
/// The icon-theme names of the folders in `NAMES`, as freedesktop themes name them.
const ICONS: [&str; 8] = [
    "user-desktop",
    "folder-documents",
    "folder-download",
    "folder-pictures",
    "folder-music",
    "folder-videos",
    "folder-templates",
    "folder-publicshare",
];

/// One process's personal-folder definitions, preserving original Unix bytes.
#[derive(Clone, Debug)]
pub struct PersonalFolders {
    paths: [PathBuf; 8],
}

impl PersonalFolders {
    /// Read a bounded regular `user-dirs.dirs` source under an absolute config
    /// root. Missing/unreadable sources and invalid assignments retain conventional
    /// Home subfolders. Neither definitions nor destination folders are modified.
    /// Relative Home/config roots are invalid requests.
    pub fn load(home: &Path, config_home: &Path) -> io::Result<Self> {
        if !config_home.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an absolute configuration root is required",
            ));
        }
        let mut folders = Self::conventional(home)?;
        if let Ok(bytes) =
            canopa_file::read_bounded(&config_home.join("user-dirs.dirs"), MAX_DEFINITION_BYTES)
        {
            for line in bytes.split(|byte| *byte == b'\n') {
                let Some(equals) = line.iter().position(|byte| *byte == b'=') else {
                    continue;
                };
                let name = line[..equals].trim_ascii();
                let Some(index) = NAMES.iter().position(|candidate| *candidate == name) else {
                    continue;
                };
                if let Some(path) = definition_path(&line[equals + 1..], home) {
                    folders.paths[index] = path;
                }
            }
        }
        Ok(folders)
    }

    pub(crate) fn conventional(home: &Path) -> io::Result<Self> {
        if !home.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an absolute Home is required",
            ));
        }
        Ok(Self {
            paths: [
                "Desktop",
                "Documents",
                "Downloads",
                "Pictures",
                "Music",
                "Videos",
                "Templates",
                "Public",
            ]
            .map(|name| home.join(name)),
        })
    }

    /// Home, Filesystem, Trash and bookmarks have separate application meanings.
    pub fn path(&self, place: PlaceKind) -> Option<&Path> {
        let index = match place {
            PlaceKind::Desktop => 0,
            PlaceKind::Documents => 1,
            PlaceKind::Downloads => 2,
            PlaceKind::Pictures => 3,
            PlaceKind::Music => 4,
            PlaceKind::Videos => 5,
            PlaceKind::Templates => 6,
            PlaceKind::Public => 7,
            PlaceKind::Home | PlaceKind::Filesystem | PlaceKind::Trash | PlaceKind::Recovery => {
                return None;
            }
        };
        Some(&self.paths[index])
    }

    /// Each personal folder with the icon-theme name that marks it, such as
    /// `folder-download`. A definition pointing at Home itself marks nothing:
    /// XDG uses that to switch a folder off, and Home keeps its own icon.
    pub fn icons<'a>(&'a self, home: &'a Path) -> impl Iterator<Item = (&'a Path, &'static str)> {
        self.paths
            .iter()
            .zip(ICONS)
            .filter(move |(path, _)| path.as_path() != home)
            .map(|(path, icon)| (path.as_path(), icon))
    }
}

fn definition_path(value: &[u8], home: &Path) -> Option<PathBuf> {
    let quoted = value.trim_ascii_start().strip_prefix(b"\"")?;
    // Recognize the sole supported expansion before unescaping, so an escaped
    // dollar cannot accidentally become an environment reference.
    let (relative, contents) = if let Some(rest) = quoted.strip_prefix(b"$HOME/") {
        (true, rest)
    } else if quoted.starts_with(b"$HOME\"") {
        (true, &quoted[5..])
    } else if quoted.starts_with(b"/") {
        (false, quoted)
    } else {
        return None;
    };
    let mut decoded = Vec::new();
    let mut index = 0;
    loop {
        match *contents.get(index)? {
            b'"' => {
                let tail = contents[index + 1..].trim_ascii();
                if !tail.is_empty() && !tail.starts_with(b"#") {
                    return None;
                }
                break;
            }
            b'\\' => {
                index += 1;
                decoded.push(*contents.get(index)?);
            }
            byte => decoded.push(byte),
        }
        index += 1;
    }
    if decoded.contains(&0) {
        return None;
    }
    if relative {
        let mut absolute = home.as_os_str().as_bytes().to_vec();
        if quoted.starts_with(b"$HOME/") {
            absolute.push(b'/');
        }
        absolute.extend(decoded);
        Some(PathBuf::from(OsString::from_vec(absolute)))
    } else {
        Some(PathBuf::from(OsString::from_vec(decoded)))
    }
}
