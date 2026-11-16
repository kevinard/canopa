use std::{io, os::fd::AsRawFd, os::unix::process::CommandExt, process::Command};

use super::store::Store;

/// `canopa-shell lock`: hand off to Canopa Lock, which shows the lock-screen
/// picture set through the Wallpaper portal (or to logind's `lock-session`
/// without it). `--swaylock` keeps the former Swaylock path. Returns only when
/// nothing could be started.
pub(crate) fn lock(swaylock: bool) -> io::Error {
    if swaylock {
        return lock_with_swaylock();
    }
    match crate::launcher::system::installed_lock_command() {
        Some(argv) => Command::new(&argv[0]).args(&argv[1..]).exec(),
        None => io::Error::new(
            io::ErrorKind::NotFound,
            "neither canopa-lock nor loginctl is installed",
        ),
    }
}

fn lock_with_swaylock() -> io::Error {
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
    command.exec()
}

/// Whether a locker that shows the portal's lock-screen picture is installed:
/// Canopa Lock, or Swaylock for `canopa-shell lock --swaylock`.
pub(super) fn available() -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            ["canopa-lock", "swaylock"].iter().any(|name| {
                std::fs::metadata(dir.join(name))
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
        })
    })
}
