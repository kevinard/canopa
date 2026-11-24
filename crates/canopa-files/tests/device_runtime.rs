//! Public runtime acceptance with the real private GIO provider. The custom
//! harness also hosts the pinned helper entrypoint in this same executable.
use canopa_files::{
    action_jobs::{ActionRequest, Phase as ActionPhase},
    actions::CustomAction,
    application::Application,
    local_volumes::{
        self, Device, DeviceAnswer, DeviceOpen, DeviceOutcome, DevicePhase, DeviceQuestion,
        DeviceRequestId, DeviceRequestSnapshot, DeviceRequests, DeviceStatus, LocalOperations,
    },
    operation_jobs::{Phase, Request},
    operations::OperationRequest,
};
use futures_channel::oneshot;
use rustix::process::{Pid, Signal, kill_process, kill_process_group};
use std::{
    fs,
    future::Future,
    os::unix::{
        fs::{PermissionsExt, symlink},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    task::{Context, Poll},
    thread,
    time::{Duration, Instant},
};

fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "private device runtime did not settle"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn reply<T>(mut receiver: oneshot::Receiver<T>) -> T {
    wait(|| receiver.try_recv().unwrap())
}

fn complete<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(futures_util::task::noop_waker_ref());
    wait(|| match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => Some(value),
        Poll::Pending => None,
    })
}

fn device(runtime: &LocalOperations) -> Device {
    wait(|| {
        let catalog = runtime.devices();
        (catalog.status == DeviceStatus::Ready)
            .then(|| {
                catalog
                    .devices
                    .into_iter()
                    .find(|device| device.name == "Fixture USB")
            })
            .flatten()
    })
}

fn finished(requests: &DeviceRequests, id: DeviceRequestId) -> DeviceRequestSnapshot {
    wait(|| {
        requests
            .snapshot()
            .into_iter()
            .find(|job| job.id == id && job.phase == DevicePhase::Finished)
    })
}

fn helper_pid() -> Pid {
    let executable = std::env::current_exe().unwrap();
    // Linux lists the helper under the observer task that starts it.
    for task in fs::read_dir("/proc/self/task").unwrap().flatten() {
        let children = fs::read_to_string(task.path().join("children")).unwrap_or_default();
        for pid in children.split_whitespace() {
            if fs::read_link(format!("/proc/{pid}/exe")).ok().as_ref() != Some(&executable) {
                continue;
            }
            let bytes = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
            if bytes
                .split(|b| *b == 0)
                .any(|arg| arg == b"--internal-volume-monitor")
            {
                return Pid::from_raw(pid.parse().unwrap()).unwrap();
            }
        }
    }
    panic!("exact owned volume helper");
}

struct Frozen(Pid);
impl Frozen {
    fn new(pid: Pid) -> Self {
        kill_process(pid, Signal::STOP).unwrap();
        wait(|| {
            fs::read_to_string(format!("/proc/{}/status", pid.as_raw_pid()))
                .ok()
                .filter(|s| {
                    s.lines()
                        .any(|line| line.starts_with("State:") && line.contains('T'))
                })
        });
        Self(pid)
    }
}
impl Drop for Frozen {
    fn drop(&mut self) {
        let _ = kill_process(self.0, Signal::CONT);
    }
}

fn exercise(root: &Path) {
    let media = root.join("volumes");
    let payload = media.join("Fixture USB");
    let original = b"Actual parent-runtime device bytes.\n";
    // This private state directory has one exclusive test owner. Exercise the
    // production composition, including its shared storage and action handles.
    let application =
        Application::start(&root.join("state"), &canopa_config::Request::Defaults).unwrap();
    let activity = application.activity().clone();
    let runtime = application.operations.as_ref().unwrap().clone();
    let actions = application.actions.as_ref().unwrap().clone();
    let frontend = runtime.device_requests();
    drop(runtime.take_notices()); // No frontend drives preparation or execution.
    wait(|| {
        let state = runtime.jobs().snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        (!state.initializing && !activity.holds_application()).then_some(())
    });
    assert!(
        fs::read_to_string(media.join("events"))
            .unwrap()
            .contains("private provider initialized")
    );
    assert_eq!(device(&runtime).root.as_deref(), Some(payload.as_path()));
    assert_eq!(fs::read(payload.join("notes.txt")).unwrap(), original);

    // The real platform request remains active while its helper is stopped.
    // Neither frontend disposal nor Cancel may release its removal guard.
    fs::write(media.join("hold-unmount"), b"").unwrap();
    let stale = device(&runtime).token;
    let removing = frontend.remove(stale).unwrap();
    wait(|| {
        fs::read_to_string(media.join("events"))
            .ok()
            .filter(|s| s.contains("unmount requested without force"))
    });
    drop(frontend);
    let requests = runtime.device_requests();
    assert_eq!(requests.root(device(&runtime).token).unwrap(), payload);
    assert!(
        matches!(requests.open(device(&runtime).token).unwrap(), DeviceOpen::Root(root) if root == payload)
    );
    assert!(complete(runtime.shutdown()).is_err());
    let paused = Frozen::new(helper_pid());
    requests.cancel(removing).unwrap();
    assert!(activity.holds_application());
    assert_eq!(requests.snapshot()[0].phase, DevicePhase::Cancelling);
    let target = root.join("created after removal callback");
    let mut file_admission = runtime
        .jobs()
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let script = root.join("action.sh");
    fs::write(
        &script,
        b"printf 'after removal callback\\n' > command-result\n",
    )
    .unwrap();
    let command = reply(
        actions
            .submit(ActionRequest {
                action: CustomAction::new("fixture", "Private command", "/bin/sh %f"),
                paths: Arc::new(vec![script]),
                directory: root.to_owned(),
                resolved_directory: fs::canonicalize(root).unwrap(),
            })
            .unwrap(),
    )
    .unwrap();
    wait(|| {
        actions
            .snapshot()
            .queue
            .jobs
            .iter()
            .any(|job| job.id == command && job.phase == ActionPhase::WaitingForDevice)
            .then_some(())
    });
    thread::sleep(Duration::from_millis(100));
    assert!(file_admission.try_recv().unwrap().is_none());
    assert!(!target.exists() && !root.join("command-result").exists());
    assert!(activity.holds_application());
    drop(paused);
    assert!(matches!(
        finished(&requests, removing).outcome,
        Some(DeviceOutcome::Cancelled)
    ));
    let created = reply(file_admission).unwrap();
    wait(|| {
        runtime
            .jobs()
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == created && job.phase == Phase::Finished)
            .then_some(())
    });
    wait(|| {
        (!actions.snapshot().holds_application() && !activity.holds_application()).then_some(())
    });
    assert!(target.is_dir());
    let undo = wait(|| runtime.jobs().snapshot().undo.map(|undo| undo.id));
    assert_eq!(
        fs::read(root.join("command-result")).unwrap(),
        b"after removal callback\n"
    );
    assert!(device(&runtime).root.is_some());
    assert_eq!(fs::read(payload.join("notes.txt")).unwrap(), original);
    println!(
        "passed: actual removal guard survives frontend disposal, shutdown refusal and cancelled native teardown; queued file work and commands resume afterward"
    );
    fs::remove_file(media.join("hold-unmount")).unwrap();

    // A successful unmount leaves the private provider's old directory intact.
    // A queued action must not treat that pathname as an available device.
    fs::write(media.join("hold-unmount"), b"").unwrap();
    let before = fs::read_to_string(media.join("events"))
        .unwrap()
        .matches("unmount requested without force")
        .count();
    let removed = requests.remove(device(&runtime).token).unwrap();
    wait(|| {
        (fs::read_to_string(media.join("events"))
            .unwrap()
            .matches("unmount requested without force")
            .count()
            > before)
            .then_some(())
    });
    let script = root.join("device-action.sh");
    fs::write(
        &script,
        b"printf 'must not run\\n' > wrong-command-result\n",
    )
    .unwrap();
    let selected = payload.join("selected-action.sh");
    fs::write(
        &selected,
        b"printf 'must not run\\n' > wrong-selected-result\n",
    )
    .unwrap();
    let referent = payload.join("referent-action.sh");
    fs::write(
        &referent,
        b"printf 'must not run\\n' > wrong-referent-result\n",
    )
    .unwrap();
    let local_link = root.join("link-to-device.sh");
    symlink(&referent, &local_link).unwrap();
    let outside = root.join("outside-action.sh");
    fs::write(&outside, b"printf 'must not run\\n' > wrong-entry-result\n").unwrap();
    let device_link = payload.join("link-to-local.sh");
    symlink(&outside, &device_link).unwrap();
    let mut commands = Vec::new();
    for (path, directory, output) in [
        (
            script,
            payload.clone(),
            payload.join("wrong-command-result"),
        ),
        (
            selected,
            root.to_owned(),
            root.join("wrong-selected-result"),
        ),
        (
            local_link,
            root.to_owned(),
            root.join("wrong-referent-result"),
        ),
        (
            device_link,
            root.to_owned(),
            root.join("wrong-entry-result"),
        ),
    ] {
        let command = reply(
            actions
                .submit(ActionRequest {
                    action: CustomAction::new(
                        "device-fixture",
                        "Unavailable device command",
                        "/bin/sh %f",
                    ),
                    paths: Arc::new(vec![path]),
                    resolved_directory: fs::canonicalize(&directory).unwrap(),
                    directory,
                })
                .unwrap(),
        )
        .unwrap();
        wait(|| {
            actions
                .snapshot()
                .queue
                .jobs
                .iter()
                .any(|job| job.id == command && job.phase == ActionPhase::WaitingForDevice)
                .then_some(())
        });
        assert!(!output.exists());
        commands.push((command, output));
    }
    fs::write(media.join("release-unmount"), b"").unwrap();
    assert!(matches!(
        finished(&requests, removed).outcome,
        Some(DeviceOutcome::Removed)
    ));
    assert!(device(&runtime).root.is_none());
    for (command, output) in &commands {
        let result = wait(|| {
            actions
                .snapshot()
                .queue
                .jobs
                .into_iter()
                .find(|job| job.id == *command && job.phase == ActionPhase::Finished)
        })
        .outcome
        .unwrap();
        assert!(
            result.as_ref().as_ref().is_err_and(|error| {
                error.contains("device")
                    && (error.contains("unavailable")
                        || error.contains("changed after the request"))
            }) && !output.exists(),
            "queued action crossed unavailable device: result={result:?}, output_exists={}",
            output.exists()
        );
    }
    assert_eq!(fs::read(payload.join("notes.txt")).unwrap(), original);
    fs::remove_file(media.join("hold-unmount")).unwrap();
    fs::remove_file(media.join("release-unmount")).unwrap();
    println!(
        "passed: queued commands refuse unavailable working folders, selected entries and symlink targets even when the old files remain present"
    );
    let local_output = root.join("local-after-unmount");
    let local_script = root.join("local-after-unmount.sh");
    fs::write(
        &local_script,
        b"printf 'local remains usable\\n' > local-after-unmount\n",
    )
    .unwrap();
    let command = reply(
        actions
            .submit(ActionRequest {
                action: CustomAction::new("local", "Local command", "/bin/sh %f"),
                paths: Arc::new(vec![local_script]),
                directory: root.to_owned(),
                resolved_directory: fs::canonicalize(root).unwrap(),
            })
            .unwrap(),
    )
    .unwrap();
    wait(|| {
        actions
            .snapshot()
            .queue
            .jobs
            .iter()
            .any(|job| job.id == command && job.phase == ActionPhase::Finished)
            .then_some(())
    });
    assert_eq!(fs::read(local_output).unwrap(), b"local remains usable\n");
    println!("passed: an unavailable device does not block an unrelated local command");
    let events = fs::read(media.join("events")).unwrap();
    assert!(requests.remove(stale).is_err());
    assert!(requests.open(stale).is_err());
    assert_eq!(fs::read(media.join("events")).unwrap(), events);
    println!("passed: stale parent observations are refused without a platform call");

    fs::write(media.join("password-mount"), b"").unwrap();
    let mounting = match requests.open(device(&runtime).token).unwrap() {
        DeviceOpen::Mount(id) => id,
        DeviceOpen::Root(_) => panic!("unmounted device returned a mounted root"),
    };
    let prompt = wait(|| {
        requests
            .snapshot()
            .into_iter()
            .find(|job| job.id == mounting)
            .and_then(|job| job.question)
    });
    assert!(matches!(prompt.question, DeviceQuestion::Password { .. }));
    assert!(activity.holds_application());
    assert!(complete(runtime.shutdown()).is_err());
    requests
        .answer(
            prompt.token,
            DeviceAnswer::Password {
                username: "private-user".into(),
                domain: "private-domain".into(),
                password: "private-secret".into(),
                anonymous: false,
                pim: 0,
                hidden_volume: false,
                system_volume: false,
                save: local_volumes::Remember::Never,
            },
        )
        .unwrap();
    let result = finished(&requests, mounting);
    assert!(matches!(result.outcome, Some(DeviceOutcome::Mounted { root }) if root == payload));
    assert_eq!(requests.mounted_root(mounting).unwrap(), payload);
    let first_mount = mounting;
    assert!(
        requests
            .answer(prompt.token, DeviceAnswer::Choice { choice: 0 })
            .is_err()
    );
    assert!(
        fs::read_to_string(media.join("events"))
            .unwrap()
            .contains("credentials accepted without saving")
    );
    fs::remove_file(media.join("password-mount")).unwrap();
    // Reconnection never replays old commands. Only a fresh explicit invocation
    // captures the new mount observation and may use the device again.
    for (_, output) in &commands {
        assert!(!output.exists());
    }
    let command = reply(
        actions
            .submit(ActionRequest {
                action: CustomAction::new("remounted", "Fresh device command", "/bin/sh %f"),
                paths: Arc::new(vec![payload.join("selected-action.sh")]),
                directory: root.to_owned(),
                resolved_directory: fs::canonicalize(root).unwrap(),
            })
            .unwrap(),
    )
    .unwrap();
    wait(|| {
        actions
            .snapshot()
            .queue
            .jobs
            .iter()
            .any(|job| job.id == command && job.phase == ActionPhase::Finished)
            .then_some(())
    });
    assert_eq!(
        fs::read(root.join("wrong-selected-result")).unwrap(),
        b"must not run\n"
    );
    println!(
        "passed: remounting does not replay refused commands; a fresh invocation can use the current device"
    );
    println!(
        "passed: backend-owned credentials retain lifetime, reach the actual GIO operation and retire their answer capability"
    );

    wait(|| (!application.holds_application()).then_some(()));
    let removed = requests.remove(device(&runtime).token).unwrap();
    assert!(matches!(
        finished(&requests, removed).outcome,
        Some(DeviceOutcome::Removed)
    ));
    assert!(requests.mounted_root(first_mount).is_err());
    fs::write(media.join("question-mount"), b"").unwrap();
    let mounting = requests.mount(device(&runtime).token).unwrap();
    let prompt = wait(|| {
        requests.snapshot().into_iter().find(|job| job.id == mounting).and_then(|job| job.question).filter(|prompt| matches!(&prompt.question, DeviceQuestion::Choice { choices, .. } if choices[1] == "Continue"))
    });
    assert!(
        requests
            .answer(prompt.token, DeviceAnswer::Choice { choice: usize::MAX })
            .is_err()
    );
    requests
        .answer(prompt.token, DeviceAnswer::Choice { choice: 1 })
        .unwrap();
    assert!(matches!(
        finished(&requests, mounting).outcome,
        Some(DeviceOutcome::Mounted { .. })
    ));
    assert_eq!(requests.mounted_root(mounting).unwrap(), payload);
    assert!(requests.mounted_root(first_mount).is_err());
    fs::remove_file(media.join("question-mount")).unwrap();
    let mounted_token = device(&runtime).token;
    let mounted_id = device(&runtime).id();
    fs::write(media.join("unplugged"), b"").unwrap();
    // GIO can report host mounts alongside the fixture.
    wait(|| {
        let catalog = runtime.devices();
        (catalog.status == DeviceStatus::Ready
            && !catalog
                .devices
                .iter()
                .any(|device| device.name == "Fixture USB"))
        .then_some(())
    });
    assert!(payload.is_dir());
    assert!(requests.mounted_root(mounting).is_err());
    assert!(requests.root(mounted_token).is_err());
    fs::remove_file(media.join("unplugged")).unwrap();
    wait(|| {
        runtime
            .devices()
            .devices
            .iter()
            .any(|device| device.root.as_ref() == Some(&payload))
            .then_some(())
    });
    assert!(requests.mounted_root(mounting).is_err());
    assert_eq!(requests.root(device(&runtime).token).unwrap(), payload);
    assert_ne!(device(&runtime).id(), mounted_id);
    assert_eq!(fs::read(payload.join("notes.txt")).unwrap(), original);
    println!(
        "passed: current-root lookup refuses old completed mounts after remount or unplug/replug, even with the old directory present"
    );
    assert_eq!(
        runtime.jobs().snapshot().undo.map(|undo| undo.id),
        Some(undo)
    );
    println!(
        "passed: updated questions expose current bounded choices; device work leaves filesystem Undo unchanged"
    );

    wait(|| (!application.holds_application()).then_some(()));
    // Missing completion is uncertainty, not a fabricated removal result. The
    // shared guard cannot retire until the exact helper has actually exited.
    fs::write(media.join("hold-unmount"), b"").unwrap();
    let before = fs::read_to_string(media.join("events"))
        .unwrap()
        .matches("unmount requested without force")
        .count();
    let removing = requests.remove(device(&runtime).token).unwrap();
    wait(|| {
        (fs::read_to_string(media.join("events"))
            .unwrap()
            .matches("unmount requested without force")
            .count()
            > before)
            .then_some(())
    });
    let pid = helper_pid();
    let paused = Frozen::new(pid);
    let uncertain_target = payload.join("must not be created after helper loss");
    let admission = runtime
        .jobs()
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: uncertain_target.clone(),
            }),
            None,
        )
        .unwrap();
    assert!(activity.holds_application());
    kill_process(pid, Signal::KILL).unwrap();
    drop(paused);
    assert!(matches!(
        finished(&requests, removing).outcome,
        Some(DeviceOutcome::Uncertain { .. })
    ));
    assert!(!PathBuf::from(format!("/proc/{}", pid.as_raw_pid())).exists());
    let refused = reply(admission).unwrap();
    wait(|| {
        runtime
            .jobs()
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == refused && job.phase == Phase::Failed)
            .then_some(())
    });
    assert!(!uncertain_target.exists());
    assert_eq!(fs::read(payload.join("notes.txt")).unwrap(), original);
    assert!(requests.mount(stale).is_err());
    assert!(
        actions
            .submit(ActionRequest {
                action: CustomAction::new("lost", "Observer lost", "/bin/sh %f"),
                paths: Arc::new(vec![root.join("outside-action.sh")]),
                directory: root.to_owned(),
                resolved_directory: fs::canonicalize(root).unwrap(),
            })
            .is_err()
    );
    assert!(!root.join("wrong-entry-result").exists());
    wait(|| (!application.holds_application()).then_some(()));
    let attention = runtime.device_attention_snapshot();
    assert_eq!(attention.pending_notifications, 0);
    assert!(!attention.notification_errors.is_empty());
    complete(application.shutdown()).unwrap();
    println!(
        "passed: helper loss is uncertain after exact reaping, closes storage authority and refuses further device work"
    );
}

fn exercise_attention(root: &Path, fixture: &Path) {
    assert_eq!(
        fs::read(fixture.join("fixture-owned")).unwrap(),
        b"Canopa private device attention\n"
    );
    let application =
        Application::start(&root.join("state"), &canopa_config::Request::Defaults).unwrap();
    let runtime = application.operations.as_ref().unwrap().clone();
    let requests = runtime.device_requests();
    drop(runtime.take_notices());
    wait(|| (!runtime.jobs().snapshot().initializing).then_some(()));
    let payload = root.join("volumes/Fixture USB");
    let removed = requests.remove(device(&runtime).token).unwrap();
    assert!(matches!(
        finished(&requests, removed).outcome,
        Some(DeviceOutcome::Removed)
    ));
    wait(|| fixture.join("notifications.held").exists().then_some(()));
    assert!(application.holds_application());
    assert!(runtime.device_attention_snapshot().pending_notifications > 0);
    assert!(complete(runtime.shutdown()).is_err());
    fs::write(fixture.join("release-notifications"), b"").unwrap();
    wait(|| (!application.holds_application()).then_some(()));
    assert_eq!(runtime.device_attention_snapshot().pending_notifications, 0);

    fs::write(root.join("volumes/password-mount"), b"").unwrap();
    let mounting = requests.mount(device(&runtime).token).unwrap();
    let prompt = wait(|| {
        requests
            .snapshot()
            .into_iter()
            .find(|job| job.id == mounting)
            .and_then(|job| job.question)
    });
    wait(|| {
        fs::read(fixture.join("notifications.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<serde_json::Value>>(&bytes).ok())
            .filter(|notices| notices.len() == 2)
    });
    assert!(device(&runtime).root.is_none());
    assert!(application.holds_application());
    requests
        .answer(
            prompt.token,
            DeviceAnswer::Password {
                username: "private-user".into(),
                domain: "private-domain".into(),
                password: "private-secret".into(),
                anonymous: false,
                pim: 0,
                hidden_volume: false,
                system_volume: false,
                save: local_volumes::Remember::Never,
            },
        )
        .unwrap();
    assert!(matches!(
        finished(&requests, mounting).outcome,
        Some(DeviceOutcome::Mounted { .. })
    ));
    assert_eq!(requests.mounted_root(mounting).unwrap(), payload);
    wait(|| (!application.holds_application()).then_some(()));
    let attention = runtime.device_attention_snapshot();
    assert_eq!(attention.pending_notifications, 0);
    assert!(attention.notification_errors.is_empty() && attention.error.is_none());
    assert_eq!(
        fs::read(payload.join("notes.txt")).unwrap(),
        b"Actual parent-runtime device bytes.\n"
    );
    complete(application.shutdown()).unwrap();
    fs::write(fixture.join("result.json"), serde_json::to_vec(&serde_json::json!({"completed":2,"delivery_hold_refused_shutdown":true,"question_required_explicit_answer":true,"original_bytes_preserved":true})).unwrap()).unwrap();
    println!(
        "passed: headless device attention retains actual delivery and decision holds, sends notifications and preserves private bytes"
    );
}

fn main() {
    if std::env::args().any(|arg| arg == "--internal-volume-monitor") {
        local_volumes::run_helper().unwrap();
        return;
    }
    if std::env::args().any(|arg| arg == "--list") {
        println!("device_runtime: test");
        return;
    }
    if let Some(root) = std::env::var_os("CANOPA_DEVICE_RUNTIME_CHILD") {
        if let Some(fixture) = std::env::var_os("CANOPA_DEVICE_ATTENTION_FIXTURE") {
            exercise_attention(Path::new(&root), Path::new(&fixture));
        } else {
            exercise(Path::new(&root));
        }
        return;
    }
    let temporary = tempfile::Builder::new()
        .prefix("canopa-files-native-parent-")
        .tempdir()
        .unwrap();
    let root = temporary.path();
    let modules = root.join("gio-modules");
    fs::create_dir(&modules).unwrap();
    for name in [
        "home",
        "config",
        "state",
        "data",
        "cache",
        "runtime",
        "volumes/Fixture USB",
    ] {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    fs::set_permissions(root.join("runtime"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        root.join("volumes/Fixture USB/notes.txt"),
        b"Actual parent-runtime device bytes.\n",
    )
    .unwrap();
    let flags = Command::new("pkg-config")
        .args(["--cflags", "--libs", "gio-2.0"])
        .output()
        .unwrap();
    assert!(flags.status.success());
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/desktop-fixture/files-volume-monitor.c");
    assert!(
        Command::new("cc")
            .args(["-shared", "-fPIC", "-Wall", "-Wextra", "-Werror"])
            .arg(source)
            .arg("-o")
            .arg(modules.join("libcanopa-volume-fixture.so"))
            .args(
                std::str::from_utf8(&flags.stdout)
                    .unwrap()
                    .split_whitespace()
            )
            .status()
            .unwrap()
            .success()
    );
    let bus = if let Some(fixture) = std::env::var_os("CANOPA_DEVICE_ATTENTION_FIXTURE") {
        assert_eq!(
            fs::read(Path::new(&fixture).join("fixture-owned")).unwrap(),
            b"Canopa private device attention\n"
        );
        std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap()
    } else {
        format!("unix:path={}/no-session-bus", root.display())
    };
    let mut child = Command::new(std::env::current_exe().unwrap())
        .env("CANOPA_DEVICE_RUNTIME_CHILD", root)
        .env("CANOPA_FIXTURE_VOLUMES", root.join("volumes"))
        .env("GIO_EXTRA_MODULES", modules)
        .env("GIO_USE_VOLUME_MONITOR", "unix")
        .env("GIO_USE_VFS", "local")
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("XDG_RUNTIME_DIR", root.join("runtime"))
        .env("DBUS_SESSION_BUS_ADDRESS", bus)
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let group = Pid::from_raw(child.id() as i32).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            break None;
        }
        thread::sleep(Duration::from_millis(10));
    };
    if status.is_none_or(|s| !s.success()) {
        let _ = kill_process_group(group, Signal::KILL);
    }
    let output = child.wait_with_output().unwrap();
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        status.is_some_and(|s| s.success()),
        "private device runtime child failed or timed out"
    );
}
