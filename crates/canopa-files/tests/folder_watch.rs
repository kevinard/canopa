//! Displayed-folder observation through the public watcher and Browser model.
use canopa_files::{
    browse::{Browser, Directory, SelectionMode},
    folder_watch::{Changes, FolderWatch, MAXIMUM_FOLDERS, Timing},
};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

fn timing() -> Timing {
    Timing {
        quiet: Duration::from_millis(50),
        maximum: Duration::from_millis(400),
    }
}

/// Accumulate published changes until `done` accepts them or time runs out.
fn changes_until(watch: &FolderWatch, mut done: impl FnMut(&Changes) -> bool) -> Changes {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen = Changes::default();
    loop {
        let next = watch.take_changes();
        seen.folders.extend(next.folders);
        seen.limited = next.limited;
        if done(&seen) {
            return seen;
        }
        assert!(
            Instant::now() < deadline,
            "watcher did not publish {seen:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The worker applies a new target set asynchronously; settle before acting.
fn settle() {
    std::thread::sleep(Duration::from_millis(150));
}

fn quiet(watch: &FolderWatch) -> BTreeSet<PathBuf> {
    std::thread::sleep(Duration::from_millis(500));
    watch.take_changes().folders
}

#[test]
fn an_outside_change_reports_only_its_displayed_folder() {
    let root = tempfile::tempdir().unwrap();
    let downloads = root.path().join("Downloads");
    let documents = root.path().join("Documents");
    let hidden = root.path().join("Hidden");
    for folder in [&downloads, &documents, &hidden] {
        fs::create_dir(folder).unwrap();
    }
    let watch = FolderWatch::start(timing()).unwrap();
    watch.watch([downloads.clone(), documents.clone()]);
    settle();
    fs::write(downloads.join("finished.iso.part"), b"partial").unwrap();
    fs::rename(
        downloads.join("finished.iso.part"),
        downloads.join("finished.iso"),
    )
    .unwrap();
    fs::write(hidden.join("other"), b"not displayed").unwrap();
    let changes = changes_until(&watch, |changes| !changes.folders.is_empty());
    assert_eq!(changes.folders, BTreeSet::from([downloads.clone()]));
    assert!(!changes.limited);
    assert!(quiet(&watch).is_empty());

    // A file saved by another application in the second folder.
    fs::write(documents.join("letter.odt"), b"saved").unwrap();
    let changes = changes_until(&watch, |changes| !changes.folders.is_empty());
    assert_eq!(changes.folders, BTreeSet::from([documents]));
}

#[test]
fn a_burst_is_coalesced_into_one_publication() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("burst");
    fs::create_dir(&folder).unwrap();
    let watch = FolderWatch::start(Timing {
        quiet: Duration::from_millis(300),
        maximum: Duration::from_secs(5),
    })
    .unwrap();
    let mut notices = watch.take_notices().unwrap();
    assert!(watch.take_notices().is_none());
    watch.watch([folder.clone()]);
    settle();
    for index in 0..200 {
        fs::write(folder.join(format!("file-{index}")), b"x").unwrap();
    }
    let changes = changes_until(&watch, |changes| !changes.folders.is_empty());
    assert_eq!(changes.folders, BTreeSet::from([folder]));
    let mut published = 0;
    while notices.try_recv().is_ok() {
        published += 1;
    }
    assert_eq!(published, 1, "the burst must produce a single notice");
}

#[test]
fn a_continuing_burst_still_publishes_within_the_maximum_delay() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("copying");
    fs::create_dir(&folder).unwrap();
    let watch = FolderWatch::start(Timing {
        quiet: Duration::from_millis(200),
        maximum: Duration::from_millis(300),
    })
    .unwrap();
    watch.watch([folder.clone()]);
    settle();
    let start = Instant::now();
    let mut index = 0;
    // Keep writing faster than the quiet period for well over the maximum.
    while start.elapsed() < Duration::from_millis(1500) {
        fs::write(folder.join(format!("chunk-{index}")), b"x").unwrap();
        index += 1;
        if !watch.take_changes().folders.is_empty() {
            assert!(start.elapsed() < Duration::from_millis(1500));
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("an uninterrupted burst was never published");
}

#[test]
fn closing_a_view_stops_its_observation_and_drops_pending_changes() {
    let root = tempfile::tempdir().unwrap();
    let kept = root.path().join("kept");
    let closed = root.path().join("closed");
    fs::create_dir(&kept).unwrap();
    fs::create_dir(&closed).unwrap();
    let watch = FolderWatch::start(Timing {
        quiet: Duration::from_millis(300),
        maximum: Duration::from_secs(2),
    })
    .unwrap();
    watch.watch([kept.clone(), closed.clone()]);
    settle();
    // A change still waiting for its quiet period, then the view closes.
    fs::write(closed.join("pending"), b"x").unwrap();
    std::thread::sleep(Duration::from_millis(50));
    watch.watch([kept.clone()]);
    settle();
    fs::write(closed.join("after"), b"x").unwrap();
    assert!(quiet(&watch).is_empty());
    fs::write(kept.join("still"), b"x").unwrap();
    let changes = changes_until(&watch, |changes| !changes.folders.is_empty());
    assert_eq!(changes.folders, BTreeSet::from([kept]));
    watch.watch([]);
    settle();
    fs::write(root.path().join("kept/none"), b"x").unwrap();
    assert!(quiet(&watch).is_empty());
}

#[test]
fn a_recreated_folder_is_observed_again() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("rebuilt");
    fs::create_dir(&folder).unwrap();
    let watch = FolderWatch::start(timing()).unwrap();
    watch.watch([folder.clone()]);
    settle();
    fs::remove_dir(&folder).unwrap();
    changes_until(&watch, |changes| changes.folders.contains(&folder));
    fs::create_dir(&folder).unwrap();
    changes_until(&watch, |changes| changes.folders.contains(&folder));
    settle();
    fs::write(folder.join("new"), b"x").unwrap();
    changes_until(&watch, |changes| changes.folders.contains(&folder));
}

#[test]
fn folders_beyond_the_budget_degrade_to_manual_refresh() {
    let root = tempfile::tempdir().unwrap();
    let folders: Vec<_> = (0..=MAXIMUM_FOLDERS)
        .map(|index| root.path().join(format!("folder-{index:03}")))
        .collect();
    for folder in &folders {
        fs::create_dir(folder).unwrap();
    }
    let watch = FolderWatch::start(timing()).unwrap();
    let mut notices = watch.take_notices().unwrap();
    watch.watch(folders.clone());
    let changes = changes_until(&watch, |changes| changes.limited);
    assert!(changes.folders.is_empty());
    assert!(notices.try_recv().is_ok(), "the limit change is published");
    // Observed folders keep working; the unobserved one stays manual.
    let first = &folders[0];
    let last = folders.last().unwrap();
    fs::write(last.join("unobserved"), b"x").unwrap();
    fs::write(first.join("observed"), b"x").unwrap();
    let changes = changes_until(&watch, |changes| !changes.folders.is_empty());
    assert_eq!(changes.folders, BTreeSet::from([first.clone()]));
    assert!(changes.limited);
    // A manual refresh still reads the unobserved folder.
    assert_eq!(Directory::read(last).unwrap().entries.len(), 1);
    watch.watch([first.clone()]);
    changes_until(&watch, |changes| !changes.limited);
}

fn names(browser: &Browser) -> Vec<String> {
    browser
        .visible_entries()
        .iter()
        .map(|entry| entry.name().into_owned())
        .collect()
}

#[test]
fn a_watched_relisting_keeps_selection_focus_and_scroll() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c", "d"] {
        fs::write(root.path().join(name), b"x").unwrap();
    }
    let path = |name: &str| root.path().join(name);
    let watch = FolderWatch::start(timing()).unwrap();
    let mut browser = Browser::open(root.path()).unwrap();
    watch.watch([browser.path().to_owned()]);
    settle();
    browser.select(&path("b"), SelectionMode::Replace);
    browser.select(&path("d"), SelectionMode::Toggle);
    browser.focus_entry(&path("c"), false);
    browser.set_scroll(48.0);

    fs::write(path("0-new"), b"downloaded").unwrap();
    fs::remove_file(path("d")).unwrap();
    let changes = changes_until(&watch, |changes| !changes.folders.is_empty());
    assert_eq!(changes.folders, BTreeSet::from([browser.path().to_owned()]));
    for folder in changes.folders {
        assert!(browser.refresh(Directory::read(&folder).unwrap()));
    }
    assert_eq!(names(&browser), ["0-new", "a", "b", "c"]);
    assert_eq!(browser.selected(), &BTreeSet::from([path("b")]));
    assert_eq!(browser.focused(), Some(path("c").as_path()));
    assert_eq!(browser.scroll(), 48.0);
    // The kept anchor still drives range extension after the relisting.
    browser.step(1, SelectionMode::Extend);
    assert!(browser.selected().contains(&path("c")));

    // A change in another folder never replaces this tab's snapshot.
    let other = tempfile::tempdir().unwrap();
    assert!(!browser.refresh(Directory::read(other.path()).unwrap()));
    assert_eq!(names(&browser), ["0-new", "a", "b", "c"]);
}

#[test]
fn relisting_a_folder_with_an_open_search_keeps_the_results() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("report.txt"), b"x").unwrap();
    let mut browser = Browser::open(root.path()).unwrap();
    let results = canopa_files::search::SearchRequest::new(
        root.path(),
        "report",
        canopa_files::search::SearchScope::CurrentFolder,
    )
    .run(&canopa_files::Cancellation::default());
    assert!(browser.show_search(results));
    browser.select(&root.path().join("report.txt"), SelectionMode::Replace);
    fs::write(root.path().join("later.txt"), b"x").unwrap();
    assert!(browser.refresh(Directory::read(root.path()).unwrap()));
    assert!(browser.search().is_some());
    assert_eq!(browser.selected().len(), 1);
    browser.close_search();
    assert_eq!(browser.directory().entries.len(), 2);
}
