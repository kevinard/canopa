//! `canopa-shell --app-scope`: names the systemd scope of a launched application.
//!
//! Niri puts every spawned process into `app-niri-<argv basename>-<pid>.scope`,
//! so all launcher apps used to share the name of `gio`. The launcher hands Niri
//! this helper instead. It waits until Niri has placed it (Niri does so right
//! after the spawn, racing the helper), moves itself into
//! `app-canopa-<desktop id>-<random>.scope`, following the systemd naming
//! convention for desktop applications, and then starts the application:
//!
//! ```text
//! canopa-shell --app-scope ID -- PROGRAM [ARG...]          exec the program
//! canopa-shell --app-scope ID --desktop-action FILE ACTION launch one [Desktop Action]
//! ```
//!
//! The scope step is best effort: without a systemd user manager the
//! application starts where it is.
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const FLAG: &str = "--app-scope";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// Replace the helper with this program.
    Exec(Vec<OsString>),
    /// Launch a desktop entry's `[Desktop Action]` through GIO.
    DesktopAction { file: PathBuf, action: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub id: String,
    pub launch: Launch,
}

/// The arguments after [`FLAG`].
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Request, String> {
    let mut args = args.into_iter();
    let id = args
        .next()
        .and_then(|id| id.into_string().ok())
        .filter(|id| !id.is_empty())
        .ok_or("--app-scope requires a desktop id")?;
    let launch = match args.next().as_ref().and_then(|word| word.to_str()) {
        Some("--") => {
            let argv: Vec<_> = args.collect();
            if argv.is_empty() {
                return Err("--app-scope requires a program".into());
            }
            Launch::Exec(argv)
        }
        Some("--desktop-action") => {
            let file = args
                .next()
                .map(PathBuf::from)
                .ok_or("missing desktop file")?;
            let action = args
                .next()
                .and_then(|action| action.into_string().ok())
                .ok_or("missing desktop action")?;
            if args.next().is_some() {
                return Err("unexpected argument after the desktop action".into());
            }
            Launch::DesktopAction { file, action }
        }
        _ => return Err("--app-scope expects -- PROGRAM or --desktop-action FILE ACTION".into()),
    };
    Ok(Request { id, launch })
}

/// `helper --app-scope ID -- argv…`.
pub fn wrap(helper: &str, id: &str, argv: &[String]) -> Vec<String> {
    [helper, FLAG, id, "--"]
        .into_iter()
        .map(str::to_owned)
        .chain(argv.iter().cloned())
        .collect()
}

/// `helper --app-scope ID --desktop-action FILE ACTION`.
pub fn desktop_action(helper: &str, id: &str, file: &str, action: &str) -> Vec<String> {
    [helper, FLAG, id, "--desktop-action", file, action]
        .map(str::to_owned)
        .to_vec()
}

/// This executable, as the helper program.
pub fn helper() -> String {
    canopa_file::executable::installed()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "canopa-shell".into())
}

/// `app-canopa-<escaped id>-<random>.scope`, or `None` when the id makes the
/// name longer than systemd accepts. The id loses a trailing `.desktop`, and
/// bytes other than ASCII letters, digits, `:`, `_` and `.` become `\xNN`,
/// like `systemd-escape` (and Niri's own scope names).
pub fn scope_name(id: &str, random: &str) -> Option<String> {
    use std::fmt::Write as _;
    let id = id.strip_suffix(".desktop").unwrap_or(id);
    let mut name = String::from("app-canopa-");
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'.') {
            name.push(char::from(byte));
        } else {
            let _ = write!(name, "\\x{byte:02x}");
        }
    }
    let _ = write!(name, "-{random}.scope");
    (!id.is_empty() && name.len() <= 255).then_some(name)
}

/// A random suffix without dashes.
fn random_suffix() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    format!("{:016x}", hasher.finish())
}

/// This process's cgroup v2 path, such as
/// `/user.slice/user-1000.slice/user@1000.service/session.slice/niri.service`.
fn own_cgroup() -> Option<String> {
    std::fs::read_to_string("/proc/self/cgroup")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .map(str::to_owned)
}

/// Whether the compositor has yet to move a just-spawned process: it starts
/// in the compositor's own service and lands in an `app-niri-…` scope.
pub fn awaiting_compositor(cgroup: &str) -> bool {
    cgroup
        .rsplit('/')
        .next()
        .is_some_and(|unit| unit.ends_with(".service"))
}

fn enter_scope(id: &str) -> Result<String, String> {
    let name = scope_name(id, &random_suffix()).ok_or("desktop id too long for a unit name")?;
    // Niri moves its child right after the spawn. Moving first would leave
    // the application in Niri's scope once that move lands.
    let deadline = Instant::now() + Duration::from_secs(1);
    while own_cgroup().is_some_and(|cgroup| awaiting_compositor(&cgroup))
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(3), start_scope(&name))
            .await
            .map_err(|_| "systemd did not start the scope in time".to_owned())?
    })?;
    Ok(name)
}

async fn start_scope(name: &str) -> Result<(), String> {
    use futures_util::StreamExt;
    use zbus::zvariant::{OwnedObjectPath, Value};
    let connection = zbus::Connection::session()
        .await
        .map_err(|error| error.to_string())?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )
    .await
    .map_err(|error| error.to_string())?;
    // Subscribe first so the job's completion cannot be missed.
    let mut removed = proxy
        .receive_signal("JobRemoved")
        .await
        .map_err(|error| error.to_string())?;
    let pids = vec![std::process::id()];
    let properties = vec![
        ("PIDs", Value::from(pids)),
        ("CollectMode", Value::from("inactive-or-failed")),
    ];
    let aux: Vec<(&str, Vec<(&str, Value<'_>)>)> = Vec::new();
    let job: OwnedObjectPath = proxy
        .call("StartTransientUnit", &(name, "fail", properties, aux))
        .await
        .map_err(|error| error.to_string())?;
    while let Some(signal) = removed.next().await {
        let Ok((_, path, _, result)) = signal
            .body()
            .deserialize::<(u32, OwnedObjectPath, String, String)>()
        else {
            continue;
        };
        if path == job {
            return if result == "done" {
                Ok(())
            } else {
                Err(format!("scope job ended with {result}"))
            };
        }
    }
    Err("systemd closed the connection".into())
}

/// `GDesktopAppInfo`, which `libgio-2.0` (already linked through the `gio`
/// crate) still exports. The gio 0.22 bindings moved it to `gio-unix`.
mod ffi {
    use std::ffi::{c_char, c_void};
    unsafe extern "C" {
        pub fn g_desktop_app_info_new_from_filename(filename: *const c_char) -> *mut c_void;
        pub fn g_desktop_app_info_list_actions(info: *mut c_void) -> *const *const c_char;
        pub fn g_desktop_app_info_launch_action(
            info: *mut c_void,
            action_name: *const c_char,
            launch_context: *mut c_void,
        );
        pub fn g_object_unref(object: *mut c_void);
    }
}

/// Launch one `[Desktop Action]` the way GIO does, including terminal entries.
pub fn launch_desktop_action(file: &std::path::Path, action: &str) -> Result<(), String> {
    use std::ffi::{CStr, CString};
    use std::os::unix::ffi::OsStrExt;
    let file = CString::new(file.as_os_str().as_bytes()).map_err(|_| "invalid desktop file")?;
    let action = CString::new(action).map_err(|_| "invalid desktop action")?;
    // SAFETY: the strings are NUL-terminated and outlive the calls; the
    // returned object is checked for NULL and released exactly once; the
    // action list is a NULL-terminated array owned by the object.
    unsafe {
        let info = ffi::g_desktop_app_info_new_from_filename(file.as_ptr());
        if info.is_null() {
            return Err("desktop entry is unavailable".into());
        }
        let mut listed = false;
        let mut names = ffi::g_desktop_app_info_list_actions(info);
        while !names.is_null() && !(*names).is_null() {
            listed |= CStr::from_ptr(*names) == action.as_c_str();
            names = names.add(1);
        }
        if listed {
            ffi::g_desktop_app_info_launch_action(info, action.as_ptr(), std::ptr::null_mut());
        }
        ffi::g_object_unref(info);
        if listed {
            Ok(())
        } else {
            Err("the desktop entry does not offer that action".into())
        }
    }
}

/// Runs the helper; returns only on failure, or after a desktop action started.
pub fn run(request: Request) -> i32 {
    if let Err(error) = enter_scope(&request.id) {
        // The application still starts, in the scope the compositor gave it.
        eprintln!("canopa-shell: application scope not created: {error}");
    }
    match request.launch {
        Launch::Exec(argv) => {
            use std::os::unix::process::CommandExt;
            let error = std::process::Command::new(&argv[0]).args(&argv[1..]).exec();
            eprintln!("canopa-shell: could not start the application: {error}");
            1
        }
        Launch::DesktopAction { file, action } => match launch_desktop_action(&file, &action) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("canopa-shell: {error}");
                1
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_names_follow_the_systemd_desktop_application_convention() {
        assert_eq!(
            scope_name("org.mozilla.firefox.desktop", "0123abcd").as_deref(),
            Some("app-canopa-org.mozilla.firefox-0123abcd.scope")
        );
        // Dashes separate the name's fields, so ids escape them.
        assert_eq!(
            scope_name("kde-org.kde.konsole.desktop", "f00").as_deref(),
            Some(r"app-canopa-kde\x2dorg.kde.konsole-f00.scope")
        );
        assert_eq!(
            scope_name("my app/é", "1").as_deref(),
            Some(r"app-canopa-my\x20app\x2f\xc3\xa9-1.scope")
        );
        assert_eq!(scope_name(".desktop", "1"), None);
        assert_eq!(scope_name(&"x".repeat(250), "1"), None);
        let random = random_suffix();
        assert_eq!(random.len(), 16);
        assert!(!random.contains('-'));
        assert_ne!(random, random_suffix());
    }

    #[test]
    fn helper_arguments_round_trip() {
        let argv = wrap(
            "/usr/bin/canopa-shell",
            "org.example.App.desktop",
            &[
                "gio".into(),
                "launch".into(),
                "/a b.desktop".into(),
                "--".into(),
            ],
        );
        assert_eq!(
            parse(argv.iter().skip(2).map(OsString::from)).unwrap(),
            Request {
                id: "org.example.App.desktop".into(),
                launch: Launch::Exec(
                    ["gio", "launch", "/a b.desktop", "--"]
                        .map(OsString::from)
                        .to_vec()
                ),
            }
        );
        let argv = desktop_action("canopa-shell", "app.desktop", "/x.desktop", "new-window");
        assert_eq!(
            parse(argv.iter().skip(2).map(OsString::from))
                .unwrap()
                .launch,
            Launch::DesktopAction {
                file: "/x.desktop".into(),
                action: "new-window".into()
            }
        );
        for bad in [
            &[][..],
            &["id"][..],
            &["id", "--"][..],
            &["id", "--desktop-action", "/x"][..],
            &["id", "--desktop-action", "/x", "a", "b"][..],
            &["", "--", "true"][..],
        ] {
            assert!(parse(bad.iter().map(OsString::from)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn helper_waits_only_while_in_the_compositor_service() {
        let service = "/user.slice/user-1000.slice/user@1000.service/session.slice/niri.service";
        assert!(awaiting_compositor(service));
        assert!(!awaiting_compositor(
            r"/user.slice/user-1000.slice/user@1000.service/app.slice/app-niri-canopa\x2dshell-42.scope"
        ));
        assert!(!awaiting_compositor(
            "/user.slice/user-1000.slice/session-2.scope"
        ));
    }

    #[test]
    fn desktop_actions_launch_their_own_exec_line() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("capture");
        let output = dir.path().join("output");
        std::fs::write(
            &script,
            "#!/bin/sh\nout=$1\nshift\nprintf '%s\\n' \"$@\" > \"$out\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let file = dir.path().join("org.example.Fixture.desktop");
        std::fs::write(
            &file,
            format!(
                "[Desktop Entry]\nType=Application\nName=Fixture\nExec={s} {o} main\nActions=private;\n\n[Desktop Action private]\nName=Private\nExec={s} {o} --private\n",
                s = script.display(),
                o = output.display()
            ),
        )
        .unwrap();
        assert!(launch_desktop_action(&file, "missing").is_err());
        launch_desktop_action(&file, "private").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while std::fs::read_to_string(&output).ok().as_deref() != Some("--private\n")
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "--private\n");
    }
}
