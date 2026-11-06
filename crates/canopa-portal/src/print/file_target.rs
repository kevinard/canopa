//! File consent retains kernel objects; a path or serialized inode is not a pin.
mod publication;
#[cfg(test)]
mod tests;
pub use publication::{
    Disposition, Preparation, Prepared, Publication, PublicationInfo, RecoveryBody, RecoveryItem,
    RecoveryProtection, RecoveryStatus, RecoveryStore,
};
use rustix::fs::{Mode, OFlags, open, openat};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{File, Metadata},
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsFd, AsRawFd, OwnedFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::MetadataExt,
            net::UnixStream,
        },
    },
    path::{Path, PathBuf},
};

fn changed() -> io::Error {
    io::Error::other("The reviewed file destination changed. Choose it again.")
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "Invalid file destination")
}
fn protocol() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid file approval handoff")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    device: u64,
    inode: u64,
}
impl Identity {
    fn of(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Version {
    identity: Identity,
    bytes: u64,
    mode: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl Version {
    fn of(metadata: &Metadata) -> Self {
        Self {
            identity: Identity::of(metadata),
            bytes: metadata.len(),
            mode: metadata.mode(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

/// This evidence is checked against received live descriptors. It is never
/// sufficient by itself to reconstruct authority by reopening a pathname.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    path: Vec<u8>,
    parent: Identity,
    existing: Option<Version>,
}
impl Selection {
    pub fn path(&self) -> &[u8] {
        &self.path
    }
    pub fn replaces(&self) -> bool {
        self.existing.is_some()
    }
    fn location(&self) -> io::Result<(PathBuf, OsString)> {
        let path = PathBuf::from(OsString::from_vec(self.path.clone()));
        if self.path.len() > 4096 || self.path.contains(&0) || !path.is_absolute() {
            return Err(invalid());
        }
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(invalid)?;
        let name = path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(invalid)?;
        // Exclude syntactically normalized trailing components such as a/./b/..
        // so the actual syscall name is exactly the byte path shown for review.
        if parent.join(name).as_os_str().as_bytes() != self.path {
            return Err(invalid());
        }
        Ok((parent.to_owned(), name.to_owned()))
    }
}

#[derive(Debug)]
pub struct Pinned {
    selection: Selection,
    parent: File,
    entry: Option<File>,
}

/// Opened by the embedded picker before asking whether to replace an existing
/// entry. Clone its Arc through that review; never refresh consent from a path.
#[derive(Debug)]
pub struct Reviewed(Pinned);
impl Reviewed {
    pub fn open(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(invalid());
        }
        let parent = path.parent().ok_or_else(invalid)?.canonicalize()?;
        let name = path.file_name().ok_or_else(invalid)?;
        let directory = File::from(open(
            &parent,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let entry = named(&directory, name)?;
        let existing = entry
            .as_ref()
            .map(|file| file.metadata().map(|metadata| Version::of(&metadata)))
            .transpose()?;
        let result = Self(Pinned {
            selection: Selection {
                path: parent.join(name).into_os_string().into_vec(),
                parent: Identity::of(&directory.metadata()?),
                existing,
            },
            parent: directory,
            entry,
        });
        result.0.verify()?;
        Ok(result)
    }
    pub fn selection(&self) -> &Selection {
        &self.0.selection
    }
    pub fn verify(&self) -> io::Result<()> {
        self.0.verify()
    }
    pub fn send(&self, socket: &UnixStream) -> io::Result<()> {
        let mut descriptors = vec![self.0.parent.as_fd()];
        if let Some(entry) = &self.0.entry {
            descriptors.push(entry.as_fd());
        }
        send(socket, &descriptors)
    }
}

fn named(directory: &File, name: &std::ffi::OsStr) -> io::Result<Option<File>> {
    match openat(
        directory,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => {
            let file = File::from(fd);
            if !file.metadata()?.is_file() {
                return Err(invalid());
            }
            Ok(Some(file))
        }
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

impl Pinned {
    pub fn selection(&self) -> &Selection {
        &self.selection
    }
    pub fn receive(socket: &UnixStream, selection: Selection) -> io::Result<Self> {
        let result = Self::receive_packet(socket, selection)?;
        result.verify()?;
        ensure_empty(socket)?;
        Ok(result)
    }
    /// Transport only. The controller's bounded worker verifies metadata before
    /// issuing an approval; publication workers check again before touching it.
    pub(crate) fn receive_handoff(socket: &UnixStream, selection: Selection) -> io::Result<Self> {
        let result = Self::receive_packet(socket, selection)?;
        // This channel is nonblocking and its producer has already exited.
        ensure_empty(socket)?;
        Ok(result)
    }
    fn receive_packet(socket: &UnixStream, selection: Selection) -> io::Result<Self> {
        let mut descriptors = receive(socket)?.ok_or_else(protocol)?;
        if descriptors.len() != 1 + usize::from(selection.replaces()) {
            return Err(protocol());
        }
        let entry = selection
            .replaces()
            .then(|| File::from(descriptors.pop().expect("entry descriptor")));
        let parent = File::from(descriptors.pop().expect("parent descriptor"));
        let result = Self {
            selection,
            parent,
            entry,
        };
        result.selection.location()?;
        Ok(result)
    }
    pub(crate) fn files(&self) -> Vec<&File> {
        let mut files = vec![&self.parent];
        files.extend(self.entry.as_ref());
        files
    }
    pub(crate) fn inherited(selection: Selection, files: Vec<File>) -> io::Result<Self> {
        if files.len() != 1 + usize::from(selection.replaces()) {
            return Err(protocol());
        }
        let mut files = files.into_iter();
        let result = Self {
            selection,
            parent: files.next().unwrap(),
            entry: files.next(),
        };
        result.verify()?;
        Ok(result)
    }
    /// Verify both held objects, the named entry and the displayed parent path.
    /// Another entry cannot borrow a reviewed inode number while this pin lives.
    pub fn verify(&self) -> io::Result<()> {
        let (path, name) = self.selection.location()?;
        let held_parent = self.parent.metadata()?;
        if !held_parent.is_dir()
            || Identity::of(&held_parent) != self.selection.parent
            || !path_only(&self.parent)?
        {
            return Err(protocol());
        }
        let current_parent = File::from(open(
            path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if Identity::of(&current_parent.metadata()?) != self.selection.parent {
            return Err(changed());
        }
        let current = named(&self.parent, &name)?;
        match (&self.entry, self.selection.existing, current) {
            (None, None, None) => Ok(()),
            (Some(held), Some(expected), Some(current)) => {
                let metadata = held.metadata()?;
                if !metadata.is_file()
                    || !path_only(held)?
                    || Version::of(&metadata) != expected
                    || Version::of(&current.metadata()?) != expected
                {
                    return Err(changed());
                }
                Ok(())
            }
            _ => Err(changed()),
        }
    }
}
fn path_only(file: &File) -> io::Result<bool> {
    // SAFETY: fcntl only reads status flags on this retained owned descriptor.
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(flags & libc::O_PATH != 0
        && rustix::io::fcntl_getfd(file)?.contains(rustix::io::FdFlags::CLOEXEC))
}

/// Adopt a duplicate of the parent's private socket and restore CLOEXEC on the
/// inherited original before any metadata workers can be spawned from this UI.
pub fn inherited_channel(number: i32) -> io::Result<UnixStream> {
    if number < 3 {
        return Err(protocol());
    }
    // SAFETY: fcntl validates the supplied integer without adopting it.
    let flags = unsafe { libc::fcntl(number, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the return value, when nonnegative, is a fresh owned descriptor.
    let duplicate = unsafe { libc::fcntl(number, libc::F_DUPFD_CLOEXEC, 3) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error());
    }
    use std::os::fd::FromRawFd;
    // SAFETY: F_DUPFD_CLOEXEC just returned this unique descriptor.
    let owned = unsafe { OwnedFd::from_raw_fd(duplicate) };
    // SAFETY: only this helper's original FD flags change, not the parent's table.
    if unsafe { libc::fcntl(number, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if rustix::net::sockopt::socket_type(&owned)? != rustix::net::SocketType::STREAM
        || rustix::net::sockopt::socket_domain(&owned)? != rustix::net::AddressFamily::UNIX
    {
        return Err(protocol());
    }
    Ok(UnixStream::from(owned))
}

pub(crate) fn ensure_empty(socket: &UnixStream) -> io::Result<()> {
    if receive(socket)?.is_some() {
        return Err(protocol());
    }
    Ok(())
}
fn send(socket: &UnixStream, descriptors: &[std::os::fd::BorrowedFd<'_>]) -> io::Result<()> {
    send_many(socket, descriptors, 2)
}
pub(super) fn send_many(
    socket: &UnixStream,
    descriptors: &[std::os::fd::BorrowedFd<'_>],
    limit: usize,
) -> io::Result<()> {
    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(5))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if descriptors.is_empty()
        || descriptors.len() > limit
        || limit > 5
        || !ancillary.push(SendAncillaryMessage::ScmRights(descriptors))
    {
        return Err(protocol());
    }
    if rustix::net::sendmsg(
        socket,
        &[io::IoSlice::new(&[1])],
        &mut ancillary,
        SendFlags::NOSIGNAL,
    )? != 1
    {
        return Err(protocol());
    }
    Ok(())
}
fn receive(socket: &UnixStream) -> io::Result<Option<Vec<OwnedFd>>> {
    receive_many(socket, 2)
}
pub(super) fn receive_many(socket: &UnixStream, limit: usize) -> io::Result<Option<Vec<OwnedFd>>> {
    use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags};
    let mut bytes = [0];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(6))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let reply = rustix::net::recvmsg(
        socket,
        &mut [io::IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )?;
    let mut descriptors = Vec::new();
    let mut unknown = false;
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
            _ => unknown = true,
        }
    }
    if unknown
        || reply
            .flags
            .intersects(ReturnFlags::CTRUNC | ReturnFlags::TRUNC)
    {
        return Err(protocol());
    }
    if reply.bytes == 0 && descriptors.is_empty() {
        return Ok(None);
    }
    if reply.bytes != 1
        || bytes != [1]
        || descriptors.is_empty()
        || descriptors.len() > limit
        || limit > 5
    {
        return Err(protocol());
    }
    Ok(Some(descriptors))
}
