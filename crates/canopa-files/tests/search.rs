use std::{fs, os::unix::fs::symlink};

use canopa_files::{
    Cancellation,
    browse::{Browser, SelectionMode},
    search::{SearchRequest, SearchScope},
};

#[test]
fn recursive_filename_search_avoids_symlink_cycles_and_restores_browsing() {
    let root = tempfile::tempdir().unwrap();
    let weekend = root.path().join("Weekend");
    fs::create_dir(&weekend).unwrap();
    fs::write(root.path().join("coast.jpg"), b"photo").unwrap();
    fs::write(weekend.join("coast.png"), b"photo").unwrap();
    let notes = root.path().join("notes.txt");
    fs::write(&notes, b"coast is in the contents only").unwrap();
    fs::write(root.path().join(".coast-secret.jpg"), b"hidden").unwrap();
    symlink(root.path(), weekend.join("loop")).unwrap();
    let mut browser = Browser::open(root.path()).unwrap();
    browser.select(&notes, SelectionMode::Replace);
    browser.set_scroll(80.0);

    let results = SearchRequest::new(root.path(), "coast", SearchScope::Recursive)
        .run(&Cancellation::default());
    let names: Vec<_> = results
        .entries
        .iter()
        .map(|entry| entry.name().into_owned())
        .collect();
    assert_eq!(names, ["coast.jpg", "coast.png"]);
    assert!(results.failures.is_empty());
    assert!(!results.cancelled);
    browser.show_search(results);
    browser.select(&weekend.join("coast.png"), SelectionMode::Replace);
    browser.set_scroll(0.0);
    browser.close_search();
    assert_eq!(browser.focused(), Some(notes.as_path()));
    assert_eq!(browser.scroll(), 80.0);
}

#[test]
fn abandoned_directory_and_search_jobs_stop_before_reading_their_root() {
    use canopa_files::browse::Directory;
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("already-removed");
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        Directory::read_cancellable(&missing, &cancellation)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    let result = SearchRequest::new(missing, "anything", SearchScope::Recursive).run(&cancellation);
    assert!(result.cancelled);
    assert!(result.failures.is_empty());
    assert!(result.entries.is_empty());
    assert_eq!(result.scanned, 0);
}

#[test]
fn stopping_from_initial_progress_keeps_the_matching_file_already_found() {
    let root = tempfile::tempdir().unwrap();
    for name in ["match-a", "match-b", "match-c"] {
        fs::write(root.path().join(name), b"fixture").unwrap();
    }
    let cancellation = Cancellation::default();
    let results = SearchRequest::new(root.path(), "match", SearchScope::CurrentFolder)
        .run_with_progress(&cancellation, |progress| {
            assert_eq!(progress.entries.len(), 1);
            cancellation.cancel();
        });
    assert!(results.cancelled);
    assert_eq!(results.entries.len(), 1);
    assert_eq!(results.entries[0].path, root.path().join("match-a"));
    assert_eq!(results.scanned, 1);
}
