//! Paths that start this program again after a package upgrade.
//!
//! A package manager replaces an installed executable while long-running
//! processes keep using the previous inode. `std::env::current_exe()` then
//! reads `/proc/self/exe` as `"<path> (deleted)"`, which cannot be executed, or
//! names a newer release whose private helper protocol may differ.
use std::{
    ffi::OsString,
    io,
    os::unix::{ffi::OsStringExt, fs::MetadataExt},
    path::{Path, PathBuf},
};

const RUNNING: &str = "/proc/self/exe";
const DELETED: &[u8] = b" (deleted)";

/// Executable for a child this process spawns itself, such as a private worker.
///
/// Returns the installed path while it is still the running image, so process
/// names and logs are unchanged. After replacement it returns `/proc/self/exe`,
/// which the kernel keeps pointing at the running image until this process
/// exits. A spawned child resolves that link while it is still a copy of this
/// process, so it starts the same release as its parent. Never hand this path to
/// another program: there it would name that program instead.
pub fn current() -> io::Result<PathBuf> {
    let running = std::fs::metadata(RUNNING)?;
    Ok(choose(std::env::current_exe()?, &running))
}

/// Installed path of this program for another program to execute later, such as
/// a command line given to an external watcher. After an upgrade this names the
/// newly installed release, because the running image is unreachable from other
/// processes.
pub fn installed() -> io::Result<PathBuf> {
    Ok(strip_deleted(std::env::current_exe()?))
}

fn choose(installed: PathBuf, running: &std::fs::Metadata) -> PathBuf {
    match std::fs::metadata(&installed) {
        Ok(found) if same_file(&found, running) => installed,
        _ => PathBuf::from(RUNNING),
    }
}

fn same_file(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

fn strip_deleted(path: PathBuf) -> PathBuf {
    let bytes = path.into_os_string().into_vec();
    match bytes.strip_suffix(DELETED) {
        Some(original) if Path::new(&OsString::from_vec(original.to_vec())).is_file() => {
            PathBuf::from(OsString::from_vec(original.to_vec()))
        }
        _ => PathBuf::from(OsString::from_vec(bytes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_image_keeps_its_installed_path() {
        let directory = tempfile::tempdir().unwrap();
        let installed = directory.path().join("canopa-lock");
        std::fs::write(&installed, b"old").unwrap();
        let running = std::fs::metadata(&installed).unwrap();
        assert_eq!(choose(installed.clone(), &running), installed);
    }

    #[test]
    fn replaced_or_deleted_image_uses_the_running_inode() {
        let directory = tempfile::tempdir().unwrap();
        let installed = directory.path().join("canopa-lock");
        std::fs::write(&installed, b"old").unwrap();
        let running = std::fs::metadata(&installed).unwrap();
        // Package managers write the new release beside the old one and rename it
        // into place, so the installed path names a different inode.
        let replacement = directory.path().join(".canopa-lock.new");
        std::fs::write(&replacement, b"new").unwrap();
        std::fs::rename(&replacement, &installed).unwrap();
        assert_eq!(choose(installed.clone(), &running), Path::new(RUNNING));

        let deleted = directory.path().join("canopa-lock (deleted)");
        assert_eq!(choose(deleted, &running), Path::new(RUNNING));
    }

    #[test]
    fn deleted_suffix_names_the_replacement_for_other_programs() {
        let directory = tempfile::tempdir().unwrap();
        let installed = directory.path().join("canopa-shell");
        std::fs::write(&installed, b"new").unwrap();
        let reported = directory.path().join("canopa-shell (deleted)");
        assert_eq!(strip_deleted(reported), installed);
        // Without a replacement there is nothing better to report.
        let missing = directory.path().join("missing (deleted)");
        assert_eq!(strip_deleted(missing.clone()), missing);
        assert_eq!(strip_deleted(installed.clone()), installed);
    }

    const STAGE: &str = "CANOPA_EXECUTABLE_TEST_STAGE";
    const TEST: &str = "executable::tests::replaced_program_still_starts_its_own_helper";

    #[test]
    fn replaced_program_still_starts_its_own_helper() {
        let rerun = |program: &Path, stage: &str| {
            std::process::Command::new(program)
                .args(["--exact", TEST, "--test-threads=1"])
                .env(STAGE, stage)
                .status()
                .unwrap()
        };
        match std::env::var(STAGE).as_deref() {
            // The helper only needs to start.
            Ok("helper") => {}
            // Simulate a package upgrade under the running program, then start
            // a helper the way the lock listener does.
            Ok("upgraded") => {
                let installed = std::env::current_exe().unwrap();
                let replacement = installed.with_extension("new");
                std::fs::write(&replacement, b"#!/bin/false\n").unwrap();
                std::fs::rename(&replacement, &installed).unwrap();
                let path = current().unwrap();
                assert_eq!(path, Path::new(RUNNING));
                assert!(rerun(&path, "helper").success());
            }
            _ => {
                let directory = tempfile::tempdir().unwrap();
                let program = directory.path().join("canopa-program");
                std::fs::copy(std::env::current_exe().unwrap(), &program).unwrap();
                assert!(rerun(&program, "upgraded").success());
            }
        }
    }

    #[test]
    fn current_starts_this_test_binary() {
        let path = current().unwrap();
        let running = std::fs::metadata(RUNNING).unwrap();
        assert!(same_file(&std::fs::metadata(path).unwrap(), &running));
    }
}
