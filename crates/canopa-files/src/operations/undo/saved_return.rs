//! A selected saved destination has one consumed return name.
use super::*;
use rustix::fs::{Mode, OFlags, RenameFlags, open, openat, renameat_with};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::operations) enum Phase {
    Prepared,
    Returning,
    Refused,
    Returned,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct Return {
    pub origin: Box<Pending>,
    pub stage: Stage,
    entry: original::Original,
    version: fingerprint::Version,
    parent: original::Original,
    pub phase: Phase,
    pub incoming: Option<Identity>,
}
pub(in crate::operations) type Parts<'a> = (
    u64,
    u64,
    &'a Change,
    Option<&'a Relocation>,
    Option<&'a recovery::Record>,
);
impl Return {
    pub fn parts(&self) -> io::Result<Parts<'_>> {
        let Pending::Undo {
            id,
            ordinal,
            change,
            stage: None,
            relocation,
            displaced,
        } = self.origin.as_ref()
        else {
            return Err(attention());
        };
        if !matches!(
            change,
            Change::Rename {
                retained: Some(_),
                ..
            } | Change::RetainedOnly { .. }
        ) {
            return Err(attention());
        }
        Ok((
            *id,
            *ordinal,
            change,
            relocation.as_ref(),
            displaced.as_deref(),
        ))
    }
    pub fn validate(&self) -> io::Result<()> {
        let (_, _, change, _, _) = self.parts()?;
        validate_change(change)?;
        if matches!(change, Change::Rename { .. }) != self.incoming.is_some() {
            return Err(attention());
        }
        let retained = change.retained().ok_or_else(attention)?;
        if self.stage != staging::completed_cleanup(&retained.stage)?
            || self
                .stage
                .ownership
                .as_ref()
                .is_none_or(|owner| owner.roots().0 != self.parent)
            || retained
                .version
                .as_ref()
                .is_some_and(|v| v != &self.version)
        {
            return Err(attention());
        }
        Ok(())
    }
    pub fn change(&self) -> &Change {
        self.parts().unwrap().2
    }
    fn target(&self) -> &Path {
        self.change().forward_path()
    }
    pub fn id(&self) -> u64 {
        self.parts().map(|parts| parts.0).unwrap_or(0)
    }
}
fn attention() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "An unfinished saved destination return needs Recovery. Its original work folder and completed work are preserved.",
    )
}
fn directory(path: &Path) -> io::Result<fs::File> {
    open(
        path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(fs::File::from)
    .map_err(Into::into)
}
fn named(parent: &fs::File, name: &std::ffi::OsStr) -> io::Result<Option<fs::File>> {
    match openat(
        parent,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => Ok(Some(fs::File::from(fd))),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn sync(parent: &fs::File) -> io::Result<()> {
    fs::File::from(openat(
        parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()
}
struct Opened {
    parent: fs::File,
    target_parent: fs::File,
    root: fs::File,
    entry: fs::File,
}
impl Opened {
    fn open(task: &Return) -> io::Result<Self> {
        task.validate()?;
        let parent = directory(task.stage.directory.parent().ok_or_else(attention)?)?;
        let owner = task.stage.ownership.as_ref().ok_or_else(attention)?;
        owner.require_parent(&parent)?;
        let root = fs::File::from(openat(
            &parent,
            task.stage.directory.file_name().ok_or_else(attention)?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        owner.require_directory(&root)?;
        let entry = named(&root, std::ffi::OsStr::new("previous"))?.ok_or_else(attention)?;
        let target_parent = directory(task.target().parent().ok_or_else(attention)?)?;
        task.parent.require(&target_parent)?;
        let opened = Self {
            parent,
            target_parent,
            root,
            entry,
        };
        opened.require(task)?;
        Ok(opened)
    }
    fn require(&self, task: &Return) -> io::Result<()> {
        task.validate()?;
        let owner = task.stage.ownership.as_ref().ok_or_else(attention)?;
        owner.require_parent(&self.parent)?;
        owner.require_directory(&self.root)?;
        let current = directory(task.stage.directory.parent().ok_or_else(attention)?)?;
        owner.require_parent(&current)?;
        let root = named(
            &current,
            task.stage.directory.file_name().ok_or_else(attention)?,
        )?
        .ok_or_else(attention)?;
        owner.require_directory(&root)?;
        task.parent.require(&self.target_parent)?;
        task.parent
            .require(&directory(task.target().parent().ok_or_else(attention)?)?)?;
        task.entry.require(&self.entry)?;
        let entry = named(&self.root, std::ffi::OsStr::new("previous"))?.ok_or_else(attention)?;
        task.entry.require(&entry)?;
        if !task.version.matches_opened(&self.entry, false)?
            || !task.version.matches_opened(&entry, false)?
            || named(
                &self.target_parent,
                task.target().file_name().ok_or_else(attention)?,
            )?
            .is_some()
        {
            return Err(attention());
        }
        Ok(())
    }
}
pub(in crate::operations) struct Prepared {
    task: Return,
    opened: Opened,
}
impl Prepared {
    pub fn task(&self) -> &Return {
        &self.task
    }
    pub fn require(&self) -> io::Result<()> {
        if self.task.phase != Phase::Prepared {
            return Err(attention());
        }
        self.opened.require(&self.task)
    }
}
pub(in crate::operations) struct Transition<'a> {
    before: &'a Return,
    phase: Phase,
    opened: Option<&'a Opened>,
    refused: bool,
}
impl Transition<'_> {
    pub fn before(&self) -> &Return {
        self.before
    }
    pub fn after(&self) -> Return {
        let mut next = self.before.clone();
        next.phase = self.phase;
        next
    }
    pub fn require(&self) -> io::Result<()> {
        self.before.validate()?;
        match (self.before.phase, self.phase) {
            (Phase::Prepared | Phase::Refused, Phase::Returning) if !self.refused => {
                self.opened.ok_or_else(attention)?.require(self.before)
            }
            (Phase::Returning, Phase::Refused) if self.refused => {
                self.opened.ok_or_else(attention)?.require(self.before)
            }
            (Phase::Returning, Phase::Returned) if !self.refused && returned(self.before)? => {
                Ok(())
            }
            _ => Err(attention()),
        }
    }
}
fn returned(task: &Return) -> io::Result<bool> {
    let parent = directory(task.target().parent().ok_or_else(attention)?)?;
    task.parent.require(&parent)?;
    let Some(entry) = named(&parent, task.target().file_name().ok_or_else(attention)?)? else {
        return Ok(false);
    };
    Ok(task.entry.require(&entry).is_ok())
}
fn advance(
    state: &mut Store,
    task: &mut Return,
    phase: Phase,
    opened: Option<&Opened>,
) -> io::Result<()> {
    let transition = Transition {
        before: task,
        phase,
        opened,
        refused: false,
    };
    state.saved_return_phase(&transition)?;
    *task = transition.after();
    Ok(())
}
fn execute(state: &mut Store, task: &mut Return, opened: &Opened) -> io::Result<()> {
    advance(state, task, Phase::Returning, Some(opened))?;
    #[cfg(test)]
    abort_boundary::move_return_boundary::cut("returning", task.change());
    // This live capability may call rename exactly once. Recovery of Returning
    // can recognize the selected entry at its target, never replay this name.
    opened.require(task)?;
    if let Err(error) = renameat_with(
        &opened.root,
        c"previous",
        &opened.target_parent,
        task.target().file_name().ok_or_else(attention)?,
        RenameFlags::NOREPLACE,
    ) {
        // EACCES/EPERM report that this native call performed no rename. Only
        // the still-held original selection can attest that refusal. Unknown
        // errors and a crash in Returning never acquire another native call.
        if matches!(error, rustix::io::Errno::ACCESS | rustix::io::Errno::PERM)
            && opened.require(task).is_ok()
        {
            let transition = Transition {
                before: task,
                phase: Phase::Refused,
                opened: Some(opened),
                refused: true,
            };
            if state.saved_return_phase(&transition).is_ok() {
                *task = transition.after();
            }
            #[cfg(test)]
            abort_boundary::move_return_boundary::cut("refused", task.change());
        }
        return Err(error.into());
    }
    sync(&opened.root)?;
    sync(&opened.target_parent)?;
    #[cfg(test)]
    abort_boundary::move_return_boundary::cut("previous-renamed", task.change());
    advance(state, task, Phase::Returned, None)?;
    #[cfg(test)]
    abort_boundary::move_return_boundary::cut("returned", task.change());
    Ok(())
}
pub(super) fn restore(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    incoming: Option<Identity>,
) -> io::Result<()> {
    let origin = state.pending()?.ok_or_else(attention)?;
    let Pending::Undo {
        id: owner,
        ordinal: step,
        change: actual,
        stage: None,
        ..
    } = &origin
    else {
        return Err(attention());
    };
    if *owner != id || *step != ordinal || store::encode(actual)? != store::encode(change)? {
        return Err(attention());
    }
    let retained = change.retained().ok_or_else(attention)?;
    require_backup(retained)?;
    let canonical = state
        .change_before(id, Some(ordinal.checked_add(1).ok_or_else(attention)?))?
        .ok_or_else(attention)?
        .1;
    let reviewed = canonical.retained().is_some_and(|previous| {
        previous.version != retained.version || previous.recovery != retained.recovery
    }) && retained.version.is_some()
        && !retained.recovery;
    let stage = staging::completed_cleanup(&retained.stage)?;
    let parent = directory(stage.directory.parent().ok_or_else(attention)?)?;
    let owner = stage.ownership.as_ref().ok_or_else(attention)?;
    owner.require_parent(&parent)?;
    let root = fs::File::from(openat(
        &parent,
        stage.directory.file_name().ok_or_else(attention)?,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    owner.require_directory(&root)?;
    let entry = named(&root, std::ffi::OsStr::new("previous"))?.ok_or_else(attention)?;
    if Identity::from_metadata(&entry.metadata()?) != retained.observed_identity() {
        return Err(attention());
    }
    let selected = if reviewed && retained.version.is_some() {
        original::Original::capture(&entry)?
    } else {
        match owner.previous(&entry)? {
            Some(origin) => origin,
            None if retained.physical_identity.is_none() => retained
                .replacement_facts
                .as_ref()
                .ok_or_else(attention)?
                .original_previous(&entry)?,
            None => return Err(attention()),
        }
    };
    let target_parent = directory(change.forward_path().parent().ok_or_else(attention)?)?;
    let task = Return {
        origin: Box::new(origin),
        stage,
        entry: selected,
        version: fingerprint::Version::from_metadata(&entry.metadata()?),
        parent: original::Original::capture(&target_parent)?,
        phase: Phase::Prepared,
        incoming,
    };
    let mut prepared = Prepared {
        task,
        opened: Opened {
            parent,
            target_parent,
            root,
            entry,
        },
    };
    prepared.require()?;
    state.begin_saved_return(&prepared)?;
    #[cfg(test)]
    abort_boundary::move_return_boundary::cut("prepared", prepared.task.change());
    execute(state, &mut prepared.task, &prepared.opened)
}
pub(in crate::operations) fn recover(state: &mut Store, mut task: Return) -> io::Result<()> {
    state.require_saved_return(&task)?;
    match task.phase {
        Phase::Prepared | Phase::Refused => {
            let opened = Opened::open(&task)?;
            execute(state, &mut task, &opened)?;
        }
        Phase::Returning => {
            if !returned(&task)? {
                return Err(attention());
            }
            advance(state, &mut task, Phase::Returned, None)?;
        }
        Phase::Returned => {}
    }
    let (id, ordinal, change, relocation, displaced) = task.parts()?;
    let displaced = displaced.map(snapshot_displaced).transpose()?;
    state.reversed_with_recovery(id, ordinal, change, relocation, displaced.as_ref())
}
