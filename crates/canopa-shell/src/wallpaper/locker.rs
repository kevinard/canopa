use std::{io, os::fd::AsRawFd, os::unix::process::CommandExt, process::Command};

use super::store::Store;

pub(crate) fn lock() -> io::Result<()> {
    let mut command = Command::new("swaylock");
    match Store::discover().and_then(|store| store.lock_image()) {
        Ok(Some(image)) => {
            command
                .arg("--image")
                .arg(format!("/proc/self/fd/{}", image.as_raw_fd()));
            // This CLI replaces itself with the session-owned locker. Only this
            // image's descriptor is made inheritable, immediately before exec.
            unsafe {
                command.pre_exec(move || {
                    rustix::io::fcntl_setfd(&image, rustix::io::FdFlags::empty())
                        .map_err(Into::into)
                });
            }
        }
        Ok(None) => {}
        Err(_) => eprintln!("canopa-shell: wallpaper unavailable; locking without an image"),
    }
    Err(command.exec())
}

pub(super) fn available() -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            std::fs::metadata(dir.join("swaylock"))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}
