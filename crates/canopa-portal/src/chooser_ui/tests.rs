use super::*;

#[test]
fn embedded_save_keeps_the_entry_reviewed_before_overwrite_confirmation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("file.pdf");
    std::fs::write(&path, b"reviewed version").unwrap();
    let request = ChooserRequest::parse(
        Mode::Save,
        String::new(),
        String::new(),
        "Save".into(),
        Default::default(),
    )
    .unwrap();
    let prepared = prepare_with_target(
        &request,
        directory.path(),
        Vec::new(),
        "file.pdf".into(),
        true,
    )
    .unwrap();
    assert_eq!(prepared.existing, vec![path.clone()]);
    let (mut picker, _) = Picker::new(
        PickerStart::fixture(request, directory.path().into()),
        PickerOptions {
            read_only_choice: false,
            retain_file_target: true,
        },
    );
    let _ = picker.update(Message::Checked(Ok(prepared)));
    assert!(picker.overwrite.is_some());
    std::fs::rename(&path, directory.path().join("original.pdf")).unwrap();
    std::fs::write(&path, b"unreviewed newer work").unwrap();
    let _ = picker.update(Message::Replace);
    assert!(matches!(picker.take_outcome(), Some(Outcome::Selected(_))));
    let reviewed = picker.take_file_target().unwrap();
    assert!(reviewed.verify().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"unreviewed newer work");
    assert_eq!(
        std::fs::read(directory.path().join("original.pdf")).unwrap(),
        b"reviewed version"
    );
}

fn request(mode: Mode) -> ChooserRequest {
    ChooserRequest::parse(
        mode,
        String::new(),
        String::new(),
        "Test".into(),
        if mode == Mode::SaveMany {
            use zbus::zvariant::{OwnedValue, Value};
            std::collections::HashMap::from([(
                "files".into(),
                OwnedValue::try_from(Value::from(vec![b"a.txt\0".to_vec(), b"b.txt\0".to_vec()]))
                    .unwrap(),
            )])
        } else {
            Default::default()
        },
    )
    .unwrap()
}

#[test]
fn new_folder_creates_one_entry_and_preserves_unix_parent_bytes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(OsString::from_vec(b"raw-\xff".to_vec()));
    std::fs::create_dir(&path).unwrap();
    let parent = folders::Parent::open(path.clone()).unwrap();
    for name in [
        "",
        ".",
        "..",
        "../escape",
        "/absolute",
        "nested/name",
        "trailing/",
        "nul\0name",
    ] {
        assert!(parent.create(name).is_err(), "{name:?}");
    }
    assert!(parent.create(&"x".repeat(256)).is_err());
    let created = parent.create("Données").unwrap();
    assert_eq!(created, path.join("Données"));
    assert!(created.is_dir());
    assert!(std::fs::read_dir(&created).unwrap().next().is_none());
    std::fs::write(path.join("existing"), b"keep").unwrap();
    std::os::unix::fs::symlink("existing", path.join("link")).unwrap();
    std::os::unix::fs::symlink("absent", path.join("dangling")).unwrap();
    for name in ["Données", "existing", "link", "dangling"] {
        assert_eq!(
            parent.create(name).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
    }
    assert_eq!(std::fs::read(path.join("existing")).unwrap(), b"keep");
    assert!(!root.path().join("escape").exists());
}

#[test]
fn new_folder_refuses_a_replaced_parent_without_mutating_either_directory() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original");
    let moved = root.path().join("moved");
    std::fs::create_dir(&original).unwrap();
    let parent = folders::Parent::open(original.clone()).unwrap();
    std::fs::rename(&original, &moved).unwrap();
    std::fs::create_dir(&original).unwrap();
    assert!(parent.create("new folder").is_err());
    assert!(!original.join("new folder").exists());
    assert!(!moved.join("new folder").exists());
}

#[test]
fn new_folder_review_cancels_without_selecting_or_creating_a_destination() {
    let root = tempfile::tempdir().unwrap();
    let (mut picker, _) = Picker::new(
        PickerStart::fixture(request(Mode::Save), root.path().into()),
        PickerOptions::default(),
    );
    let _ = picker.update(Message::Loaded(
        picker.revision,
        false,
        Ok(Directory::read(root.path()).unwrap()),
    ));
    let _ = picker.update(Message::Name("report.txt".into()));
    let parent = Arc::new(folders::Parent::open(root.path().into()).unwrap());
    let _ = picker.update(Message::FolderParent(picker.revision, Ok(parent)));
    let _ = picker.update(Message::FolderName("../escape".into()));
    let _ = picker.update(Message::CreateFolder);
    assert!(picker.error.is_some());
    assert!(!picker.busy);
    let _ = picker.update(Message::FolderName("proposed".into()));
    let _ = picker.update(Message::Accept);
    assert!(picker.take_outcome().is_none());
    let _ = picker.update(Message::KeepChoosing);
    assert!(picker.new_folder.is_none());
    assert_eq!(picker.name, "report.txt");
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

#[test]
fn new_folder_completion_enters_the_child_and_keeps_the_save_name() {
    let root = tempfile::tempdir().unwrap();
    let (mut picker, _) = Picker::new(
        PickerStart::fixture(request(Mode::Save), root.path().into()),
        PickerOptions::default(),
    );
    let _ = picker.update(Message::Loaded(
        picker.revision,
        false,
        Ok(Directory::read(root.path()).unwrap()),
    ));
    let _ = picker.update(Message::Name("report.txt".into()));
    picker.busy = true;
    let _ = picker.update(Message::FolderCreated(
        picker.revision - 1,
        Ok(root.path().into()),
    ));
    assert!(
        picker.busy,
        "An old result must not unlock a newer operation"
    );
    let child = folders::Parent::open(root.path().into())
        .unwrap()
        .create("created")
        .unwrap();
    let _ = picker.update(Message::FolderCreated(picker.revision, Ok(child.clone())));
    assert!(picker.loading);
    assert!(!picker.busy && picker.new_folder.is_none());
    let _ = picker.update(Message::Loaded(
        picker.revision,
        false,
        Ok(Directory::read(&child).unwrap()),
    ));
    assert_eq!(picker.browser.as_ref().unwrap().path(), child);
    assert_eq!(picker.name, "report.txt");
    assert!(picker.take_outcome().is_none());
    assert!(!child.join("report.txt").exists());
}

#[test]
fn acceptance_is_read_only_and_requires_overwrite_confirmation() {
    let root = tempfile::tempdir().unwrap();
    let existing = root.path().join("a.txt");
    std::fs::write(&existing, "keep me").unwrap();
    let save = prepare(&request(Mode::Save), root.path(), vec![], "a.txt".into()).unwrap();
    assert_eq!(save.existing, std::slice::from_ref(&existing));
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep me");
    let many = prepare(
        &request(Mode::SaveMany),
        root.path(),
        vec![],
        OsString::new(),
    )
    .unwrap();
    assert_eq!(many.paths, [existing.clone(), root.path().join("b.txt")]);
    assert_eq!(many.existing, [existing]);
    assert!(!root.path().join("b.txt").exists());
    for name in [
        "",
        ".",
        "..",
        "../escape",
        "/absolute",
        "folder/file",
        "trailing/",
        "nul\0name",
    ] {
        assert!(
            prepare(&request(Mode::Save), root.path(), vec![], name.into()).is_err(),
            "{name:?}"
        );
    }
    std::fs::create_dir(root.path().join("folder")).unwrap();
    assert!(prepare(&request(Mode::Save), root.path(), vec![], "folder".into()).is_err());
    std::os::unix::fs::symlink("a.txt", root.path().join("link")).unwrap();
    assert!(prepare(&request(Mode::Save), root.path(), vec![], "link".into()).is_err());
}

#[test]
fn open_revalidates_types_and_preserves_non_unicode_paths() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(OsString::from_vec(b"raw-\xff".to_vec()));
    std::fs::write(&file, "bytes").unwrap();
    let req = request(Mode::Open);
    assert_eq!(
        prepare(&req, root.path(), vec![file.clone()], OsString::new())
            .unwrap()
            .paths,
        std::slice::from_ref(&file)
    );
    assert!(prepare(&req, root.path(), vec![], OsString::new()).is_err());
    assert!(
        prepare(
            &req,
            root.path(),
            vec![root.path().to_owned()],
            OsString::new()
        )
        .is_err()
    );
    let mut req = req;
    req.directory = true;
    assert_eq!(
        prepare(&req, root.path(), vec![], OsString::new())
            .unwrap()
            .paths,
        [root.path().to_owned()]
    );
    assert!(prepare(&req, root.path(), vec![file.clone()], OsString::new()).is_err());
    std::fs::remove_file(&file).unwrap();
    req.directory = false;
    assert!(prepare(&req, root.path(), vec![file], OsString::new()).is_err());
}

#[test]
fn embedded_completion_retires_the_scan_and_ignores_late_messages() {
    let root = tempfile::tempdir().unwrap();
    for cancelled in [false, true] {
        let (mut picker, _) = Picker::new(
            PickerStart::fixture(request(Mode::Open), root.path().to_owned()),
            PickerOptions {
                read_only_choice: false,
                retain_file_target: false,
            },
        );
        let scan = picker.scan.clone();
        if cancelled {
            let _ = picker.update(Message::Cancel);
            assert!(matches!(picker.take_outcome(), Some(Outcome::Cancelled)));
        } else {
            let path = root.path().join("chosen.png");
            let _ = picker.finish(Prepared {
                paths: vec![path.clone()],
                existing: Vec::new(),
                file_target: None,
            });
            let Some(Outcome::Selected(selection)) = picker.take_outcome() else {
                panic!("expected embedded selection");
            };
            assert_eq!(selection.paths, [path.into_os_string().into_vec()]);
        }
        assert!(scan.is_cancelled());
        let revision = picker.revision;
        let _ = picker.update(Message::Navigate(root.path().join("late")));
        let _ = picker.update(Message::Cancel);
        assert_eq!(picker.revision, revision);
        assert!(picker.take_outcome().is_none());
    }
}
