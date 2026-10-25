use super::tests::{desktop, loaded};
use super::*;
use crate::search::{SearchRequest, SearchResults, SearchScope};
use search_ui::{Event, Target, Ticket};
use std::fs;

fn ready_desktop(root: &Path) -> (Desktop, window::Id) {
    let (mut desktop, window) = desktop(root, vec![root.to_owned()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, root);
    (desktop, window)
}

#[test]
fn empty_search_does_not_reposition_the_underlying_folder_in_any_view() {
    use crate::preferences::{ViewMode, ViewSettings};
    for mode in [ViewMode::Columns, ViewMode::List, ViewMode::Icons] {
        let root = tempfile::tempdir().unwrap();
        for index in 0..100 {
            fs::write(root.path().join(format!("item-{index:03}.txt")), b"x").unwrap();
        }
        let (mut desktop, window) = ready_desktop(root.path());
        let _ = desktop.update(Message::Resized(window, Size::new(360.0, 800.0)));
        desktop
            .active_mut(window)
            .unwrap()
            .browser
            .as_mut()
            .unwrap()
            .set_view(ViewSettings {
                mode,
                ..Default::default()
            })
            .unwrap();
        let tab = desktop.active(window).unwrap().id;
        let epoch = desktop.active(window).unwrap().list_epoch;
        let _ = desktop.update(Message::Viewport(
            window,
            tab,
            epoch,
            root.path().to_owned(),
            Size::new(360.0, 600.0),
        ));
        desktop
            .active_mut(window)
            .unwrap()
            .browser
            .as_mut()
            .unwrap()
            .select(&root.path().join("item-050.txt"), SelectionMode::Replace);
        let _ = desktop.reveal_focus(window);
        let original = desktop
            .active(window)
            .unwrap()
            .browser
            .as_ref()
            .unwrap()
            .scroll();
        assert!(original > 0.0);
        let _ = desktop.open_search(window);
        let epoch = desktop.active(window).unwrap().list_epoch;
        let _ = desktop.update(Message::Viewport(
            window,
            tab,
            epoch,
            root.path().to_owned(),
            Size::new(360.0, 500.0),
        ));
        assert_eq!(
            desktop
                .active(window)
                .unwrap()
                .browser
                .as_ref()
                .unwrap()
                .scroll(),
            original,
            "empty Search changed {mode:?} before its first result"
        );
        let _ = desktop.close_search(window);
        let epoch = desktop.active(window).unwrap().list_epoch;
        let _ = desktop.update(Message::Viewport(
            window,
            tab,
            epoch,
            root.path().to_owned(),
            Size::new(360.0, 600.0),
        ));
        let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
        assert_eq!(
            browser.scroll(),
            original,
            "closing Search changed {mode:?}"
        );
        assert_eq!(
            browser.focused(),
            Some(root.path().join("item-050.txt").as_path())
        );
    }
}

fn target(desktop: &Desktop, window: window::Id) -> Target {
    let tab = desktop.active(window).unwrap();
    Target {
        window,
        tab: tab.id,
        instance: tab.search.as_ref().unwrap().instance,
    }
}

fn ticket(desktop: &Desktop, window: window::Id) -> Ticket {
    let tab = desktop.active(window).unwrap();
    Ticket {
        target: target(desktop, window),
        navigation: tab.revision,
        serial: tab.search_serial,
    }
}

fn query(desktop: &mut Desktop, window: window::Id, value: &str) -> Ticket {
    let _task = desktop.command(window, Command::Search);
    let target = target(desktop, window);
    let _task = desktop.search_event(Event::Query(target, value.into()));
    ticket(desktop, window)
}

fn deliver(desktop: &mut Desktop, ticket: Ticket, results: SearchResults, finished: bool) {
    let _task = desktop.search_event(Event::Ready(ticket, Arc::new(results), finished));
}

#[test]
fn closing_search_restores_folder_state_and_rejects_late_list_events() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("notes.txt");
    let matched = root.path().join("coast.txt");
    fs::write(&original, b"coast only in the contents").unwrap();
    fs::write(&matched, b"fixture").unwrap();
    for index in 0..40 {
        fs::write(root.path().join(format!("other-{index}")), b"fixture").unwrap();
    }
    let (mut desktop, window) = ready_desktop(root.path());
    let browser = desktop
        .active_mut(window)
        .unwrap()
        .browser
        .as_mut()
        .unwrap();
    browser.select(&original, SelectionMode::Replace);
    browser.set_scroll(480.0);
    let ticket = query(&mut desktop, window, "coast");
    let results = SearchRequest::new(root.path(), "coast", SearchScope::CurrentFolder)
        .run(&Cancellation::default());
    assert_eq!(results.entries.len(), 1);
    deliver(&mut desktop, ticket, results.clone(), true);
    let _task = desktop.update(Message::Select(window, matched.clone()));
    let epoch = desktop.active(window).unwrap().list_epoch;
    let rendered = {
        let tab = desktop.active(window).unwrap();
        Size::new(tab.viewport_width, tab.viewport_height)
    };
    let _task = desktop.update(Message::Scroll(
        window,
        ticket.target.tab,
        epoch,
        root.path().to_owned(),
        ScrollUpdate {
            offset: 100.0,
            rendered,
            measured: rendered,
        },
    ));
    let _task = desktop.key(
        window,
        keyboard::Key::Named(keyboard::key::Named::Escape),
        keyboard::Modifiers::default(),
        None,
    );
    let _task = desktop.update(Message::Scroll(
        window,
        ticket.target.tab,
        epoch,
        root.path().to_owned(),
        ScrollUpdate {
            offset: 0.0,
            rendered,
            measured: rendered,
        },
    ));
    let _task = desktop.update(Message::Viewport(
        window,
        ticket.target.tab,
        epoch,
        root.path().to_owned(),
        Size::new(360.0, 10.0),
    ));
    deliver(&mut desktop, ticket, results, false);
    let tab = desktop.active(window).unwrap();
    let browser = tab.browser.as_ref().unwrap();
    assert!(tab.search.is_none());
    assert!(browser.search().is_none());
    assert_eq!(browser.focused(), Some(original.as_path()));
    assert_eq!(browser.selected(), &[original].into_iter().collect());
    assert_eq!(browser.scroll(), 480.0);
    assert_ne!(tab.viewport_height, 10.0);
}

#[test]
fn superseded_results_and_old_controls_cannot_retarget_a_reopened_search() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt"] {
        fs::write(root.path().join(name), b"fixture").unwrap();
    }
    let (mut desktop, window) = ready_desktop(root.path());
    let first = query(&mut desktop, window, "a.txt");
    let first_cancel = desktop
        .active(window)
        .unwrap()
        .search
        .as_ref()
        .unwrap()
        .cancellation
        .clone();
    let _task = desktop.search_event(Event::Query(first.target, "b.txt".into()));
    let second = ticket(&desktop, window);
    assert!(first_cancel.is_cancelled());
    deliver(
        &mut desktop,
        first,
        SearchRequest::new(root.path(), "a.txt", SearchScope::CurrentFolder)
            .run(&Cancellation::default()),
        true,
    );
    assert!(
        desktop
            .active(window)
            .unwrap()
            .browser
            .as_ref()
            .unwrap()
            .search()
            .unwrap()
            .entries
            .is_empty()
    );
    deliver(
        &mut desktop,
        second,
        SearchRequest::new(root.path(), "b.txt", SearchScope::CurrentFolder)
            .run(&Cancellation::default()),
        true,
    );
    assert_eq!(
        desktop
            .active(window)
            .unwrap()
            .browser
            .as_ref()
            .unwrap()
            .visible_entries()[0]
            .path,
        root.path().join("b.txt")
    );
    let _task = desktop.close_search(window);
    let _task = desktop.open_search(window);
    assert_ne!(target(&desktop, window).instance, first.target.instance);
    let _task = desktop.search_event(Event::Query(first.target, "stale control".into()));
    deliver(
        &mut desktop,
        second,
        SearchRequest::new(root.path(), "b.txt", SearchScope::CurrentFolder)
            .run(&Cancellation::default()),
        true,
    );
    assert_eq!(
        desktop
            .active(window)
            .unwrap()
            .search
            .as_ref()
            .unwrap()
            .query,
        ""
    );
    assert!(
        desktop
            .active(window)
            .unwrap()
            .browser
            .as_ref()
            .unwrap()
            .search()
            .is_none()
    );
}

#[test]
fn recursive_pointer_selection_and_folder_entry_use_the_actual_result_path() {
    let root = tempfile::tempdir().unwrap();
    let nested = root.path().join("nested");
    fs::create_dir(&nested).unwrap();
    let file = nested.join(std::ffi::OsString::from_vec(b"needle-\xff.txt".to_vec()));
    let folder = nested.join("needle-folder");
    fs::create_dir(&folder).unwrap();
    fs::write(&file, b"fixture").unwrap();
    let (mut desktop, window) = ready_desktop(root.path());
    let _first = query(&mut desktop, window, "needle");
    let _task = desktop.command(window, Command::SearchSubfolders);
    let ticket = ticket(&desktop, window);
    deliver(
        &mut desktop,
        ticket,
        SearchRequest::new(root.path(), "needle", SearchScope::Recursive)
            .run(&Cancellation::default()),
        true,
    );
    let _task = desktop.update(Message::Select(window, file.clone()));
    assert_eq!(desktop.active(window).unwrap().path(), root.path());
    assert_eq!(
        desktop
            .active(window)
            .unwrap()
            .browser
            .as_ref()
            .unwrap()
            .focused(),
        Some(file.as_path())
    );
    let _task = desktop.update(Message::Select(window, folder.clone()));
    let _task = desktop.command(window, Command::EnterFolder);
    let tab = desktop.active(window).unwrap();
    assert_eq!(tab.target, folder);
    assert!(tab.loading);
    assert!(tab.search.is_none());
}

#[test]
fn reveal_result_targets_its_containing_folder_with_raw_path_bytes() {
    let root = tempfile::tempdir().unwrap();
    let nested = root
        .path()
        .join(std::ffi::OsString::from_vec(b"nested-\xfe".to_vec()));
    fs::create_dir(&nested).unwrap();
    let file = nested.join("needle.txt");
    fs::write(&file, b"fixture").unwrap();
    let (mut desktop, window) = ready_desktop(root.path());
    let _first = query(&mut desktop, window, "needle");
    let _task = desktop.command(window, Command::SearchSubfolders);
    let ticket = ticket(&desktop, window);
    deliver(
        &mut desktop,
        ticket,
        SearchRequest::new(root.path(), "needle", SearchScope::Recursive)
            .run(&Cancellation::default()),
        true,
    );
    let _task = desktop.update(Message::Select(window, file));
    let _task = desktop.command(window, Command::RevealSearchResult);
    assert_eq!(desktop.active(window).unwrap().target, nested);
    assert!(desktop.active(window).unwrap().search.is_none());
}

#[test]
fn inactive_tab_delivery_stays_with_its_owner_and_closing_cancels_it() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("needle.txt"), b"fixture").unwrap();
    let (mut desktop, window) = ready_desktop(root.path());
    let first = query(&mut desktop, window, "needle");
    let _task = desktop.command(window, Command::NewTab);
    let second_id = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, second_id, 1, root.path());
    let second = query(&mut desktop, window, "other");
    let cancel = desktop
        .active(window)
        .unwrap()
        .search
        .as_ref()
        .unwrap()
        .cancellation
        .clone();
    deliver(
        &mut desktop,
        first,
        SearchRequest::new(root.path(), "needle", SearchScope::CurrentFolder)
            .run(&Cancellation::default()),
        true,
    );
    assert_eq!(
        desktop
            .active(window)
            .unwrap()
            .browser
            .as_ref()
            .unwrap()
            .search()
            .unwrap()
            .query,
        "other"
    );
    assert_eq!(
        desktop.windows[&window].tabs[0]
            .browser
            .as_ref()
            .unwrap()
            .search()
            .unwrap()
            .entries
            .len(),
        1
    );
    let _task = desktop.command(window, Command::CloseTab);
    assert!(cancel.is_cancelled());
    deliver(
        &mut desktop,
        second,
        SearchRequest::new(root.path(), "other", SearchScope::CurrentFolder)
            .run(&Cancellation::default()),
        true,
    );
    assert_eq!(desktop.active(window).unwrap().id, first.target.tab);
    assert_eq!(
        desktop
            .active(window)
            .unwrap()
            .browser
            .as_ref()
            .unwrap()
            .search()
            .unwrap()
            .query,
        "needle"
    );
}

#[test]
fn cancelling_a_larger_budget_keeps_prior_results_and_selection() {
    let root = tempfile::tempdir().unwrap();
    for name in ["match-a", "match-b", "match-c"] {
        fs::write(root.path().join(name), b"fixture").unwrap();
    }
    let (mut desktop, window) = ready_desktop(root.path());
    let _first = query(&mut desktop, window, "match");
    assert_eq!(
        desktop
            .active(window)
            .unwrap()
            .search
            .as_ref()
            .unwrap()
            .maximum_results,
        50_000
    );
    desktop
        .active_mut(window)
        .unwrap()
        .search
        .as_mut()
        .unwrap()
        .maximum_results = 2;
    let _task = desktop.restart_search(window);
    let initial = ticket(&desktop, window);
    let mut request = SearchRequest::new(root.path(), "match", SearchScope::CurrentFolder);
    request.maximum_results = 2;
    let results = request.run(&Cancellation::default());
    assert!(results.truncated);
    assert_eq!(results.entries.len(), 2);
    deliver(&mut desktop, initial, results, true);
    let _task = desktop.search_event(Event::FocusResults(initial.target));
    let focused = desktop
        .active(window)
        .unwrap()
        .browser
        .as_ref()
        .unwrap()
        .focused()
        .unwrap()
        .to_owned();
    let _task = desktop.command(window, Command::MoreSearchResults);
    let more = ticket(&desktop, window);
    assert_eq!(
        desktop
            .active(window)
            .unwrap()
            .search
            .as_ref()
            .unwrap()
            .maximum_results,
        50_002
    );
    let _task = desktop.command(window, Command::StopSearch);
    let mut partial = request.run(&Cancellation::default());
    partial.entries.truncate(1);
    partial.cancelled = true;
    partial.truncated = false;
    deliver(&mut desktop, more, partial, true);
    let tab = desktop.active(window).unwrap();
    let browser = tab.browser.as_ref().unwrap();
    assert!(!tab.search.as_ref().unwrap().running);
    assert!(browser.search().unwrap().cancelled);
    assert!(browser.search().unwrap().truncated);
    assert_eq!(browser.visible_entries().len(), 2);
    assert_eq!(browser.focused(), Some(focused.as_path()));
}

#[test]
fn large_result_snapshots_build_bounded_widgets_and_reconcile_back_to_the_folder() {
    use iced::advanced::widget::Tree;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file-template");
    fs::write(&file, b"fixture").unwrap();
    let (mut desktop, window) = ready_desktop(root.path());
    let mut tree = {
        let element = desktop.view(window);
        Tree::new(element.as_widget())
    };
    let ticket = query(&mut desktop, window, "file");
    let template = crate::browse::Entry::read(file).unwrap();
    let entries = (0..50_001)
        .map(|index| {
            let mut entry = template.clone();
            entry.path = root.path().join(format!("file-{index:05}"));
            entry
        })
        .collect();
    deliver(
        &mut desktop,
        ticket,
        SearchResults {
            root: root.path().to_owned(),
            query: "file".into(),
            scope: SearchScope::CurrentFolder,
            entries,
            failures: vec![crate::browse::ReadFailure {
                path: root.path().join("unreadable"),
                message: "fixture denied".into(),
            }],
            cancelled: false,
            truncated: false,
            scanned: 50_001,
        },
        true,
    );
    let _task = desktop.command(window, Command::SearchProblems);
    fn count(tree: &Tree) -> usize {
        1 + tree.children.iter().map(count).sum::<usize>()
    }
    fn shape(actual: &Tree, expected: &Tree) {
        assert_eq!(actual.tag, expected.tag);
        assert_eq!(actual.children.len(), expected.children.len());
        for (a, b) in actual.children.iter().zip(&expected.children) {
            shape(a, b);
        }
    }
    {
        let element = desktop.view(window);
        tree.diff(element.as_widget());
        let expected = Tree::new(element.as_widget());
        shape(&tree, &expected);
        assert!(
            count(&tree) < 1500,
            "result widgets must be bounded by the viewport"
        );
    }
    let _task = desktop.close_search(window);
    let element = desktop.view(window);
    tree.diff(element.as_widget());
    shape(&tree, &Tree::new(element.as_widget()));
}
