use super::*;
use std::fs;

pub(super) fn desktop(root: &Path, paths: Vec<PathBuf>) -> (Desktop, window::Id) {
    let configuration = ConfigurationSnapshot::load(&canopa_config::Request::Defaults).unwrap();
    let preferences = PreferenceStore::load(
        &root.join("views.json"),
        configuration.settings.view.clone(),
    )
    .unwrap();
    let configuration_worker =
        ConfigurationJobs::start(configuration.clone()).map_err(|error| error.to_string());
    let (desktop, _task) = Desktop::boot(
        Some(paths),
        configuration,
        preferences,
        root.to_owned(),
        root.join("views.json"),
        PersonalFolders::conventional(root).unwrap(),
        configuration_worker.map(Some),
    );
    // Unit controllers never send notifications to the developer's session.
    // Native acceptance supplies its own private notification service.
    let id = *desktop.windows.keys().next().unwrap();
    (desktop, id)
}

pub(super) fn loaded(
    desktop: &mut Desktop,
    window: window::Id,
    tab: u64,
    revision: u64,
    path: &Path,
) {
    let _task = desktop.update(Message::Loaded(
        window,
        tab,
        revision,
        Navigation::Visit,
        Arc::new(Ok(Scan {
            directory: Directory::read(path).unwrap(),
            selected: Vec::new(),
            remote: false,
        })),
    ));
}

#[test]
fn directory_results_keep_their_tab_and_reject_superseded_or_closed_requests() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let c = root.path().join("c");
    for path in [&a, &b, &c] {
        fs::create_dir(path).unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), vec![a.clone(), b.clone()]);
    let first = desktop.active(window).unwrap().id;
    let _task = desktop.update(Message::SwitchTab(window, 1));
    let second = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, first, 1, &a);
    assert!(desktop.active(window).unwrap().browser.is_none());
    assert_eq!(
        desktop.windows[&window].tabs[0]
            .browser
            .as_ref()
            .unwrap()
            .path(),
        a
    );
    let _task = desktop.navigate(window, c.clone(), Navigation::Visit, None);
    loaded(&mut desktop, window, second, 1, &b);
    assert!(desktop.active(window).unwrap().browser.is_none());
    loaded(&mut desktop, window, second, 2, &c);
    assert_eq!(
        desktop
            .active(window)
            .unwrap()
            .browser
            .as_ref()
            .unwrap()
            .path(),
        c
    );
    let _task = desktop.command(window, Command::CloseTab);
    loaded(&mut desktop, window, second, 2, &b);
    assert_eq!(desktop.active(window).unwrap().path(), a);
    assert_eq!(desktop.windows[&window].tabs.len(), 1);
}

#[test]
fn moved_tabs_keep_the_shown_tab_and_a_detached_tab_takes_its_history_along() {
    let root = tempfile::tempdir().unwrap();
    let folders: Vec<_> = ["a", "b", "c"]
        .into_iter()
        .map(|name| root.path().join(name))
        .collect();
    for path in &folders {
        fs::create_dir(path).unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), folders.clone());
    let ids: Vec<_> = desktop.windows[&window]
        .tabs
        .iter()
        .map(|tab| tab.id)
        .collect();
    loaded(&mut desktop, window, ids[0], 1, &folders[0]);
    let _task = desktop.update(Message::SwitchTab(window, 1));
    loaded(&mut desktop, window, ids[1], 1, &folders[1]);
    let order = |desktop: &Desktop| -> Vec<u64> {
        desktop.windows[&window]
            .tabs
            .iter()
            .map(|tab| tab.id)
            .collect()
    };
    let _task = desktop.update(Message::MoveTab(window, ids[0], 2));
    assert_eq!(order(&desktop), [ids[1], ids[2], ids[0]]);
    assert_eq!(desktop.active(window).unwrap().id, ids[1]);
    let _task = desktop.command(window, Command::MoveTabRight);
    assert_eq!(order(&desktop), [ids[2], ids[1], ids[0]]);
    assert_eq!(desktop.active(window).unwrap().id, ids[1]);

    let _task = desktop.update(Message::DetachTab(window, ids[1]));
    assert_eq!(order(&desktop), [ids[2], ids[0]]);
    let (&new, detached) = desktop
        .windows
        .iter()
        .find(|(id, _)| **id != window)
        .expect("the tab has its own window");
    assert_eq!(detached.tabs.len(), 1);
    assert_eq!(detached.tabs[0].id, ids[1]);
    assert_eq!(detached.tabs[0].target, folders[1]);
    // Its browser, with its history and selection, came along; it is read again.
    assert!(detached.tabs[0].browser.is_some());
    assert!(desktop.active(new).unwrap().loading);
    // The last tab of a window stays there.
    let _task = desktop.update(Message::DetachTab(new, ids[1]));
    assert_eq!(desktop.windows.len(), 2);

    // Dropped on the first window's bar, it moves back between its tabs and
    // shows there; the window it leaves empty closes.
    let _task = desktop.update(Message::TransferTab {
        from: new,
        tab: ids[1],
        to: window,
        index: 1,
    });
    assert_eq!(order(&desktop), [ids[2], ids[1], ids[0]]);
    assert_eq!(desktop.active(window).unwrap().id, ids[1]);
    assert!(desktop.active(window).unwrap().loading);
    assert!(!desktop.windows.contains_key(&new));
}

#[test]
fn asynchronous_open_errors_stay_with_the_originating_tab() {
    let root = tempfile::tempdir().unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned(); 2]);
    let origin = desktop.active(window).unwrap().id;
    let _task = desktop.update(Message::SwitchTab(window, 1));
    let _task = desktop.update(Message::Error(
        window,
        origin,
        "fixture launch failed".into(),
    ));
    assert!(desktop.active(window).unwrap().error.is_none());
    assert_eq!(
        desktop.windows[&window].tabs[0].error.as_deref(),
        Some("fixture launch failed")
    );
}

#[test]
fn dismissing_location_editing_restores_browsing_without_committing_the_text() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("notes.txt"), b"fixture").unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, root.path());
    let _task = desktop.command(window, Command::Location);
    let _task = desktop.update(Message::Location(window, "/uncommitted".into()));
    let _task = desktop.key(
        window,
        keyboard::Key::Named(keyboard::key::Named::Escape),
        keyboard::Modifiers::default(),
        None,
    );
    let _task = desktop.key(
        window,
        keyboard::Key::Named(keyboard::key::Named::ArrowDown),
        keyboard::Modifiers::default(),
        None,
    );
    let tab = desktop.active(window).unwrap();
    assert!(tab.location.is_none());
    assert_eq!(tab.path(), root.path());
    assert_eq!(
        tab.browser.as_ref().unwrap().focused(),
        Some(root.path().join("notes.txt").as_path())
    );
}

#[test]
fn keyboard_focus_can_move_without_changing_selection_and_extends_from_the_anchor() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c"] {
        fs::write(root.path().join(name), b"fixture").unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, root.path());
    let browser = desktop
        .active_mut(window)
        .unwrap()
        .browser
        .as_mut()
        .unwrap();
    browser.select(&root.path().join("a"), SelectionMode::Replace);
    let _task = desktop.key(
        window,
        keyboard::Key::Named(keyboard::key::Named::ArrowDown),
        keyboard::Modifiers::CTRL,
        None,
    );
    let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
    assert_eq!(browser.focused(), Some(root.path().join("b").as_path()));
    assert_eq!(
        browser.selected(),
        &[root.path().join("a")].into_iter().collect()
    );
    let _task = desktop.key(
        window,
        keyboard::Key::Named(keyboard::key::Named::ArrowDown),
        keyboard::Modifiers::SHIFT,
        None,
    );
    let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
    assert_eq!(browser.selected().len(), 3);
    assert_eq!(browser.focused(), Some(root.path().join("c").as_path()));
}

#[test]
fn location_editor_round_trips_raw_names_and_resolves_relative_to_the_folder() {
    let home = Path::new("/fixture/home");
    let base = home.join("Downloads");
    let raw = base.join(std::ffi::OsString::from_vec(b"raw-\xff #%.txt".to_vec()));
    let display = display_location(&raw);
    assert!(display.starts_with("file:///fixture/"));
    assert_eq!(
        local_path(Path::new(&display), home, Some(&base)).unwrap(),
        raw
    );
    assert_eq!(
        local_path(Path::new("child"), home, Some(&base)).unwrap(),
        base.join("child")
    );
    assert_eq!(
        local_path(Path::new("~/Pictures"), home, Some(&base)).unwrap(),
        home.join("Pictures")
    );
    assert_eq!(
        local_path(
            Path::new("file://localhost/fixture/a%20b"),
            home,
            Some(&base)
        )
        .unwrap(),
        Path::new("/fixture/a b")
    );
    for invalid in [
        "file://server/fixture",
        "file:///fixture/%00",
        "file:///fixture/%xz",
        "file:///fixture/%",
        "file:///fixture?a",
        "file:///fixture#a",
    ] {
        assert!(
            local_path(Path::new(invalid), home, Some(&base)).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn hidden_columns_cancel_and_old_results_cannot_replace_a_new_request() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("a/b/c");
    fs::create_dir_all(&path).unwrap();
    let notes = path.join("notes.txt");
    fs::write(&notes, b"fixture").unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![path.clone()]);
    let tab_id = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab_id, 1, &path);
    // The requested startup size is not the compositor's final tile size.
    assert!(desktop.active(window).unwrap().columns.is_empty());
    let _task = desktop.update(Message::Resized(window, Size::new(360.0, 800.0)));
    assert!(desktop.active(window).unwrap().columns.is_empty());
    let browser = desktop
        .active_mut(window)
        .unwrap()
        .browser
        .as_mut()
        .unwrap();
    browser.select(&notes, SelectionMode::Replace);
    browser.set_scroll(120.0);
    let _task = desktop.update(Message::Resized(window, Size::new(1280.0, 800.0)));
    let parent = path.parent().unwrap().to_owned();
    let column = &desktop.active(window).unwrap().columns[&parent];
    let old_request = column.request;
    let cancellation = column.cancellation.clone();
    let _task = desktop.update(Message::Resized(window, Size::new(360.0, 800.0)));
    assert!(cancellation.is_cancelled());
    assert!(desktop.active(window).unwrap().columns.is_empty());
    let _task = desktop.update(Message::Resized(window, Size::new(1280.0, 800.0)));
    let request = desktop.active(window).unwrap().columns[&parent].request;
    assert_ne!(old_request, request);
    desktop.finish_column(
        window,
        tab_id,
        1,
        parent.clone(),
        old_request,
        Arc::new(Err("stale fixture".into())),
    );
    assert!(
        desktop.active(window).unwrap().columns[&parent]
            .result
            .is_none()
    );
    desktop.finish_column(
        window,
        tab_id,
        1,
        parent.clone(),
        request,
        Arc::new(Directory::read(&parent).map_err(|error| error.to_string())),
    );
    let tab = desktop.active(window).unwrap();
    assert!(matches!(tab.columns[&parent].result, Some(Ok(_))));
    assert_eq!(
        tab.browser.as_ref().unwrap().focused(),
        Some(notes.as_path())
    );
    assert_eq!(tab.browser.as_ref().unwrap().scroll(), 120.0);
    let scan_cancel = tab.scan_cancel.clone();
    let column_cancel = tab.columns[&parent].cancellation.clone();
    let _task = desktop.command(window, Command::NewTab);
    assert!(column_cancel.is_cancelled());
    assert!(desktop.windows[&window].tabs[0].columns.is_empty());
    let _task = desktop.update(Message::Close(window));
    assert!(scan_cancel.is_cancelled());
    assert!(column_cancel.is_cancelled());
}

#[test]
fn viewport_growth_reconciles_visible_rows_without_incompatible_widget_state() {
    use iced::advanced::widget::Tree;
    let root = tempfile::tempdir().unwrap();
    for index in 0..80 {
        fs::write(root.path().join(format!("file-{index:03}")), b"fixture").unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab_id = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab_id, 1, root.path());
    let mut tree = {
        let element = desktop.view(window);
        Tree::new(element.as_widget())
    };
    let epoch = desktop.active(window).unwrap().list_epoch;
    let _task = desktop.update(Message::Viewport(
        window,
        tab_id,
        epoch,
        root.path().to_owned(),
        Size::new(1000.0, 1200.0),
    ));
    let element = desktop.view(window);
    tree.diff(element.as_widget());
    let expected = Tree::new(element.as_widget());
    fn same_shape(actual: &Tree, expected: &Tree) {
        assert_eq!(
            actual.tag, expected.tag,
            "reconciled row retained another widget type"
        );
        assert_eq!(actual.children.len(), expected.children.len());
        for (actual, expected) in actual.children.iter().zip(&expected.children) {
            same_shape(actual, expected);
        }
    }
    same_shape(&tree, &expected);
}

fn reflow_scroll_feedback(viewport_first: bool) {
    let root = tempfile::tempdir().unwrap();
    for index in 0..2000 {
        fs::write(root.path().join(format!("item-{index:03}.txt")), b"x").unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab_id = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab_id, 1, root.path());
    let rendered = Size::new(360.0, 413.7);
    let measured = Size::new(1024.0, 449.3);
    let appearance = desktop.configuration.appearance.clone();
    let icon = desktop.icon_size();
    let tab = desktop.active_mut(window).unwrap();
    tab.viewport_width = rendered.width;
    tab.viewport_height = rendered.height;
    let browser = tab.browser.as_mut().unwrap();
    browser
        .set_view(crate::preferences::ViewSettings {
            mode: crate::preferences::ViewMode::Icons,
            ..Default::default()
        })
        .unwrap();
    browser.select(&root.path().join("item-159.txt"), SelectionMode::Replace);
    listing_ui::Geometry::new(&appearance, browser.view(), false, rendered.width, icon)
        .reveal(browser, rendered.height);
    let old_scroll = browser.scroll();
    let geometry =
        listing_ui::Geometry::new(&appearance, browser.view(), false, measured.width, icon);
    // Iced reports the old native offset clamped to the new grid extent. Its
    // producer still captured the narrow model, as in the native resize trace.
    let clamped = old_scroll.min(
        (browser.visible_entries().len().div_ceil(geometry.columns) as f32 * geometry.height
            - measured.height)
            .max(0.0),
    );
    let epoch = tab.list_epoch;
    let feedback = Message::Scroll(
        window,
        tab_id,
        epoch,
        root.path().to_owned(),
        ScrollUpdate {
            offset: clamped,
            rendered,
            measured,
        },
    );
    if !viewport_first {
        let _ = desktop.update(feedback.clone());
        assert_eq!(
            desktop
                .active(window)
                .unwrap()
                .browser
                .as_ref()
                .unwrap()
                .scroll(),
            old_scroll,
            "a resized native viewport cannot replace the old logical anchor"
        );
    }
    let _ = desktop.update(Message::Viewport(
        window,
        tab_id,
        epoch,
        root.path().to_owned(),
        measured,
    ));
    let remapped = desktop
        .active(window)
        .unwrap()
        .browser
        .as_ref()
        .unwrap()
        .scroll();
    if viewport_first {
        let _ = desktop.update(feedback);
        assert_eq!(
            desktop
                .active(window)
                .unwrap()
                .browser
                .as_ref()
                .unwrap()
                .scroll(),
            remapped,
            "feedback from the old model cannot overwrite the reflowed anchor"
        );
    }
    // A delayed Browser resize adds the sidebar, causing a second grid reflow.
    let _ = desktop.update(Message::Resized(window, Size::new(1024.0, 604.8)));
    let _ = desktop.update(Message::Viewport(
        window,
        tab_id,
        epoch,
        root.path().to_owned(),
        Size::new(863.0, 449.3),
    ));
    let tab = desktop.active(window).unwrap();
    let browser = tab.browser.as_ref().unwrap();
    let index = browser
        .visible_entries()
        .iter()
        .position(|entry| Some(entry.path.as_path()) == browser.focused())
        .unwrap();
    assert!(listing_ui::geometry(&desktop, tab).visible(
        index,
        browser.scroll(),
        tab.viewport_height
    ));
}

#[test]
fn scroll_feedback_before_viewport_resize_preserves_the_icon_anchor() {
    reflow_scroll_feedback(false);
}

#[test]
fn scroll_feedback_after_viewport_resize_preserves_the_icon_anchor() {
    reflow_scroll_feedback(true);
}

#[test]
fn settled_viewport_feedback_preserves_deliberate_scrolling_away_from_focus() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..50 {
        fs::write(root.path().join(format!("item-{index:03}")), b"x").unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab_id = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab_id, 1, root.path());
    let size = Size::new(360.0, 400.0);
    let tab = desktop.active_mut(window).unwrap();
    tab.viewport_width = size.width;
    tab.viewport_height = size.height;
    let epoch = tab.list_epoch;
    tab.browser
        .as_mut()
        .unwrap()
        .select(&root.path().join("item-049"), SelectionMode::Replace);
    for offset in [480.0, 0.0] {
        let _ = desktop.update(Message::Scroll(
            window,
            tab_id,
            epoch,
            root.path().to_owned(),
            ScrollUpdate {
                offset,
                rendered: size,
                measured: size,
            },
        ));
        assert_eq!(
            desktop
                .active(window)
                .unwrap()
                .browser
                .as_ref()
                .unwrap()
                .scroll(),
            offset
        );
    }
}

#[test]
fn shared_backend_activity_prevents_last_window_exit_until_retirement() {
    let root = tempfile::tempdir().unwrap();
    let (mut desktop, _) = desktop(root.path(), vec![]);
    desktop.windows.clear();
    let activity = crate::local_activity::LocalActivity::default();
    desktop.activity_notices = activity.take_notices().map(activation_ui::Notices::new);
    desktop.activity = Some(activity.clone());
    assert_eq!(desktop.exit_if_empty().units(), 1);
    let removal = activity.try_remove().unwrap();
    assert_eq!(desktop.exit_if_empty().units(), 0);
    assert_eq!(desktop.update(Message::ActivityPoll).units(), 0);
    drop(removal);
    assert_eq!(desktop.update(Message::ActivityPoll).units(), 1);
}

#[test]
fn requested_quick_look_waits_for_source_size_and_never_previews_a_neighbor() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("a.txt");
    fs::write(&path, "fixture").unwrap();
    for exists in [true, false] {
        let requested = if exists {
            path.clone()
        } else {
            root.path().join("missing.txt")
        };
        let (mut desktop, _) = desktop(root.path(), vec![root.path().to_owned()]);
        let old: std::collections::HashSet<_> = desktop.windows.keys().copied().collect();
        let _task = desktop.new_target_window_with_preview(
            vec![crate::presentation::BrowserTarget {
                directory: root.path().to_owned(),
                selected: vec![requested.clone()],
            }],
            true,
        );
        let source = *desktop.windows.keys().find(|id| !old.contains(id)).unwrap();
        let tab = desktop.active(source).unwrap().id;
        let _task = desktop.update(Message::Loaded(
            source,
            tab,
            1,
            Navigation::Visit,
            Arc::new(Ok(Scan {
                directory: Directory::read(root.path()).unwrap(),
                selected: vec![requested],
                remote: false,
            })),
        ));
        assert!(desktop.previews.is_empty());
        let _task = desktop.update(Message::Resized(source, Size::new(1000.0, 640.0)));
        if exists {
            assert_eq!(desktop.previews.len(), 1);
            assert_eq!(desktop.previews.values().next().unwrap().source, source);
        } else {
            assert!(desktop.previews.is_empty());
            assert!(
                desktop
                    .active(source)
                    .unwrap()
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("unavailable")
            );
        }
    }
}

#[test]
fn a_folder_clicked_in_a_parent_column_opens_and_a_file_there_is_selected() {
    let root = tempfile::tempdir().unwrap();
    let current = root.path().join("current");
    let sibling = root.path().join("sibling");
    let note = root.path().join("note.txt");
    for folder in [&current, &sibling] {
        fs::create_dir(folder).unwrap();
    }
    fs::write(&note, b"x").unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![current.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &current);
    // The parent column's listing, as its worker would install it.
    desktop.active_mut(window).unwrap().columns.insert(
        root.path().to_owned(),
        super::loading::ColumnSnapshot {
            request: 1,
            cancellation: Cancellation::default(),
            result: Some(Ok(Directory::read(root.path()).unwrap())),
            scroll: 0.0,
        },
    );
    let _task = desktop.update(Message::Select(window, sibling.clone()));
    assert_eq!(desktop.active(window).unwrap().target, sibling);
    let tab = desktop.active_mut(window).unwrap();
    tab.columns.insert(
        root.path().to_owned(),
        super::loading::ColumnSnapshot {
            request: 2,
            cancellation: Cancellation::default(),
            result: Some(Ok(Directory::read(root.path()).unwrap())),
            scroll: 0.0,
        },
    );
    let _task = desktop.update(Message::Select(window, note.clone()));
    assert_eq!(desktop.active(window).unwrap().target, root.path());
}
