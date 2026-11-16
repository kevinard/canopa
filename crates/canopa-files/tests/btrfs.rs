//! btrfs fast paths through the public interfaces (ADR 0152). These cases need
//! a writable btrfs folder in `CANOPA_FILES_TEST_BTRFS`; without it they return
//! early, and the generic behavior is covered by the other cases.
use canopa_files::{
    Cancellation, btrfs,
    operations::{OperationEngine, OperationRequest},
    transfers::{TransferKind, TransferReply, TransferRequest},
    trash::{
        TrashLocation, TrashOutcome,
        catalog::{Catalog, Scope},
    },
};
use std::{fs, os::unix::fs::MetadataExt, path::Path};

fn btrfs_root() -> Option<tempfile::TempDir> {
    let Some(path) = std::env::var_os("CANOPA_FILES_TEST_BTRFS") else {
        eprintln!("skipped: set CANOPA_FILES_TEST_BTRFS to a writable btrfs folder");
        return None;
    };
    let root = tempfile::Builder::new()
        .prefix("canopa-btrfs-")
        .tempdir_in(path)
        .unwrap();
    assert!(btrfs::is_btrfs(fs::File::open(root.path()).unwrap()));
    Some(root)
}

fn contents(size: usize, seed: usize) -> Vec<u8> {
    (0..size).map(|i| ((i + seed) % 251) as u8).collect()
}

fn shared(first: &Path, second: &Path) -> bool {
    btrfs::shares_all_extents(
        &fs::File::open(first).unwrap(),
        &fs::File::open(second).unwrap(),
    )
    .unwrap()
}

fn transfer(engine: &OperationEngine, kind: TransferKind, source: &Path, destination: &Path) {
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind,
                sources: vec![source.to_owned()],
                destination: destination.to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(outcome.failures.is_empty(), "{outcome:?}");
}

#[test]
fn copies_are_clones_and_undo_removes_only_an_unchanged_clone() {
    let Some(root) = btrfs_root() else { return };
    let source = root.path().join("source.bin");
    let data = contents(8 * 1024 * 1024, 1);
    fs::write(&source, &data).unwrap();
    let destination = root.path().join("copies");
    fs::create_dir(&destination).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    transfer(&engine, TransferKind::Copy, &source, &destination);
    let copy = destination.join("source.bin");
    assert_eq!(fs::read(&copy).unwrap(), data);
    assert!(shared(&source, &copy), "the copy is not a clone");
    // The copy keeps its own permission bits and times, like any copy.
    assert_eq!(
        fs::metadata(&copy).unwrap().mtime(),
        fs::metadata(&source).unwrap().mtime()
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!copy.exists());
    assert_eq!(fs::read(&source).unwrap(), data);
}

#[test]
fn an_edited_clone_is_still_a_changed_copy_for_undo() {
    let Some(root) = btrfs_root() else { return };
    let source = root.path().join("draft.bin");
    let data = contents(2 * 1024 * 1024, 2);
    fs::write(&source, &data).unwrap();
    let destination = root.path().join("copies");
    fs::create_dir(&destination).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    transfer(&engine, TransferKind::Copy, &source, &destination);
    let copy = destination.join("draft.bin");
    // Same size and time, different contents: only the content receipt sees it.
    let modified = fs::metadata(&copy).unwrap().modified().unwrap();
    let mut edited = data.clone();
    edited[4096] ^= 0xff;
    fs::write(&copy, &edited).unwrap();
    fs::File::options()
        .write(true)
        .open(&copy)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert!(engine.undo(&Cancellation::default()).is_err());
    assert_eq!(fs::read(&copy).unwrap(), edited);
    assert_eq!(fs::read(&source).unwrap(), data);
}

#[test]
fn moves_between_subvolumes_clone_and_their_undo_returns_the_item() {
    let Some(root) = btrfs_root() else { return };
    btrfs::create_subvolume(root.path(), "photos").unwrap();
    btrfs::create_subvolume(root.path(), "archive").unwrap();
    let folder = root.path().join("photos/trip");
    fs::create_dir(&folder).unwrap();
    let large = contents(6 * 1024 * 1024, 3);
    fs::write(folder.join("large.raw"), &large).unwrap();
    fs::write(folder.join("small.txt"), b"a small note").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let archive = root.path().join("archive");
    transfer(&engine, TransferKind::Move, &folder, &archive);
    assert!(!folder.exists());
    assert_eq!(fs::read(archive.join("trip/large.raw")).unwrap(), large);
    assert_eq!(
        fs::read(archive.join("trip/small.txt")).unwrap(),
        b"a small note"
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!archive.join("trip").exists());
    assert_eq!(fs::read(folder.join("large.raw")).unwrap(), large);
    assert_eq!(fs::read(folder.join("small.txt")).unwrap(), b"a small note");
}

#[test]
fn trash_inside_a_nested_subvolume_uses_its_own_top_and_is_listed() {
    let Some(root) = btrfs_root() else { return };
    let data_home = root.path().join("data");
    btrfs::create_subvolume(root.path(), "projects").unwrap();
    let projects = root.path().join("projects");
    let notes = projects.join("site/notes.txt");
    fs::create_dir(notes.parent().unwrap()).unwrap();
    fs::write(&notes, b"draft notes").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let outcome = engine
        .trash_files(
            &data_home,
            std::slice::from_ref(&notes),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(outcome.failures.is_empty(), "{outcome:?}");
    assert!(outcome.unavailable.is_empty(), "{outcome:?}");
    assert_eq!(outcome.completed, 1);
    assert!(!notes.exists());
    let uid = rustix::process::geteuid().as_raw();
    let storage = projects.join(format!(".Trash-{uid}"));
    assert_eq!(fs::read_dir(storage.join("files")).unwrap().count(), 1);
    // The freedesktop path is relative to the subvolume top.
    let info = fs::read_dir(storage.join("info"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(
        fs::read_to_string(info)
            .unwrap()
            .contains("Path=site/notes.txt\n")
    );
    assert!(
        TrashLocation::available(&data_home)
            .unwrap()
            .iter()
            .any(|location| location.directory() == storage)
    );
    let catalog = Catalog::read(&data_home, Scope::All, &Cancellation::default()).unwrap();
    let item = catalog
        .items
        .iter()
        .find(|item| item.original.as_deref() == Some(notes.as_path()))
        .expect("the subvolume Trash item is listed")
        .clone();
    engine
        .restore(&item, None, &Cancellation::default())
        .unwrap();
    assert_eq!(fs::read(&notes).unwrap(), b"draft notes");
    // Undo of Restore puts it back into the same subvolume Trash.
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!notes.exists());
    assert_eq!(fs::read_dir(storage.join("files")).unwrap().count(), 1);
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&notes).unwrap(), b"draft notes");
    assert_eq!(fs::read_dir(storage.join("files")).unwrap().count(), 0);
}

#[test]
fn trash_in_the_data_home_subvolume_keeps_the_main_trash() {
    let Some(root) = btrfs_root() else { return };
    let data_home = root.path().join("data");
    let report = root.path().join("report.txt");
    fs::write(&report, b"report").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let outcome = engine
        .trash_files(
            &data_home,
            std::slice::from_ref(&report),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(outcome.completed, 1, "{outcome:?}");
    assert_eq!(
        fs::read_dir(data_home.join("Trash/files")).unwrap().count(),
        1
    );
    assert!(!data_home.join("canopa-files/trash-tops.json").exists());
}

fn trash_one(engine: &OperationEngine, data_home: &Path, source: &Path) -> TrashOutcome {
    engine
        .trash_files(data_home, &[source.to_owned()], &Cancellation::default())
        .unwrap()
}

fn listed(data_home: &Path) -> Vec<canopa_files::trash::TrashItem> {
    Catalog::read(data_home, Scope::All, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .filter(|item| item.path.starts_with(data_home.parent().unwrap()))
        .collect()
}

#[test]
fn renaming_a_subvolume_or_its_parent_keeps_its_trash_listed() {
    let Some(root) = btrfs_root() else { return };
    let data_home = root.path().join("data");
    let area = root.path().join("area");
    fs::create_dir(&area).unwrap();
    btrfs::create_subvolume(&area, "projects").unwrap();
    let notes = area.join("projects/site/notes.txt");
    fs::create_dir(notes.parent().unwrap()).unwrap();
    fs::write(&notes, b"draft notes").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    assert_eq!(trash_one(&engine, &data_home, &notes).completed, 1);

    // Renamed through Files.
    engine
        .execute(
            OperationRequest::Rename {
                source: area.join("projects"),
                destination: area.join("work"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    let items = listed(&data_home);
    assert_eq!(items.len(), 1, "the renamed subvolume's Trash is listed");
    let renamed = area.join("work/site/notes.txt");
    assert_eq!(items[0].original.as_deref(), Some(renamed.as_path()));

    // An ancestor renamed by another program.
    let zone = root.path().join("zone");
    fs::rename(&area, &zone).unwrap();
    let items = listed(&data_home);
    assert_eq!(items.len(), 1, "the moved subvolume's Trash is listed");
    let moved = zone.join("work/site/notes.txt");
    assert_eq!(items[0].original.as_deref(), Some(moved.as_path()));
    engine
        .restore(&items[0], None, &Cancellation::default())
        .unwrap();
    assert_eq!(fs::read(&moved).unwrap(), b"draft notes");
}

#[test]
fn an_unreachable_subvolume_trash_keeps_its_record() {
    use std::os::unix::fs::PermissionsExt;
    let Some(root) = btrfs_root() else { return };
    let data_home = root.path().join("data");
    let hidden = root.path().join("removable");
    fs::create_dir(&hidden).unwrap();
    btrfs::create_subvolume(&hidden, "first").unwrap();
    btrfs::create_subvolume(root.path(), "second").unwrap();
    let first = hidden.join("first/a.txt");
    let second = root.path().join("second/b.txt");
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    assert_eq!(trash_one(&engine, &data_home, &first).completed, 1);
    // As if its drive were disconnected: neither its path nor a search finds it.
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o000)).unwrap();
    let outcome = trash_one(&engine, &data_home, &second);
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(outcome.completed, 1, "{outcome:?}");
    let mut originals: Vec<_> = listed(&data_home)
        .into_iter()
        .filter_map(|item| item.original)
        .collect();
    originals.sort();
    assert_eq!(originals, vec![first, second]);
}

#[test]
fn a_full_record_refuses_new_trash_and_reclaims_only_empty_tops_undo_no_longer_needs() {
    const LIMIT: usize = 256;
    let Some(root) = btrfs_root() else { return };
    let data_home = root.path().join("data");
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let mut sources = Vec::new();
    for index in 0..=LIMIT {
        let name = format!("volume-{index}");
        btrfs::create_subvolume(root.path(), &name).unwrap();
        let source = root.path().join(name).join("item.txt");
        fs::write(&source, index.to_string()).unwrap();
        sources.push(source);
    }
    for source in &sources[..LIMIT] {
        assert_eq!(trash_one(&engine, &data_home, source).completed, 1);
    }
    let item_from = |index: usize| {
        listed(&data_home)
            .into_iter()
            .find(|item| item.original.as_deref() == Some(sources[index].as_path()))
    };
    // Every recorded top holds an item: the next one fails before it moves,
    // as an ordinary failure rather than an offer to delete permanently.
    let last = &sources[LIMIT];
    let refused = |outcome: TrashOutcome| {
        assert_eq!(outcome.completed, 0, "{outcome:?}");
        assert_eq!(outcome.failures.len(), 1, "{outcome:?}");
        assert!(outcome.unavailable.is_empty(), "{outcome:?}");
        assert_eq!(fs::read_to_string(last).unwrap(), LIMIT.to_string());
    };
    refused(trash_one(&engine, &data_home, last));
    assert_eq!(listed(&data_home).len(), LIMIT);
    // Restoring empties the first top, but Undo of that Restore moves the
    // item back there, so its record is still needed.
    engine
        .restore(&item_from(0).unwrap(), None, &Cancellation::default())
        .unwrap();
    refused(trash_one(&engine, &data_home, last));
    // Deleting the second top's item permanently leaves nothing to undo
    // there: its Trash operation has left the 100-operation history.
    engine
        .delete_from_trash(&[item_from(1).unwrap()], &Cancellation::default())
        .unwrap();
    assert_eq!(trash_one(&engine, &data_home, last).completed, 1);
    assert!(item_from(LIMIT).is_some());
    // Undo the new Trash, then the Restore: the first top is still listed.
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read_to_string(last).unwrap(), LIMIT.to_string());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!sources[0].exists());
    assert!(item_from(0).is_some(), "the restored top lost its record");
    assert_eq!(listed(&data_home).len(), LIMIT - 1);
}

#[test]
fn a_trashed_subvolume_is_one_item_with_a_symlinked_data_home() {
    let Some(root) = btrfs_root() else { return };
    let real = root.path().join("real-data");
    fs::create_dir(&real).unwrap();
    let data_home = root.path().join("data");
    std::os::unix::fs::symlink(&real, &data_home).unwrap();
    btrfs::create_subvolume(root.path(), "bundle").unwrap();
    let bundle = root.path().join("bundle");
    let inner = bundle.join("inner.txt");
    fs::write(&inner, b"inner").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    // The subvolume gets its own Trash, then is trashed itself.
    assert_eq!(trash_one(&engine, &data_home, &inner).completed, 1);
    assert_eq!(trash_one(&engine, &data_home, &bundle).completed, 1);
    let items = listed(&data_home);
    assert_eq!(
        items.len(),
        1,
        "{:?}",
        items.iter().map(|item| &item.path).collect::<Vec<_>>()
    );
    assert_eq!(items[0].original.as_deref(), Some(bundle.as_path()));
}

#[test]
fn an_unreadable_record_is_never_overwritten() {
    let Some(root) = btrfs_root() else { return };
    let data_home = root.path().join("data");
    btrfs::create_subvolume(root.path(), "volume").unwrap();
    let source = root.path().join("volume/item.txt");
    fs::write(&source, b"item").unwrap();
    let record = data_home.join("canopa-files/trash-tops.json");
    fs::create_dir_all(record.parent().unwrap()).unwrap();
    fs::write(&record, b"{ not a record").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let outcome = trash_one(&engine, &data_home, &source);
    assert_eq!(outcome.completed, 0, "{outcome:?}");
    assert!(outcome.unavailable.is_empty(), "{outcome:?}");
    assert_eq!(fs::read(&source).unwrap(), b"item");
    assert_eq!(fs::read(&record).unwrap(), b"{ not a record");
}

#[test]
fn subvolumes_named_like_trash_keep_their_own_trash_listed() {
    let Some(root) = btrfs_root() else { return };
    let data_home = root.path().join("data");
    let uid = rustix::process::geteuid().as_raw();
    // A subvolume whose name begins with ".Trash", one below an ordinary
    // ".Trash" folder, and one below a ".Trash-$uid" folder that is not storage.
    btrfs::create_subvolume(root.path(), ".Trash-documents").unwrap();
    let ordinary = root.path().join(".Trash");
    fs::create_dir(&ordinary).unwrap();
    btrfs::create_subvolume(&ordinary, "notes").unwrap();
    let lookalike = root.path().join(format!("work/.Trash-{uid}"));
    fs::create_dir_all(&lookalike).unwrap();
    btrfs::create_subvolume(&lookalike, "drafts").unwrap();
    let sources = [
        root.path().join(".Trash-documents/report.txt"),
        ordinary.join("notes/todo.txt"),
        lookalike.join("drafts/plan.txt"),
    ];
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    for source in &sources {
        fs::write(source, b"keep me").unwrap();
        assert_eq!(trash_one(&engine, &data_home, source).completed, 1);
    }
    let items = listed(&data_home);
    for source in &sources {
        let item = items
            .iter()
            .find(|item| item.original.as_deref() == Some(source.as_path()))
            .unwrap_or_else(|| panic!("{} is not listed", source.display()));
        engine
            .restore(item, None, &Cancellation::default())
            .unwrap();
        assert_eq!(fs::read(source).unwrap(), b"keep me");
    }
}
