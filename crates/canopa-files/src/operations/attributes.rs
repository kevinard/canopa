//! Extended attributes and POSIX ACLs kept by copies (ADR0174).
//!
//! Copies keep the `user.*` attributes (tags, origin URLs, application data)
//! and the POSIX access and default ACLs, which Linux stores as the
//! `system.posix_acl_*` attributes. Other namespaces are deliberately left to
//! the destination: `security.*` labels and file capabilities follow the
//! destination's policy, and `trusted.*` needs privileges Files does not use.
//!
//! Attributes never fail a copy. A destination that cannot hold them (a FAT
//! drive, a share without ACLs) or a source that cannot be read reports the
//! item as partly kept, so the transfer can say so instead of losing them
//! silently.
use rustix::fs::XattrFlags;
use std::{fs, io, os::fd::AsRawFd};

const ACCESS_ACL: &[u8] = b"system.posix_acl_access";
const DEFAULT_ACL: &[u8] = b"system.posix_acl_default";
/// A larger single value is not a tag or an ACL; Linux caps values at 64 KiB.
const VALUE_LIMIT: usize = 64 * 1024;

/// Whether everything the source had reached the copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Kept {
    #[default]
    All,
    Partly,
}

/// The preservable attributes of one source, read before its copy is written.
pub(super) struct Attributes {
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    unreadable: bool,
}

impl Attributes {
    /// Read from an open regular file.
    pub(super) fn of_file(source: &fs::File) -> Self {
        Self::read(
            |buffer| rustix::fs::flistxattr(source, buffer),
            |name, buffer| rustix::fs::fgetxattr(source, name, buffer),
            false,
        )
    }

    /// Read from a folder held by any descriptor, including `O_PATH`, through
    /// its descriptor link. No user path is resolved again.
    pub(super) fn of_directory(source: &fs::File) -> Self {
        let link = format!("/proc/self/fd/{}", source.as_raw_fd());
        Self::read(
            |buffer| rustix::fs::listxattr(link.as_str(), buffer),
            |name, buffer| rustix::fs::getxattr(link.as_str(), name, buffer),
            true,
        )
    }

    fn read(
        list: impl Fn(&mut [u8]) -> rustix::io::Result<usize>,
        get: impl Fn(&[u8], &mut [u8]) -> rustix::io::Result<usize>,
        directory: bool,
    ) -> Self {
        let names = match sized(|buffer| list(buffer)) {
            Ok(names) => names,
            // The source filesystem has no attributes, so none can be lost.
            Err(error) if unsupported(error) => return Self::none(),
            Err(_) => return Self::unreadable(),
        };
        let mut entries = Vec::new();
        let mut unreadable = false;
        for name in names
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            if !preserved(name, directory) {
                continue;
            }
            match sized(|buffer| get(name, buffer)) {
                Ok(value) => entries.push((name.to_vec(), value)),
                // Removed between listing and reading.
                Err(rustix::io::Errno::NODATA) => {}
                Err(_) => unreadable = true,
            }
        }
        // Apply ACLs first: an access ACL rewrites the group bits, and the
        // later permission restore must see the final ACL to keep its mask.
        entries.sort_by_key(|(name, _)| !name.starts_with(b"system."));
        Self {
            entries,
            unreadable,
        }
    }

    fn none() -> Self {
        Self {
            entries: Vec::new(),
            unreadable: false,
        }
    }

    fn unreadable() -> Self {
        Self {
            entries: Vec::new(),
            unreadable: true,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty() && !self.unreadable
    }

    /// Write every read attribute to the copy. Unsupported or refused
    /// attributes are reported, never returned as errors.
    pub(super) fn apply(&self, output: &fs::File) -> Kept {
        let mut kept = if self.unreadable {
            Kept::Partly
        } else {
            Kept::All
        };
        for (name, value) in &self.entries {
            if rustix::fs::fsetxattr(output, name.as_slice(), value, XattrFlags::empty()).is_err() {
                kept = Kept::Partly;
            }
        }
        kept
    }
}

fn preserved(name: &[u8], directory: bool) -> bool {
    name.starts_with(b"user.") || name == ACCESS_ACL || (directory && name == DEFAULT_ACL)
}

fn unsupported(error: rustix::io::Errno) -> bool {
    error == rustix::io::Errno::NOTSUP || error == rustix::io::Errno::OPNOTSUPP
}

/// Read a variable-length attribute list or value, growing the buffer when it
/// changed between the size query and the read.
fn sized(read: impl Fn(&mut [u8]) -> rustix::io::Result<usize>) -> rustix::io::Result<Vec<u8>> {
    for _ in 0..4 {
        let size = read(&mut [])?;
        if size > VALUE_LIMIT {
            return Err(rustix::io::Errno::TOOBIG);
        }
        let mut buffer = vec![0; size];
        match read(&mut buffer) {
            Ok(written) => {
                buffer.truncate(written);
                return Ok(buffer);
            }
            Err(rustix::io::Errno::RANGE) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(rustix::io::Errno::RANGE)
}

/// Keep a folder's attributes on its staged copy. The copy's permissions are
/// restored afterward, so an access ACL whose owner entry would deny writes
/// cannot lock Files out of filling the folder.
pub(super) fn keep_on_directory(source: &fs::File, output: &fs::File) -> io::Result<Kept> {
    let attributes = Attributes::of_directory(source);
    if attributes.is_empty() {
        return Ok(Kept::All);
    }
    let mode = rustix::fs::fstat(output)?.st_mode & 0o7777;
    let kept = attributes.apply(output);
    rustix::fs::fchmod(output, rustix::fs::Mode::from_raw_mode(mode))?;
    Ok(kept)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A POSIX access ACL granting `user` read access, in the kernel's
    /// `posix_acl_xattr` encoding.
    pub(crate) fn acl_for(user: u32) -> Vec<u8> {
        let mut bytes = 2u32.to_le_bytes().to_vec();
        for (tag, perm, id) in [
            (0x01u16, 6u16, u32::MAX),
            (0x02, 4, user),
            (0x04, 4, u32::MAX),
            (0x10, 4, u32::MAX),
            (0x20, 0, u32::MAX),
        ] {
            bytes.extend(tag.to_le_bytes());
            bytes.extend(perm.to_le_bytes());
            bytes.extend(id.to_le_bytes());
        }
        bytes
    }

    pub(crate) fn supports_user_attributes(path: &std::path::Path) -> bool {
        rustix::fs::setxattr(path, "user.canopa.probe", b"1", XattrFlags::empty()).is_ok()
            && rustix::fs::removexattr(path, "user.canopa.probe").is_ok()
    }

    #[test]
    fn user_attributes_and_acls_reach_the_copy_and_other_namespaces_do_not() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let output = root.path().join("output");
        fs::write(&source, b"data").unwrap();
        fs::write(&output, b"data").unwrap();
        if !supports_user_attributes(&source) {
            eprintln!("skipped: the test filesystem has no user attributes");
            return;
        }
        rustix::fs::setxattr(
            &source,
            "user.xdg.origin.url",
            b"https://example.org/",
            XattrFlags::empty(),
        )
        .unwrap();
        let acl =
            rustix::fs::setxattr(&source, ACCESS_ACL, &acl_for(4242), XattrFlags::empty()).is_ok();
        let attributes = Attributes::of_file(&fs::File::open(&source).unwrap());
        let file = fs::File::open(&output).unwrap();
        assert_eq!(attributes.apply(&file), Kept::All);
        let mut value = [0; 64];
        let count = rustix::fs::getxattr(&output, "user.xdg.origin.url", &mut value[..]).unwrap();
        assert_eq!(&value[..count], b"https://example.org/");
        if acl {
            let mut value = [0; 128];
            let count = rustix::fs::getxattr(&output, ACCESS_ACL, &mut value[..]).unwrap();
            assert_eq!(&value[..count], acl_for(4242).as_slice());
        }
    }

    #[test]
    fn a_refused_attribute_is_reported_without_failing() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::write(&source, b"data").unwrap();
        if !supports_user_attributes(&source) {
            eprintln!("skipped: the test filesystem has no user attributes");
            return;
        }
        rustix::fs::setxattr(&source, "user.tag", b"red", XattrFlags::empty()).unwrap();
        let attributes = Attributes::of_file(&fs::File::open(&source).unwrap());
        // A read-only descriptor cannot take attributes: the kernel refuses
        // the write the way a filesystem without support does.
        let output = root.path().join("output");
        fs::write(&output, b"data").unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o400)).unwrap();
        let readonly = fs::File::open(&output).unwrap();
        let kept = attributes.apply(&readonly);
        if rustix::process::geteuid().is_root() {
            return;
        }
        assert_eq!(kept, Kept::Partly);
    }

    #[test]
    fn a_folder_acl_denying_its_owner_writes_does_not_lock_the_copy() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let output = root.path().join("output");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&output).unwrap();
        if !supports_user_attributes(&source) {
            eprintln!("skipped: the test filesystem has no user attributes");
            return;
        }
        rustix::fs::setxattr(&source, "user.tag", b"blue", XattrFlags::empty()).unwrap();
        let acl =
            rustix::fs::setxattr(&source, DEFAULT_ACL, &acl_for(4242), XattrFlags::empty()).is_ok();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700)).unwrap();
        let held = fs::File::from(
            rustix::fs::open(
                &source,
                rustix::fs::OFlags::PATH | rustix::fs::OFlags::DIRECTORY,
                rustix::fs::Mode::empty(),
            )
            .unwrap(),
        );
        let target = fs::File::open(&output).unwrap();
        assert_eq!(keep_on_directory(&held, &target).unwrap(), Kept::All);
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::write(output.join("child"), b"x").unwrap();
        let mut value = [0; 16];
        let count = rustix::fs::getxattr(&output, "user.tag", &mut value[..]).unwrap();
        assert_eq!(&value[..count], b"blue");
        if acl {
            let mut value = [0; 128];
            assert!(rustix::fs::getxattr(&output, DEFAULT_ACL, &mut value[..]).is_ok());
        }
    }
}
