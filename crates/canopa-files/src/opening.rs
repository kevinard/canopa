//! What opening a file means, decided on a worker before anything runs:
//! folders are browsed, desktop entries launch only when trusted, executable
//! programs and scripts always ask first, and documents open with an
//! application. Launch commands carry the activation token and never pass
//! through a shell.
use crate::mounts::MountFacts;
use std::{
    ffi::OsString,
    io,
    os::unix::{fs::MetadataExt, process::CommandExt},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

const MAX_ENTRY_BYTES: usize = 256 * 1024;
const SNIFF_BYTES: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesktopKind {
    Application,
    /// `Type=Link` with its `URL=`.
    Link(String),
}

/// How an activated entry is handled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opening {
    Directory,
    /// A `.desktop` file. Untrusted entries are launched only after the user
    /// confirms, once.
    DesktopEntry {
        name: String,
        kind: DesktopKind,
        trusted: bool,
    },
    /// An executable file with the executable bit on a filesystem where that
    /// bit means something: an ELF program or a `#!` script. The user chooses
    /// Run, Run in Terminal or (for scripts) Open.
    Program {
        script: bool,
    },
    /// Everything else opens with an application for its type.
    Document,
}

/// Classify `path`. `application_dirs` are the XDG `applications` directories
/// (see `canopa_apps::mime_apps::MimeAppsPaths::application_dirs`).
pub fn classify(
    path: &Path,
    facts: MountFacts,
    application_dirs: &[PathBuf],
) -> io::Result<Opening> {
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() {
        return Ok(Opening::Directory);
    }
    if !metadata.is_file() {
        return Ok(Opening::Document);
    }
    let executable = facts.meaningful_executable_bit()
        && metadata.uid() == rustix::process::getuid().as_raw()
        && rustix::fs::access(path, rustix::fs::Access::EXEC_OK).is_ok();
    if path
        .extension()
        .is_some_and(|extension| extension == "desktop")
        && let Some((name, kind)) = desktop_entry(path)
    {
        let installed = in_application_dir(path, application_dirs);
        return Ok(Opening::DesktopEntry {
            name,
            kind,
            trusted: installed || executable,
        });
    }
    if executable && metadata.mode() & 0o111 != 0 {
        let prefix = canopa_file::read_prefix(path, SNIFF_BYTES).unwrap_or_default();
        if prefix.starts_with(b"\x7fELF") {
            return Ok(Opening::Program { script: false });
        }
        if prefix.starts_with(b"#!") {
            return Ok(Opening::Program { script: true });
        }
    }
    Ok(Opening::Document)
}

/// Inside an applications directory, by its own lexical path or its resolved one.
pub fn in_application_dir(path: &Path, application_dirs: &[PathBuf]) -> bool {
    let lexical = path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir));
    let resolved = std::fs::canonicalize(path).ok();
    application_dirs.iter().any(|dir| {
        let canonical_dir = std::fs::canonicalize(dir).ok();
        (lexical && path.starts_with(dir))
            || resolved.as_ref().is_some_and(|resolved| {
                resolved.starts_with(dir)
                    || canonical_dir
                        .as_ref()
                        .is_some_and(|canonical| resolved.starts_with(canonical))
            })
    })
}

/// The name and kind of a `[Desktop Entry]`; `None` for anything else.
fn desktop_entry(path: &Path) -> Option<(String, DesktopKind)> {
    let body = String::from_utf8(canopa_file::read_bounded(path, MAX_ENTRY_BYTES).ok()?).ok()?;
    let mut in_entry = false;
    let (mut kind, mut name, mut url) = (None, None, None);
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim().to_owned();
            match key.trim() {
                "Type" if kind.is_none() => kind = Some(value),
                "Name" if name.is_none() => name = Some(value),
                "URL" if url.is_none() => url = Some(value),
                _ => {}
            }
        }
    }
    let kind = match kind?.as_str() {
        "Application" => DesktopKind::Application,
        "Link" => DesktopKind::Link(url.filter(|url| !url.is_empty())?),
        _ => return None,
    };
    let name = name.unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    Some((name, kind))
}

/// Check what `gio launch` needs for an entry file, with the shared launch rules.
pub fn check_desktop_entry(path: &Path) -> Result<(), canopa_apps::LaunchProblem> {
    let body = canopa_file::read_bounded(path, MAX_ENTRY_BYTES)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or(canopa_apps::LaunchProblem::EntryUnavailable)?;
    canopa_apps::check_launch_entry(&body, &canopa_apps::process_search_path())
}

fn detached(mut command: Command, token: Option<&str>) -> Command {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env_remove("XDG_ACTIVATION_TOKEN")
        .env_remove("DESKTOP_STARTUP_ID")
        .process_group(0);
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        // Wayland clients read the first; X11 startup notification the second.
        command
            .env("XDG_ACTIVATION_TOKEN", token)
            .env("DESKTOP_STARTUP_ID", token);
    }
    command
}

/// `gio launch ENTRY [FILE…]`: GIO expands field codes, handles
/// `DBusActivatable=` and `Terminal=`, as the launcher does.
pub fn launch_entry(entry: &Path, files: &[PathBuf], token: Option<&str>) -> Command {
    let mut command = Command::new("gio");
    command.arg("launch").arg(entry);
    command.args(files);
    detached(command, token)
}

/// `gio open URL`, for `Type=Link` entries.
pub fn open_url(url: &str, token: Option<&str>) -> Command {
    let mut command = Command::new("gio");
    command.arg("open").arg(url);
    detached(command, token)
}

/// The program itself, in its own folder, without arguments.
pub fn run_program(path: &Path, token: Option<&str>) -> Command {
    let mut command = Command::new(path);
    if let Some(parent) = path.parent() {
        command.current_dir(parent);
    }
    detached(command, token)
}

/// `xdg-terminal-exec PROGRAM` in the program's folder. `None` without it.
pub fn run_in_terminal(path: &Path, token: Option<&str>) -> Option<Command> {
    let terminal =
        canopa_apps::find_program("xdg-terminal-exec", &canopa_apps::process_search_path())?;
    let mut command = Command::new(terminal);
    command.arg(path);
    if let Some(parent) = path.parent() {
        command.current_dir(parent);
    }
    Some(detached(command, token))
}

/// Start a command built here. Short-lived launchers (`gio`) are awaited and a
/// failure reports their first diagnostic line; anything else is reaped on a
/// thread so it outlives this call without becoming a zombie.
pub fn spawn(mut command: Command, await_exit: bool) -> io::Result<()> {
    let program: OsString = command.get_program().to_owned();
    let mut child = command.spawn().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("{} could not start: {error}", Path::new(&program).display()),
        )
    })?;
    if !await_exit {
        drop(child.stderr.take());
        std::thread::Builder::new()
            .name("files-launched".into())
            .spawn(move || {
                let _ = child.wait();
            })?;
        return Ok(());
    }
    let mut stderr = child.stderr.take();
    let status = child.wait()?;
    if status.success() {
        return Ok(());
    }
    let mut diagnostic = Vec::new();
    if let Some(stderr) = stderr.as_mut() {
        use std::io::Read;
        let _ = stderr.take(4096).read_to_end(&mut diagnostic);
    }
    let line = String::from_utf8_lossy(&diagnostic)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_owned());
    Err(io::Error::other(line.unwrap_or_else(|| {
        format!("the application launcher failed ({status})")
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mounts::Placement;
    use std::os::unix::fs::PermissionsExt;

    const LOCAL: MountFacts = MountFacts {
        placement: Placement::Local,
        unix_permissions: true,
        noexec: false,
    };

    fn file(path: &Path, body: &[u8], mode: u32) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn programs_need_the_executable_bit_and_a_program_header() {
        let root = tempfile::tempdir().unwrap();
        let path = |name: &str| root.path().join(name);
        file(&path("run.sh"), b"#!/bin/sh\necho hi\n", 0o755);
        file(&path("plain.sh"), b"#!/bin/sh\necho hi\n", 0o644);
        file(&path("tool"), b"\x7fELF\x02\x01", 0o700);
        file(&path("photo.jpg"), b"\xff\xd8\xff", 0o755);
        assert_eq!(
            classify(&path("run.sh"), LOCAL, &[]).unwrap(),
            Opening::Program { script: true }
        );
        assert_eq!(
            classify(&path("tool"), LOCAL, &[]).unwrap(),
            Opening::Program { script: false }
        );
        assert_eq!(
            classify(&path("plain.sh"), LOCAL, &[]).unwrap(),
            Opening::Document
        );
        // An executable bit on a photo is not a reason to run it.
        assert_eq!(
            classify(&path("photo.jpg"), LOCAL, &[]).unwrap(),
            Opening::Document
        );
        assert_eq!(
            classify(root.path(), LOCAL, &[]).unwrap(),
            Opening::Directory
        );
        // FAT sticks, noexec and network mounts mark everything executable or nothing.
        for facts in [
            MountFacts {
                unix_permissions: false,
                ..LOCAL
            },
            MountFacts {
                noexec: true,
                ..LOCAL
            },
            MountFacts {
                placement: Placement::Removable,
                ..LOCAL
            },
            MountFacts {
                placement: Placement::Network,
                ..LOCAL
            },
        ] {
            assert_eq!(
                classify(&path("run.sh"), facts, &[]).unwrap(),
                Opening::Document
            );
        }
    }

    #[test]
    fn desktop_entries_are_trusted_only_when_installed_or_marked_executable() {
        let root = tempfile::tempdir().unwrap();
        let applications = root.path().join("share/applications");
        let entry = b"[Desktop Entry]\nType=Application\nName=Editor\nExec=true\n";
        file(&applications.join("editor.desktop"), entry, 0o644);
        file(&root.path().join("Downloads/editor.desktop"), entry, 0o644);
        file(&root.path().join("Desktop/editor.desktop"), entry, 0o755);
        file(
            &root.path().join("Desktop/docs.desktop"),
            b"[Desktop Entry]\nType=Link\nName=Docs\nURL=https://example.com\n",
            0o644,
        );
        file(
            &root.path().join("Desktop/notes.desktop"),
            b"just text\n",
            0o644,
        );
        std::os::unix::fs::symlink(
            root.path().join("Downloads/editor.desktop"),
            applications.join("linked.desktop"),
        )
        .unwrap();
        let dirs = [applications.clone()];
        let trusted = |path: PathBuf, facts| match classify(&path, facts, &dirs).unwrap() {
            Opening::DesktopEntry { trusted, .. } => trusted,
            other => panic!("{other:?}"),
        };
        assert!(trusted(applications.join("editor.desktop"), LOCAL));
        // A link placed in the folder by the user counts as installed.
        assert!(trusted(applications.join("linked.desktop"), LOCAL));
        assert!(!trusted(
            root.path().join("Downloads/editor.desktop"),
            LOCAL
        ));
        assert!(trusted(root.path().join("Desktop/editor.desktop"), LOCAL));
        let stick = MountFacts {
            placement: Placement::Removable,
            unix_permissions: false,
            noexec: false,
        };
        assert!(!trusted(root.path().join("Desktop/editor.desktop"), stick));
        assert_eq!(
            classify(&root.path().join("Desktop/docs.desktop"), LOCAL, &dirs).unwrap(),
            Opening::DesktopEntry {
                name: "Docs".into(),
                kind: DesktopKind::Link("https://example.com".into()),
                trusted: false
            }
        );
        assert_eq!(
            classify(&root.path().join("Desktop/notes.desktop"), LOCAL, &dirs).unwrap(),
            Opening::Document
        );
        let escaped = applications.join("../../Downloads/editor.desktop");
        assert!(!in_application_dir(&escaped, &dirs));
    }

    #[test]
    fn launch_commands_carry_the_token_and_no_shell() {
        let command = launch_entry(
            Path::new("/usr/share/applications/viewer.desktop"),
            &[PathBuf::from("/tmp/a b.png")],
            Some("token-1"),
        );
        assert_eq!(command.get_program(), "gio");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "launch",
                "/usr/share/applications/viewer.desktop",
                "/tmp/a b.png"
            ]
        );
        let envs: Vec<_> = command.get_envs().collect();
        assert!(envs.contains(&("XDG_ACTIVATION_TOKEN".as_ref(), Some("token-1".as_ref()))));
        assert!(envs.contains(&("DESKTOP_STARTUP_ID".as_ref(), Some("token-1".as_ref()))));
        let run = run_program(Path::new("/home/me/bin/tool"), None);
        assert_eq!(run.get_program(), "/home/me/bin/tool");
        assert_eq!(run.get_args().count(), 0);
        assert_eq!(run.get_current_dir(), Some(Path::new("/home/me/bin")));
        assert!(
            run.get_envs()
                .any(|(key, value)| key == "XDG_ACTIVATION_TOKEN" && value.is_none())
        );
    }

    #[test]
    fn awaited_launch_failures_report_the_launcher_diagnostic() {
        let mut command = Command::new("sh");
        command.args(["-c", "echo 'gio: no such application' >&2; exit 2"]);
        let error = spawn(detached(command, None), true).unwrap_err();
        assert_eq!(error.to_string(), "gio: no such application");
        let mut command = Command::new("true");
        command.arg("x");
        spawn(detached(command, None), true).unwrap();
        assert!(spawn(detached(Command::new("/nonexistent/program"), None), false).is_err());
    }
}
