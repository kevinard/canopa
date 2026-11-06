use super::super::observer::Window;
use super::*;
use std::{os::unix::fs::symlink, time::Instant};

const PID: i32 = 173;
fn stat(pid: i32, start: u64, state: &str) -> String {
    let mut fields = vec![state.to_owned()];
    fields.extend((1..19).map(|_| "0".to_owned()));
    fields.push(start.to_string());
    format!("{pid} (worker with ) spaces) {}\n", fields.join(" "))
}
fn metadata(id: &str) -> String {
    format!("[Application]\nname={id}\n[Instance]\ninstance-id=fixture-173\n")
}
struct Fixture {
    root: tempfile::TempDir,
    process: PathBuf,
    sandbox: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let process = root.path().join(PID.to_string());
        let sandbox = root.path().join("sandbox");
        std::fs::create_dir_all(&process).unwrap();
        std::fs::create_dir(&sandbox).unwrap();
        std::fs::write(process.join("stat"), stat(PID, 100, "S")).unwrap();
        std::fs::write(
            sandbox.join(".flatpak-info"),
            metadata("org.canopa.Fixture"),
        )
        .unwrap();
        symlink(&sandbox, process.join("root")).unwrap();
        Self {
            root,
            process,
            sandbox,
        }
    }
    fn cache(&self) -> Processes {
        Processes {
            proc_root: self.root.path().to_owned(),
            cache: BTreeMap::new(),
        }
    }
}
fn job(windows: &[(u64, Option<i32>, bool)]) -> Job {
    Job {
        revision: 1,
        queued: Instant::now(),
        windows: windows
            .iter()
            .map(|&(birth, pid, focused)| Window {
                key: WindowKey(1, birth),
                pid,
                focused,
            })
            .collect(),
    }
}

#[test]
fn bounded_metadata_extracts_identity_without_a_wayland_label() {
    assert_eq!(
        identity(metadata("org.canopa.Fixture").as_bytes()).unwrap(),
        "org.canopa.Fixture"
    );
    assert_eq!(
        identity(b"# comment\n[Runtime]\nname=org.canopa.Runtime\n[Instance]\ninstance-id=123\n")
            .unwrap(),
        "org.canopa.Runtime"
    );
    for source in [
        "[Application]\nname=org.canopa.Fixture\n",
        "[Application]\nname=org.canopa.Bad App\n[Instance]\ninstance-id=1\n",
        "[Application]\nname=org.canopa.Fixture \n[Instance]\ninstance-id=1\n",
        "[Application]\nname=org.canopa.First\nname=org.canopa.Second\n[Instance]\ninstance-id=1\n",
        "[Application]\nname=org.canopa.First\n[Runtime]\nname=org.canopa.Second\n[Instance]\ninstance-id=1\n",
        "[Application]\nname=org.canopa.Fixture\nmalformed\n[Instance]\ninstance-id=1\n",
    ] {
        assert!(identity(source.as_bytes()).is_err(), "{source:?}");
    }
    assert!(identity(b"\xff").is_err());
    assert!(identity(b"[Application]\nname=org.canopa.A\0B\n[Instance]\ninstance-id=1").is_err());
    for id in ["org.canopa.Valid-app", "snap.test", "3org.7app"] {
        assert!(app_id(id));
    }
    for id in [
        "single",
        "org..app",
        ".org.App",
        "org.App.",
        "bad-prefix.org.App",
        "org.canopa.A/B",
        "org.canopa.å",
        &"x".repeat(256),
    ] {
        assert!(!app_id(id), "{id}");
    }
}

#[test]
fn process_stat_ignores_raw_comm_bytes_and_parentheses() {
    let fixture = Fixture::new();
    let source = stat(PID, 100, "S");
    let mut bytes = source.into_bytes();
    bytes[6] = 0xff;
    std::fs::write(fixture.process.join("stat"), bytes).unwrap();
    let process = Process::open(fixture.root.path(), PID, &mut { BATCH_BYTES }).unwrap();
    assert_eq!(process.start, 100);
    assert!(process.verify().is_ok());
}

#[test]
fn process_roots_drive_aggregated_state_and_unknown_windows_fail_the_whole_projection() {
    let fixture = Fixture::new();
    let mut cache = fixture.cache();
    let apps = cache
        .project(&job(&[(1, Some(PID), false), (2, Some(PID), true)]))
        .unwrap();
    assert_eq!(
        apps,
        BTreeMap::from([("org.canopa.Fixture".into(), Activity::Active)])
    );
    assert_eq!(
        cache.project(&job(&[(1, Some(PID), false)])).unwrap()["org.canopa.Fixture"],
        Activity::Running
    );
    assert_eq!(cache.cache.len(), 1);
    assert_eq!(
        cache.project(&job(&[(1, Some(PID), false), (3, None, false)])),
        Err(Unavailable::Identity)
    );
    assert!(cache.project(&job(&[])).unwrap().is_empty());
    assert!(cache.cache.is_empty());
}

#[test]
fn exit_pid_reuse_and_changed_roots_cannot_relabel_an_existing_window() {
    let fixture = Fixture::new();
    let mut cache = fixture.cache();
    let original = job(&[(1, Some(PID), true)]);
    cache.project(&original).unwrap();
    std::fs::write(fixture.process.join("stat"), stat(PID, 101, "S")).unwrap();
    std::fs::write(
        fixture.sandbox.join(".flatpak-info"),
        metadata("org.canopa.NewIdentity"),
    )
    .unwrap();
    assert_eq!(cache.project(&original), Err(Unavailable::Identity));
    assert_eq!(
        cache.project(&job(&[(2, Some(PID), true)])).unwrap()["org.canopa.NewIdentity"],
        Activity::Active
    );
    std::fs::write(fixture.process.join("stat"), stat(PID, 101, "Z")).unwrap();
    assert_eq!(
        cache.project(&job(&[(2, Some(PID), true)])),
        Err(Unavailable::Identity)
    );
}

#[test]
fn namespace_and_metadata_replacement_invalidate_the_original_process_binding() {
    let fixture = Fixture::new();
    let process = Process::open(fixture.root.path(), PID, &mut { BATCH_BYTES }).unwrap();
    let alternate = fixture.root.path().join("alternate");
    std::fs::create_dir(&alternate).unwrap();
    std::fs::remove_file(fixture.process.join("root")).unwrap();
    symlink(&alternate, fixture.process.join("root")).unwrap();
    assert!(process.verify().is_err());
    std::fs::remove_file(fixture.process.join("root")).unwrap();
    symlink(&fixture.sandbox, fixture.process.join("root")).unwrap();
    assert!(process.verify().is_ok());
    std::fs::remove_file(fixture.sandbox.join(".flatpak-info")).unwrap();
    std::fs::write(
        fixture.sandbox.join(".flatpak-info"),
        metadata("org.canopa.Replacement"),
    )
    .unwrap();
    assert!(process.verify().is_err());
}

#[test]
fn only_a_confirmed_host_root_can_omit_flatpak_metadata() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.sandbox.join(".flatpak-info")).unwrap();
    assert!(Process::open(fixture.root.path(), PID, &mut { BATCH_BYTES }).is_err());
    std::fs::remove_file(fixture.process.join("root")).unwrap();
    symlink("/", fixture.process.join("root")).unwrap();
    let host = Process::open(fixture.root.path(), PID, &mut { BATCH_BYTES }).unwrap();
    assert!(host.app_id.is_none());
    assert!(host.verify().is_ok());
}

#[test]
fn symlinks_special_files_and_oversized_metadata_are_not_read_as_identity() {
    let fixture = Fixture::new();
    let info = fixture.sandbox.join(".flatpak-info");
    File::create(&info)
        .unwrap()
        .set_len(INFO_BYTES as u64 + 1)
        .unwrap();
    assert!(Process::open(fixture.root.path(), PID, &mut { BATCH_BYTES }).is_err());
    std::fs::remove_file(&info).unwrap();
    let outside = fixture.root.path().join("outside");
    std::fs::write(&outside, metadata("org.canopa.Outside")).unwrap();
    symlink(&outside, &info).unwrap();
    assert!(Process::open(fixture.root.path(), PID, &mut { BATCH_BYTES }).is_err());
    std::fs::remove_file(&info).unwrap();
    let socket = std::os::unix::net::UnixListener::bind(&info).unwrap();
    assert!(Process::open(fixture.root.path(), PID, &mut { BATCH_BYTES }).is_err());
    drop(socket);
    std::fs::remove_file(&info).unwrap();
    rustix::fs::mkfifoat(rustix::fs::CWD, &info, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(Process::open(fixture.root.path(), PID, &mut { BATCH_BYTES }).is_err());
    std::fs::remove_file(&info).unwrap();
    std::fs::write(&info, metadata("org.canopa.Fixture")).unwrap();
    assert!(Process::open(fixture.root.path(), PID, &mut 1usize).is_err());
}

#[test]
fn pinned_proc_directory_does_not_follow_a_recycled_numeric_path() {
    let fixture = Fixture::new();
    let mut cache = fixture.cache();
    let original = job(&[(1, Some(PID), true)]);
    cache.project(&original).unwrap();
    let retired = fixture.root.path().join("retired");
    std::fs::rename(&fixture.process, &retired).unwrap();
    std::fs::create_dir(&fixture.process).unwrap();
    std::fs::write(fixture.process.join("stat"), stat(PID, 200, "S")).unwrap();
    symlink(&fixture.sandbox, fixture.process.join("root")).unwrap();
    std::fs::write(retired.join("stat"), stat(PID, 100, "Z")).unwrap();
    assert_eq!(cache.project(&original), Err(Unavailable::Identity));
    assert!(cache.project(&job(&[(2, Some(PID), true)])).is_ok());
}
