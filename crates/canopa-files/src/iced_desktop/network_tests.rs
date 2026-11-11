//! Network locations through the Desktop model, without a compositor, GIO
//! helper or server: address entry, remote-folder policies and the sidebar.
use super::tests::{desktop, loaded};
use super::*;
use crate::local_volumes::{DeviceSnapshot, DeviceStatus};
use std::fs;

fn load_remote(desktop: &mut Desktop, window: window::Id, tab: u64, revision: u64, path: &Path) {
    let _task = desktop.update(Message::Loaded(
        window,
        tab,
        revision,
        Navigation::Visit,
        Arc::new(Ok(Scan {
            directory: Directory::read(path).unwrap(),
            selected: Vec::new(),
            remote: true,
        })),
    ));
}

#[test]
fn typed_server_addresses_connect_instead_of_browsing_a_local_path() {
    let root = tempfile::tempdir().unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, root.path());
    let revision = desktop.active(window).unwrap().revision;
    for (input, expected) in [
        ("sftp://me:secret@box/", "Don't include a password"),
        ("gopher://host/", "cannot connect to gopher"),
        // Without a device runtime the request itself is refused.
        ("smb://nas/share", "Network locations are unavailable"),
    ] {
        let _ = desktop.update(Message::Location(window, input.into()));
        let _ = desktop.update(Message::SubmitLocation(window));
        let current = desktop.active(window).unwrap();
        assert_eq!(current.revision, revision, "{input} did not navigate");
        assert!(!current.loading);
        let error = current.error.as_deref().unwrap_or_default();
        assert!(error.contains(expected), "{input}: {error}");
        assert!(!error.contains("secret"), "no password is echoed");
    }
    // Local paths and file:// URIs still navigate.
    let _ = desktop.update(Message::Location(
        window,
        format!("file://{}", root.path().display()),
    ));
    let _ = desktop.update(Message::SubmitLocation(window));
    assert_eq!(desktop.active(window).unwrap().revision, revision + 1);
    assert!(network_location("/home/me").is_none());
    assert!(network_location("file:///home/me").is_none());
    assert!(network_location("ssh://me@box").unwrap().is_ok());
}

#[test]
fn remote_folders_are_polled_instead_of_watched() {
    let root = tempfile::tempdir().unwrap();
    let local = root.path().join("local");
    let share = root.path().join("gvfs/smb-share:server=nas,share=photos");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&share).unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![local.clone()]);
    desktop.network.fuse_root = Some(root.path().join("gvfs"));
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &local);
    assert!(desktop.watched_folders().contains(&local));
    assert!(!desktop.remote_polled());

    let _ = desktop.navigate(window, share.clone(), Navigation::Visit, None);
    let revision = desktop.active(window).unwrap().revision;
    load_remote(&mut desktop, window, tab, revision, &share);
    let current = desktop.active(window).unwrap();
    assert!(current.remote);
    assert!(desktop.remote_polled());
    assert!(
        !desktop.watched_folders().contains(&share),
        "inotify cannot see changes made on the server"
    );
    assert!(desktop.remote_path(&share.join("2024")));
    assert!(!desktop.remote_path(&root.path().join("gvfs")));
    assert!(!desktop.remote_path(&local));

    // The low-rate poll rescans the visible remote folder, keeping its state.
    let _ = desktop.update(Message::Network(network_ui::Event::PollRemote));
    let current = desktop.active(window).unwrap();
    assert!(current.watch_scanning);
    assert_eq!(current.revision, revision, "a poll is not a navigation");
    // A second tick while that rescan runs does not start another.
    let request = current.watch_request;
    let _ = desktop.update(Message::Network(network_ui::Event::PollRemote));
    assert_eq!(desktop.active(window).unwrap().watch_request, request);

    // F5 still re-reads it as before.
    let _ = desktop.command(window, Command::Refresh);
    assert!(desktop.active(window).unwrap().loading);
}

#[test]
fn the_sidebar_lists_network_mounts_servers_and_devices_by_kind() {
    use crate::local_volumes::LocationKind;
    let root = tempfile::tempdir().unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, root.path());
    let _ = desktop.update(Message::Network(network_ui::Event::BookmarksLoaded(
        Arc::new(Ok(canopa_file::gtk_bookmarks::parse(&format!(
            "smb://nas/photos Photos\nfile://{} Here\nfile:///srv/projects Projects\n",
            root.path().display()
        )))),
    )));
    let model = network_ui::sidebar(&desktop.devices.catalog, &desktop.network.bookmarks);
    assert_eq!(
        model.servers,
        vec![("Photos".to_owned(), "smb://nas/photos".to_owned())]
    );
    assert!(model.mounted.is_empty() && model.devices.is_empty());
    let configured: Vec<_> = desktop
        .configuration
        .settings
        .places
        .iter()
        .filter_map(|place| desktop.place_path(place))
        .collect();
    let local = network_ui::local_bookmarks(&desktop.network.bookmarks, &configured);
    assert!(local.contains(&("Projects".to_owned(), PathBuf::from("/srv/projects"))));
    // The view renders every section, including Other Locations.
    let _ = desktop.view(window);
    let catalog = DeviceSnapshot {
        status: DeviceStatus::Ready,
        ..DeviceSnapshot::default()
    };
    let _ = network_ui::sidebar(&catalog, &[]);
    assert_eq!(
        network_ui::kind_icon(LocationKind::Phone),
        chrome::Icon::Phone
    );

    // Removing a server bookmark updates the sidebar at once.
    let _ = desktop.update(Message::Network(network_ui::Event::RemoveBookmark(
        "smb://nas/photos".into(),
    )));
    assert!(
        network_ui::sidebar(&desktop.devices.catalog, &desktop.network.bookmarks)
            .servers
            .is_empty()
    );
}

#[test]
fn the_connect_window_validates_before_mounting_and_is_one_per_session() {
    let root = tempfile::tempdir().unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, root.path());
    let _ = desktop.command(window, Command::ConnectToServer);
    let dialog = desktop.network.window.as_ref().unwrap().window;
    assert!(desktop.network.owns(dialog));
    assert_eq!(desktop.title(dialog), "Connect to Server — Canopa Files");
    let _ = desktop.view(dialog);
    // A second request focuses the open window instead of replacing it.
    let _ = desktop.update(Message::Network(network_ui::Event::Show(
        window,
        Some("sftp://box/".into()),
        None,
    )));
    assert_eq!(desktop.network.window.as_ref().unwrap().window, dialog);
    let _ = desktop.update(Message::Network(network_ui::Event::Address(
        dialog,
        "nas/share".into(),
    )));
    let _ = desktop.update(Message::Network(network_ui::Event::Connect(dialog)));
    assert!(
        desktop.network.owns(dialog),
        "an invalid address keeps it open"
    );
    let _ = desktop.view(dialog);
    let _ = desktop.update(Message::Network(network_ui::Event::Pick(
        dialog,
        "smb://nas/share".into(),
    )));
    let _ = desktop.update(Message::Network(network_ui::Event::Connect(dialog)));
    assert!(
        !desktop.network.owns(dialog),
        "connecting closes the window so questions belong to the Browser"
    );
    assert!(
        desktop
            .active(window)
            .unwrap()
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unavailable"))
    );
    // Escape and closing the owning Browser close it too.
    let _ = desktop.command(window, Command::ConnectToServer);
    let dialog = desktop.network.window.as_ref().unwrap().window;
    let _ = desktop.update(Message::Key(
        dialog,
        keyboard::Key::Named(keyboard::key::Named::Escape),
        keyboard::Modifiers::empty(),
        None,
    ));
    assert!(desktop.network.window.is_none());
    let _ = desktop.command(window, Command::ConnectToServer);
    let _ = desktop.update(Message::Closed(window));
    assert!(desktop.network.window.is_none());
}
