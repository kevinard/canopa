//! Read-only file metadata prepared off the renderer thread; symlinks are inspected as links.
use crate::{Cancellation, browse::EntryKind};
use std::{fs, io, os::unix::fs::MetadataExt, path::PathBuf, time::SystemTime};

#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    pub metadata: Result<Metadata, String>,
}

#[derive(Clone, Debug)]
pub struct Metadata {
    pub kind: EntryKind,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
    /// The birth time, where the filesystem records one.
    pub created: Option<SystemTime>,
    pub permissions: u32,
    pub owner: u32,
    pub group: u32,
    pub link_target: Option<PathBuf>,
}

/// Individual failures stay inspectable alongside the other requested items.
pub fn read(paths: &[PathBuf], cancellation: &Cancellation) -> io::Result<Vec<Item>> {
    if paths.len() > 64 {
        return Err(io::Error::other(fl!("files-properties-too-many")));
    }
    paths
        .iter()
        .map(|path| {
            if cancellation.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    fl!("files-properties-cancelled"),
                ));
            }
            let metadata = fs::symlink_metadata(path)
                .map(|m| Metadata {
                    kind: if m.is_symlink() {
                        EntryKind::Symlink
                    } else if m.is_dir() {
                        EntryKind::Directory
                    } else if m.is_file() {
                        EntryKind::File
                    } else {
                        EntryKind::Other
                    },
                    bytes: m.len(),
                    modified: m.modified().ok(),
                    created: m.created().ok(),
                    permissions: m.mode() & 0o7777,
                    owner: m.uid(),
                    group: m.gid(),
                    link_target: m.is_symlink().then(|| fs::read_link(path).ok()).flatten(),
                })
                .map_err(|e| e.to_string());
            Ok(Item {
                path: path.clone(),
                metadata,
            })
        })
        .collect()
}
