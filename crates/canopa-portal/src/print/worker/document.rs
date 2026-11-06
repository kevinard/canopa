use super::{Cancellation, Document};
use crate::print::Format;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{FileExt, MetadataExt},
};

pub const MAX_DOCUMENT_BYTES: u64 = 128 * 1024 * 1024;
const SEALS: rustix::fs::SealFlags = rustix::fs::SealFlags::SHRINK
    .union(rustix::fs::SealFlags::GROW)
    .union(rustix::fs::SealFlags::WRITE)
    .union(rustix::fs::SealFlags::SEAL);

fn identify(prefix: &[u8]) -> Option<Format> {
    if prefix.windows(5).any(|bytes| bytes == b"%PDF-") {
        return Some(Format::Pdf);
    }
    if prefix.starts_with(b"%!") {
        return Some(Format::Ps);
    }
    let mut text = std::str::from_utf8(prefix)
        .ok()?
        .trim_start_matches('\u{feff}')
        .trim_start();
    if text.starts_with("<?xml") {
        text = text.split_once("?>")?.1.trim_start();
    }
    while text.starts_with("<!--") {
        text = text.split_once("-->")?.1.trim_start();
    }
    text.strip_prefix("<svg")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/'))
        .then_some(Format::Svg)
}

pub(super) fn snapshot(
    source: File,
    target: File,
    cancel: &Cancellation,
) -> anyhow::Result<Document> {
    copy(source, target, cancel, MAX_DOCUMENT_BYTES, false)
}

pub(crate) fn snapshot_recovery(
    source: File,
    target: File,
    cancel: &Cancellation,
) -> anyhow::Result<Document> {
    anyhow::ensure!(
        source.metadata()?.is_file(),
        "Recovery source is not a regular file"
    );
    copy(source, target, cancel, MAX_DOCUMENT_BYTES, true)
}

fn copy(
    mut source: File,
    mut target: File,
    cancel: &Cancellation,
    limit: u64,
    allow_empty: bool,
) -> anyhow::Result<Document> {
    cancel.check()?;
    let before = source.metadata()?;
    let output = target.metadata()?;
    anyhow::ensure!(
        output.len() == 0
            && (before.dev(), before.ino()) != (output.dev(), output.ino())
            && rustix::fs::fcntl_get_seals(&target)?.is_empty(),
        "Print snapshot target is not a fresh sealable file"
    );
    // SEEK_CUR with zero only queries the shared offset. Regular-file reads use
    // pread, so neither that offset nor the caller's status flags are changed.
    let offset = if before.is_file() {
        Some(source.stream_position()?)
    } else {
        None
    };
    let mut bytes = 0_u64;
    let mut prefix = Vec::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        cancel.check()?;
        if offset.is_none() {
            use rustix::event::{PollFd, PollFlags, Timespec, poll};
            let mut descriptors = [
                PollFd::new(&source, PollFlags::IN),
                PollFd::new(&cancel.0, PollFlags::IN),
            ];
            let timeout = Timespec {
                tv_sec: 0,
                tv_nsec: 100_000_000,
            };
            if poll(&mut descriptors, Some(&timeout))? == 0 {
                continue;
            }
            cancel.check()?;
            if descriptors[0].revents().is_empty() {
                continue;
            }
        }
        let size = buffer.len().min((limit - bytes + 1) as usize);
        let result = match offset {
            Some(offset) => source.read_at(
                &mut buffer[..size],
                offset
                    .checked_add(bytes)
                    .ok_or_else(|| anyhow::anyhow!("Print offset overflow"))?,
            ),
            None => source.read(&mut buffer[..size]),
        };
        let read = match result {
            Ok(read) => read,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(read as u64)
            .ok_or_else(|| anyhow::anyhow!("Print size overflow"))?;
        anyhow::ensure!(bytes <= limit, "Print document exceeds 128 MiB limit");
        prefix.extend_from_slice(&buffer[..read.min(1024 - prefix.len())]);
        target.write_all(&buffer[..read])?;
    }
    anyhow::ensure!(allow_empty || bytes != 0, "Print document is empty");
    if offset.is_some() {
        let after = source.metadata()?;
        anyhow::ensure!(
            (
                before.len(),
                before.mtime(),
                before.mtime_nsec(),
                before.ctime(),
                before.ctime_nsec()
            ) == (
                after.len(),
                after.mtime(),
                after.mtime_nsec(),
                after.ctime(),
                after.ctime_nsec()
            ),
            "Print document changed during preparation"
        );
    }
    cancel.check()?;
    rustix::fs::fcntl_add_seals(&target, SEALS)?;
    target.seek(SeekFrom::Start(0))?;
    Ok(Document {
        bytes,
        format: identify(&prefix),
    })
}

pub(crate) fn verify(file: &File) -> anyhow::Result<Document> {
    let document = verify_recovery(file)?;
    anyhow::ensure!(document.bytes > 0, "Print document is empty");
    Ok(document)
}

pub(crate) fn verify_recovery(file: &File) -> anyhow::Result<Document> {
    let bytes = file.metadata()?.len();
    anyhow::ensure!(
        bytes <= MAX_DOCUMENT_BYTES && rustix::fs::fcntl_get_seals(file)?.contains(SEALS),
        "Print document is not a bounded sealed snapshot"
    );
    let mut prefix = [0; 1024];
    let read = file.read_at(&mut prefix, 0)?;
    Ok(Document {
        bytes,
        format: identify(&prefix[..read]),
    })
}

pub(crate) fn finish(file: &File) -> anyhow::Result<Document> {
    let bytes = file.metadata()?.len();
    anyhow::ensure!(
        bytes > 0 && bytes <= MAX_DOCUMENT_BYTES,
        "Print render output exceeds limits or is empty"
    );
    rustix::fs::fcntl_add_seals(file, SEALS)?;
    verify(file)
}

/// Copy only an already frozen document, starting at zero with pread/pwrite.
/// The ordinary output file is private staging, never the selected destination.
pub(super) fn stage(
    source: &File,
    target: &File,
    format: Format,
    cancel: &Cancellation,
) -> anyhow::Result<()> {
    let document = verify(source)?;
    anyhow::ensure!(
        document.format == Some(format),
        "Print document does not match the approved file format"
    );
    stage_recovery(source, target, cancel)
}

pub(super) fn stage_recovery(
    source: &File,
    target: &File,
    cancel: &Cancellation,
) -> anyhow::Result<()> {
    let document = verify_recovery(source)?;
    anyhow::ensure!(
        target.metadata()?.is_file() && target.metadata()?.len() == 0,
        "Print file stage is not empty"
    );
    let mut offset = 0;
    let mut buffer = [0; 64 * 1024];
    while offset < document.bytes {
        cancel.check()?;
        let length = buffer.len().min((document.bytes - offset) as usize);
        source.read_exact_at(&mut buffer[..length], offset)?;
        target.write_all_at(&buffer[..length], offset)?;
        offset += length as u64;
    }
    cancel.check()?;
    target.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests;
