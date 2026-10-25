//! One original Copy Undo task owns each source and payload namespace once.
use super::*;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, open, openat, renameat_with, unlinkat};
mod previous;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::operations) enum Phase {
    Prepared,
    Taking,
    Detached,
    Restoring,
    Restored,
    Consumed,
    Cleared,
    Preserved,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct Removal {
    pub id: u64,
    pub ordinal: u64,
    pub change: Change,
    pub stage: Stage,
    selected: Option<original::Original>,
    selected_version: Option<fingerprint::Version>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous: Option<Box<previous::Saved>>,
    #[serde(default, skip_serializing_if = "is_false")]
    previous_returned: bool,
    pub phase: Phase,
    detached: Option<fingerprint::Version>,
}
impl Removal {
    pub fn validate(&self) -> io::Result<()> {
        if !matches!(self.change, Change::Copied { .. }) {
            return Err(invalid("Expected an original Copy Undo removal."));
        }
        validate_change(&self.change)?;
        if self.selected.is_some() != self.selected_version.is_some()
            || (self.selected.is_none() && self.previous.is_none())
            || self.previous.is_some() != self.change.retained().is_some()
            || (self.previous_returned && self.previous.is_none())
            || (matches!(self.phase, Phase::Restoring | Phase::Restored) && self.previous.is_none())
            || (matches!(self.phase, Phase::Restored) && !self.previous_returned)
            || (self.previous_returned
                && matches!(
                    self.phase,
                    Phase::Prepared | Phase::Taking | Phase::Detached | Phase::Restoring
                ))
        {
            return Err(attention());
        }
        if let Some(saved) = &self.previous {
            saved.validate(self)?;
        }
        validate_stage(Some(&self.stage), self.change.forward_path())?;
        if self.stage.ownership.is_none() || staging::completed_cleanup(&self.stage)? != self.stage
        {
            return Err(invalid(
                "Copy Undo needs its original empty-only work folder.",
            ));
        }
        if matches!(
            self.phase,
            Phase::Detached | Phase::Consumed | Phase::Cleared
        ) && self.selected.is_some()
            && self.detached.is_none()
        {
            return Err(invalid("Copy Undo lost its recorded detached item."));
        }
        Ok(())
    }
    pub fn complete(&self) -> bool {
        matches!(self.phase, Phase::Cleared | Phase::Preserved)
            && (self.previous.is_none() || self.previous_returned)
    }
    pub fn cleanup_roots(&self) -> io::Result<Vec<Stage>> {
        if !self.complete() {
            return Err(attention());
        }
        let mut roots = vec![staging::completed_cleanup(&self.stage)?];
        if let Some(saved) = &self.previous {
            roots.push(saved.stage.clone());
        }
        Ok(roots)
    }
    pub fn kept_saved_cleanup(&self) -> io::Result<Option<Stage>> {
        self.validate()?;
        Ok(self
            .previous
            .as_ref()
            .filter(|_| self.previous_returned)
            .map(|saved| saved.stage.clone()))
    }
    pub fn storage_paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![
            self.change.forward_path().to_owned(),
            self.stage.directory.clone(),
        ];
        if let Some(saved) = &self.previous {
            paths.push(saved.stage.directory.clone());
        }
        paths
    }
    fn fingerprint(&self) -> &Fingerprint {
        let Change::Copied { fingerprint, .. } = &self.change else {
            unreachable!()
        };
        fingerprint
    }
}
fn attention() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Interrupted Copy Undo work is preserved. Copy it or keep its work folder in Recovery before retrying Undo.",
    )
}
struct Root {
    parent: fs::File,
    directory: fs::File,
}
impl Root {
    fn open(removal: &Removal) -> io::Result<Self> {
        removal.validate()?;
        Self::stage(&removal.stage)
    }
    fn stage(stage: &Stage) -> io::Result<Self> {
        let parent = fs::File::from(open(
            stage.directory.parent().unwrap(),
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let owner = stage.ownership.as_ref().ok_or_else(attention)?;
        owner.require_parent(&parent)?;
        let directory = fs::File::from(openat(
            &parent,
            stage.directory.file_name().unwrap(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        owner.require_directory(&directory)?;
        Ok(Self { parent, directory })
    }
    fn require(&self, removal: &Removal) -> io::Result<()> {
        let fresh = Self::open(removal)?;
        let owner = removal.stage.ownership.as_ref().unwrap();
        owner.require_parent(&self.parent)?;
        owner.require_directory(&self.directory)?;
        owner.require_directory(&fresh.directory)
    }
    fn payload(&self) -> io::Result<Option<fs::File>> {
        match openat(
            &self.directory,
            c"folder",
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(file) => Ok(Some(fs::File::from(file))),
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn selected_at_source(&self, removal: &Removal) -> io::Result<bool> {
        let file = match openat(
            &self.parent,
            removal.change.forward_path().file_name().unwrap(),
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(file) => fs::File::from(file),
            Err(rustix::io::Errno::NOENT) => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        Ok(Some(original::Original::capture(&file)?) == removal.selected)
    }
    fn source_absent(&self, removal: &Removal) -> io::Result<bool> {
        match openat(
            &self.parent,
            removal.change.forward_path().file_name().unwrap(),
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Err(rustix::io::Errno::NOENT) => Ok(true),
            Ok(_) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    fn sync(&self) -> io::Result<()> {
        self.directory.sync_all()?;
        fs::File::from(openat(
            &self.parent,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?)
        .sync_all()
    }
    fn empty(&self, removal: &Removal) -> io::Result<()> {
        self.require(removal)?;
        unlinkat(
            &self.parent,
            removal.stage.directory.file_name().unwrap(),
            AtFlags::REMOVEDIR,
        )?;
        fs::File::from(openat(
            &self.parent,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?)
        .sync_all()
    }
}
pub(in crate::operations) struct Prepared {
    removal: Removal,
    source: Option<fingerprint::source::Opened>,
    created: staging::Opened,
    previous: Option<previous::Opened>,
    temporary: Option<tempfile::TempDir>,
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if self.temporary.is_some() {
            let _ = self.created.clean_empty();
        }
    }
}
impl Prepared {
    pub fn removal(&self) -> &Removal {
        &self.removal
    }
    pub fn require(&self) -> io::Result<()> {
        self.removal.validate()?;
        if self.removal.phase != Phase::Prepared {
            return Err(attention());
        }
        self.created.require_created(&self.removal.stage)?;
        if let Some(selected) = self.removal.selected {
            let source = self.source.as_ref().ok_or_else(attention)?;
            self.created
                .registration(source, &self.removal.stage)?
                .require()?;
            selected.require(source.entry())?;
            if source.path() != self.removal.change.forward_path()
                || !self
                    .removal
                    .selected_version
                    .as_ref()
                    .ok_or_else(attention)?
                    .matches_opened(source.entry(), false)?
            {
                return Err(attention());
            }
        } else if !Root::open(&self.removal)?.source_absent(&self.removal)? {
            return Err(attention());
        }
        if let Some(previous) = &self.previous {
            previous.require(
                self.removal.previous.as_ref().ok_or_else(attention)?,
                &self.removal,
            )?;
        }
        Ok(())
    }
    fn keep(&mut self) {
        if let Some(temporary) = self.temporary.take() {
            let _ = temporary.keep();
        }
    }
}
pub(in crate::operations) struct Transition<'a> {
    before: Removal,
    phase: Phase,
    detached: Option<fingerprint::Version>,
    root: &'a Root,
    file: Option<&'a fs::File>,
}
impl Transition<'_> {
    pub fn before(&self) -> &Removal {
        &self.before
    }
    pub fn after(&self) -> Removal {
        // Immutable task fields come only from the bound predecessor. This
        // capability cannot carry a second Change, root or selected entry.
        let mut after = self.before.clone();
        after.phase = self.phase;
        if self.phase == Phase::Restored {
            after.previous_returned = true;
        }
        if self.phase == Phase::Detached {
            after.detached = self.detached.clone();
        }
        after
    }
    pub fn require(&self) -> io::Result<()> {
        let after = self.after();
        self.before.validate()?;
        after.validate()?;
        self.root.require(&self.before)?;
        match (self.before.phase, after.phase) {
            (Phase::Prepared, Phase::Taking) => {
                if let Some(saved) = &self.before.previous {
                    saved.require_original(&self.before)?;
                }
                let file = self.file.ok_or_else(attention)?;
                self.before.selected.ok_or_else(attention)?.require(file)?;
                if !self
                    .before
                    .selected_version
                    .as_ref()
                    .ok_or_else(attention)?
                    .matches_opened(file, false)?
                    || !self.root.selected_at_source(&self.before)?
                    || self.root.payload()?.is_some()
                {
                    return Err(attention());
                }
            }
            (Phase::Taking, Phase::Detached) => {
                let file = self.file.ok_or_else(attention)?;
                self.before.selected.ok_or_else(attention)?.require(file)?;
                let named = self.root.payload()?.ok_or_else(attention)?;
                self.before
                    .selected
                    .ok_or_else(attention)?
                    .require(&named)?;
                let version = after.detached.as_ref().ok_or_else(attention)?;
                if !version.matches_opened(file, false)?
                    || !version.matches_opened(&named, false)?
                {
                    return Err(attention());
                }
            }
            (Phase::Detached | Phase::Restored, Phase::Consumed) => {
                if self.before.previous.is_some() && !self.before.previous_returned {
                    return Err(attention());
                }
                require_payload_snapshot(
                    self.root,
                    &self.before,
                    self.file.ok_or_else(attention)?,
                )?;
            }
            (Phase::Consumed, Phase::Cleared) => {
                if self.root.payload()?.is_some() {
                    return Err(attention());
                }
            }
            (Phase::Prepared | Phase::Detached, Phase::Restoring) => {
                previous::require_start(&self.before, self.root)?;
            }
            (Phase::Restoring, Phase::Restored) => {
                if !after.previous_returned || !previous::returned(&self.before, self.root)? {
                    return Err(attention());
                }
            }
            (Phase::Restored, Phase::Cleared) => {
                if self.before.selected.is_some()
                    || !self.before.previous_returned
                    || self.root.payload()?.is_some()
                {
                    return Err(attention());
                }
            }
            (Phase::Taking, Phase::Preserved) => {
                if self.root.selected_at_source(&self.before)? {
                    return Err(attention());
                }
            }
            (Phase::Detached | Phase::Restored | Phase::Consumed, Phase::Preserved) => {}
            _ => return Err(attention()),
        }
        Ok(())
    }
}
fn advance(
    state: &mut Store,
    removal: &mut Removal,
    phase: Phase,
    root: &Root,
    file: Option<&fs::File>,
) -> io::Result<()> {
    let detached = if phase == Phase::Detached {
        Some(fingerprint::Version::from_metadata(
            &file.ok_or_else(attention)?.metadata()?,
        ))
    } else {
        None
    };
    let transition = Transition {
        before: removal.clone(),
        phase,
        detached,
        root,
        file,
    };
    state.copy_undo_phase(&transition)?;
    *removal = transition.after();
    #[cfg(test)]
    if removal.previous.is_some() {
        abort_boundary::saved_copy_boundary::cut(
            match phase {
                Phase::Prepared => "prepared",
                Phase::Taking => "taking",
                Phase::Detached => "detached",
                Phase::Restoring => "restoring",
                Phase::Restored => "restored",
                Phase::Consumed => "consumed",
                Phase::Cleared => "cleared",
                Phase::Preserved => "preserved",
            },
            &removal.change,
            &removal.stage,
        );
    }
    Ok(())
}
fn require_payload(
    root: &Root,
    removal: &Removal,
    file: &fs::File,
    cancellation: &Cancellation,
) -> io::Result<()> {
    require_payload_snapshot(root, removal, file)?;
    if Fingerprint::read_opened(file, cancellation)? != *removal.fingerprint() {
        return Err(attention());
    }
    require_payload_snapshot(root, removal, file)
}
fn require_payload_snapshot(root: &Root, removal: &Removal, file: &fs::File) -> io::Result<()> {
    root.require(removal)?;
    removal.selected.ok_or_else(attention)?.require(file)?;
    let named = root.payload()?.ok_or_else(attention)?;
    removal.selected.ok_or_else(attention)?.require(&named)?;
    let version = removal.detached.as_ref().ok_or_else(attention)?;
    if !version.matches_opened(file, false)? || !version.matches_opened(&named, false)? {
        return Err(attention());
    }
    Ok(())
}
fn remove_payload(
    state: &mut Store,
    removal: &mut Removal,
    root: &Root,
    file: &fs::File,
    cancellation: &Cancellation,
) -> io::Result<()> {
    require_payload(root, removal, file, cancellation)?;
    advance(state, removal, Phase::Consumed, root, Some(file))?;
    #[cfg(test)]
    abort_boundary::copy_boundary::cut("consumed", removal);
    require_payload(root, removal, file, cancellation)?;
    unlinkat(&root.directory, c"folder", AtFlags::empty())?;
    root.sync()?;
    #[cfg(test)]
    abort_boundary::copy_boundary::cut("unlinked", removal);
    #[cfg(test)]
    abort_boundary::saved_copy_boundary::cut("unlinked", &removal.change, &removal.stage);
    advance(state, removal, Phase::Cleared, root, None)?;
    #[cfg(test)]
    abort_boundary::copy_boundary::cut("cleared", removal);
    Ok(())
}
fn take(
    state: &mut Store,
    removal: &mut Removal,
    root: &Root,
    source: &fingerprint::source::Opened,
    cancellation: &Cancellation,
) -> io::Result<()> {
    source.require_current()?;
    removal
        .selected
        .ok_or_else(attention)?
        .require(source.entry())?;
    if Fingerprint::read_opened(source.entry(), cancellation)? != *removal.fingerprint() {
        return Err(attention());
    }
    advance(state, removal, Phase::Taking, root, Some(source.entry()))?;
    #[cfg(test)]
    abort_boundary::copy_boundary::cut("taking", removal);
    source.require_current()?;
    root.require(removal)?;
    renameat_with(
        &root.parent,
        removal.change.forward_path().file_name().unwrap(),
        &root.directory,
        c"folder",
        RenameFlags::NOREPLACE,
    )?;
    root.sync()?;
    #[cfg(test)]
    abort_boundary::copy_boundary::cut("renamed", removal);
    #[cfg(test)]
    if removal.previous.is_some() {
        abort_boundary::saved_copy_boundary::cut("copy-renamed", &removal.change, &removal.stage);
    }
    advance(state, removal, Phase::Detached, root, Some(source.entry()))?;
    #[cfg(test)]
    abort_boundary::copy_boundary::cut("detached", removal);
    if removal.previous.is_some() {
        previous::restore(state, removal, root, cancellation)?;
    }
    remove_payload(state, removal, root, source.entry(), cancellation)
}
pub(super) fn begin(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
) -> io::Result<Stage> {
    let (step, canonical) = state
        .change_before(id, Some(ordinal.checked_add(1).ok_or_else(attention)?))?
        .ok_or_else(attention)?;
    if step != ordinal {
        return Err(attention());
    }
    let reviewed = store::encode(&canonical)? != store::encode(change)?;
    let (previous, saved) = match change.retained() {
        Some(retained) => {
            let (opened, saved) = previous::Opened::capture(retained, reviewed)?;
            (Some(opened), Some(saved))
        }
        None => (None, None),
    };
    let source = match fingerprint::source::Opened::open(change.forward_path()) {
        Ok(source) => Some(source),
        Err(e) if e.kind() == io::ErrorKind::NotFound && saved.is_some() => None,
        Err(e) => return Err(e),
    };
    if let Some(source) = &source {
        let expected = state.resolve_for(
            id,
            ordinal,
            change.forward_path(),
            match change {
                Change::Copied { identity, .. } => *identity,
                _ => return Err(attention()),
            },
        )?;
        let Change::Copied { fingerprint, .. } = change else {
            return Err(attention());
        };
        if Identity::from_metadata(&source.entry().metadata()?) != expected
            || Fingerprint::read_opened(source.entry(), cancellation)? != *fingerprint
        {
            return Err(attention());
        }
    }
    let parent = match &source {
        Some(source) => source.parent(),
        None => &previous.as_ref().ok_or_else(attention)?.root.parent,
    };
    let selected = source
        .as_ref()
        .map(|s| original::Original::capture(s.entry()))
        .transpose()?;
    let selected_version = source
        .as_ref()
        .map(|s| {
            s.entry()
                .metadata()
                .map(|m| fingerprint::Version::from_metadata(&m))
        })
        .transpose()?;
    let (created, stage, temporary) =
        staging::Opened::prepare_in(change.forward_path().parent().unwrap(), parent)?;
    let mut prepared = Prepared {
        removal: Removal {
            id,
            ordinal,
            change: change.clone(),
            stage,
            selected,
            selected_version,
            previous: saved,
            previous_returned: false,
            phase: Phase::Prepared,
            detached: None,
        },
        source,
        created,
        previous,
        temporary: Some(temporary),
    };
    prepared.require()?;
    state.begin_copy_undo(&prepared)?;
    prepared.keep();
    let mut removal = prepared.removal.clone();
    #[cfg(test)]
    abort_boundary::copy_boundary::cut("prepared", &removal);
    #[cfg(test)]
    abort_boundary::saved_copy_boundary::cut("prepared", &removal.change, &removal.stage);
    let root = Root::open(&removal)?;
    if let Some(source) = &prepared.source {
        take(state, &mut removal, &root, source, cancellation)?;
    } else {
        previous::restore(state, &mut removal, &root, cancellation)?;
        advance(state, &mut removal, Phase::Cleared, &root, None)?;
    }
    Ok(removal.stage)
}
fn finish_restored(
    state: &mut Store,
    removal: &mut Removal,
    root: &Root,
    cancellation: &Cancellation,
) -> io::Result<()> {
    if removal.selected.is_none() {
        return advance(state, removal, Phase::Cleared, root, None);
    }
    if let Some(file) = root.payload()?
        && require_payload(root, removal, &file, cancellation).is_ok()
    {
        remove_payload(state, removal, root, &file, cancellation)
    } else {
        advance(state, removal, Phase::Preserved, root, None)
    }
}

/// Old generic journals never recorded consumed payload names. They can only
/// finish metadata for an independently proved returned saved entry; the
/// resulting receipt has empty-root rights and cannot unlink any payload.
pub(super) fn recover_legacy(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: Option<&Stage>,
) -> io::Result<()> {
    state.require_original_change_history(id, ordinal)?;
    let (step, canonical) = state
        .change_before(id, Some(ordinal.checked_add(1).ok_or_else(attention)?))?
        .ok_or_else(attention)?;
    if step != ordinal || store::encode(&canonical)? != store::encode(change)? {
        return Err(attention());
    }
    let retained = canonical.retained().ok_or_else(attention)?;
    let expected = staging::completed_cleanup(&retained.stage)?;
    if let Some(stage) = stage
        && staging::completed_cleanup(stage)? != expected
    {
        return Err(attention());
    }
    let saved_root = Root::stage(&expected)?;
    match openat(
        &saved_root.directory,
        c"previous",
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Err(rustix::io::Errno::NOENT) => {}
        _ => return Err(attention()),
    }
    let parent = fs::File::from(open(
        change.forward_path().parent().ok_or_else(attention)?,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let target = fs::File::from(openat(
        &parent,
        change.forward_path().file_name().ok_or_else(attention)?,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let owned = expected.ownership.as_ref().ok_or_else(attention)?;
    if owned.previous(&target)?.is_none() {
        if retained.physical_identity.is_some() {
            return Err(attention());
        }
        retained
            .replacement_facts
            .as_ref()
            .ok_or_else(attention)?
            .original_previous(&target)?;
    }
    state.reversed_with_recovery(id, ordinal, change, None, None)
}
pub(in crate::operations) fn recover(
    state: &mut Store,
    mut removal: Removal,
    cancellation: &Cancellation,
) -> io::Result<()> {
    state.require_copy_undo(&removal)?;
    if removal.complete() {
        return state.reversed_with_recovery(
            removal.id,
            removal.ordinal,
            &removal.change,
            None,
            None,
        );
    }
    let root = Root::open(&removal)?;
    match removal.phase {
        Phase::Prepared if removal.selected.is_none() => {
            previous::restore(state, &mut removal, &root, cancellation)?;
            finish_restored(state, &mut removal, &root, cancellation)?;
        }
        Phase::Prepared => {
            if let Ok(source) = fingerprint::source::Opened::open(removal.change.forward_path())
                && removal
                    .selected
                    .ok_or_else(attention)?
                    .require(source.entry())
                    .is_ok()
                && removal
                    .selected_version
                    .as_ref()
                    .ok_or_else(attention)?
                    .matches_opened(source.entry(), false)?
                && Fingerprint::read_opened(source.entry(), cancellation)? == *removal.fingerprint()
            {
                take(state, &mut removal, &root, &source, cancellation)?;
            } else {
                root.empty(&removal)?;
                return state.clear_intent();
            }
        }
        Phase::Taking => {
            if root.selected_at_source(&removal)? {
                root.empty(&removal)?;
                return state.clear_intent();
            }
            if removal.previous.is_some() {
                return Err(attention());
            }
            advance(state, &mut removal, Phase::Preserved, &root, None)?;
        }
        Phase::Detached if removal.previous.is_some() => {
            previous::restore(state, &mut removal, &root, cancellation)?;
            finish_restored(state, &mut removal, &root, cancellation)?;
        }
        Phase::Detached => {
            finish_restored(state, &mut removal, &root, cancellation)?;
        }
        Phase::Restoring => {
            if !previous::returned(&removal, &root)? {
                return Err(attention());
            }
            advance(state, &mut removal, Phase::Restored, &root, None)?;
            finish_restored(state, &mut removal, &root, cancellation)?;
        }
        Phase::Restored => {
            finish_restored(state, &mut removal, &root, cancellation)?;
        }
        Phase::Consumed => {
            advance(state, &mut removal, Phase::Preserved, &root, None)?;
        }
        Phase::Cleared | Phase::Preserved => return Err(attention()),
    }
    state.reversed_with_recovery(removal.id, removal.ordinal, &removal.change, None, None)
}
