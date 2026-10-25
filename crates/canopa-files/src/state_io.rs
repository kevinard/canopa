use std::{
    fs,
    io::{self, Write},
    os::unix::fs::DirBuilderExt,
    path::Path,
};

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    limit: usize,
) -> io::Result<T> {
    let bytes = canopa_file::read_bounded(path, limit)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(crate) fn write_json<T: serde::Serialize>(
    path: &Path,
    value: &T,
    limit: usize,
) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "state exceeds its storage budget",
        ));
    }
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".canopa-state-")
        .tempfile_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    fs::File::open(parent)?.sync_all()
}
