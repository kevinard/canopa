//! Explicit compositor selection belongs to the login launcher, before Wayland connects.
use clap::ValueEnum;
use std::{ffi::OsString, path::Path, process::Command};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Compositor {
    Cage,
    Weston,
}
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Renderer {
    #[default]
    Auto,
    Software,
}
pub fn command(
    compositor: Compositor,
    executable: Option<&Path>,
    renderer: Renderer,
    extra: &[OsString],
    greeter: &Path,
    arguments: &[OsString],
) -> Command {
    let mut command = Command::new(executable.unwrap_or_else(|| match compositor {
        Compositor::Cage => Path::new("cage"),
        Compositor::Weston => Path::new("weston"),
    }));
    match compositor {
        Compositor::Cage => {
            command.args(["-s", "-m", "last"]);
            if matches!(renderer, Renderer::Software) {
                command.env("WLR_RENDERER", "pixman");
            }
        }
        Compositor::Weston => {
            command.args(["--shell=kiosk-shell.so", "--no-config", "--idle-time=0"]);
            if matches!(renderer, Renderer::Software) {
                command.arg("--renderer=pixman");
            }
        }
    }
    command.args(extra).arg("--").arg(greeter).args(arguments);
    command
}
