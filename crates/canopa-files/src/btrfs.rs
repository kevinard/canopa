//! Optional btrfs fast paths (ADR 0152).
//!
//! Everything here is an optimization or an extra fact. On another filesystem,
//! or when btrfs refuses a request, callers keep their generic behavior: a
//! clone that cannot happen becomes an ordinary copy and a sharing proof that
//! cannot be made becomes a full content read. Nothing here mutates a file the
//! caller does not already own.
use rustix::ioctl::{Opcode, Updater, ioctl, opcode};
use std::{
    fs, io,
    os::{fd::AsFd, unix::fs::MetadataExt},
    path::Path,
};

const BTRFS_SUPER_MAGIC: u32 = 0x9123_683e;

/// Whether the descriptor's filesystem is btrfs.
pub fn is_btrfs(file: impl AsFd) -> bool {
    rustix::fs::fstatfs(file).is_ok_and(|status| status.f_type as u32 == BTRFS_SUPER_MAGIC)
}

/// `struct btrfs_ioctl_fs_info_args` (1,024 bytes).
#[allow(dead_code)]
#[repr(C)]
struct FsInfo {
    max_id: u64,
    num_devices: u64,
    fsid: [u8; 16],
    nodesize: u32,
    sectorsize: u32,
    clone_alignment: u32,
    csum_type: u16,
    csum_size: u16,
    flags: u64,
    generation: u64,
    metadata_uuid: [u8; 16],
    reserved: [u8; 944],
}
const _: () = assert!(std::mem::size_of::<FsInfo>() == 1024);
const BTRFS_IOC_FS_INFO: Opcode = opcode::read::<FsInfo>(0x94, 31);

/// The btrfs filesystem UUID, or `None` on another filesystem. Subvolumes of
/// one filesystem have different device numbers but share this identifier,
/// and extent addresses are only comparable within it.
fn filesystem(file: impl AsFd) -> Option<[u8; 16]> {
    if !is_btrfs(&file) {
        return None;
    }
    let mut info = FsInfo {
        max_id: 0,
        num_devices: 0,
        fsid: [0; 16],
        nodesize: 0,
        sectorsize: 0,
        clone_alignment: 0,
        csum_type: 0,
        csum_size: 0,
        // No optional fields are requested; the kernel reads this input.
        flags: 0,
        generation: 0,
        metadata_uuid: [0; 16],
        reserved: [0; 944],
    };
    // SAFETY: BTRFS_IOC_FS_INFO reads and writes exactly one initialized
    // `btrfs_ioctl_fs_info_args`, whose layout `FsInfo` reproduces.
    unsafe { ioctl(&file, Updater::<BTRFS_IOC_FS_INFO, FsInfo>::new(&mut info)) }.ok()?;
    Some(info.fsid)
}

/// Clone all of `input` into the new, empty `output` when both are on one
/// btrfs filesystem. The clone shares the source's data extents, so it takes
/// no data space and no data is read or written. Returns `false`, with
/// `output` empty again, whenever the clone is not made: the caller then
/// copies the bytes itself. File offsets are left unchanged.
pub(crate) fn clone_file(input: &fs::File, output: &fs::File) -> io::Result<bool> {
    if !is_btrfs(input) || !is_btrfs(output) {
        return Ok(false);
    }
    match rustix::fs::ioctl_ficlone(output, input) {
        Ok(()) => Ok(true),
        // Different filesystems, a nodatacow/datacow mismatch, a swap file or
        // a failure part way: the ordinary copy decides. A partial clone is
        // removed first so that copy starts from an empty output.
        Err(_) => {
            rustix::fs::ftruncate(output, 0)?;
            Ok(false)
        }
    }
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Extent {
    logical: u64,
    physical: u64,
    length: u64,
    reserved64: [u64; 2],
    flags: u32,
    reserved: [u32; 3],
}

/// `struct fiemap` followed by its extent array.
#[allow(dead_code)]
#[repr(C)]
struct Map {
    start: u64,
    length: u64,
    flags: u32,
    mapped: u32,
    count: u32,
    reserved: u32,
    extents: [Extent; EXTENTS_PER_CALL],
}

/// `struct fiemap` without the flexible array; the opcode encodes its size.
#[allow(dead_code)]
#[repr(C)]
struct MapHeader {
    start: u64,
    length: u64,
    flags: u32,
    mapped: u32,
    count: u32,
    reserved: u32,
}
const FS_IOC_FIEMAP: Opcode = opcode::read_write::<MapHeader>(b'f', 11);
const EXTENTS_PER_CALL: usize = 256;
/// Larger maps are not compared; the caller reads the contents instead.
const MAXIMUM_EXTENTS: usize = 64 * 1024;

const FIEMAP_FLAG_SYNC: u32 = 0x1;
const FIEMAP_EXTENT_LAST: u32 = 0x1;
const FIEMAP_EXTENT_MERGED: u32 = 0x1000;
const FIEMAP_EXTENT_SHARED: u32 = 0x2000;
/// Flags whose extents prove equality by address alone. Every other flag
/// (unknown or delayed location, compressed or encrypted data, inline or tail
/// data, preallocated space) refuses the proof. A compressed extent in
/// particular reports the start of its compressed data for every reference,
/// so equal addresses would not mean equal contents.
const PROVING_FLAGS: u32 = FIEMAP_EXTENT_LAST | FIEMAP_EXTENT_MERGED | FIEMAP_EXTENT_SHARED;

/// The extent map of a whole file, after flushing its dirty data so delayed
/// writes have their final location. `None` when the map is too large.
fn extents(file: &fs::File) -> io::Result<Option<Vec<Extent>>> {
    let mut all = Vec::new();
    let mut start = 0;
    loop {
        let mut map = Map {
            start,
            length: u64::MAX - start,
            flags: FIEMAP_FLAG_SYNC,
            mapped: 0,
            count: EXTENTS_PER_CALL as u32,
            reserved: 0,
            extents: [Extent::default(); EXTENTS_PER_CALL],
        };
        // SAFETY: FS_IOC_FIEMAP reads the header and writes at most `count`
        // extents into the array that follows it in the same allocation.
        unsafe { ioctl(file, Updater::<FS_IOC_FIEMAP, Map>::new(&mut map)) }?;
        let mapped = (map.mapped as usize).min(EXTENTS_PER_CALL);
        if mapped == 0 {
            return Ok(Some(all));
        }
        all.extend_from_slice(&map.extents[..mapped]);
        if all.len() > MAXIMUM_EXTENTS {
            return Ok(None);
        }
        let last = map.extents[mapped - 1];
        if last.flags & FIEMAP_EXTENT_LAST != 0 {
            return Ok(Some(all));
        }
        let Some(next) = last.logical.checked_add(last.length) else {
            return Ok(Some(all));
        };
        if next <= start {
            return Ok(None);
        }
        start = next;
    }
}

/// Whether two open regular files provably have the same contents because they
/// reference the same data extents on one btrfs filesystem, as a clone and its
/// source do until either is written. btrfs never writes a shared extent in
/// place, even for nodatacow files, so a write to either file gives it a new
/// extent and ends the proof. `false` means only "not proven": the caller
/// compares contents. The caller brackets this with its own metadata checks.
pub(crate) fn share_all_extents(first: &fs::File, second: &fs::File) -> io::Result<bool> {
    let (Some(first_fs), Some(second_fs)) = (filesystem(first), filesystem(second)) else {
        return Ok(false);
    };
    let (first_metadata, second_metadata) = (first.metadata()?, second.metadata()?);
    if first_fs != second_fs
        || !first_metadata.is_file()
        || !second_metadata.is_file()
        || first_metadata.len() != second_metadata.len()
        || first_metadata.len() == 0
        // A file is never offered as proof for itself.
        || (first_metadata.dev() == second_metadata.dev()
            && first_metadata.ino() == second_metadata.ino())
    {
        return Ok(false);
    }
    let (Some(first_map), Some(second_map)) = (extents(first)?, extents(second)?) else {
        return Ok(false);
    };
    if first_map.is_empty() || first_map.len() != second_map.len() {
        return Ok(false);
    }
    let size = first_metadata.len();
    let mut covered = 0u64;
    for (left, right) in first_map.iter().zip(&second_map) {
        if left.flags & !PROVING_FLAGS != 0
            || right.flags & !PROVING_FLAGS != 0
            || left.logical != right.logical
            || left.physical != right.physical
            || left.length != right.length
            || left.length == 0
        {
            return Ok(false);
        }
        covered = covered.max(left.logical.saturating_add(left.length));
    }
    // Gaps between extents are holes in both files and read as zeros; the map
    // must still reach the end of the data.
    Ok(covered >= size)
}

/// A subvolume as btrfs knows it: its filesystem UUID and subvolume ID. Both
/// survive a rename of the subvolume or of its ancestors, and a remount of its
/// filesystem at another path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Subvolume {
    filesystem: [u8; 16],
    id: u64,
}

/// `struct btrfs_ioctl_ino_lookup_args` (4,096 bytes).
#[allow(dead_code)]
#[repr(C)]
struct InodeLookup {
    treeid: u64,
    objectid: u64,
    name: [u8; 4080],
}
const _: () = assert!(std::mem::size_of::<InodeLookup>() == 4096);
const BTRFS_IOC_INO_LOOKUP: Opcode = opcode::read_write::<InodeLookup>(0x94, 18);
const BTRFS_FIRST_FREE_OBJECTID: u64 = 256;

/// The subvolume whose root directory `file` is, or `None` when it is not
/// the root of a btrfs subvolume.
pub(crate) fn subvolume_root(file: &fs::File) -> Option<Subvolume> {
    let filesystem = filesystem(file)?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_dir() || metadata.ino() != BTRFS_FIRST_FREE_OBJECTID {
        return None;
    }
    let mut lookup = InodeLookup {
        // Zero names the descriptor's own subvolume; looking up its root
        // inode is the one form of this request open to every user.
        treeid: 0,
        objectid: BTRFS_FIRST_FREE_OBJECTID,
        name: [0; 4080],
    };
    // SAFETY: BTRFS_IOC_INO_LOOKUP reads and writes one initialized
    // `btrfs_ioctl_ino_lookup_args`, whose layout `InodeLookup` reproduces.
    unsafe {
        ioctl(
            file,
            Updater::<BTRFS_IOC_INO_LOOKUP, InodeLookup>::new(&mut lookup),
        )
    }
    .ok()?;
    Some(Subvolume {
        filesystem,
        id: lookup.treeid,
    })
}

/// `struct btrfs_ioctl_get_subvol_rootref_args` (4,096 bytes).
#[allow(dead_code)]
#[repr(C)]
struct ChildSubvolumes {
    min_treeid: u64,
    rootref: [[u64; 2]; 255],
    num_items: u8,
    align: [u8; 7],
}
const _: () = assert!(std::mem::size_of::<ChildSubvolumes>() == 4096);
const BTRFS_IOC_GET_SUBVOL_ROOTREF: Opcode = opcode::read_write::<ChildSubvolumes>(0x94, 61);

/// `struct btrfs_ioctl_ino_lookup_user_args` (4,096 bytes).
#[allow(dead_code)]
#[repr(C)]
struct ChildLookup {
    dirid: u64,
    treeid: u64,
    name: [u8; 256],
    path: [u8; 3824],
}
const _: () = assert!(std::mem::size_of::<ChildLookup>() == 4096);
const BTRFS_IOC_INO_LOOKUP_USER: Opcode = opcode::read_write::<ChildLookup>(0x94, 62);

/// Most subvolumes searched for one lookup; snapshot-heavy filesystems stop
/// there and the subvolume is reported missing.
const SEARCH_LIMIT: usize = 4096;

/// The subvolumes directly inside the subvolume whose root is `root`, as
/// `(id, folder inode)` pairs.
fn child_subvolumes(root: &fs::File) -> io::Result<Vec<(u64, u64)>> {
    let mut children = Vec::new();
    let mut request = ChildSubvolumes {
        min_treeid: 0,
        rootref: [[0; 2]; 255],
        num_items: 0,
        align: [0; 7],
    };
    loop {
        request.num_items = 0;
        // SAFETY: BTRFS_IOC_GET_SUBVOL_ROOTREF reads and writes one initialized
        // `btrfs_ioctl_get_subvol_rootref_args`, reproduced by `ChildSubvolumes`.
        let result = unsafe {
            ioctl(
                root,
                Updater::<BTRFS_IOC_GET_SUBVOL_ROOTREF, ChildSubvolumes>::new(&mut request),
            )
        };
        let count = usize::from(request.num_items).min(255);
        children.extend(request.rootref[..count].iter().map(|[id, dir]| (*id, *dir)));
        match result {
            Ok(_) => return Ok(children),
            // More references follow; the kernel advanced `min_treeid`.
            Err(rustix::io::Errno::OVERFLOW) if count > 0 => {}
            Err(error) => return Err(error.into()),
        }
        if children.len() > SEARCH_LIMIT {
            return Ok(children);
        }
    }
}

/// Where the child subvolume `id`, in folder inode `dir`, is below `root`.
fn child_path(root: &fs::File, root_path: &Path, id: u64, dir: u64) -> Option<std::path::PathBuf> {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let mut lookup = ChildLookup {
        dirid: dir,
        treeid: id,
        name: [0; 256],
        path: [0; 3824],
    };
    // SAFETY: BTRFS_IOC_INO_LOOKUP_USER reads and writes one initialized
    // `btrfs_ioctl_ino_lookup_user_args`, reproduced by `ChildLookup`. Both
    // outputs are NUL-terminated within their arrays.
    unsafe {
        ioctl(
            root,
            Updater::<BTRFS_IOC_INO_LOOKUP_USER, ChildLookup>::new(&mut lookup),
        )
    }
    .ok()?;
    let terminated = |bytes: &[u8]| -> Option<Vec<u8>> {
        Some(bytes[..bytes.iter().position(|byte| *byte == 0)?].to_vec())
    };
    let folder = terminated(&lookup.path)?;
    let name = terminated(&lookup.name)?;
    if name.is_empty() || name.contains(&b'/') || name == b"." || name == b".." {
        return None;
    }
    let folder = Path::new(OsStr::from_bytes(&folder));
    if folder.is_absolute()
        || folder
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(root_path.join(folder).join(OsStr::from_bytes(&name)))
}

/// Find the current folder of `target` below the btrfs mount `mounts` of its
/// filesystem, following only subvolume links the user may look up. `accept`
/// can refuse a found folder (one inside Trash storage, for example). Only a
/// folder that reopens as the root of `target` is returned.
pub(crate) fn find_subvolume(
    target: Subvolume,
    mounts: impl IntoIterator<Item = std::path::PathBuf>,
    accept: impl Fn(&Path) -> bool,
) -> Option<std::path::PathBuf> {
    let mut searched = 0;
    for mount in mounts {
        let Ok(root) = fs::File::open(&mount) else {
            continue;
        };
        if filesystem(&root) != Some(target.filesystem) || subvolume_root(&root).is_none() {
            continue;
        }
        let mut pending = std::collections::VecDeque::from([(mount, root)]);
        while let Some((path, root)) = pending.pop_front() {
            let Ok(children) = child_subvolumes(&root) else {
                continue;
            };
            for (id, dir) in children {
                searched += 1;
                if searched > SEARCH_LIMIT {
                    return None;
                }
                let Some(child) = child_path(&root, &path, id, dir) else {
                    continue;
                };
                let Ok(opened) = fs::File::open(&child) else {
                    continue;
                };
                let found = subvolume_root(&opened);
                if found.is_none_or(|found| found.filesystem != target.filesystem) {
                    continue;
                }
                if id == target.id {
                    if found == Some(target) && accept(&child) {
                        return Some(child);
                    }
                    continue;
                }
                pending.push_back((child, opened));
            }
        }
    }
    None
}

/// `struct btrfs_ioctl_vol_args` (4,096 bytes).
#[allow(dead_code)]
#[repr(C)]
struct VolumeArguments {
    fd: i64,
    name: [u8; 4088],
}
const BTRFS_IOC_SUBVOL_CREATE: Opcode = opcode::write::<VolumeArguments>(0x94, 14);

/// Create an empty subvolume named `name` in the btrfs folder `parent`. Users
/// may create subvolumes in folders they can write; an empty one is removed
/// with an ordinary `rmdir`. Used by tests that need a nested subvolume.
#[doc(hidden)]
pub fn create_subvolume(parent: &Path, name: &str) -> io::Result<()> {
    let parent = fs::File::open(parent)?;
    if !is_btrfs(&parent) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "subvolumes need a btrfs folder",
        ));
    }
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() >= 4088 || bytes.contains(&b'/') || bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid subvolume name",
        ));
    }
    let mut arguments = VolumeArguments {
        fd: 0,
        name: [0; 4088],
    };
    arguments.name[..bytes.len()].copy_from_slice(bytes);
    // SAFETY: BTRFS_IOC_SUBVOL_CREATE reads one `btrfs_ioctl_vol_args` with a
    // NUL-terminated name, which the zeroed array guarantees.
    unsafe {
        ioctl(
            &parent,
            Updater::<BTRFS_IOC_SUBVOL_CREATE, VolumeArguments>::new(&mut arguments),
        )
    }?;
    Ok(())
}

/// Whether two open files have provably equal contents by shared extents.
/// Public for filesystem tests; see [`share_all_extents`].
#[doc(hidden)]
pub fn shares_all_extents(first: &fs::File, second: &fs::File) -> io::Result<bool> {
    share_all_extents(first, second)
}

/// A private folder on btrfs for tests, below `CANOPA_FILES_TEST_BTRFS`.
/// Without that variable btrfs-only tests return early and only the generic
/// fallbacks are exercised.
#[cfg(test)]
pub(crate) fn test_root() -> Option<tempfile::TempDir> {
    let path = std::env::var_os("CANOPA_FILES_TEST_BTRFS")?;
    let root = tempfile::Builder::new()
        .prefix("canopa-btrfs-")
        .tempdir_in(path)
        .expect("CANOPA_FILES_TEST_BTRFS must be a writable folder");
    assert!(
        is_btrfs(fs::File::open(root.path()).unwrap()),
        "CANOPA_FILES_TEST_BTRFS must be on btrfs"
    );
    Some(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, Write};

    fn written(path: &Path, bytes: &[u8]) -> fs::File {
        let mut file = fs::File::create_new(path).unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        file
    }

    #[test]
    fn other_filesystems_keep_the_generic_path() {
        let root = tempfile::tempdir().unwrap();
        if is_btrfs(fs::File::open(root.path()).unwrap()) {
            return;
        }
        let input = written(&root.path().join("input"), b"generic contents");
        let output = fs::File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(root.path().join("output"))
            .unwrap();
        assert!(!clone_file(&input, &output).unwrap());
        assert_eq!(output.metadata().unwrap().len(), 0);
        assert!(!share_all_extents(&input, &input).unwrap());
        assert!(create_subvolume(root.path(), "volume").is_err());
    }

    #[test]
    fn a_clone_shares_every_extent_until_either_file_is_written() {
        let Some(root) = test_root() else { return };
        let contents: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let input = written(&root.path().join("input"), &contents);
        let output = fs::File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(root.path().join("output"))
            .unwrap();
        assert!(clone_file(&input, &output).unwrap());
        assert_eq!(fs::read(root.path().join("output")).unwrap(), contents);
        assert!(share_all_extents(&input, &output).unwrap());
        assert!(share_all_extents(&output, &input).unwrap());
        // Writing one byte in place, without changing the size, breaks it.
        let mut edited = fs::File::options()
            .write(true)
            .open(root.path().join("output"))
            .unwrap();
        edited.seek(io::SeekFrom::Start(4096)).unwrap();
        edited.write_all(b"x").unwrap();
        assert!(!share_all_extents(&input, &output).unwrap());
    }

    #[test]
    fn equal_contents_written_separately_are_not_proved_equal() {
        let Some(root) = test_root() else { return };
        let first = written(&root.path().join("first"), &[7; 64 * 1024]);
        let second = written(&root.path().join("second"), &[7; 64 * 1024]);
        assert!(!share_all_extents(&first, &second).unwrap());
        // A file is not offered as proof for itself.
        let again = fs::File::open(root.path().join("first")).unwrap();
        assert!(!share_all_extents(&first, &again).unwrap());
    }

    #[test]
    fn small_inline_files_are_left_to_the_content_check() {
        let Some(root) = test_root() else { return };
        let input = written(&root.path().join("input"), b"tiny");
        let output = fs::File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(root.path().join("output"))
            .unwrap();
        // Whether or not btrfs clones an inline extent, it never proves it.
        let _ = clone_file(&input, &output).unwrap();
        assert!(!share_all_extents(&input, &output).unwrap());
    }

    #[test]
    fn subvolumes_share_one_filesystem_and_accept_clones() {
        let Some(root) = test_root() else { return };
        create_subvolume(root.path(), "volume").unwrap();
        let nested = root.path().join("volume");
        assert_ne!(
            fs::metadata(root.path()).unwrap().dev(),
            fs::metadata(&nested).unwrap().dev()
        );
        let contents = vec![3; 256 * 1024];
        let input = written(&root.path().join("input"), &contents);
        let output = fs::File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(nested.join("output"))
            .unwrap();
        assert!(clone_file(&input, &output).unwrap());
        assert!(share_all_extents(&input, &output).unwrap());
        fs::remove_file(nested.join("output")).unwrap();
        // An empty subvolume is removed like a folder.
        fs::remove_dir(&nested).unwrap();
    }
}
