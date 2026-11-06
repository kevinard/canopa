use super::*;

fn store(root: &Path) -> Store {
    Store {
        root: root.join("wallpaper"),
    }
}

fn prepared(root: &Path, color: [u8; 4]) -> Prepared {
    let source = root.join("source.png");
    image::RgbaImage::from_pixel(37, 19, image::Rgba(color))
        .save(&source)
        .unwrap();
    Prepared::read(&mut File::open(source).unwrap()).unwrap()
}

#[test]
fn absent_state_is_read_without_creating_anything() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    assert!(store.snapshot().unwrap().background.is_none());
    assert!(store.lock_image().unwrap().is_none());
    assert!(!store.root.exists());
}

#[test]
fn lock_handoff_never_waits_behind_an_image_installation() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let _held = store.acquire(true, true).unwrap();
    assert!(store.lock_image().is_err());
}

#[test]
fn next_install_reaps_interrupted_writes_without_touching_unmanaged_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    fs::create_dir_all(&store.root).unwrap();
    fs::write(store.root.join(".pending-interrupted"), b"unfinished image").unwrap();
    fs::write(store.root.join("notes.txt"), b"unmanaged").unwrap();
    store
        .install(
            prepared(dir.path(), [1, 2, 3, 255]),
            Targets::new(3).unwrap(),
        )
        .unwrap();
    assert!(!store.root.join(".pending-interrupted").exists());
    assert_eq!(
        fs::read(store.root.join("notes.txt")).unwrap(),
        b"unmanaged"
    );
}

#[test]
fn independent_targets_survive_restart_and_cleanup_preserves_open_locker_image() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install(
            prepared(dir.path(), [200, 10, 30, 255]),
            Targets::new(3).unwrap(),
        )
        .unwrap();
    let initial = store.selection().unwrap();
    assert_eq!(initial.background, initial.lockscreen);
    let mut pinned = store.lock_image().unwrap().unwrap();
    let old_bytes = read_regular(&mut pinned, MAX_PNG_BYTES).unwrap();
    fs::remove_file(dir.path().join("source.png")).unwrap();
    store
        .install(
            prepared(dir.path(), [2, 3, 90, 255]),
            Targets::new(1).unwrap(),
        )
        .unwrap();
    let desktop_only = store.selection().unwrap();
    assert_eq!(desktop_only.lockscreen, initial.lockscreen);
    assert_ne!(desktop_only.background, initial.background);
    store
        .install(
            prepared(dir.path(), [5, 100, 9, 255]),
            Targets::new(2).unwrap(),
        )
        .unwrap();
    let final_selection = store.selection().unwrap();
    assert_eq!(final_selection.background, desktop_only.background);
    assert_ne!(final_selection.lockscreen, desktop_only.lockscreen);
    assert!(
        !store
            .image_path(initial.lockscreen.as_ref().unwrap())
            .exists()
    );
    assert_eq!(read_regular(&mut pinned, MAX_PNG_BYTES).unwrap(), old_bytes);
    let restarted = self::store(dir.path());
    assert!(restarted.snapshot().unwrap().background.is_some());
    assert!(restarted.lock_image().unwrap().is_some());
    assert_eq!(
        fs::metadata(&store.root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(store.root.join("selection.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(store.image_path(final_selection.background.as_ref().unwrap()))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o400
    );
    let retained = fs::read_dir(&store.root)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
        .count();
    assert_eq!(retained, 2);
}

#[test]
fn corrupt_state_and_nonregular_or_changed_images_never_publish_a_new_selection() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install(
            prepared(dir.path(), [8, 9, 10, 255]),
            Targets::new(3).unwrap(),
        )
        .unwrap();
    let selected = store.root.join("selection.json");
    let original = fs::read(&selected).unwrap();
    for bad in [
        br#"{"version":99,"background":null,"lockscreen":null}"#.as_slice(),
        br#"{"version":1,"background":"../outside","lockscreen":null}"#,
        br#"{"version":1,"background":null,"lockscreen":null,"extra":true}"#,
    ] {
        fs::write(&selected, bad).unwrap();
        assert!(store.snapshot().is_err());
        assert!(
            store
                .install(
                    prepared(dir.path(), [77, 8, 9, 255]),
                    Targets::new(3).unwrap()
                )
                .is_err()
        );
        assert_eq!(fs::read(&selected).unwrap(), bad);
    }
    fs::write(&selected, original).unwrap();
    let hash = store.selection().unwrap().background.unwrap();
    let path = store.image_path(&hash);
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(dir.path().join("source.png"), &path).unwrap();
    assert!(store.snapshot().is_err());
    assert!(store.lock_image().is_err());
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"different image").unwrap();
    assert!(store.snapshot().is_err());
}

#[test]
fn admission_rejects_non_png_nonregular_oversize_and_invalid_targets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid");
    fs::write(&path, b"not an image").unwrap();
    assert!(Prepared::read(&mut File::open(&path).unwrap()).is_err());
    assert!(Prepared::read(&mut File::open(dir.path()).unwrap()).is_err());
    let mut large = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    large.set_len(MAX_PNG_BYTES + 1).unwrap();
    assert!(Prepared::read(&mut large).is_err());
    for mask in [0, 4, u32::MAX] {
        assert!(Targets::new(mask).is_err());
    }
}

#[test]
fn serialized_writers_merge_destinations_instead_of_losing_the_other_selection() {
    let dir = tempfile::tempdir().unwrap();
    let a = prepared(dir.path(), [23, 10, 8, 255]);
    let b = prepared(dir.path(), [5, 6, 7, 255]);
    let root = dir.path().to_path_buf();
    let other = root.clone();
    let left =
        std::thread::spawn(move || store(&root).install(a, Targets::new(1).unwrap()).unwrap());
    let right =
        std::thread::spawn(move || store(&other).install(b, Targets::new(2).unwrap()).unwrap());
    left.join().unwrap();
    right.join().unwrap();
    let selected = store(dir.path()).selection().unwrap();
    assert!(selected.background.is_some());
    assert!(selected.lockscreen.is_some());
    assert_ne!(selected.background, selected.lockscreen);
}
