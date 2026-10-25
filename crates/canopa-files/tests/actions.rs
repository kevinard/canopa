use std::{ffi::OsString, fs, os::unix::ffi::OsStringExt};

use canopa_files::{Cancellation, actions::CustomAction};

#[test]
fn graphical_action_edits_use_the_shared_file_and_preserve_other_owners() {
    use canopa_files::configuration::ConfigurationSnapshot;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    let original = "// Keep my desktop settings\nschema-version 1\nappearance { color-scheme \"light\"; }\nshell { bar \"work\"; }\nnotifications { rule \"quiet\"; }\n";
    fs::write(&path, original).unwrap();
    let snapshot = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    assert!(snapshot.actions.is_empty());
    let mut action = CustomAction::new("image-copy", "Copy images", "/bin/cp %F %d");
    action.conditions.patterns = vec!["*.png".into(), "*.jpg".into()];
    action.shortcut = Some("<Control><Alt>i".into());
    action.toolbar = true;
    let saved = snapshot.save_actions(vec![action]).unwrap();
    assert_eq!(saved.actions[0].name, "Copy images");
    let reopened = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    assert!(reopened.actions[0].toolbar);
    assert_eq!(reopened.actions[0].conditions.patterns, ["*.png", "*.jpg"]);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(original));
    canopa_config::Document::load(
        &canopa_config::Request::common(&path),
        canopa_config::Component::Shell,
        "",
    )
    .unwrap();
    // Saving an editor opened against older bytes must not erase another edit.
    fs::write(&path, format!("{text}// external edit\n")).unwrap();
    assert_eq!(
        saved.save_actions(Vec::new()).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .ends_with("// external edit\n")
    );
}

#[test]
fn invalid_action_catalogs_are_rejected_without_exposing_commands_or_changing_config() {
    use canopa_files::configuration::ConfigurationSnapshot;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    for contents in [
        "action \"one\" name=\"One\" command=\"private-command %bad\"",
        "action \"one\" name=\"One\" command=\"private-command\"; action \"one\" name=\"Two\" command=\"private-command\"",
        "action \"one\" name=\"One\" command=\"private-command\" { selection min=2 max=1; }",
        "action \"one\" name=\"One\" command=\"private-command\" { secret #true; }",
    ] {
        let text = format!("schema-version 1\nfiles {{ {contents}; }}\n");
        fs::write(&path, &text).unwrap();
        let error =
            ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("private-command"));
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
    fs::write(&path, "schema-version 1\n").unwrap();
    let catalog = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let action = CustomAction::new("bad", "Hidden\0name", "/bin/true");
    assert!(catalog.save_actions(vec![action]).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "schema-version 1\n");
}

#[test]
fn custom_actions_pass_selected_paths_verbatim_without_shell_interpretation() {
    let root = tempfile::tempdir().unwrap();
    let first = root
        .path()
        .join("one $(touch CANOPA_ACTION_INJECTION) ' space.txt");
    let second = root
        .path()
        .join(OsString::from_vec(b"two-\xff.txt".to_vec()));
    fs::write(&first, b"first\n").unwrap();
    fs::write(&second, b"second\n").unwrap();
    let mut action = CustomAction::new("inspect-text", "Inspect text", "/bin/cat %F");
    action.conditions.patterns = vec!["*.txt".into()];
    let prepared = action.prepare(&[first, second], root.path()).unwrap();
    let outcome = prepared.run(&Cancellation::default()).unwrap();
    assert!(outcome.success);
    assert_eq!(outcome.stdout, b"first\nsecond\n");
    assert!(outcome.stderr.is_empty());
    assert!(!root.path().join("CANOPA_ACTION_INJECTION").exists());
}

#[test]
fn cancelling_an_action_stops_descendants_even_if_its_parent_exits_first() {
    use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
    use std::{
        os::fd::OwnedFd,
        thread,
        time::{Duration, Instant},
    };
    struct Cleanup(OwnedFd);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = pidfd_send_signal(&self.0, Signal::KILL);
        }
    }
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("parent.sh");
    fs::write(&script, b"trap 'exit 0' TERM\n(trap '' TERM; : > child.ready; while :; do /bin/sleep 1; done) &\nprintf '%s' \"$!\" > child.pid\nwait\n").unwrap();
    let action = CustomAction::new("cancel", "Cancel fixture", "/bin/sh %f");
    let prepared = action.prepare(&[script], root.path()).unwrap();
    let cancellation = Cancellation::default();
    let worker_cancel = cancellation.clone();
    let worker = thread::spawn(move || prepared.run(&worker_cancel));
    let deadline = Instant::now() + Duration::from_secs(3);
    let pid = loop {
        if let Some(pid) = fs::read_to_string(root.path().join("child.pid"))
            .ok()
            .and_then(|text| text.parse::<i32>().ok())
            .filter(|_| root.path().join("child.ready").exists())
        {
            break Pid::from_raw(pid).unwrap();
        }
        if Instant::now() >= deadline {
            cancellation.cancel();
            panic!("fixture did not start");
        }
        thread::sleep(Duration::from_millis(5));
    };
    let _cleanup = Cleanup(pidfd_open(pid, PidfdFlags::empty()).unwrap());
    cancellation.cancel();
    assert!(worker.join().unwrap().unwrap().cancelled);
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let running = fs::read_to_string(format!("/proc/{}/stat", pid.as_raw_pid()))
            .ok()
            .is_some_and(|stat| {
                !matches!(
                    stat.rsplit_once(')').unwrap().1.split_whitespace().next(),
                    Some("Z" | "X")
                )
            });
        if !running {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "cancelled action left a running descendant"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn verbose_action_output_is_bounded_without_blocking_the_command() {
    let root = tempfile::tempdir().unwrap();
    let mut action = CustomAction::new(
        "verbose",
        "Verbose fixture",
        "/usr/bin/head -c 131072 /dev/zero",
    );
    action.conditions.minimum_selection = 0;
    let outcome = action
        .prepare(&[], root.path())
        .unwrap()
        .run(&Cancellation::default())
        .unwrap();
    assert!(outcome.success);
    assert_eq!(outcome.stdout.len(), 65536);
    assert!(outcome.stdout_truncated);
    assert!(!outcome.stderr_truncated);
}
