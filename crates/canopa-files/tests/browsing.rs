use std::fs;

use canopa_files::browse::{Browser, Directory, HistoryDirection, SelectionMode};
use canopa_files::preferences::{SortField, ViewMode, ViewSettings};

#[test]
fn pointer_selection_keeps_keyboard_focus_and_range_anchor() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c"] {
        fs::write(root.path().join(name), b"x").unwrap();
    }
    let a = root.path().join("a");
    let b = root.path().join("b");
    let c = root.path().join("c");
    let mut browser = Browser::open(root.path()).unwrap();
    browser.focus_entry(&a, true);
    browser.set_selection([a.clone()]);
    browser.focus_entry(&c, false);
    browser.set_selection([a.clone(), b.clone(), c.clone()]);
    browser.step(-1, SelectionMode::Extend);
    assert_eq!(browser.focused(), Some(b.as_path()));
    assert_eq!(browser.selected(), &[a, b].into_iter().collect());
    browser.set_selection([root.path().join("missing")]);
    assert!(browser.selected().is_empty());
}

#[test]
fn narrow_navigation_preserves_state_and_parent_selects_the_folder_just_left() {
    let root = tempfile::tempdir().unwrap();
    let downloads = root.path().join("Downloads");
    fs::create_dir(&downloads).unwrap();
    let photo = downloads.join("coast.jpg");
    fs::write(&photo, b"image fixture").unwrap();
    let mut browser = Browser::open(&downloads).unwrap();
    browser.select(&photo, SelectionMode::Replace);
    browser.set_scroll(120.0);

    assert_eq!(browser.column_paths(320.0), vec![downloads.clone()]);
    assert!(browser.column_paths(1200.0).len() >= 2);
    // Explicit sidebar choices must use the content width, without applying
    // the automatic sidebar threshold for a second time.
    assert_eq!(browser.column_paths_for_content(960.0).len(), 4);
    assert_eq!(browser.column_paths_for_content(800.0).len(), 3);
    assert_eq!(
        browser.column_paths_for_content(200.0),
        vec![downloads.clone()]
    );
    assert_eq!(browser.focused(), Some(photo.as_path()));
    assert_eq!(browser.scroll(), 120.0);

    browser.visit(Directory::read(browser.parent().unwrap()).unwrap());
    assert_eq!(browser.focused(), Some(downloads.as_path()));

    browser.visit(Directory::read(&downloads).unwrap());
    assert_eq!(browser.focused(), Some(photo.as_path()));
    assert_eq!(browser.scroll(), 120.0);
}

#[test]
fn range_selection_and_toggle_operate_on_visible_files() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.txt", ".secret"] {
        fs::write(root.path().join(name), b"fixture").unwrap();
    }
    let mut browser = Browser::open(root.path()).unwrap();
    assert_eq!(browser.visible_entries().len(), 3);
    browser.select(&root.path().join("a.txt"), SelectionMode::Replace);
    browser.select(&root.path().join("c.txt"), SelectionMode::Extend);
    assert_eq!(browser.selected().len(), 3);
    browser.select(&root.path().join("b.txt"), SelectionMode::Toggle);
    assert_eq!(browser.selected().len(), 2);
    assert!(!browser.selected().contains(&root.path().join("b.txt")));

    browser.set_show_hidden(true);
    browser.select_all();
    assert_eq!(browser.selected().len(), 4);
    browser.set_show_hidden(false);
    assert_eq!(browser.selected().len(), 3);
    assert!(!browser.selected().contains(&root.path().join(".secret")));
}

#[test]
fn keyboard_selection_and_history_restore_each_locations_state() {
    let root = tempfile::tempdir().unwrap();
    let projects = root.path().join("Projects");
    fs::create_dir(&projects).unwrap();
    let notes = projects.join("notes.txt");
    fs::write(&notes, b"notes").unwrap();
    fs::write(root.path().join("todo.txt"), b"todo").unwrap();
    let mut browser = Browser::open(root.path()).unwrap();
    browser.step(1, SelectionMode::Replace);
    assert_eq!(browser.focused(), Some(projects.as_path()));
    browser.visit(Directory::read(&projects).unwrap());
    assert!(browser.select_prefix("NOTE"));
    browser.set_scroll(42.0);

    let target = browser
        .history_target(HistoryDirection::Back)
        .unwrap()
        .to_owned();
    assert!(browser.visit_history(Directory::read(target).unwrap(), HistoryDirection::Back));
    assert_eq!(browser.focused(), Some(projects.as_path()));
    let target = browser
        .history_target(HistoryDirection::Forward)
        .unwrap()
        .to_owned();
    assert!(browser.visit_history(Directory::read(target).unwrap(), HistoryDirection::Forward));
    assert_eq!(browser.focused(), Some(notes.as_path()));
    assert_eq!(browser.scroll(), 42.0);

    fs::remove_file(&notes).unwrap();
    assert!(browser.refresh(Directory::read(&projects).unwrap()));
    assert!(browser.selected().is_empty());
    assert!(browser.focused().is_none());
    assert_eq!(browser.scroll(), 42.0);
}

#[test]
fn changing_folder_view_sorts_files_and_changes_column_width_without_losing_selection() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Z-folder")).unwrap();
    let large = root.path().join("large.dat");
    fs::write(&large, b"12345").unwrap();
    fs::write(root.path().join("small.dat"), b"1").unwrap();
    let mut browser = Browser::open(root.path()).unwrap();
    browser.select(&large, SelectionMode::Replace);
    browser
        .set_view(ViewSettings {
            mode: ViewMode::List,
            sort: SortField::Size,
            descending: true,
            column_width: 320,
            ..ViewSettings::default()
        })
        .unwrap();
    let names: Vec<_> = browser
        .visible_entries()
        .iter()
        .map(|entry| entry.name().into_owned())
        .collect();
    assert_eq!(names, ["Z-folder", "large.dat", "small.dat"]);
    assert_eq!(browser.focused(), Some(large.as_path()));
    assert_eq!(browser.view().mode, ViewMode::List);
    assert_eq!(browser.column_paths(600.0).len(), 1);
}

#[test]
fn leaving_an_explicit_hidden_folder_keeps_that_child_visible_without_showing_other_hidden_files() {
    let root = tempfile::tempdir().unwrap();
    let active = root.path().join(".active");
    let other = root.path().join(".other");
    for path in [&active, &other, &root.path().join("visible")] {
        fs::create_dir(path).unwrap();
    }
    let mut browser = Browser::open(&active).unwrap();
    assert!(!browser.show_hidden());
    browser.visit(Directory::read(root.path()).unwrap());
    assert_eq!(browser.focused(), Some(active.as_path()));
    assert_eq!(browser.selected(), &[active.clone()].into_iter().collect());
    let shown = || {
        browser
            .visible_entries()
            .into_iter()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>()
    };
    assert!(shown().contains(&active));
    assert!(!shown().contains(&other));
    assert!(!browser.show_hidden());
    assert_eq!(browser.column_paths(320.0), vec![root.path().to_owned()]);
    browser.refresh(Directory::read(root.path()).unwrap());
    assert_eq!(browser.focused(), Some(active.as_path()));
    let elsewhere = tempfile::tempdir().unwrap();
    browser.visit(Directory::read(elsewhere.path()).unwrap());
    let back = browser
        .history_target(HistoryDirection::Back)
        .unwrap()
        .to_owned();
    assert!(browser.visit_history(Directory::read(back).unwrap(), HistoryDirection::Back));
    assert_eq!(browser.focused(), Some(active.as_path()));
    assert!(
        browser
            .visible_entries()
            .iter()
            .any(|entry| entry.path == active)
    );
    browser.set_show_hidden(true);
    assert!(
        browser
            .visible_entries()
            .iter()
            .any(|entry| entry.path == other)
    );
    browser.set_show_hidden(false);
    assert!(
        !browser
            .visible_entries()
            .iter()
            .any(|entry| entry.is_hidden())
    );
    assert!(browser.selected().is_empty());
    assert_eq!(browser.focused(), None);
}

#[test]
fn parent_selection_retains_an_explicit_hidden_directory_link_with_a_raw_unix_name() {
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::symlink},
    };
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("real folder");
    fs::create_dir(&target).unwrap();
    let link = root
        .path()
        .join(OsString::from_vec(b".linked-\xff".to_vec()));
    symlink(&target, &link).unwrap();
    fs::create_dir(root.path().join(".other")).unwrap();
    let mut browser = Browser::open(&link).unwrap();
    browser.visit(Directory::read(root.path()).unwrap());
    assert!(!browser.show_hidden());
    assert_eq!(browser.focused(), Some(link.as_path()));
    assert_eq!(browser.selected(), &[link.clone()].into_iter().collect());
    assert_eq!(browser.visible_entries().len(), 2);
    assert!(
        browser
            .visible_entries()
            .iter()
            .any(|entry| entry.path == link && entry.is_directory())
    );
}
#[test]
fn explicit_reveal_selects_only_requested_hidden_items_and_survives_refresh() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join(".first");
    let second = root.path().join(".second");
    let unrelated = root.path().join(".unrelated");
    for path in [&first, &second, &unrelated] {
        std::fs::write(path, b"private reveal fixture").unwrap();
    }
    let mut browser = canopa_files::browse::Browser::open(root.path()).unwrap();
    browser.reveal_selection(vec![second.clone(), first.clone()]);
    assert!(!browser.show_hidden());
    assert_eq!(browser.selected().len(), 2);
    assert_eq!(browser.focused(), Some(second.as_path()));
    assert_eq!(browser.visible_entries().len(), 2);
    browser.refresh(canopa_files::browse::Directory::read(root.path()).unwrap());
    assert_eq!(browser.selected().len(), 2);
    assert!(
        !browser
            .visible_entries()
            .iter()
            .any(|e| e.path == unrelated)
    );
    browser.set_show_hidden(false);
    assert!(browser.visible_entries().is_empty());
    assert!(browser.selected().is_empty());
}

#[test]
fn list_tree_opens_folders_in_place_and_closing_keeps_the_keyboard_on_the_folder() {
    let root = tempfile::tempdir().unwrap();
    let alpha = root.path().join("alpha");
    let nested = alpha.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(alpha.join("inside.txt"), b"x").unwrap();
    fs::write(nested.join("deep.txt"), b"x").unwrap();
    fs::write(root.path().join("zulu.txt"), b"x").unwrap();
    let mut browser = Browser::open(root.path()).unwrap();
    // Columns view has no tree: a listing cannot be opened in place.
    assert!(!browser.expand(Directory::read(&alpha).unwrap()));
    browser
        .set_view(ViewSettings {
            mode: ViewMode::List,
            tree: true,
            ..ViewSettings::default()
        })
        .unwrap();
    assert!(browser.tree_view());
    // Only listed folders open, outermost first.
    assert!(!browser.expand(Directory::read(&nested).unwrap()));
    assert!(browser.expand(Directory::read(&alpha).unwrap()));
    assert!(browser.expand(Directory::read(&nested).unwrap()));
    let rows: Vec<_> = browser
        .visible_entries()
        .iter()
        .map(|entry| entry.path.strip_prefix(root.path()).unwrap().to_owned())
        .collect();
    let expected: Vec<std::path::PathBuf> = [
        "alpha",
        "alpha/nested",
        "alpha/nested/deep.txt",
        "alpha/inside.txt",
        "zulu.txt",
    ]
    .iter()
    .map(Into::into)
    .collect();
    assert_eq!(rows, expected);
    let deep = nested.join("deep.txt");
    assert_eq!(browser.depth(&deep), 2);
    assert_eq!(browser.depth(&alpha), 0);
    assert!(browser.lists(&deep));
    browser.select(&deep, SelectionMode::Replace);
    assert_eq!(browser.focused(), Some(deep.as_path()));
    // Closing the outer folder closes the inner one and keeps the keyboard there.
    browser.collapse(&alpha);
    assert!(!browser.is_expanded(&alpha) && !browser.is_expanded(&nested));
    assert_eq!(browser.focused(), Some(alpha.as_path()));
    assert_eq!(browser.selected(), &[alpha.clone()].into_iter().collect());
    assert_eq!(browser.visible_entries().len(), 2);
    // Leaving List view hides the open folders and their selection.
    assert!(browser.expand(Directory::read(&alpha).unwrap()));
    browser.select(&alpha.join("inside.txt"), SelectionMode::Replace);
    browser
        .set_view(ViewSettings {
            mode: ViewMode::List,
            ..ViewSettings::default()
        })
        .unwrap();
    assert!(!browser.tree_view() && browser.selected().is_empty());
    assert_eq!(browser.visible_entries().len(), 2);
}

#[test]
fn folders_can_sort_among_files_instead_of_first() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("beta")).unwrap();
    fs::write(root.path().join("alpha.txt"), b"x").unwrap();
    fs::write(root.path().join("gamma.txt"), b"x").unwrap();
    let mut browser = Browser::open(root.path()).unwrap();
    let names = |browser: &Browser| -> Vec<String> {
        browser
            .visible_entries()
            .iter()
            .map(|entry| entry.name().into_owned())
            .collect()
    };
    assert_eq!(names(&browser), ["beta", "alpha.txt", "gamma.txt"]);
    browser
        .set_view(ViewSettings {
            folders_first: false,
            ..ViewSettings::default()
        })
        .unwrap();
    assert_eq!(names(&browser), ["alpha.txt", "beta", "gamma.txt"]);
    browser
        .set_view(ViewSettings {
            folders_first: false,
            descending: true,
            ..ViewSettings::default()
        })
        .unwrap();
    assert_eq!(names(&browser), ["gamma.txt", "beta", "alpha.txt"]);
}

#[test]
fn names_sort_by_number_value() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "file10",
        "file2",
        "File1",
        "file02",
        "notes",
        "2024-12.txt",
        "2024-2.txt",
    ] {
        fs::write(root.path().join(name), b"x").unwrap();
    }
    let browser = Browser::open(root.path()).unwrap();
    let names: Vec<String> = browser
        .visible_entries()
        .iter()
        .map(|entry| entry.name().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "2024-2.txt",
            "2024-12.txt",
            "File1",
            "file2",
            "file02",
            "file10",
            "notes"
        ]
    );
    let natural = |name: &str| canopa_files::browse::NaturalName::new(name);
    assert!(natural("file2") < natural("file10"));
    assert!(natural("a99999999999999999999999") < natural("a100000000000000000000000"));
    assert!(natural("Élan2") < natural("élan10"));
}
