//! The owner keeps only a descriptor bundle and non-I/O recovery hints. Every
//! filesystem operation, including reconstruction/validation, runs in a worker.
use super::*;
use crate::print::file_target::{ensure_empty, path_only, protocol, receive_many, send_many};
use std::{os::fd::AsFd, os::unix::net::UnixStream};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preparation {
    pub(super) selection: Selection,
    pub(super) directory_name: Vec<u8>,
}
impl Preparation {
    /// Choose the recovery hint before setup starts. This does not consult or
    /// reopen any filesystem path and does not itself authorize publication.
    pub fn new(pin: &Pinned) -> io::Result<Self> {
        let mut random = [0_u8; 16];
        if rustix::rand::getrandom(&mut random, rustix::rand::GetRandomFlags::empty())?
            != random.len()
        {
            return Err(io::Error::other("Print staging randomness unavailable"));
        }
        let name = format!(
            ".canopa-print-{}",
            random
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        Ok(Self {
            selection: pin.selection.clone(),
            directory_name: name.into_bytes(),
        })
    }
    pub fn retained(&self, acknowledged: bool) -> Disposition {
        // A moved parent can make this original location stale. It is a hint,
        // never authority to reopen, remove or retry a transaction after loss.
        let path = PathBuf::from(OsString::from_vec(self.selection.path.clone()));
        let hint = path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("/"))
            .join(OsString::from_vec(self.directory_name.clone()));
        Disposition {
            published: acknowledged,
            uncertain: !acknowledged,
            recovery: Some(hint.into_os_string().into_vec()),
        }
    }
    pub fn prepare(self, files: Vec<File>) -> io::Result<Publication> {
        work_name(&self.directory_name)?;
        let pin = Pinned::inherited(self.selection.clone(), files)?;
        Publication::prepare_with(pin, self)
    }
}

/// No metadata or path syscalls are performed when the owner receives this.
/// The following PublishFile/SettleFile workers revalidate all live objects.
pub struct Prepared {
    info: PublicationInfo,
    files: Vec<File>,
}
impl Prepared {
    pub fn receive(
        channel: &UnixStream,
        info: PublicationInfo,
        expected: &Preparation,
    ) -> io::Result<Self> {
        let files = receive_many(channel, 5)?.ok_or_else(protocol)?;
        ensure_empty(channel)?;
        if info.selection != expected.selection
            || info.directory_name != expected.directory_name
            || files.len() != 4 + usize::from(info.selection.replaces())
        {
            return Err(protocol());
        }
        let files = files.into_iter().map(File::from).collect::<Vec<_>>();
        // O_PATH has no writable file description whose close could flush
        // remote data in the owner. These flag checks perform no metadata I/O.
        for file in &files {
            if !path_only(file)? {
                return Err(protocol());
            }
        }
        Ok(Self { info, files })
    }
    pub fn info(&self) -> &PublicationInfo {
        &self.info
    }
    pub fn files(&self) -> &[File] {
        &self.files
    }
}
impl Publication {
    pub fn send(&self, channel: &UnixStream) -> io::Result<()> {
        let files = self
            .files()
            .into_iter()
            .map(|file| {
                openat(
                    rustix::fs::CWD,
                    format!("/proc/self/fd/{}", file.as_raw_fd()),
                    OFlags::PATH | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(io::Error::from)
            })
            .collect::<io::Result<Vec<_>>>()?;
        let descriptors = files.iter().map(|file| file.as_fd()).collect::<Vec<_>>();
        send_many(channel, &descriptors, 5)
    }
}
