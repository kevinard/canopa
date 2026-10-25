//! One disposal namespace per preparation. Restart never repeats a consumed unlink.
use super::*;
use crate::operations::store::encode;
mod undo_return;
pub(in crate::operations) use undo_return::Return;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::operations) enum Phase {
    Reserved,
    Claimed,
    Consumed,
    CopyCleared,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct Disposal {
    pub id: u64,
    #[serde(with = "crate::path_serde")]
    pub destination: PathBuf,
    pub original: Stage,
    pub holding: Stage,
    pub phase: Phase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo: Option<Box<Pending>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returned: Option<Return>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub empty_copy: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}
impl Disposal {
    pub fn preparation(&self) -> Pending {
        if let Some(undo) = &self.undo {
            return (**undo).clone();
        }
        Pending::Preparing {
            id: self.id,
            destination: self.destination.clone(),
            stage: self.original.clone(),
        }
    }
    fn pending(&self) -> Pending {
        Pending::Disposing {
            disposal: Box::new(self.clone()),
        }
    }
    pub fn validate(&self) -> io::Result<()> {
        if let Some(undo) = &self.undo {
            let Pending::Undo {
                id, change, stage, ..
            } = undo.as_ref()
            else {
                return Err(attention());
            };
            let original = match change {
                Change::SourceRemoved { original, .. } => original,
                Change::Copied {
                    path,
                    retained: None,
                    ..
                } if self.empty_copy && stage.is_none() => path,
                Change::Rename { original, .. } | Change::Trashed { original, .. }
                    if self.empty_copy && stage.is_none() =>
                {
                    original
                }
                Change::RetainedOnly { path, .. } if self.empty_copy && stage.is_none() => path,
                _ => return Err(attention()),
            };
            let expected = stage.as_ref().or_else(|| {
                self.empty_copy
                    .then(|| self.displaced().map(|r| &r.retained.stage))
                    .flatten()
            });
            if *id != self.id || original != &self.destination || expected != Some(&self.original) {
                return Err(attention());
            }
        }
        if self.phase == Phase::CopyCleared || self.returned.is_some() {
            self.displaced().ok_or_else(attention)?;
            if self.phase != Phase::CopyCleared {
                return Err(attention());
            }
        }
        if let Some(returned) = &self.returned {
            returned.validate(self)?;
        }
        ordinary(&self.original, &self.destination)?;
        ordinary(&self.holding, &self.destination)?;
        if self.empty_copy {
            if self.phase != Phase::CopyCleared
                || self.displaced().is_none()
                || self.original != self.holding
            {
                return Err(attention());
            }
            return Ok(());
        }
        let a = self.original.ownership.as_ref().unwrap();
        let b = self.holding.ownership.as_ref().unwrap();
        if self.original.directory == self.holding.directory
            || self.original.identity == self.holding.identity
            || self.original.payload_identity.is_none()
            || self.original.payload_identity != self.holding.payload_identity
            || a.payload.is_none()
            || a.payload != b.payload
            || a.parent != b.parent
            || b.previous.is_some()
        {
            return Err(attention());
        }
        Ok(())
    }
    fn displaced(&self) -> Option<&recovery::Record> {
        match self.undo.as_deref()? {
            Pending::Undo {
                displaced: Some(record),
                ..
            } => Some(record),
            _ => None,
        }
    }
    fn stages(&self) -> [Option<&Stage>; 4] {
        [
            if self.empty_copy
                && self
                    .displaced()
                    .is_some_and(|r| r.retained.stage == self.original)
            {
                None
            } else {
                Some(&self.original)
            },
            if self.empty_copy {
                None
            } else {
                Some(&self.holding)
            },
            self.displaced().map(|r| &r.retained.stage),
            self.returned.as_ref().and_then(|r| r.holding.as_ref()),
        ]
    }
}
fn attention() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Interrupted staging disposal needs review; retained or reused entries are preserved.",
    )
}
fn ordinary(stage: &Stage, target: &Path) -> io::Result<()> {
    validate_stage(Some(stage), target)?;
    if stage.ownership.is_none()
        || stage.directory_claim.is_some()
        || stage.replacement.is_some()
        || stage.entry_move.is_some()
        || stage.source_claim.is_some()
        || stage.sibling_payload
    {
        return Err(attention());
    }
    Ok(())
}
struct Root {
    parent: fs::File,
    directory: fs::File,
}
/// A fresh, read-only observation of both original work namespaces. Missing
/// roots are distinct from replacement roots, which must never be adopted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::operations) struct Review([Option<RootReview>; 4]);

#[derive(Clone, Debug, PartialEq, Eq)]
struct RootReview {
    directory: fingerprint::Version,
    payload: Option<fingerprint::Version>,
}
impl Review {
    pub fn contains(&self, part: u8) -> bool {
        let (index, container) = match part {
            0 => (0, false),
            1 => (1, false),
            2 => (0, true),
            3 => (1, true),
            4 => (2, false),
            5 => (2, true),
            6 => (3, false),
            7 => (3, true),
            _ => return false,
        };
        self.0[index]
            .as_ref()
            .is_some_and(|root| container || root.payload.is_some())
    }
}

pub(in crate::operations) struct Retention {
    disposal: Disposal,
    roots: [Option<Root>; 4],
    review: Review,
}
impl Retention {
    pub fn open(disposal: &Disposal) -> io::Result<Self> {
        disposal.validate()?;
        let stages = disposal.stages();
        let roots = [
            stages[0].map(Root::open).transpose()?.flatten(),
            stages[1].map(Root::open).transpose()?.flatten(),
            stages[2].map(Root::open).transpose()?.flatten(),
            stages[3].map(Root::open).transpose()?.flatten(),
        ];
        let mut this = Self {
            disposal: disposal.clone(),
            roots,
            review: Review([None, None, None, None]),
        };
        this.review = this.observe()?;
        Ok(this)
    }
    fn observe(&self) -> io::Result<Review> {
        let mut observed = [None, None, None, None];
        for (index, stage) in self.disposal.stages().into_iter().enumerate() {
            let Some(stage) = stage else {
                continue;
            };
            match &self.roots[index] {
                Some(root) => {
                    let parent = fs::File::from(open(
                        stage.directory.parent().ok_or_else(attention)?,
                        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                        Mode::empty(),
                    )?);
                    stage
                        .ownership
                        .as_ref()
                        .ok_or_else(attention)?
                        .parent
                        .require(&parent)?;
                    root.current(stage)?;
                    let payload = (if index == 2 {
                        match named(&root.directory, c"previous") {
                            Ok(file) => Some(file),
                            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                            Err(e) => return Err(e),
                        }
                    } else {
                        root.payload()?
                    })
                    .map(|file| {
                        file.metadata()
                            .map(|m| fingerprint::Version::from_metadata(&m))
                    })
                    .transpose()?;
                    observed[index] = Some(RootReview {
                        directory: fingerprint::Version::from_metadata(&root.directory.metadata()?),
                        payload,
                    });
                }
                None => {
                    if Root::open(stage)?.is_some() {
                        return Err(attention());
                    }
                }
            }
        }
        Ok(Review(observed))
    }
    pub fn review(&self) -> Review {
        self.review.clone()
    }
    pub fn require(&self, reviewed: &Review) -> io::Result<()> {
        if &self.observe()? != reviewed {
            return Err(attention());
        }
        Ok(())
    }
}

pub(in crate::operations) fn container_version(stage: &Stage) -> io::Result<fingerprint::Version> {
    let root = Root::open(stage)?.ok_or_else(attention)?;
    root.current(stage)?;
    Ok(fingerprint::Version::from_metadata(
        &root.directory.metadata()?,
    ))
}
impl Root {
    fn open(stage: &Stage) -> io::Result<Option<Self>> {
        let owner = stage.ownership.as_ref().ok_or_else(attention)?;
        let parent = fs::File::from(open(
            stage.directory.parent().ok_or_else(attention)?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        owner.parent.require(&parent)?;
        let directory = match openat(
            &parent,
            stage.directory.file_name().ok_or_else(attention)?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(file) => fs::File::from(file),
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        owner.directory.require(&directory)?;
        Ok(Some(Self { parent, directory }))
    }
    fn current(&self, stage: &Stage) -> io::Result<()> {
        let owner = stage.ownership.as_ref().ok_or_else(attention)?;
        owner.parent.require(&self.parent)?;
        owner.directory.require(&named(
            &self.parent,
            stage.directory.file_name().ok_or_else(attention)?,
        )?)
    }
    fn only(&self, allowed: &[&[u8]]) -> io::Result<()> {
        let readable = openat(
            &self.directory,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        for entry in rustix::fs::Dir::new(readable)? {
            let entry = entry?;
            let name = entry.file_name().to_bytes();
            if name != b"." && name != b".." && !allowed.contains(&name) {
                return Err(attention());
            }
        }
        Ok(())
    }
    fn payload(&self) -> io::Result<Option<fs::File>> {
        match named(&self.directory, c"folder") {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
    fn empty(self, stage: &Stage) -> io::Result<()> {
        self.only(&[])?;
        self.current(stage)?;
        rustix::fs::unlinkat(
            &self.parent,
            stage.directory.file_name().ok_or_else(attention)?,
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
/// Constructed only while creating a holding folder or performing a verified phase.
/// Decoded journal fields cannot supply the actual creation handles.
pub(in crate::operations) struct Transition {
    before: Pending,
    after: Pending,
    created: Option<(Opened, Root, fs::File)>,
    returning: Option<undo_return::Creation>,
    empty: bool,
}
impl Transition {
    pub fn before(&self) -> &Pending {
        &self.before
    }
    pub fn after(&self) -> &Pending {
        &self.after
    }
    pub fn require(&self) -> io::Result<()> {
        let Pending::Disposing { disposal: after } = &self.after else {
            return Err(attention());
        };
        after.validate()?;
        if self.empty {
            if !after.empty_copy
                || after.phase != Phase::CopyCleared
                || after.returned.is_some()
                || encode(&self.before)? != encode(&after.preparation())?
            {
                return Err(attention());
            }
            if let Some(root) = Root::open(&after.original)? {
                root.current(&after.original)?;
                if after
                    .displaced()
                    .is_none_or(|r| r.retained.stage != after.original)
                    && root.payload()?.is_some()
                {
                    return Err(attention());
                }
            }
            return Ok(());
        }
        if let Pending::Disposing { disposal: before } = &self.before
            && before.phase == Phase::CopyCleared
        {
            return undo_return::require_transition(before, after, self.returning.as_ref());
        }
        match (&self.before, &self.created) {
            (_, Some((holding, source, payload))) => {
                if encode(&self.before)? != encode(&after.preparation())?
                    || after.phase != Phase::Reserved
                {
                    return Err(attention());
                }
                source.current(&after.original)?;
                after
                    .original
                    .ownership
                    .as_ref()
                    .unwrap()
                    .require_payload(payload)?;
                after
                    .original
                    .ownership
                    .as_ref()
                    .unwrap()
                    .require_payload(&named(&source.directory, c"folder")?)?;
                holding.require_current()?;
                let proof = after.holding.ownership.as_ref().unwrap();
                proof.parent.require(&holding.parent)?;
                proof.directory.require(&holding.directory)?;
                if holding.identity != after.holding.identity
                    || holding.parent_path.join(&holding.name) != after.holding.directory
                {
                    return Err(attention());
                }
            }
            (Pending::Disposing { disposal: before }, None) => {
                let expected = match before.phase {
                    Phase::Reserved => Phase::Claimed,
                    Phase::Claimed => Phase::Consumed,
                    Phase::Consumed if before.displaced().is_some() => Phase::CopyCleared,
                    Phase::Consumed | Phase::CopyCleared => return Err(attention()),
                };
                let mut normalized = (**after).clone();
                normalized.phase = before.phase;
                if after.phase != expected || encode(&normalized)? != encode(before.as_ref())? {
                    return Err(attention());
                }
                if after.phase == Phase::CopyCleared
                    && (Root::open(&after.original)?.is_some()
                        || Root::open(&after.holding)?.is_some())
                {
                    return Err(attention());
                }
            }
            _ => return Err(attention()),
        }
        Ok(())
    }
}
fn advance(state: &mut Store, disposal: &mut Disposal, phase: Phase) -> io::Result<()> {
    let before = disposal.pending();
    let mut after = disposal.clone();
    after.phase = phase;
    state.disposal_transition(&Transition {
        before,
        after: after.pending(),
        created: None,
        returning: None,
        empty: false,
    })?;
    *disposal = after;
    Ok(())
}
pub(in crate::operations) fn begin(
    state: &mut Store,
    id: u64,
    destination: PathBuf,
    stage: Stage,
    cancellation: &Cancellation,
) -> io::Result<()> {
    begin_task(state, id, destination, stage, None, cancellation)
}
pub(in crate::operations) fn begin_undo(
    state: &mut Store,
    cancellation: &Cancellation,
) -> io::Result<()> {
    let pending = state.pending()?.ok_or_else(attention)?;
    let Pending::Undo {
        id,
        change: Change::SourceRemoved { original, .. },
        stage: Some(stage),
        ..
    } = &pending
    else {
        return Err(attention());
    };
    begin_task(
        state,
        *id,
        original.clone(),
        stage.clone(),
        Some(Box::new(pending)),
        cancellation,
    )
}
pub(in crate::operations) fn begin_empty_undo(
    state: &mut Store,
    cancellation: &Cancellation,
) -> io::Result<()> {
    state.require_bound_forward()?;
    let value = state.pending()?.ok_or_else(attention)?;
    let Pending::Undo {
        id,
        change,
        stage,
        displaced: Some(record),
        ..
    } = &value
    else {
        return Err(attention());
    };
    let original = match change {
        Change::SourceRemoved { original, .. } => original,
        Change::Copied {
            path,
            retained: None,
            ..
        } if stage.is_none() => path,
        Change::Rename { original, .. } | Change::Trashed { original, .. } if stage.is_none() => {
            original
        }
        Change::RetainedOnly { path, .. } if stage.is_none() => path,
        _ => return Err(attention()),
    };
    let stage = stage.as_ref().unwrap_or(&record.retained.stage).clone();
    let mut disposal = Disposal {
        id: *id,
        destination: original.clone(),
        original: stage.clone(),
        holding: stage,
        phase: Phase::CopyCleared,
        undo: Some(Box::new(value.clone())),
        returned: None,
        empty_copy: true,
    };
    state.disposal_transition(&Transition {
        before: value,
        after: disposal.pending(),
        created: None,
        returning: None,
        empty: true,
    })?;
    recover(state, &mut disposal, cancellation)
}
fn begin_task(
    state: &mut Store,
    id: u64,
    destination: PathBuf,
    stage: Stage,
    undo: Option<Box<Pending>>,
    cancellation: &Cancellation,
) -> io::Result<()> {
    ordinary(&stage, &destination)?;
    state.require_bound_forward()?;
    let expected = undo
        .as_deref()
        .cloned()
        .unwrap_or_else(|| Pending::Preparing {
            id,
            destination: destination.clone(),
            stage: stage.clone(),
        });
    if state.pending()?.as_ref().map(encode).transpose()? != Some(encode(&expected)?) {
        return Err(attention());
    }
    #[cfg(test)]
    undo::abort_boundary::disposal_boundary::before_disposal_open(&stage);
    let Some(source) = Root::open(&stage)? else {
        if matches!(
            undo.as_deref(),
            Some(Pending::Undo {
                displaced: Some(_),
                ..
            })
        ) {
            return begin_empty_undo(state, cancellation);
        }
        return state.clear_intent();
    };
    let Some(payload) = source.payload()? else {
        if matches!(
            undo.as_deref(),
            Some(Pending::Undo {
                displaced: Some(_),
                ..
            })
        ) {
            return begin_empty_undo(state, cancellation);
        }
        source.empty(&stage)?;
        return state.clear_intent();
    };
    source.only(&[b"folder"])?;
    stage
        .ownership
        .as_ref()
        .unwrap()
        .require_payload(&payload)?;
    if undo.is_some() {
        // Preserve the original Undo namespace if its data is already edited
        // or its original source is unavailable. The claimed phase repeats the
        // comparison and holds its readers through the one-shot unlink.
        state.reproduce_partial(&payload, cancellation)?;
        if let Some(Pending::Undo {
            displaced: Some(record),
            ..
        }) = undo.as_deref()
        {
            undo::displaced::observe(record)?;
            let displaced = Root::open(&record.retained.stage)?.ok_or_else(attention)?;
            displaced.only(&[b"previous"])?;
        }
    }
    let (opened, mut holding, temporary) =
        Opened::prepare(stage.directory.parent().ok_or_else(attention)?)?;
    let _retained = temporary.keep();
    holding.payload_identity = stage.payload_identity;
    holding.ownership.as_mut().ok_or_else(attention)?.payload =
        stage.ownership.as_ref().unwrap().payload;
    let mut disposal = Disposal {
        id,
        destination,
        original: stage,
        holding,
        phase: Phase::Reserved,
        undo,
        returned: None,
        empty_copy: false,
    };
    let transition = Transition {
        before: disposal.preparation(),
        after: disposal.pending(),
        created: Some((opened, source, payload)),
        returning: None,
        empty: false,
    };
    if let Err(error) = state.disposal_transition(&transition) {
        if let Some((opened, _, _)) = &transition.created {
            let _ = opened.clean_empty();
        }
        return Err(error);
    }
    drop(transition);
    #[cfg(test)]
    transfer::stage_boundary::disposal_point("reserved", &disposal);
    recover(state, &mut disposal, cancellation)
}
pub(in crate::operations) fn recover(
    state: &mut Store,
    disposal: &mut Disposal,
    cancellation: &Cancellation,
) -> io::Result<()> {
    disposal.validate()?;
    state.require_bound_forward()?;
    if state.pending()?.as_ref().map(encode).transpose()? != Some(encode(&disposal.pending())?) {
        return Err(attention());
    }
    if disposal.phase == Phase::CopyCleared {
        return undo_return::recover(state, disposal, cancellation);
    }
    let original = Root::open(&disposal.original)?;
    if disposal.phase == Phase::Reserved {
        let source = Root::open(&disposal.original)?;
        let holding = Root::open(&disposal.holding)?.ok_or_else(attention)?;
        holding.only(&[b"folder"])?;
        let source_payload = source.as_ref().map(Root::payload).transpose()?.flatten();
        let held_payload = holding.payload()?;
        match (source_payload, held_payload) {
            (Some(payload), None) => {
                let source = source.as_ref().unwrap();
                source.only(&[b"folder"])?;
                disposal
                    .original
                    .ownership
                    .as_ref()
                    .unwrap()
                    .require_payload(&payload)?;
                source.current(&disposal.original)?;
                holding.current(&disposal.holding)?;
                rustix::fs::renameat_with(
                    &source.directory,
                    c"folder",
                    &holding.directory,
                    c"folder",
                    RenameFlags::NOREPLACE,
                )?;
                source.directory.sync_all()?;
                holding.directory.sync_all()?;
                #[cfg(test)]
                transfer::stage_boundary::disposal_point("after-claim", disposal);
            }
            (None, Some(payload)) => disposal
                .holding
                .ownership
                .as_ref()
                .unwrap()
                .require_payload(&payload)?,
            (None, None) => {
                holding.empty(&disposal.holding)?;
                if let Some(source) = source {
                    source.empty(&disposal.original)?;
                }
                return state.clear_intent();
            }
            (Some(_), Some(_)) => return Err(attention()),
        }
        advance(state, disposal, Phase::Claimed)?;
        #[cfg(test)]
        transfer::stage_boundary::disposal_point("claimed", disposal);
    }
    if disposal.phase == Phase::Claimed {
        let holding = Root::open(&disposal.holding)?.ok_or_else(attention)?;
        holding.only(&[b"folder"])?;
        let payload = holding.payload()?.ok_or_else(attention)?;
        disposal
            .holding
            .ownership
            .as_ref()
            .unwrap()
            .require_payload(&payload)?;
        holding.current(&disposal.holding)?;
        let reproduced = state.reproduce_partial(&payload, cancellation)?;
        advance(state, disposal, Phase::Consumed)?;
        #[cfg(test)]
        fingerprint::source::reproduction_boundary("consumed");
        #[cfg(test)]
        transfer::stage_boundary::disposal_point("consumed", disposal);
        holding.current(&disposal.holding)?;
        if let Some(original) = &original {
            original.current(&disposal.original)?;
        } else {
            let _ = Root::open(&disposal.original)?;
        }
        disposal
            .holding
            .ownership
            .as_ref()
            .unwrap()
            .require_payload(&named(&holding.directory, c"folder")?)?;
        if let Some(reproduced) = &reproduced {
            reproduced
                .require(&named(&holding.directory, c"folder")?, cancellation)
                .map_err(fingerprint::source::reproduction_problem)?;
        }
        // Only this live, proven transition retains permission to unlink once.
        // A restarted Consumed record can retire empty folders only.
        let flags = if payload.metadata()?.is_dir() {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        rustix::fs::unlinkat(&holding.directory, c"folder", flags)?;
        holding.directory.sync_all()?;
        #[cfg(test)]
        transfer::stage_boundary::disposal_point("after-unlink", disposal);
        #[cfg(test)]
        transfer::stage_boundary::disposed(&disposal.original, &payload);
    }
    if let Some(holding) = Root::open(&disposal.holding)? {
        holding.empty(&disposal.holding)?;
    }
    #[cfg(test)]
    transfer::stage_boundary::disposal_point("holding-gone", disposal);
    if let Some(source) = Root::open(&disposal.original)? {
        source.empty(&disposal.original)?;
    }
    #[cfg(test)]
    transfer::stage_boundary::disposal_point("roots-gone", disposal);
    if disposal.displaced().is_some() {
        advance(state, disposal, Phase::CopyCleared)?;
        undo_return::recover(state, disposal, cancellation)
    } else {
        state.clear_intent()
    }
}
