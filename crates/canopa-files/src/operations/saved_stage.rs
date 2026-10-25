//! Return a catalog-owned saved container without treating it as ordinary user data.
use super::*;
pub(super) mod forward;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Movement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    proof: Option<Box<Container>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    copied: Option<CopyBinding>,
    pub parent_identity: Identity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_access: Option<StageParentAccess>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_parent: Option<Box<Parent>>,
    pub context: Option<ContextMovement>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CopyBinding {
    recipe: [u8; 32],
    bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Container {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<fingerprint::Version>,
    source_parent: original::Original,
    directory: original::Original,
    previous: original::Original,
    destination_parent: original::Original,
    counted: bool,
}
pub(super) struct Capture<'a> {
    pub source: &'a Path,
    pub destination: &'a Path,
    pub record: &'a recovery::Record,
    pub parent: &'a fs::File,
    pub directory: &'a fs::File,
    pub target_parent: &'a fs::File,
    pub access: StageParentAccess,
    pub context: Option<ContextMovement>,
    pub counted: bool,
}

impl Movement {
    pub fn capture(
        source: &Path,
        destination: &Path,
        source_access: Option<StageParentAccess>,
        destination_access: StageParentAccess,
        context: Option<ContextMovement>,
    ) -> io::Result<Self> {
        Ok(Self {
            proof: None,
            copied: None,
            source_access,
            destination_parent: Some(Box::new(Parent {
                original: None,
                identity: Identity::from_metadata(&fs::metadata(destination.parent().unwrap())?),
                access: destination_access,
            })),
            parent_identity: Identity::from_metadata(&fs::metadata(source.parent().unwrap())?),
            context,
        })
    }
    pub(super) fn capture_owned(capture: Capture<'_>) -> io::Result<Self> {
        let Capture {
            source,
            destination,
            record,
            parent,
            directory,
            target_parent,
            access,
            context,
            counted,
        } = capture;
        if record.payload
            || record.retained.stage.directory != source
            || Identity::from_metadata(&directory.metadata()?) != record.retained.stage.identity
        {
            return Err(attention(
                "The selected saved container differs from its catalog owner.",
            ));
        }
        let ownership =
            record.retained.stage.ownership.as_ref().ok_or_else(|| {
                attention("The saved container has no original generation proof.")
            })?;
        ownership.require_parent(parent)?;
        ownership.require_directory(directory)?;
        let previous = fs::File::from(rustix::fs::openat(
            directory,
            c"previous",
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        if Identity::from_metadata(&previous.metadata()?) != record.retained.observed_identity() {
            return Err(attention(
                "The saved version differs from its catalog owner.",
            ));
        }
        let previous = match ownership.previous(&previous)? {
            Some(proof) => proof,
            None if record.retained.physical_identity.is_none() => record
                .retained
                .replacement_facts
                .as_ref()
                .ok_or_else(|| attention("The saved version has no original generation proof."))?
                .original_previous(&previous)?,
            None => {
                return Err(attention(
                    "The recreated saved version has no original generation proof.",
                ));
            }
        };
        let (source_parent, source_directory) = ownership.roots();
        let destination_parent = original::Original::capture(target_parent)?;
        if destination.parent().is_none() {
            return Err(attention("A saved move needs its selected target folder."));
        }
        let mut movement = Self {
            proof: None,
            copied: None,
            parent_identity: Identity::from_metadata(&parent.metadata()?),
            source_access: record.retained.stage.parent_access,
            destination_parent: Some(Box::new(Parent {
                original: Some(destination_parent),
                identity: Identity::from_metadata(&target_parent.metadata()?),
                access,
            })),
            context,
        };
        movement.proof = Some(Box::new(Container {
            version: Some(fingerprint::Version::from_metadata(&directory.metadata()?)),
            source_parent,
            directory: source_directory,
            previous,
            destination_parent,
            counted,
        }));
        Ok(movement)
    }
    pub(super) fn matches_move(&self, facts: &entry_move::Facts) -> bool {
        self.copied.is_none()
            && self.proof.as_ref().is_some_and(|proof| {
                facts.saved_origins()
                    == Some((
                        proof.source_parent,
                        proof.directory,
                        proof.destination_parent,
                    ))
            })
    }
    pub(in crate::operations) fn copied_bytes(&self) -> Option<u64> {
        self.copied.as_ref().map(|c| c.bytes)
    }
    pub(super) fn require_previous(&self, directory: &fs::File) -> io::Result<()> {
        let proof = self
            .proof
            .as_ref()
            .ok_or_else(|| attention("The saved move has no original container proof."))?;
        proof.directory.require(directory)?;
        let previous = fs::File::from(rustix::fs::openat(
            directory,
            c"previous",
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        proof.previous.require(&previous)
    }
    pub(super) fn counted(&self) -> Option<bool> {
        self.proof.as_ref().map(|proof| proof.counted)
    }
    pub(super) fn ownership(&self, reverse: bool) -> Option<staging::Ownership> {
        self.proof.as_ref().map(|proof| {
            staging::Ownership::saved(
                if reverse {
                    proof.source_parent
                } else {
                    proof.destination_parent
                },
                proof.directory,
                proof.previous,
            )
        })
    }

    /// A Merge child describes the reverse of the actual selected movement.
    /// Keep the opened container and both parent proofs; only their roles swap.
    pub(super) fn into_return(mut self) -> Self {
        let destination = self.destination_parent.as_mut().unwrap();
        std::mem::swap(&mut self.parent_identity, &mut destination.identity);
        let source_access = self.source_access.replace(destination.access);
        destination.access = source_access.unwrap_or(StageParentAccess::Existing);
        if let Some(proof) = &mut self.proof {
            std::mem::swap(&mut proof.source_parent, &mut proof.destination_parent);
            destination.original = Some(proof.destination_parent);
        }
        if let Some(context) = &mut self.context {
            std::mem::swap(&mut context.from, &mut context.to);
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Parent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<original::Original>,
    pub identity: Identity,
    pub access: StageParentAccess,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContextMovement {
    #[serde(with = "crate::path_serde")]
    pub from: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub to: PathBuf,
}

pub(super) fn omit_nested_copy(record: &recovery::Record) -> io::Result<()> {
    if record.payload {
        return Err(attention(
            "Keep claimed Recovery data intact; recover an explicit copy first.",
        ));
    }
    require_only(&record.retained.stage, &["previous"])?;
    let metadata = fs::symlink_metadata(record.path())?;
    if metadata.is_file() || metadata.is_symlink() {
        Ok(())
    } else {
        Err(attention(
            "the saved-data folder contains an unexpected payload; its contents are preserved",
        ))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Migration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cleanup: Option<forward::Phase>,
    pub id: u64,
    pub ordinal: u64,
    pub change: Change,
    pub source: Stage,
    pub source_version: fingerprint::Version,
    pub source_fingerprint: Fingerprint,
    pub stage: Stage,
    pub copy_identity: Identity,
    pub fingerprint: Fingerprint,
    pub context: Option<recovery::Context>,
    pub relocation: Option<undo::Relocation>,
    pub catalogued: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub forward: bool,
}

impl Migration {
    pub(super) fn has_claim(&self) -> bool {
        self.forward || self.cleanup.is_some()
    }
    pub fn destination(&self) -> &Path {
        if self.forward {
            self.change.forward_path()
        } else {
            self.change.reverse_path().unwrap()
        }
    }
    pub fn destination_stage(&self) -> Stage {
        Stage {
            replacement: None,
            entry_move: None,
            source_claim: None,
            directory_claim: None,
            ownership: self
                .stage
                .ownership
                .as_ref()
                .and_then(|proof| proof.as_saved_copy().map(Box::new)),
            parent_access: self.stage.parent_access,
            directory: self.destination().to_owned(),
            identity: self.stage.identity,
            payload_identity: None,
            parent_identity: self.stage.parent_identity,
            sibling_payload: self.stage.sibling_payload,
        }
    }
    pub fn record(&self, label: String) -> recovery::Record {
        let destination = if self.forward {
            Some(self.change.forward_path())
        } else {
            self.change.reverse_path()
        };
        let stage = if self.catalogued {
            let mut source = self.source.clone();
            if self.cleanup.is_some_and(|p| p != forward::Phase::Ready) {
                source.directory = forward::claim_path(self);
            }
            source
        } else if destination.is_some_and(|path| self.stage.identity.matches(path)) {
            self.destination_stage()
        } else {
            self.stage.clone()
        };
        let payload = fs::symlink_metadata(stage.payload()).is_ok();
        recovery::Record {
            context_moves: Vec::new(),
            retained: Retained {
                replacement_facts: None,
                stage,
                identity: if self.catalogued {
                    self.source_version.identity
                } else {
                    self.copy_identity
                },
                physical_identity: None,
                recovery: true,
                version: None,
            },
            context: Some(recovery::Context {
                original: self.context.as_ref().map_or_else(
                    || {
                        self.change
                            .reverse_path()
                            .unwrap_or(self.change.forward_path())
                            .to_owned()
                    },
                    |context| context.original.clone(),
                ),
                label,
            }),
            payload,
            container: false,
        }
    }
}

pub(super) fn prepare(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    record: &recovery::Record,
    cancellation: &Cancellation,
    relocation: Option<&undo::Relocation>,
) -> io::Result<()> {
    let destination = change
        .reverse_path()
        .ok_or_else(|| invalid("saved return has no destination"))?;
    let cleanup_parent = state.owned_return_parent(id, ordinal, destination.parent().unwrap())?;
    prepare_inner(
        state,
        Preparation {
            cleanup_parent,
            id,
            ordinal,
            change,
            record,
            relocation,
            forward: false,
            selection: None,
        },
        cancellation,
        |_| {},
        || Ok(()),
    )
}

pub(super) struct Forward<'a> {
    pub selection: entry_move::Selection,
    pub counted: bool,
    pub cleanup_parent: Option<Identity>,
    pub id: u64,
    pub ordinal: u64,
    pub source: &'a Path,
    pub destination: &'a Path,
    pub record: &'a recovery::Record,
    pub context: Option<ContextMovement>,
}

pub(super) fn prepare_forward(
    state: &mut Store,
    request: Forward<'_>,
    cancellation: &Cancellation,
    progress: impl FnMut(u64),
    checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let ordinal = request.ordinal;
    if state.next_ordinal(request.id)? != ordinal {
        return Err(invalid("the saved handoff no longer owns its history slot"));
    }
    let change = Change::Rename {
        move_facts: None,
        original: request.source.to_owned(),
        current: request.destination.to_owned(),
        identity: request.record.retained.stage.identity,
        retained: None,
        reviewed: None,
        saved: Some(
            request.selection.saved_movement(
                request.destination,
                request.record,
                request
                    .cleanup_parent
                    .map_or(StageParentAccess::Existing, StageParentAccess::Created),
                request.context,
                request.counted,
            )?,
        ),
    };
    prepare_inner(
        state,
        Preparation {
            cleanup_parent: request.cleanup_parent,
            id: request.id,
            ordinal,
            change: &change,
            record: request.record,
            relocation: None,
            forward: true,
            selection: Some(&request.selection),
        },
        cancellation,
        progress,
        checkpoint,
    )
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Receipt {
    pub id: u64,
    pub ordinal: u64,
    pub identity: Identity,
}

struct Preparation<'a> {
    cleanup_parent: Option<Identity>,
    id: u64,
    ordinal: u64,
    change: &'a Change,
    record: &'a recovery::Record,
    relocation: Option<&'a undo::Relocation>,
    forward: bool,
    selection: Option<&'a entry_move::Selection>,
}

fn prepare_inner(
    state: &mut Store,
    request: Preparation<'_>,
    cancellation: &Cancellation,
    mut progress: impl FnMut(u64),
    mut checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let Preparation {
        cleanup_parent,
        id,
        ordinal,
        change,
        record,
        relocation,
        forward,
        selection,
    } = request;
    if record.payload {
        return Err(attention(
            "Claimed Recovery data cannot migrate as an Undo backup.",
        ));
    }
    checkpoint()?;
    if !forward {
        require_return_parent(state, id, ordinal, change)?;
    }
    let source = if forward {
        change.reverse_path().unwrap()
    } else {
        change.forward_path()
    };
    let destination = if forward {
        change.forward_path()
    } else {
        change.reverse_path().unwrap()
    };
    // An unrelated occupant must not be merged into an owned metadata container.
    if fs::symlink_metadata(destination).is_ok() {
        return Err(attention(
            "the folder receiving saved Recovery data is occupied; its contents are preserved",
        ));
    }
    require_only(&record.retained.stage, &["previous"])?;
    let mut source_stage = record.retained.stage.clone();
    if forward {
        let Change::Rename {
            saved: Some(saved), ..
        } = change
        else {
            unreachable!()
        };
        source_stage.parent_identity = Some(saved.parent_identity);
        source_stage.ownership = saved.ownership(true).map(Box::new);
    } else {
        source_stage.parent_identity = Some(Identity::from_metadata(&fs::metadata(
            source.parent().unwrap(),
        )?));
    }
    let previous = source.join("previous");
    let source_version = fingerprint::Version::read(&previous)?;
    let register =
        |created: &staging::Registration<'_>| state.register_preparing(id, destination, created);
    let verify = || {
        checkpoint()?;
        if source_stage.identity.matches(source) && source_version.matches(&previous) {
            Ok(())
        } else {
            Err(attention(
                "the saved source changed while its return was prepared",
            ))
        }
    };
    let mut copied = if let Some(selection) = selection {
        selection.copy_saved(cancellation, register, &mut progress, verify)?
    } else {
        fingerprint::prepare_copy(
            &previous,
            destination.parent().unwrap(),
            cancellation,
            register,
            &mut progress,
            verify,
        )?
    };
    check_cancel(cancellation)?;
    if !forward {
        require_return_parent(state, id, ordinal, change)?;
    }
    checkpoint()?;
    if !source_version.matches(&previous) || copied.source_identity != source_version.identity {
        return Err(attention(
            "the saved source changed before its return committed",
        ));
    }
    copied.opened.require_current()?;
    if copied.opened.fingerprint(cancellation)? != copied.fingerprint {
        return Err(attention(
            "the prepared saved copy changed before metadata restoration",
        ));
    }
    if copied.fingerprint != copied.source_fingerprint {
        copied.opened.restore_metadata(&copied.source_fingerprint)?;
    }
    let fingerprint = copied.opened.fingerprint(cancellation)?;
    copied.opened.require_current()?;
    if fingerprint != copied.source_fingerprint {
        return Err(attention(
            "the prepared saved copy changed during metadata restoration",
        ));
    }
    copied.stage.record_parent_access(cleanup_parent);
    let mut migration = Migration {
        cleanup: forward.then_some(forward::Phase::Ready),
        forward,
        id,
        ordinal,
        change: change.clone(),
        source: source_stage,
        source_version,
        source_fingerprint: copied.source_fingerprint,
        stage: copied.stage,
        copy_identity: copied.identity,
        fingerprint,
        context: record.context.clone(),
        relocation: relocation.cloned(),
        catalogued: false,
    };
    if forward {
        forward::bind_prepared(&mut migration, &copied.source, &copied.opened)?;
    }
    state.intent(&Pending::StageMigration {
        migration: Box::new(migration.clone()),
    })?;
    recover(state, &migration)
}

pub(super) fn recover(state: &mut Store, migration: &Migration) -> io::Result<()> {
    if migration.has_claim() {
        return forward::recover(state, migration);
    }
    validate_change(&migration.change)?;
    let Change::Rename {
        original,
        current,
        identity,
        retained: None,
        saved,
        ..
    } = &migration.change
    else {
        return Err(invalid("invalid saved-stage return change"));
    };
    let (source, destination) = if migration.forward {
        (original, current)
    } else {
        (current, original)
    };
    if (migration.forward
        && saved.as_ref().is_none_or(|saved| {
            *identity != migration.source.identity
                || Some(saved.parent_identity) != migration.source.parent_identity
        }))
        || source != &migration.source.directory
        || destination.parent() != migration.stage.directory.parent()
        || migration.stage.payload_identity != Some(migration.copy_identity)
        || migration.source.identity.device == migration.stage.identity.device
        || migration.source.directory == migration.stage.directory
        || migration.source.directory == *destination
    {
        return Err(invalid("invalid saved-stage return placement"));
    }
    validate_stage(Some(&migration.source), &migration.source.directory)?;
    validate_stage(Some(&migration.stage), migration.destination())?;
    let cancellation = Cancellation::default();
    let mut migration = migration.clone();
    if !migration.catalogued {
        if !migration.forward {
            require_return_parent(state, migration.id, migration.ordinal, &migration.change)?;
        }
        require_only(&migration.source, &["previous"])?;
        let previous = migration.source.directory.join("previous");
        if !migration.source_version.matches(&previous)
            || !migration
                .source_fingerprint
                .matches(&previous, &cancellation)?
        {
            return Err(attention(
                "the saved source changed before catalog ownership could move",
            ));
        }
        if !migration.stage.identity.matches(migration.destination()) {
            require_parent(&migration.stage)?;
            if !migration.stage.identity.matches(&migration.stage.directory) {
                return Err(attention(
                    "the prepared saved-data container is unavailable",
                ));
            }
            let prepared_previous = migration.stage.directory.join("previous");
            if fs::symlink_metadata(&prepared_previous)
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
            {
                require_only(&migration.stage, &["folder"])?;
                require_copy(&migration, &migration.stage.payload(), &cancellation)?;
                rename_no_replace(&migration.stage.payload(), &prepared_previous)?;
            }
            require_only(&migration.stage, &["previous"])?;
            require_copy(&migration, &prepared_previous, &cancellation)?;
            fs::File::open(&migration.stage.directory)?.sync_all()?;
            rename_no_replace(&migration.stage.directory, migration.destination())?;
            sync_parent(migration.destination())?;
        }
        let target = migration.destination_stage();
        require_parent(&target)?;
        require_only(&target, &["previous"])?;
        let previous = target.directory.join("previous");
        require_copy(&migration, &previous, &cancellation)?;
        let version = fingerprint::Version::read(&previous)?;
        if !migration.fingerprint.matches(&previous, &cancellation)? || !version.matches(&previous)
        {
            return Err(attention(
                "the saved destination changed before catalog ownership could move",
            ));
        }
        state.publish_saved_stage(&migration, &version)?;
        migration.catalogued = true;
    }
    cleanup_source(state, &migration, &cancellation)
}

fn cleanup_source(
    state: &mut Store,
    migration: &Migration,
    cancellation: &Cancellation,
) -> io::Result<()> {
    require_parent(&migration.destination_stage())?;
    require_copy(
        migration,
        &migration.destination().join("previous"),
        cancellation,
    )?;
    if fs::symlink_metadata(&migration.source.directory)
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    {
        return state.clear_intent();
    }
    require_parent(&migration.source)?;
    require_only(&migration.source, &["previous", "folder"])?;
    let previous = migration.source.directory.join("previous");
    let quarantine = migration.source.payload();
    if fs::symlink_metadata(&previous).is_ok() {
        if !migration.source_version.identity.matches(&previous)
            || !migration
                .source_fingerprint
                .matches(&previous, cancellation)?
        {
            return Err(attention(
                "the original saved version changed; both copies are preserved",
            ));
        }
        rename_no_replace(&previous, &quarantine)?;
    }
    if fs::symlink_metadata(&quarantine).is_ok() {
        if !migration.source_version.identity.matches(&quarantine)
            || !migration
                .source_fingerprint
                .matches(&quarantine, cancellation)?
        {
            if fs::symlink_metadata(&previous)
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
            {
                rename_no_replace(&quarantine, &previous)?;
            }
            return Err(attention(
                "the quarantined saved version changed; it is preserved",
            ));
        }
        fs::remove_file(&quarantine)?;
    }
    fs::File::open(&migration.source.directory)?.sync_all()?;
    fs::remove_dir(&migration.source.directory)?;
    sync_parent(&migration.source.directory)?;
    state.clear_intent()
}

pub(super) fn require_only(stage: &Stage, allowed: &[&str]) -> io::Result<()> {
    if !stage.identity.matches(&stage.directory) {
        return Err(attention("the owned saved-data folder changed"));
    }
    for entry in fs::read_dir(&stage.directory)? {
        if !allowed
            .iter()
            .any(|name| entry.as_ref().is_ok_and(|entry| entry.file_name() == *name))
        {
            return Err(attention(
                "the saved-data folder contains additional work; its contents are preserved",
            ));
        }
    }
    Ok(())
}
fn require_parent(stage: &Stage) -> io::Result<()> {
    if stage.parent_identity.is_some_and(|expected| {
        fs::metadata(stage.directory.parent().unwrap())
            .is_ok_and(|metadata| Identity::from_metadata(&metadata) == expected)
    }) {
        Ok(())
    } else {
        Err(attention("the folder containing saved data changed"))
    }
}
fn require_copy(migration: &Migration, path: &Path, cancellation: &Cancellation) -> io::Result<()> {
    if migration.stage.identity.matches(path.parent().unwrap())
        && migration.copy_identity.matches(path)
        && migration.fingerprint.matches(path, cancellation)?
    {
        Ok(())
    } else {
        Err(attention(
            "the returned saved version changed; source data is preserved",
        ))
    }
}
fn attention(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, message)
}

pub(super) fn require_return_parent(
    state: &Store,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    state.require_directory_parents(id, ordinal)?;
    if let Change::Rename {
        original,
        saved: Some(saved),
        ..
    } = change
    {
        let parent = original.parent().unwrap();
        let expected = state.resolve_for(id, ordinal, parent, saved.parent_identity)?;
        if !fs::metadata(parent)
            .is_ok_and(|metadata| Identity::from_metadata(&metadata) == expected)
        {
            return Err(attention(
                "the folder receiving saved data changed; its contents are preserved",
            ));
        }
    }
    Ok(())
}
