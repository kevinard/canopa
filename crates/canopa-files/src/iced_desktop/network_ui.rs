//! Network locations: Connect to Server, discovered hosts and bookmarked
//! servers, the sidebar Network section, the read-only GIO listing for mounts
//! without a local folder, and the remote-folder refresh policy. Mounting and
//! listing belong to the device runtime's GIO helper; this module only
//! presents them and remembers the user's intent.
use super::{chrome::Icon, devices_ui::Origin, *};
use crate::{
    local_volumes::{
        Device, DeviceOutcome, DevicePhase, DeviceRequestId, DeviceSnapshot, DeviceToken, Listed,
        ListingId, LocationKind, RemoteEntry,
    },
    network::ServerAddress,
};
use canopa_file::gtk_bookmarks::{self, Bookmark};
use canopa_theme::UiKind;
use iced::{
    Alignment, Length,
    widget::{checkbox, column, container, row, scrollable, text, text_input, tooltip},
};

/// How often a displayed remote folder is re-read. inotify sees no changes
/// made on the server, so only the active tab of each window is polled.
pub(super) const REMOTE_POLL: Duration = Duration::from_secs(10);

/// The discovery root GVFS offers for Avahi/DNS-SD, WS-Discovery and SMB hosts.
const NETWORK: &str = "network:///";

#[derive(Default)]
pub(super) struct State {
    pub window: Option<Window>,
    /// Every line of the GTK bookmarks file, local folders and servers.
    pub bookmarks: Vec<Bookmark>,
    pub bookmarks_error: Option<String>,
    /// Where GTK bookmarks live. Unset in unit tests, which never touch the
    /// session's real configuration.
    pub bookmarks_path: Option<PathBuf>,
    /// GVFS's FUSE folder (`$XDG_RUNTIME_DIR/gvfs`). Paths below it are remote.
    pub fuse_root: Option<PathBuf>,
    /// Connect requests to bookmark once they succeed.
    bookmark_on_success: BTreeMap<DeviceRequestId, Bookmark>,
    /// Connect requests whose origin may receive a GIO listing fallback.
    listing_owners: BTreeMap<DeviceRequestId, window::Id>,
}

pub(super) struct Window {
    pub window: window::Id,
    pub owner: window::Id,
    address: String,
    bookmark: bool,
    error: Option<String>,
    /// Locations entered from discovery or a mount without a local folder.
    /// Empty shows the network discovery root.
    trail: Vec<(String, String)>,
    listing: Option<ListingId>,
    result: Option<Result<Listed, String>>,
}

impl Window {
    pub fn title(&self) -> &'static str {
        "Connect to Server"
    }

    fn browsing(&self) -> &str {
        self.trail.last().map_or(NETWORK, |(_, uri)| uri.as_str())
    }
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    /// Open the window from a Browser, optionally on an address or a location.
    Show(window::Id, Option<String>, Option<(String, String)>),
    Opened(window::Id, Option<window::Id>),
    Address(window::Id, String),
    ToggleBookmark(window::Id),
    Connect(window::Id),
    Close(window::Id),
    /// Enter a discovered folder, host or share list.
    Browse(window::Id, String, String),
    Up(window::Id),
    Refresh(window::Id),
    /// Use a discovered or bookmarked server as the address.
    Pick(window::Id, String),
    /// Open a file from the GIO listing with its default application.
    OpenRemote(window::Id, String),
    /// A sidebar server bookmark.
    OpenServer(window::Id, String),
    RemoveBookmark(String),
    /// A mounted location without a local folder: show its GIO listing.
    ShowRemote(window::Id, DeviceToken),
    BookmarksLoaded(Arc<Result<Vec<Bookmark>, String>>),
    BookmarkSaved(Arc<Result<(), String>>),
    Opening(Arc<Result<(), String>>),
    PollRemote,
}

fn control(id: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-network-{id:?}-{name}"))
}

/// What the sidebar shows for network locations, devices and GTK bookmarks.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Sidebar<'a> {
    /// Mounted (or mountable) network locations from the device catalog.
    pub mounted: Vec<&'a Device>,
    /// Bookmarked servers, as (name, URI).
    pub servers: Vec<(String, String)>,
    /// Local storage, phones and cameras.
    pub devices: Vec<&'a Device>,
}

/// Split the catalog by kind and list bookmarked servers. A server already
/// mounted is still listed, so it can be opened at its bookmarked folder.
pub(super) fn sidebar<'a>(catalog: &'a DeviceSnapshot, bookmarks: &[Bookmark]) -> Sidebar<'a> {
    let (mounted, devices) = catalog
        .devices
        .iter()
        .partition(|device| device.kind == LocationKind::Network);
    let mut seen = std::collections::BTreeSet::new();
    let servers = bookmarks
        .iter()
        .filter(|bookmark| bookmark.local_path().is_none())
        .filter(|bookmark| ServerAddress::parse(&bookmark.uri).is_ok())
        .filter(|bookmark| seen.insert(bookmark.uri.clone()))
        .map(|bookmark| (bookmark.display_name(), bookmark.uri.clone()))
        .collect();
    Sidebar {
        mounted,
        servers,
        devices,
    }
}

/// Local GTK bookmarks not already among the configured places.
pub(super) fn local_bookmarks(
    bookmarks: &[Bookmark],
    configured: &[PathBuf],
) -> Vec<(String, PathBuf)> {
    let mut seen: std::collections::BTreeSet<_> = configured.iter().cloned().collect();
    bookmarks
        .iter()
        .filter_map(|bookmark| Some((bookmark.display_name(), bookmark.local_path()?)))
        .filter(|(_, path)| path.is_absolute() && seen.insert(path.clone()))
        .collect()
}

/// Whether a discovered entry is a server Files can connect to directly. An
/// SMB host without a share is entered instead, to list its shares.
fn connectable(entry: &RemoteEntry) -> bool {
    ServerAddress::parse(&entry.uri).is_ok_and(|address| {
        address.scheme != crate::network::Scheme::Smb || !address.path.trim_matches('/').is_empty()
    })
}

impl State {
    pub fn start() -> Self {
        Self {
            bookmarks_path: gtk_bookmarks::default_path(),
            fuse_root: Some(crate::network::gvfs_fuse_root(
                std::env::var_os("XDG_RUNTIME_DIR")
                    .map(PathBuf::from)
                    .as_deref(),
            )),
            ..Self::default()
        }
    }

    pub fn owns(&self, id: window::Id) -> bool {
        self.window
            .as_ref()
            .is_some_and(|window| window.window == id)
    }
}

impl Desktop {
    /// Paths GVFS exposes through FUSE, or below a mounted network location.
    /// Memory only: no filesystem access on the UI thread.
    pub(super) fn remote_path(&self, path: &Path) -> bool {
        self.network
            .fuse_root
            .as_deref()
            .is_some_and(|root| path.starts_with(root) && path != root)
            || self.devices.catalog.devices.iter().any(|device| {
                device.kind.remote()
                    && device
                        .root
                        .as_deref()
                        .is_some_and(|root| path.starts_with(root))
            })
    }

    pub(super) fn load_gtk_bookmarks(&self) -> Task<Message> {
        let Some(path) = self.network.bookmarks_path.clone() else {
            return Task::none();
        };
        Task::perform(
            async move { gtk_bookmarks::read(&path).map_err(|error| error.to_string()) },
            |result| Message::Network(Event::BookmarksLoaded(Arc::new(result))),
        )
    }

    fn save_bookmark(&self, bookmark: Bookmark, remove: bool) -> Task<Message> {
        let Some(path) = self.network.bookmarks_path.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                if remove {
                    gtk_bookmarks::remove(&path, &bookmark.uri).map(|_| ())
                } else {
                    gtk_bookmarks::add(&path, &bookmark).map(|_| ())
                }
                .map_err(|error| format!("Could not update the GTK bookmarks: {error}"))
            },
            |result| Message::Network(Event::BookmarkSaved(Arc::new(result))),
        )
    }

    /// Mount a server address for a Browser window. A successful mount opens
    /// its folder in the window's current tab, as a mounted device does.
    pub(super) fn connect_server(
        &mut self,
        owner: window::Id,
        address: &ServerAddress,
        bookmark: bool,
    ) -> Task<Message> {
        if let Some(tab) = self.active_mut(owner) {
            tab.location = None;
            tab.error = None;
        }
        self.devices.error = None;
        let Some(origin) = Origin::capture(self, owner) else {
            self.devices.error = Some("Wait for the folder to load, then connect again.".into());
            return Task::none();
        };
        let result = self
            .device_runtime()
            .ok_or_else(|| io::Error::other("Network locations are unavailable."))
            .and_then(|runtime| runtime.device_requests().connect(address));
        match result {
            Ok(id) => {
                self.devices.owners.insert(id, owner);
                self.devices.origins.insert(id, origin);
                self.network.listing_owners.insert(id, owner);
                if bookmark {
                    self.network.bookmark_on_success.insert(
                        id,
                        Bookmark {
                            uri: address.uri.clone(),
                            label: Some(address.label()),
                        },
                    );
                }
                self.poll_devices()
            }
            Err(error) => {
                if let Some(tab) = self.active_mut(owner) {
                    tab.error = Some(error.to_string());
                }
                self.devices.error = Some(error.to_string());
                Task::none()
            }
        }
    }

    /// Finished Connect requests: bookmark them, report failures on their
    /// tab, and show the GIO listing for a mount without a local folder.
    /// Mounted folders navigate through the device flow (`poll_devices`).
    pub(super) fn poll_network(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        let finished: Vec<_> = self
            .devices
            .requests
            .iter()
            .filter(|job| {
                job.phase == DevicePhase::Finished
                    && (self.network.bookmark_on_success.contains_key(&job.id)
                        || self.network.listing_owners.contains_key(&job.id))
            })
            .map(|job| (job.id, job.outcome.clone()))
            .collect();
        for (id, outcome) in finished {
            let bookmark = self.network.bookmark_on_success.remove(&id);
            let owner = self.network.listing_owners.remove(&id);
            match &outcome {
                Some(DeviceOutcome::Connected { .. } | DeviceOutcome::ConnectedRemote { .. }) => {
                    if let Some(bookmark) = bookmark {
                        tasks.push(self.save_bookmark(bookmark, false));
                    }
                }
                Some(DeviceOutcome::Failed { message } | DeviceOutcome::Uncertain { message }) => {
                    if let Some(owner) = owner
                        && let Some(tab) = self.active_mut(owner)
                    {
                        tab.error = Some(message.clone());
                    }
                }
                _ => {}
            }
            if let (Some(owner), Some(DeviceOutcome::ConnectedRemote { uri })) = (owner, &outcome)
                && self.windows.contains_key(&owner)
            {
                tasks.push(self.show_network(
                    owner,
                    None,
                    Some((
                        crate::network::LocationKind::of_uri(uri).label(),
                        uri.clone(),
                    )),
                ));
            }
        }
        if let Some(window) = &mut self.network.window
            && window.result.is_none()
            && let Some(id) = window.listing
            && let Some(runtime) = self.file_operations.local_runtime()
        {
            window.result = runtime
                .device_requests()
                .listing(id)
                .and_then(|listing| listing.result);
        }
        Task::batch(tasks)
    }

    fn list_location(&mut self) {
        let Some(window) = &mut self.network.window else {
            return;
        };
        window.result = None;
        let uri = window.browsing().to_owned();
        match self
            .file_operations
            .local_runtime()
            .ok_or_else(|| io::Error::other("Network locations are unavailable."))
            .and_then(|runtime| runtime.device_requests().enumerate(&uri))
        {
            Ok(id) => window.listing = Some(id),
            Err(error) => {
                window.listing = None;
                window.result = Some(Err(error.to_string()));
            }
        }
    }

    pub(super) fn show_network(
        &mut self,
        owner: window::Id,
        address: Option<String>,
        location: Option<(String, String)>,
    ) -> Task<Message> {
        if let Some(window) = &mut self.network.window {
            if let Some(address) = address {
                window.address = address;
                window.error = None;
            }
            if let Some(location) = location {
                window.trail = vec![location];
                self.list_location();
            }
            return window::gain_focus(self.network.window.as_ref().unwrap().window);
        }
        if !self.windows.contains_key(&owner) {
            return Task::none();
        }
        let (window, opened) = window::open_transient(
            owner,
            window::Settings {
                size: Size::new(520.0, 560.0),
                min_size: Some(Size::new(340.0, 380.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.network.window = Some(Window {
            window,
            owner,
            address: address.unwrap_or_default(),
            bookmark: false,
            error: None,
            trail: location.into_iter().collect(),
            listing: None,
            result: None,
        });
        self.list_location();
        opened.map(move |opened| Message::Network(Event::Opened(window, opened)))
    }

    pub(super) fn close_network(&mut self, id: window::Id) -> Task<Message> {
        let Some(window) = self.network.window.take_if(|window| window.window == id) else {
            return Task::none();
        };
        let focus = if self.focused_window == Some(id) && self.windows.contains_key(&window.owner) {
            window::gain_focus(window.owner)
        } else {
            Task::none()
        };
        Task::batch([focus.chain(window::close(id)), self.exit_if_empty()])
    }

    pub(super) fn network_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || modifiers.control() && key.as_ref() == keyboard::Key::Character("w")
        {
            return self.close_network(id);
        }
        Task::none()
    }

    /// Re-read the active tab of each window that shows a remote folder.
    fn poll_remote_folders(&mut self) -> Task<Message> {
        let due: Vec<_> = self
            .windows
            .iter()
            .filter_map(|(id, window)| {
                let tab = window.tabs.get(window.active)?;
                (tab.remote && tab.browser.is_some() && !tab.loading && !tab.watch_scanning)
                    .then_some((*id, tab.id))
            })
            .collect();
        Task::batch(
            due.into_iter()
                .map(|(window, tab)| self.watch_rescan(window, tab)),
        )
    }

    /// Whether any window shows a remote folder in its active tab.
    pub(super) fn remote_polled(&self) -> bool {
        self.windows.values().any(|window| {
            window
                .tabs
                .get(window.active)
                .is_some_and(|tab| tab.remote && tab.browser.is_some())
        })
    }

    pub(super) fn remote_poll_subscription(&self) -> Subscription<Message> {
        if !self.remote_polled() {
            return Subscription::none();
        }
        Subscription::run_with("files-remote-folder-poll", |_| {
            iced::futures::stream::unfold((), |()| async {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::Builder::new()
                    .name("files-remote-poll".into())
                    .spawn(move || {
                        std::thread::sleep(REMOTE_POLL);
                        let _ = sender.send(());
                    })
                    .ok()?;
                receiver.await.ok()?;
                Some((Message::Network(Event::PollRemote), ()))
            })
        })
    }

    pub(super) fn network_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Show(owner, address, location) => {
                return self.show_network(owner, address, location);
            }
            Event::Opened(id, opened) => {
                if !self.network.owns(id) {
                    return if opened.is_some() {
                        window::close(id)
                    } else {
                        Task::none()
                    };
                }
                if opened.is_none() {
                    self.network.window = None;
                    self.devices.error = Some("Could not open Connect to Server.".into());
                    return self.exit_if_empty();
                }
                return window::gain_focus(id)
                    .chain(iced::widget::operation::focus(control(id, "address")));
            }
            Event::Address(id, value) => {
                if let Some(window) = self.network.window.as_mut().filter(|w| w.window == id) {
                    if value.len() <= crate::network::MAX_ADDRESS {
                        window.address = value;
                    }
                    window.error = None;
                }
            }
            Event::ToggleBookmark(id) => {
                if let Some(window) = self.network.window.as_mut().filter(|w| w.window == id) {
                    window.bookmark = !window.bookmark;
                }
            }
            Event::Connect(id) => {
                let Some(window) = self.network.window.as_mut().filter(|w| w.window == id) else {
                    return Task::none();
                };
                match ServerAddress::parse(&window.address) {
                    Ok(address) => {
                        let (owner, bookmark) = (window.owner, window.bookmark);
                        if !self.windows.contains_key(&owner) {
                            window.error = Some("Its Files window was closed.".into());
                            return Task::none();
                        }
                        // Questions and the opened folder belong to the Browser
                        // window, so the dialog closes before mounting starts.
                        let close = self.close_network(id);
                        let connect = self.connect_server(owner, &address, bookmark);
                        return close.chain(connect);
                    }
                    Err(error) => window.error = Some(error.to_string()),
                }
            }
            Event::Close(id) => return self.close_network(id),
            Event::Browse(id, name, uri) => {
                if let Some(window) = self.network.window.as_mut().filter(|w| w.window == id) {
                    if window.trail.len() < 16 {
                        window.trail.push((name, uri));
                    }
                    self.list_location();
                }
            }
            Event::Up(id) => {
                if let Some(window) = self.network.window.as_mut().filter(|w| w.window == id) {
                    window.trail.pop();
                    self.list_location();
                }
            }
            Event::Refresh(id) => {
                if self.network.owns(id) {
                    self.list_location();
                }
            }
            Event::Pick(id, uri) => {
                if let Some(window) = self.network.window.as_mut().filter(|w| w.window == id) {
                    window.address = uri;
                    window.error = None;
                    return iced::widget::operation::focus(control(id, "connect"));
                }
            }
            Event::OpenRemote(id, uri) => {
                if !self.network.owns(id) {
                    return Task::none();
                }
                return Task::perform(
                    async move {
                        crate::opening::spawn(crate::opening::open_url(&uri, None), true)
                            .map_err(|error| format!("Could not open the file: {error}"))
                    },
                    |result| Message::Network(Event::Opening(Arc::new(result))),
                );
            }
            Event::Opening(result) => {
                if let (Err(error), Some(window)) = (&*result, &mut self.network.window) {
                    window.error = Some(error.clone());
                }
            }
            Event::OpenServer(owner, uri) => match ServerAddress::parse(&uri) {
                Ok(address) => return self.connect_server(owner, &address, false),
                Err(error) => {
                    if let Some(tab) = self.active_mut(owner) {
                        tab.error = Some(error.to_string());
                    }
                }
            },
            Event::RemoveBookmark(uri) => {
                self.network
                    .bookmarks
                    .retain(|bookmark| bookmark.uri != uri);
                return self.save_bookmark(Bookmark { uri, label: None }, true);
            }
            Event::ShowRemote(owner, token) => {
                let device = self
                    .devices
                    .catalog
                    .devices
                    .iter()
                    .find(|device| device.token == token && device.remote_only)
                    .and_then(|device| Some((device.name.clone(), device.uri.clone()?)));
                if let Some(location) = device {
                    return self.show_network(owner, None, Some(location));
                }
            }
            Event::BookmarksLoaded(result) => match &*result {
                Ok(bookmarks) => {
                    self.network.bookmarks = bookmarks.clone();
                    self.network.bookmarks_error = None;
                }
                Err(error) => self.network.bookmarks_error = Some(error.clone()),
            },
            Event::BookmarkSaved(result) => {
                if let Err(error) = &*result {
                    self.network.bookmarks_error = Some(error.clone());
                }
                return self.load_gtk_bookmarks();
            }
            Event::PollRemote => return self.poll_remote_folders(),
        }
        Task::none()
    }
}

impl LocationKind {
    fn label(self) -> String {
        match self {
            Self::Local => "Location",
            Self::Network => "Network location",
            Self::Phone => "Phone",
            Self::Camera => "Camera",
        }
        .into()
    }
}

pub(super) fn kind_icon(kind: LocationKind) -> Icon {
    match kind {
        LocationKind::Local => Icon::Drive,
        LocationKind::Network => Icon::Network,
        LocationKind::Phone => Icon::Phone,
        LocationKind::Camera => Icon::Camera,
    }
}

/// The sidebar's Network section: mounted locations with Disconnect,
/// bookmarked servers, and Other Locations.
pub(super) fn places<'a>(desktop: &'a Desktop, window: window::Id) -> Element<'a, Message> {
    use super::devices_ui::Event as DeviceEvent;
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().body_size.px();
    let font = canopa_ui::font(appearance);
    let model = sidebar(&desktop.devices.catalog, &desktop.network.bookmarks);
    let ready = desktop.devices.catalog.status == crate::local_volumes::DeviceStatus::Ready;
    let idle = ready && !desktop.devices.busy();
    let origin = Origin::capture(desktop, window);
    let mut body = column![chrome::section(appearance, "Network")].spacing(2);
    let entry = |key: String, icon: Icon, label: String, message: Option<Message>, active: bool| {
        let node = super::accessibility_ui::Node::option(label.clone(), active)
            .key(key)
            .disabled(message.is_none())
            .on_click(message.clone());
        let button = canopa_ui::styled_action(
            row![
                desktop
                    .icons
                    .chrome(icon, 17.0, if message.is_some() { 0.74 } else { 0.35 }),
                chrome::single_line(label, font, size, None, true)
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            message,
            if active {
                canopa_ui::Action::Normal
            } else {
                canopa_ui::Action::Quiet
            },
            false,
            appearance,
        )
        .width(Length::Fill);
        super::accessibility_ui::semantic(node, button)
    };
    let trailing = |key: String, icon: Icon, label: String, message: Option<Message>| {
        let enabled = message.is_some();
        super::accessibility_ui::named_button(
            key,
            label.clone(),
            message.clone(),
            tooltip(
                canopa_ui::styled_action(
                    desktop
                        .icons
                        .chrome(icon, 18.0, if enabled { 0.95 } else { 0.35 }),
                    message,
                    canopa_ui::Action::Quiet,
                    false,
                    appearance,
                )
                .padding([7, 8]),
                text(label)
                    .font(font)
                    .size(appearance.typography().label_size.px()),
                tooltip::Position::Right,
            ),
        )
    };
    for device in &model.mounted {
        let open = if device.remote_only {
            Some(Message::Network(Event::ShowRemote(window, device.token)))
        } else {
            origin
                .as_ref()
                .filter(|_| idle && (device.root.is_some() || device.can_mount))
                .map(|origin| Message::Device(DeviceEvent::Open(origin.clone(), device.token)))
        };
        let active = device.root.as_deref().is_some_and(|root| {
            desktop
                .active(window)
                .is_some_and(|tab| tab.path().starts_with(root))
        });
        let mut item = row![super::drag_ui::device_region(
            desktop,
            window,
            device.token,
            entry(
                format!("files-device-{:?}", device.id()),
                Icon::Network,
                device.name.clone(),
                open,
                active
            )
        )]
        .align_y(Alignment::Center);
        if device.removal.is_some() {
            item = item.push(trailing(
                format!("files-device-{:?}-remove", device.id()),
                Icon::Eject,
                format!("Disconnect {}", device.name),
                idle.then_some(Message::Device(DeviceEvent::Remove(window, device.token))),
            ));
        }
        body = body.push(super::row_state::keyed_row(
            super::view::RowKey::Device(device.id()),
            item,
        ));
    }
    for (name, uri) in model.servers {
        let open = idle.then(|| Message::Network(Event::OpenServer(window, uri.clone())));
        let key = format!("files-server-{uri}");
        let item = row![
            entry(key.clone(), Icon::Folder, name.clone(), open, false),
            trailing(
                format!("{key}-remove"),
                Icon::Trash,
                format!("Remove bookmark {name}"),
                Some(Message::Network(Event::RemoveBookmark(uri))),
            )
        ]
        .align_y(Alignment::Center);
        body = body.push(item);
    }
    body = body.push(entry(
        "files-other-locations".into(),
        Icon::Places,
        "Other Locations…".into(),
        Some(Message::Network(Event::Show(window, None, None))),
        false,
    ));
    if let Some(error) = &desktop.network.bookmarks_error {
        body = body.push(text(error).size(size).font(font));
    }
    super::accessibility_ui::semantic(
        super::accessibility_ui::Node::list("Network").key("files-network"),
        body,
    )
}

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Window) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let muted = chrome::ink(appearance, canopa_theme::ColorRole::Muted);
    let id = dialog.window;
    let mut order = vec![control(id, "address")];
    let action = |label: String, name: String, message: Message, kind| {
        super::focus::button(
            control(id, &name),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(small),
                Some(message),
                kind,
                false,
                appearance,
            ),
        )
    };
    let input = text_input("smb://server/share", &dialog.address)
        .id(control(id, "address"))
        .font(font)
        .size(size)
        .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
        .style(move |theme, status| super::view::input_style(appearance, theme, status))
        .on_input(move |value| Message::Network(Event::Address(id, value)))
        .on_submit(Message::Network(Event::Connect(id)));
    // Named by the "Server address" caption drawn before it.
    let input = super::accessibility_ui::semantic(
        super::accessibility_ui::Node::new(super::accessibility_ui::Role::TextInput)
            .key("files-server-address")
            .value(dialog.address.clone())
            .placeholder("smb://server/share")
            .invalid(dialog.error.is_some())
            .on_text(move |value| Message::Network(Event::Address(id, value))),
        input,
    );
    let toggle = Message::Network(Event::ToggleBookmark(id));
    order.push(control(id, "bookmark"));
    let mut body = column![
        chrome::dialog_heading(desktop, Icon::Network, dialog.title(), ""),
        column![
            chrome::section_title(appearance, "Server address").size(small + 1.0),
            input,
            text("smb://, sftp://, ftp://, ftps://, dav://, davs:// or nfs://. Files asks for a password when the server needs one.")
                .font(font)
                .size(small)
                .color(muted)
                .wrapping(text::Wrapping::WordOrGlyph),
        ]
        .spacing(4),
        super::focus::check_box(
            control(id, "bookmark"),
            dialog.bookmark,
            Some(toggle.clone()),
            appearance,
            checkbox(dialog.bookmark)
                .label("Add to bookmarks")
                .font(font)
                .text_size(size)
                .style(canopa_ui::checkbox_style)
                .on_toggle(move |_| toggle.clone()),
        ),
    ]
    .spacing(10);
    if let Some(error) = &dialog.error {
        body = body.push(super::accessibility_ui::semantic(
            super::accessibility_ui::Node::alert(error.clone()).key("files-server-error"),
            text(error)
                .font(font)
                .size(small)
                .color(chrome::ink(appearance, canopa_theme::ColorRole::Danger))
                .wrapping(text::Wrapping::WordOrGlyph),
        ));
    }
    // Bookmarked servers first, then what the current location lists.
    let servers = sidebar(&desktop.devices.catalog, &desktop.network.bookmarks).servers;
    let mut list = column![].spacing(2);
    if dialog.trail.is_empty() && !servers.is_empty() {
        list = list.push(chrome::section(appearance, "Bookmarked servers"));
        for (index, (name, uri)) in servers.into_iter().enumerate() {
            let name_id = format!("server-{index}");
            order.push(control(id, &name_id));
            list = list.push(action(
                format!("{name} — {uri}"),
                name_id,
                Message::Network(Event::Pick(id, uri)),
                canopa_ui::Action::Quiet,
            ));
        }
    }
    let heading = dialog
        .trail
        .last()
        .map_or("On this network", |(name, _)| name.as_str());
    let mut header = row![chrome::section(appearance, heading)]
        .spacing(8)
        .align_y(Alignment::Center);
    header = header.push(iced::widget::space().width(Length::Fill));
    if !dialog.trail.is_empty() {
        order.push(control(id, "up"));
        header = header.push(action(
            "Back".into(),
            "up".into(),
            Message::Network(Event::Up(id)),
            canopa_ui::Action::Quiet,
        ));
    }
    order.push(control(id, "refresh"));
    header = header.push(action(
        "Refresh".into(),
        "refresh".into(),
        Message::Network(Event::Refresh(id)),
        canopa_ui::Action::Quiet,
    ));
    list = list.push(header);
    match &dialog.result {
        None => list = list.push(text("Looking…").font(font).size(small).color(muted)),
        Some(Err(error)) => {
            list = list.push(
                text(error.as_str())
                    .font(font)
                    .size(small)
                    .color(muted)
                    .wrapping(text::Wrapping::WordOrGlyph),
            );
        }
        Some(Ok(listed)) if listed.entries.is_empty() => {
            list = list.push(
                text(if dialog.trail.is_empty() {
                    "No servers were found. Enter an address above."
                } else {
                    "This location is empty."
                })
                .font(font)
                .size(small)
                .color(muted),
            );
        }
        Some(Ok(listed)) => {
            for (index, entry) in listed.entries.iter().enumerate() {
                let name = format!("entry-{index}");
                order.push(control(id, &name));
                let (message, label) = if dialog.trail.is_empty() || !entry.directory {
                    if connectable(entry) {
                        (
                            Message::Network(Event::Pick(id, entry.uri.clone())),
                            format!("{} — {}", entry.name, entry.uri),
                        )
                    } else if entry.directory {
                        (
                            Message::Network(Event::Browse(
                                id,
                                entry.name.clone(),
                                entry.uri.clone(),
                            )),
                            format!("{} ›", entry.name),
                        )
                    } else {
                        (
                            Message::Network(Event::OpenRemote(id, entry.uri.clone())),
                            entry.name.clone(),
                        )
                    }
                } else {
                    (
                        Message::Network(Event::Browse(id, entry.name.clone(), entry.uri.clone())),
                        format!("{} ›", entry.name),
                    )
                };
                list = list.push(action(label, name, message, canopa_ui::Action::Quiet));
            }
            if listed.truncated {
                list = list.push(
                    text("Only the first entries are shown.")
                        .font(font)
                        .size(small)
                        .color(muted),
                );
            }
        }
    }
    order.push(control(id, "cancel"));
    order.push(control(id, "connect"));
    let footer = row![
        iced::widget::space().width(Length::Fill),
        action(
            "Cancel".into(),
            "cancel".into(),
            Message::Network(Event::Close(id)),
            canopa_ui::Action::Quiet,
        ),
        action(
            "Connect".into(),
            "connect".into(),
            Message::Network(Event::Connect(id)),
            canopa_ui::Action::Primary,
        ),
    ]
    .spacing(8);
    let list = super::accessibility_ui::semantic(
        super::accessibility_ui::Node::group(heading)
            .key("files-server-list")
            .busy(dialog.result.is_none()),
        scrollable(list).height(Length::Fill),
    );
    let body = column![body, list, footer].spacing(12);
    let appearance = appearance.clone();
    let body = container(body)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
    super::accessibility_ui::dialog(
        "files-server-dialog",
        dialog.title(),
        "",
        false,
        super::focus::group(body, order, Vec::new(), Some(control(id, "address"))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_volumes::DeviceStatus;

    fn bookmark(uri: &str, label: Option<&str>) -> Bookmark {
        Bookmark {
            uri: uri.into(),
            label: label.map(Into::into),
        }
    }

    #[test]
    fn servers_and_local_bookmarks_are_split_and_deduplicated() {
        let bookmarks = vec![
            bookmark("file:///home/me/Projects", None),
            bookmark("smb://nas/share", Some("Family")),
            bookmark("smb://nas/share", Some("Duplicate")),
            bookmark("sftp://me@box/srv", None),
            bookmark("recent:///", None),
            bookmark("file:///home/me/Documents", Some("Docs")),
        ];
        let catalog = DeviceSnapshot {
            generation: 1,
            status: DeviceStatus::Ready,
            devices: Vec::new(),
        };
        let model = sidebar(&catalog, &bookmarks);
        assert_eq!(
            model.servers,
            vec![
                ("Family".to_owned(), "smb://nas/share".to_owned()),
                (
                    "sftp://me@box/srv".to_owned(),
                    "sftp://me@box/srv".to_owned()
                ),
            ]
        );
        let local = local_bookmarks(&bookmarks, &[PathBuf::from("/home/me/Documents")]);
        assert_eq!(
            local,
            vec![("Projects".to_owned(), PathBuf::from("/home/me/Projects"))]
        );
    }

    #[test]
    fn smb_hosts_are_entered_but_shares_and_other_servers_connect() {
        let entry = |uri: &str, directory| RemoteEntry {
            name: "x".into(),
            uri: uri.into(),
            directory,
            size: None,
        };
        assert!(!connectable(&entry("smb://nas/", true)));
        assert!(connectable(&entry("smb://nas/share", true)));
        assert!(connectable(&entry("sftp://box.local:22/", true)));
        assert!(!connectable(&entry("smb:///", true)));
        assert!(!connectable(&entry("dns-sd://local/", true)));
    }
}
