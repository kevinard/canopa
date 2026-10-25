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
}

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
                    message: "Unsupported or stopping device request.".into(),
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
                    self.refuse(request.id, "This device request has already settled.");
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
                    None => {
                        self.refuse(request.id, "This device request no longer accepts answers.")
                    }
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
            return Err(io::Error::other(
                "The device observation has changed. Refresh and try again.",
            ));
        }
        let observed = self
            .monitor
            .mounts()
            .into_iter()
            .filter(|m| !m.is_shadowed() && m.root().path().is_some())
            .map(|m| m.as_ptr() as usize as u64)
            .collect();
        let device = catalog::collect(&self.monitor, &observed)
            .into_iter()
            .find(|d| d.instance == instance)
            .ok_or_else(|| io::Error::other("The device is no longer available."))?;
        if removal.map_or(!device.can_mount, |kind| device.removal != Some(kind)) {
            return Err(io::Error::other(
                "The device no longer supports this operation.",
            ));
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
            .ok_or_else(|| io::Error::other("The device is no longer available."))
    }

    fn start(self: &Rc<Self>, id: u64, instance: u64, generation: u64, removal: Option<Removal>) {
        if id <= self.last.get() {
            self.refuse(id, "This device request ID has already been used.");
            return;
        }
        self.last.set(id);
        if self.active.borrow().is_some() {
            self.refuse(id, "Another device request is still in progress.");
            return;
        }
        let target = match self.target(instance, generation, removal) {
            Ok(target) => target,
            Err(error) => {
                self.refuse(id, &error.to_string());
                return;
            }
        };
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

    fn finish(&self, id: u64, target: Target, mounting: bool, result: Result<(), glib::Error>) {
        let active = self
            .active
            .borrow_mut()
            .take()
            .expect("one outstanding device callback");
        assert_eq!(active.id, id);
        active.interaction.retire();
        let outcome = match result {
            Ok(()) if mounting => match target
                .mount()
                .filter(|_| target.present(&self.monitor))
                .and_then(|m| m.root().path())
            {
                Some(root) => Outcome::Mounted { root },
                None => Outcome::Failed {
                    message: "The device is no longer available as a local folder.".into(),
                },
            },
            Ok(()) => Outcome::Removed,
            Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => Outcome::Cancelled,
            Err(error) => Outcome::Failed {
                message: error.to_string(),
            },
        };
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
}

fn unavailable() -> glib::Error {
    glib::Error::new(
        gio::IOErrorEnum::NotFound,
        "The device capability is no longer available.",
    )
}
