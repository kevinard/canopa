//! Read-only selection and explicit folder creation shared by picker hosts.
pub mod filters;
pub mod folders;
use canopa_file::picker::{ChooserRequest, Mode, file_name, path_bytes};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Prepared {
    pub paths: Vec<PathBuf>,
    pub existing: Vec<PathBuf>,
}

/// Read-only acceptance: the requesting application owns all actual writes.
pub fn prepare(
    request: &ChooserRequest,
    folder: &Path,
    selected: Vec<PathBuf>,
    name: OsString,
) -> io::Result<Prepared> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidInput, message);
    let folder = std::fs::canonicalize(folder)?;
    if !folder.is_dir() {
        return Err(invalid(fl!("files-picker-choose-existing-folder")));
    }
    let paths = match request.mode {
        Mode::Open => {
            let paths = if selected.is_empty() && request.directory {
                vec![folder]
            } else {
                selected
            };
            if paths.is_empty() || (!request.multiple && paths.len() > 1) {
                return Err(invalid(fl!("files-picker-select-file")));
            }
            for path in &paths {
                let metadata = std::fs::metadata(path)?;
                if if request.directory {
                    !metadata.is_dir()
                } else {
                    !metadata.is_file()
                } {
                    return Err(invalid(fl!("files-picker-select-type")));
                }
            }
            paths
        }
        Mode::Save => {
            if name.as_bytes().contains(&0) || !file_name(Path::new(&name)) {
                return Err(invalid(fl!("files-picker-one-file-name")));
            }
            vec![folder.join(name)]
        }
        Mode::SaveMany => {
            let folder = if selected.len() == 1 {
                std::fs::canonicalize(&selected[0])?
            } else {
                folder
            };
            if !folder.is_dir() {
                return Err(invalid(fl!("files-picker-choose-folder")));
            }
            request
                .files
                .iter()
                .map(|bytes| {
                    path_bytes(bytes)
                        .filter(|p| file_name(p))
                        .map(|p| folder.join(p))
                        .ok_or_else(|| invalid(fl!("files-picker-invalid-name")))
                })
                .collect::<io::Result<Vec<_>>>()?
        }
    };
    let mut existing = Vec::new();
    if request.mode != Mode::Open {
        let mut unique = BTreeSet::new();
        for path in &paths {
            if !unique.insert(path) {
                return Err(invalid(fl!("files-picker-distinct-names")));
            }
            match std::fs::symlink_metadata(path) {
                Ok(metadata) => {
                    if !metadata.is_file() {
                        return Err(invalid(fl!("files-picker-special-destination")));
                    }
                    existing.push(path.clone());
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(Prepared { paths, existing })
}
