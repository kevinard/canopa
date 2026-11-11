//! Async platform requests, owned entirely by the helper's main context.
use super::{
    command_wire::{self as wire, Command, Outcome, Request, Update},
    interaction::Interaction,
    *,
};

#[derive(Clone)]
enum Target {
    Volume(gio::Volume),
    Mount(gio::Mount),
}

impl Target {
    fn mount(&self) -> Option<gio::Mount> {
        match self {
            Self::Volume(v) => v.get_mount(),
            Self::Mount(m) => Some(m.clone()),
        }
    }
    fn drive(&self) -> Option<gio::Drive> {
        match self {
            Self::Volume(v) => v.drive(),
            Self::Mount(m) => m.drive(),
        }
    }
    fn present(&self, monitor: &gio::VolumeMonitor) -> bool {
        match self {
            Self::Volume(v) => monitor.volumes().contains(v),
            Self::Mount(m) => monitor.mounts().contains(m),
        }
    }
    /// The location's URI scheme, to name a missing GVFS backend.
    fn scheme(&self) -> Option<String> {
        let root = match self {
            Self::Volume(v) => v.activation_root(),
            Self::Mount(m) => Some(m.root()),
        }?;
        root.uri_scheme().map(|scheme| scheme.to_string())
    }
}

struct Active {
    id: u64,
    cancel: gio::Cancellable,
    interaction: Rc<Interaction>,
}

pub(super) struct Commands {
    monitor: gio::VolumeMonitor,
    generation: Rc<Cell<u64>>,
    revisions: Rc<RefCell<BTreeMap<u64, u64>>>,
    main_loop: Arc<glib::MainLoop>,
    failure: Rc<RefCell<Option<io::Error>>>,
    active: RefCell<Option<Active>>,
    last: Cell<u64>,
    stopping: Cell<bool>,
    listings: Cell<usize>,
}

/// Concurrent read-only GIO listings; further ones are refused.
const LISTINGS: usize = 4;
const LISTING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

impl Commands {
    pub fn new(
        monitor: gio::VolumeMonitor,
        generation: Rc<Cell<u64>>,
        revisions: Rc<RefCell<BTreeMap<u64, u64>>>,
        main_loop: Arc<glib::MainLoop>,
        failure: Rc<RefCell<Option<io::Error>>>,
    ) -> Rc<Self> {
        Rc::new(Self {
            monitor,
            generation,
            revisions,
            main_loop,
            failure,
            active: RefCell::new(None),
            last: Cell::new(0),
            stopping: Cell::new(false),
            listings: Cell::new(0),
        })
    }

    fn send(&self, id: u64, event: Update) {
        if let Err(error) = wire::emit(id, event) {
            *self.failure.borrow_mut() = Some(error);
            self.stop();
        }
    }

    pub fn stop(&self) {
        self.stopping.set(true);
        let active = self
            .active
            .borrow()
            .as_ref()
            .map(|a| (a.cancel.clone(), a.interaction.clone()));
        if let Some((cancel, interaction)) = active {
            cancel.cancel();
            interaction.abort();
        } else {
            self.main_loop.quit();
        }
    }

    pub fn receive(self: &Rc<Self>, request: Request) {
        if request.version != 1 || request.id == 0 || self.stopping.get() {
            self.send(
                request.id,
                Update::Refused {
                    message: fl!("files-device-request-unsupported"),
                    terminal: self
                        .active
                        .borrow()
                        .as_ref()
                        .is_none_or(|a| a.id != request.id),
                },
            );
            return;
        }
        match request.command {
            Command::Mount {
                instance,
                generation,
            } => self.start(request.id, instance, generation, None),
            Command::Remove {
                instance,
                generation,
                removal,
            } => self.start(request.id, instance, generation, Some(removal)),
            Command::Connect { uri } => self.connect(request.id, uri),
            Command::Enumerate { uri } => self.enumerate(request.id, uri),
            Command::Cancel => {
                let active = self
                    .active
                    .borrow()
                    .as_ref()
                    .filter(|a| a.id == request.id)
                    .map(|a| (a.cancel.clone(), a.interaction.clone()));
                if let Some((cancel, interaction)) = active {
                    cancel.cancel();
                    interaction.abort();
                } else {
                    self.refuse(request.id, &fl!("files-device-already-settled"));
                }
            }
            Command::Answer { question, answer } => {
                let interaction = self
                    .active
                    .borrow()
                    .as_ref()
                    .filter(|a| a.id == request.id && !a.cancel.is_cancelled())
                    .map(|a| a.interaction.clone());
                match interaction {
                    Some(interaction) => {
                        if let Err(error) = interaction.answer(question, answer) {
                            self.refuse(request.id, &error.to_string());
                        }
                    }
                    None => self.refuse(request.id, &fl!("files-device-no-more-answers")),
                }
            }
        }
    }

    fn refuse(&self, id: u64, message: &str) {
        self.send(
            id,
            Update::Refused {
                message: message.into(),
                terminal: self.active.borrow().as_ref().is_none_or(|a| a.id != id),
            },
        );
    }

    fn target(
        &self,
        instance: u64,
        generation: u64,
        removal: Option<Removal>,
    ) -> io::Result<Target> {
        if generation != self.generation.get() {
            return Err(io::Error::other(fl!("files-device-observation-changed")));
        }
        let observed = catalog::observed_mounts(&self.monitor);
        let device = catalog::collect(&self.monitor, &observed)
            .into_iter()
            .find(|d| d.instance == instance)
            .ok_or_else(|| io::Error::other(fl!("files-device-gone")))?;
        if removal.map_or(!device.can_mount, |kind| device.removal != Some(kind)) {
            return Err(io::Error::other(fl!("files-device-unsupported-operation")));
        }
        if let Some(volume) = self
            .monitor
            .volumes()
            .into_iter()
            .find(|v| v.as_ptr() as usize as u64 == instance)
        {
            return Ok(Target::Volume(volume));
        }
        self.monitor
            .mounts()
            .into_iter()
            .find(|m| m.as_ptr() as usize as u64 == instance)
            .map(Target::Mount)
            .ok_or_else(|| io::Error::other(fl!("files-device-gone")))
    }

    /// A fresh request ID while nothing else is active, else a terminal refusal.
    fn admit(&self, id: u64) -> bool {
        if id <= self.last.get() {
            self.refuse(id, &fl!("files-device-request-id-used"));
            return false;
        }
        self.last.set(id);
        if self.active.borrow().is_some() {
            self.refuse(id, &fl!("files-device-another-request"));
            return false;
        }
        true
    }

    fn activate(self: &Rc<Self>, id: u64) -> (gio::Cancellable, Rc<Interaction>) {
        let weak = Rc::downgrade(self);
        let interaction = Rc::new(Interaction::new(move |question, prompt| {
            if let Some(owner) = weak.upgrade() {
                owner.send(id, Update::Question { question, prompt });
            }
        }));
        let cancel = gio::Cancellable::new();
        *self.active.borrow_mut() = Some(Active {
            id,
            cancel: cancel.clone(),
            interaction: interaction.clone(),
        });
        (cancel, interaction)
    }

    fn start(self: &Rc<Self>, id: u64, instance: u64, generation: u64, removal: Option<Removal>) {
        if !self.admit(id) {
            return;
        }
        let target = match self.target(instance, generation, removal) {
            Ok(target) => target,
            Err(error) => {
                self.refuse(id, &error.to_string());
                return;
            }
        };
        let (cancel, interaction) = self.activate(id);
        let owner = self.clone();
        let completed_target = target.clone();
        let callback = move |result| owner.finish(id, completed_target, removal.is_none(), result);
        let operation = Some(&interaction.operation);
        match removal {
            None => match &target {
                Target::Volume(volume) => volume.mount(
                    gio::MountMountFlags::NONE,
                    operation,
                    Some(&cancel),
                    callback,
                ),
                Target::Mount(_) => callback(Err(unavailable())),
            },
            Some(Removal::PowerOff) => {
                if let Some(drive) = target.drive().filter(|d| d.can_stop()) {
                    drive.stop(
                        gio::MountUnmountFlags::NONE,
                        operation,
                        Some(&cancel),
                        callback,
                    );
                } else {
                    callback(Err(unavailable()));
                }
            }
            Some(Removal::Eject) => {
                if let Some(drive) = target.drive().filter(|d| d.can_eject()) {
                    drive.eject_with_operation(
                        gio::MountUnmountFlags::NONE,
                        operation,
                        Some(&cancel),
                        callback,
                    );
                } else if let Target::Volume(volume) = &target
                    && volume.can_eject()
                {
                    volume.eject_with_operation(
                        gio::MountUnmountFlags::NONE,
                        operation,
                        Some(&cancel),
                        callback,
                    );
                } else if let Some(mount) = target.mount().filter(|m| m.can_eject()) {
                    mount.eject_with_operation(
                        gio::MountUnmountFlags::NONE,
                        operation,
                        Some(&cancel),
                        callback,
                    );
                } else {
                    callback(Err(unavailable()));
                }
            }
            Some(Removal::Unmount) => {
                if let Some(mount) = target.mount().filter(|m| m.can_unmount()) {
                    mount.unmount_with_operation(
                        gio::MountUnmountFlags::NONE,
                        operation,
                        Some(&cancel),
                        callback,
                    );
                } else {
                    callback(Err(unavailable()));
                }
            }
        }
    }

    fn retire(&self, id: u64) {
        let active = self
            .active
            .borrow_mut()
            .take()
            .expect("one outstanding device callback");
        assert_eq!(active.id, id);
        active.interaction.retire();
    }

    /// Publish the completed platform state, then the request's outcome.
    fn settle(&self, id: u64, outcome: Outcome) {
        // Even providers that omit a signal must publish their completed state
        // before the parent can expose a successful mounted root to a browser.
        let refreshed = self
            .generation
            .get()
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Volume observation counter exhausted"))
            .and_then(|generation| {
                self.generation.set(generation);
                publish(&self.monitor, generation, &mut self.revisions.borrow_mut())
            });
        if let Err(error) = refreshed {
            *self.failure.borrow_mut() = Some(error);
            self.stopping.set(true);
        }
        self.send(id, Update::Finished { outcome });
        if self.stopping.get() {
            self.main_loop.quit();
        }
    }

    fn finish(&self, id: u64, target: Target, mounting: bool, result: Result<(), glib::Error>) {
        self.retire(id);
        let outcome = match result {
            Ok(()) if mounting => match target
                .mount()
                .filter(|_| target.present(&self.monitor))
                .and_then(|m| m.root().path())
            {
                Some(root) => Outcome::Mounted { root },
                None => Outcome::Failed {
                    message: fl!("files-device-not-local"),
                },
            },
            Ok(()) => Outcome::Removed,
            Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => Outcome::Cancelled,
            Err(error) if mounting && error.matches(gio::IOErrorEnum::NotSupported) => {
                Outcome::Failed {
                    message: target
                        .scheme()
                        .map(|scheme| crate::network::missing_backend(&scheme))
                        .unwrap_or_else(|| error.to_string()),
                }
            }
            Err(error) if error.matches(gio::IOErrorEnum::FailedHandled) => Outcome::Cancelled,
            Err(error) => Outcome::Failed {
                message: error.to_string(),
            },
        };
        self.settle(id, outcome);
    }

    /// Mount the enclosing volume of a server address with this request's
    /// mount operation, so credentials and host-key questions reach the user.
    fn connect(self: &Rc<Self>, id: u64, uri: String) {
        if !self.admit(id) {
            return;
        }
        // Never trust the parent's validation: reparse and require the exact
        // normalized URI, so no password or query can reach GVFS from here.
        let address = match crate::network::ServerAddress::parse(&uri) {
            Ok(address) if address.uri == uri => address,
            _ => {
                self.refuse(id, &fl!("files-device-address-invalid"));
                return;
            }
        };
        if !supported(address.scheme.id()) {
            self.refuse(id, &crate::network::missing_backend(address.scheme.id()));
            return;
        }
        let (cancel, interaction) = self.activate(id);
        let file = gio::File::for_uri(&address.uri);
        let owner = self.clone();
        let mounted = file.clone();
        file.mount_enclosing_volume(
            gio::MountMountFlags::NONE,
            Some(&interaction.operation),
            Some(&cancel),
            move |result| owner.finish_connect(id, mounted, result, 0),
        );
    }

    fn finish_connect(
        self: &Rc<Self>,
        id: u64,
        file: gio::File,
        result: Result<(), glib::Error>,
        attempt: u32,
    ) {
        let uri = file.uri().to_string();
        let result = match result {
            Err(error) if error.matches(gio::IOErrorEnum::AlreadyMounted) => Ok(()),
            result => result,
        };
        let outcome = match result {
            Ok(()) => match file.find_enclosing_mount(gio::Cancellable::NONE) {
                Ok(mount) => match (mount.root().path(), file.path()) {
                    (Some(root), Some(location)) if location.starts_with(&root) => {
                        // The volume monitor learns of a new GVFS mount by a
                        // separate signal. Wait briefly for it, so the parent
                        // sees the mount among its prepared observations.
                        let listed =
                            self.monitor.mounts().iter().any(|known| {
                                known.root().path().as_deref() == Some(root.as_path())
                            });
                        if !listed && attempt < 40 {
                            let owner = self.clone();
                            glib::timeout_add_local_once(
                                std::time::Duration::from_millis(50),
                                move || owner.finish_connect(id, file, Ok(()), attempt + 1),
                            );
                            return;
                        }
                        Outcome::Connected { root, location }
                    }
                    (Some(_), Some(_)) => Outcome::Failed {
                        message: fl!("files-device-outside-mount"),
                    },
                    _ => Outcome::ConnectedRemote { uri: uri.clone() },
                },
                Err(error) => Outcome::Failed {
                    message: crate::network::describe_failure(
                        &uri,
                        crate::network::Failure::Other,
                        error.message(),
                    ),
                },
            },
            Err(error)
                if error.matches(gio::IOErrorEnum::Cancelled)
                    || error.matches(gio::IOErrorEnum::FailedHandled) =>
            {
                Outcome::Cancelled
            }
            Err(error) => Outcome::Failed {
                message: crate::network::describe_failure(&uri, classify(&error), error.message()),
            },
        };
        self.retire(id);
        self.settle(id, outcome);
    }

    /// List a GVFS location's children: network discovery, or a mount whose
    /// local folder is missing. It never changes anything on the server.
    fn enumerate(self: &Rc<Self>, id: u64, uri: String) {
        let scheme = uri
            .split_once(':')
            .map_or(String::new(), |(scheme, _)| scheme.to_ascii_lowercase());
        let allowed = uri.len() <= 4096
            && !uri.chars().any(char::is_control)
            && (scheme == "network"
                || scheme == "dns-sd"
                || crate::network::LocationKind::of_scheme(&scheme).remote());
        let reply = |entries, error, truncated| Update::Enumerated {
            entries,
            error,
            truncated,
        };
        if !allowed {
            self.send(
                id,
                reply(Vec::new(), Some(fl!("files-device-cannot-list")), false),
            );
            return;
        }
        if !supported(&scheme) {
            self.send(
                id,
                reply(
                    Vec::new(),
                    Some(crate::network::missing_backend(&scheme)),
                    false,
                ),
            );
            return;
        }
        if self.listings.get() >= LISTINGS || self.stopping.get() {
            self.send(
                id,
                reply(Vec::new(), Some(fl!("files-device-still-listing")), false),
            );
            return;
        }
        self.listings.set(self.listings.get() + 1);
        let owner = self.clone();
        glib::MainContext::default().spawn_local(async move {
            let file = gio::File::for_uri(&uri);
            let listed = glib::future_with_timeout(LISTING_TIMEOUT, list(file)).await;
            owner.listings.set(owner.listings.get() - 1);
            let update = match listed {
                Ok(Ok((entries, truncated))) => reply(entries, None, truncated),
                Ok(Err(error)) => reply(
                    Vec::new(),
                    Some(crate::network::describe_failure(
                        &uri,
                        classify(&error),
                        error.message(),
                    )),
                    false,
                ),
                Err(_) => reply(Vec::new(), Some(fl!("files-device-listing-timeout")), false),
            };
            owner.send(id, update);
        });
    }
}

/// Whether GIO's VFS (GVFS, when installed) handles a URI scheme.
fn supported(scheme: &str) -> bool {
    gio::Vfs::default()
        .supported_uri_schemes()
        .iter()
        .any(|known| known.as_str() == scheme)
}

/// Readable failure class of a GIO mount or listing error.
fn classify(error: &glib::Error) -> crate::network::Failure {
    use crate::network::Failure;
    let code = error.kind::<gio::IOErrorEnum>();
    match code {
        Some(
            gio::IOErrorEnum::HostNotFound
            | gio::IOErrorEnum::HostUnreachable
            | gio::IOErrorEnum::NetworkUnreachable
            | gio::IOErrorEnum::ConnectionRefused
            | gio::IOErrorEnum::TimedOut
            | gio::IOErrorEnum::NotConnected
            | gio::IOErrorEnum::BrokenPipe,
        ) => return Failure::Unreachable,
        Some(gio::IOErrorEnum::PermissionDenied) => return Failure::Credentials,
        Some(gio::IOErrorEnum::NotSupported) => return Failure::Unsupported,
        _ => {}
    }
    // Several backends report network trouble as a generic failure.
    let message = error.message().to_ascii_lowercase();
    if [
        "timed out",
        "no route to host",
        "connection refused",
        "name or service not known",
        "could not resolve",
        "unable to resolve",
        "network is unreachable",
        "host is down",
        "connection reset",
    ]
    .iter()
    .any(|text| message.contains(text))
    {
        Failure::Unreachable
    } else if [
        "permission denied",
        "authentication",
        "logon failure",
        "login",
    ]
    .iter()
    .any(|text| message.contains(text))
    {
        Failure::Credentials
    } else {
        Failure::Other
    }
}

/// Children of a location, mounting it first when GIO says it is not mounted
/// (network:/// mounts itself without credentials).
async fn list(file: gio::File) -> Result<(Vec<wire::RemoteEntry>, bool), glib::Error> {
    const ATTRIBUTES: &str = "standard::name,standard::display-name,standard::type,standard::size,standard::target-uri,standard::is-hidden";
    let enumerator = match file
        .enumerate_children_future(
            ATTRIBUTES,
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::DEFAULT,
        )
        .await
    {
        Err(error) if error.matches(gio::IOErrorEnum::NotMounted) => {
            file.mount_enclosing_volume_future(
                gio::MountMountFlags::NONE,
                gio::MountOperation::NONE,
            )
            .await?;
            file.enumerate_children_future(
                ATTRIBUTES,
                gio::FileQueryInfoFlags::NONE,
                glib::Priority::DEFAULT,
            )
            .await?
        }
        result => result?,
    };
    let mut entries = Vec::new();
    let mut truncated = false;
    loop {
        let infos = enumerator
            .next_files_future(64, glib::Priority::DEFAULT)
            .await?;
        if infos.is_empty() {
            break;
        }
        for info in infos {
            if info.is_hidden() {
                continue;
            }
            if entries.len() >= wire::ENUMERATION_LIMIT {
                truncated = true;
                break;
            }
            let uri = info
                .attribute_string(gio::FILE_ATTRIBUTE_STANDARD_TARGET_URI)
                .map(|uri| uri.to_string())
                .unwrap_or_else(|| file.child(info.name()).uri().to_string());
            let kind = info.file_type();
            let entry = wire::RemoteEntry {
                name: info.display_name().to_string(),
                uri,
                directory: matches!(
                    kind,
                    gio::FileType::Directory | gio::FileType::Mountable | gio::FileType::Shortcut
                ),
                size: (kind == gio::FileType::Regular)
                    .then(|| u64::try_from(info.size()).unwrap_or(0)),
            };
            if wire::valid_entry(&entry) {
                entries.push(entry);
            }
        }
        if truncated {
            break;
        }
    }
    let _ = enumerator.close_future(glib::Priority::DEFAULT).await;
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok((entries, truncated))
}

fn unavailable() -> glib::Error {
    glib::Error::new(
        gio::IOErrorEnum::NotFound,
        &fl!("files-device-capability-gone"),
    )
}
