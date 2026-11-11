//! Watch rescans through the Desktop model, without a compositor or workers.
use super::tests::{desktop, loaded};
use super::*;
use std::{collections::BTreeSet, fs};

fn rescan(desktop: &mut Desktop, window: window::Id, tab: u64) -> (u64, u64) {
    let _task = desktop.watch_rescan(window, tab);
    let tab = desktop.active(window).unwrap();
    assert!(tab.watch_scanning);
    (tab.revision, tab.watch_request)
}

fn deliver(
    desktop: &mut Desktop,
    window: window::Id,
    tab: u64,
    (revision, request): (u64, u64),
    path: &Path,
) {
    let _task = desktop.update(Message::WatchLoaded {
        window,
        tab,
        revision,
        request,
        result: Arc::new(Ok(Directory::read(path).unwrap())),
    });
}

fn names(desktop: &Desktop, window: window::Id) -> Vec<String> {
    desktop
        .active(window)
        .unwrap()
        .browser
        .as_ref()
        .unwrap()
        .visible_entries()
        .iter()
        .map(|entry| entry.name().into_owned())
        .collect()
}

#[test]
fn an_outside_change_relists_without_disturbing_selection_scroll_or_origins() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Downloads");
    fs::create_dir(&folder).unwrap();
    for name in ["a", "b", "c"] {
        fs::write(folder.join(name), b"x").unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &folder);
    let browser = desktop
        .active_mut(window)
        .unwrap()
        .browser
        .as_mut()
        .unwrap();
    browser.select(&folder.join("b"), SelectionMode::Replace);
    browser.set_scroll(32.0);
    let origin = file_operations::Origin::capture(&desktop, window).unwrap();
    let epoch = desktop.active(window).unwrap().list_epoch;
    assert!(folder_watch_ui::displayed(&desktop.windows).contains(&folder));

    fs::write(folder.join("0-download.iso"), b"finished").unwrap();
    fs::remove_file(folder.join("c")).unwrap();
    let _task = desktop.apply_folder_changes(&BTreeSet::from([folder.clone()]));
    let current = desktop.active(window).unwrap();
    assert!(current.watch_scanning);
    let request = (current.revision, current.watch_request);
    // Bursts while a rescan runs are remembered, not run concurrently.
    let _task = desktop.apply_folder_changes(&BTreeSet::from([folder.clone()]));
    assert!(desktop.active(window).unwrap().watch_stale);
    assert_eq!(desktop.active(window).unwrap().watch_request, request.1);
    deliver(&mut desktop, window, tab, request, &folder);

    assert_eq!(names(&desktop, window), ["0-download.iso", "a", "b"]);
    let tab_state = desktop.active(window).unwrap();
    let browser = tab_state.browser.as_ref().unwrap();
    assert_eq!(browser.selected(), &BTreeSet::from([folder.join("b")]));
    assert_eq!(browser.focused(), Some(folder.join("b").as_path()));
    assert_eq!(browser.scroll(), 32.0);
    assert_eq!(tab_state.list_epoch, epoch);
    assert!(!tab_state.loading);
    // The remembered burst started one follow-up rescan.
    assert!(tab_state.watch_scanning && !tab_state.watch_stale);
    // Rename dialogs, drags and menus captured before the change stay valid.
    assert!(origin.current(&desktop));
}

#[test]
fn superseded_rescans_and_navigation_win_over_late_results() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![first.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &first);
    let old = rescan(&mut desktop, window, tab);
    let _task = desktop.watch_rescan(window, tab);
    fs::write(first.join("late"), b"x").unwrap();
    deliver(&mut desktop, window, tab, old, &first);
    assert!(names(&desktop, window).is_empty(), "an older request lost");

    let pending = (old.0, desktop.active(window).unwrap().watch_request);
    let _task = desktop.navigate(window, second.clone(), Navigation::Visit, None);
    let navigating = desktop.active(window).unwrap();
    assert!(!navigating.watch_scanning);
    assert!(folder_watch_ui::displayed(&desktop.windows).contains(&second));
    // A change in the destination while it loads is replayed after loading.
    let _task = desktop.apply_folder_changes(&BTreeSet::from([second.clone()]));
    assert!(desktop.active(window).unwrap().watch_stale);
    deliver(&mut desktop, window, tab, pending, &first);
    loaded(&mut desktop, window, tab, 2, &second);
    let loaded_tab = desktop.active(window).unwrap();
    assert_eq!(loaded_tab.browser.as_ref().unwrap().path(), second);
    assert!(loaded_tab.watch_scanning && !loaded_tab.watch_stale);
}

#[test]
fn hidden_tabs_refresh_when_shown_and_closed_tabs_stop_being_watched() {
    let root = tempfile::tempdir().unwrap();
    let shown = root.path().join("shown");
    let hidden = root.path().join("hidden");
    fs::create_dir(&shown).unwrap();
    fs::create_dir(&hidden).unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![shown.clone(), hidden.clone()]);
    let first = desktop.windows[&window].tabs[0].id;
    let second = desktop.windows[&window].tabs[1].id;
    loaded(&mut desktop, window, first, 1, &shown);
    let _task = desktop.update(Message::SwitchTab(window, 1));
    loaded(&mut desktop, window, second, 1, &hidden);
    let _task = desktop.update(Message::SwitchTab(window, 0));
    assert_eq!(
        folder_watch_ui::displayed(&desktop.windows),
        BTreeSet::from([shown.clone(), hidden.clone()])
    );

    fs::write(hidden.join("saved-elsewhere"), b"x").unwrap();
    let _task = desktop.apply_folder_changes(&BTreeSet::from([hidden.clone()]));
    let background = &desktop.windows[&window].tabs[1];
    assert!(background.watch_stale && !background.watch_scanning);
    let _task = desktop.update(Message::SwitchTab(window, 1));
    let foreground = desktop.active(window).unwrap();
    assert!(foreground.watch_scanning && !foreground.watch_stale);
    let request = (foreground.revision, foreground.watch_request);
    deliver(&mut desktop, window, second, request, &hidden);
    assert_eq!(names(&desktop, window), ["saved-elsewhere"]);

    let _task = desktop.command(window, Command::CloseTab);
    assert_eq!(
        folder_watch_ui::displayed(&desktop.windows),
        BTreeSet::from([shown])
    );
}

#[test]
fn a_vanished_folder_reports_its_error_and_recovers_when_recreated() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("removable");
    fs::create_dir(&folder).unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &folder);
    let request = rescan(&mut desktop, window, tab);
    let _task = desktop.update(Message::WatchLoaded {
        window,
        tab,
        revision: request.0,
        request: request.1,
        result: Arc::new(Err((false, "No such file or directory".into()))),
    });
    assert!(desktop.active(window).unwrap().error.is_some());
    let request = rescan(&mut desktop, window, tab);
    deliver(&mut desktop, window, tab, request, &folder);
    assert!(desktop.active(window).unwrap().error.is_none());
}

#[test]
fn ancestor_columns_are_watched_and_relisted_in_place() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    let child = parent.join("child");
    fs::create_dir_all(&child).unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![child.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &child);
    let _task = desktop.update(Message::Resized(window, Size::new(1600.0, 600.0)));
    let column = &desktop.active(window).unwrap().columns[&parent];
    let first = column.request;
    let _task = desktop.update(Message::ColumnLoaded {
        window,
        tab,
        revision: 1,
        path: parent.clone(),
        request: first,
        result: Arc::new(Ok(Directory::read(&parent).unwrap())),
    });
    desktop
        .active_mut(window)
        .unwrap()
        .columns
        .get_mut(&parent)
        .unwrap()
        .scroll = 24.0;
    assert!(folder_watch_ui::displayed(&desktop.windows).contains(&parent));

    fs::write(parent.join("sibling.txt"), b"x").unwrap();
    let _task = desktop.apply_folder_changes(&BTreeSet::from([parent.clone()]));
    let column = &desktop.active(window).unwrap().columns[&parent];
    let second = column.request;
    assert!(second > first);
    // The previous listing stays visible until the new one arrives.
    assert_eq!(
        column
            .result
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .entries
            .len(),
        1
    );
    let _task = desktop.update(Message::ColumnLoaded {
        window,
        tab,
        revision: 1,
        path: parent.clone(),
        request: second,
        result: Arc::new(Ok(Directory::read(&parent).unwrap())),
    });
    let column = &desktop.active(window).unwrap().columns[&parent];
    assert_eq!(
        column
            .result
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .entries
            .len(),
        2
    );
    assert_eq!(column.scroll, 24.0);
}
