//! Return displaced work through a separate consumed namespace. Restart never
//! repeats a consumed rename; a verified Returned receipt has empty-root rights.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ReturnPhase {
    Skipped,
    Reserved,
    Claimed,
    Consumed,
    Returned,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct Return {
    pub(in crate::operations) holding: Option<Stage>,
    phase: ReturnPhase,
}
pub(super) enum Creation {
    Reserved {
        opened: Opened,
        source: Root,
        file: fs::File,
    },
    Skipped {
        source: Option<Root>,
        parent: fs::File,
        file: fs::File,
    },
    Claimed,
    Consuming {
        root: Root,
        file: fs::File,
        version: fingerprint::Version,
    },
    Returned {
        parent: fs::File,
        file: fs::File,
    },
}
fn record(disposal: &Disposal) -> io::Result<&recovery::Record> {
    disposal.displaced().ok_or_else(attention)
}
fn origin(disposal: &Disposal) -> io::Result<Original> {
    record(disposal)?
        .retained
        .stage
        .ownership
        .as_ref()
        .and_then(|o| o.previous_origin())
        .ok_or_else(attention)
}
fn parent(disposal: &Disposal) -> io::Result<fs::File> {
    let stage = &record(disposal)?.retained.stage;
    let file = fs::File::from(open(
        stage.directory.parent().ok_or_else(attention)?,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    stage
        .ownership
        .as_ref()
        .ok_or_else(attention)?
        .require_parent(&file)?;
    Ok(file)
}
fn target(disposal: &Disposal, parent: &fs::File) -> io::Result<fs::File> {
    named(
        parent,
        disposal.destination.file_name().ok_or_else(attention)?,
    )
}
fn require_target(disposal: &Disposal, parent: &fs::File, file: &fs::File) -> io::Result<()> {
    record(disposal)?
        .retained
        .stage
        .ownership
        .as_ref()
        .ok_or_else(attention)?
        .require_parent(parent)?;
    origin(disposal)?.require(file)?;
    origin(disposal)?.require(&target(disposal, parent)?)
}
fn previous(root: &Root) -> io::Result<Option<fs::File>> {
    match named(&root.directory, c"previous") {
        Ok(file) => Ok(Some(file)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
impl Return {
    pub(super) fn validate(&self, disposal: &Disposal) -> io::Result<()> {
        let old = &record(disposal)?.retained.stage;
        if self.phase == ReturnPhase::Skipped {
            if self.holding.is_some() {
                return Err(attention());
            }
            return Ok(());
        }
        let stage = self.holding.as_ref().ok_or_else(attention)?;
        ordinary(stage, &disposal.destination)?;
        let owned = stage.ownership.as_ref().ok_or_else(attention)?;
        if owned.payload != Some(origin(disposal)?)
            || owned.parent != old.ownership.as_ref().ok_or_else(attention)?.parent
            || owned.previous.is_some()
        {
            return Err(attention());
        }
        for other in [&disposal.original, &disposal.holding, old] {
            if stage.directory == other.directory || stage.identity == other.identity {
                return Err(attention());
            }
        }
        Ok(())
    }
}
pub(super) fn require_transition(
    before: &Disposal,
    after: &Disposal,
    proof: Option<&Creation>,
) -> io::Result<()> {
    let mut normalized = after.clone();
    normalized.returned = before.returned.clone();
    if encode(&normalized)? != encode(before)? {
        return Err(attention());
    }
    let next = after.returned.as_ref().ok_or_else(attention)?;
    match (
        before.returned.as_ref().map(|r| r.phase),
        next.phase,
        proof.ok_or_else(attention)?,
    ) {
        (
            None,
            ReturnPhase::Reserved,
            Creation::Reserved {
                opened,
                source,
                file,
            },
        ) => {
            source.current(&record(after)?.retained.stage)?;
            source.only(&[b"previous"])?;
            origin(after)?.require(file)?;
            origin(after)?.require(&previous(source)?.ok_or_else(attention)?)?;
            opened.require_current()?;
            let stage = next.holding.as_ref().ok_or_else(attention)?;
            let owned = stage.ownership.as_ref().ok_or_else(attention)?;
            owned.require_parent(&opened.parent)?;
            owned.require_directory(&opened.directory)?;
            if opened.parent_path.join(&opened.name) != stage.directory
                || opened.identity != stage.identity
            {
                return Err(attention());
            }
        }
        (
            None,
            ReturnPhase::Skipped,
            Creation::Skipped {
                source,
                parent,
                file,
            },
        ) => {
            if let Some(source) = source {
                source.current(&record(after)?.retained.stage)?;
                source.only(&[])?;
            } else if Root::open(&record(after)?.retained.stage)?.is_some() {
                return Err(attention());
            }
            require_target(after, parent, file)?;
        }
        (Some(ReturnPhase::Reserved), ReturnPhase::Claimed, Creation::Claimed) => {
            if let Some(source) = Root::open(&record(after)?.retained.stage)?
                && previous(&source)?.is_some()
            {
                return Err(attention());
            }
            let stage = next.holding.as_ref().ok_or_else(attention)?;
            let root = Root::open(stage)?.ok_or_else(attention)?;
            stage
                .ownership
                .as_ref()
                .ok_or_else(attention)?
                .require_payload(&root.payload()?.ok_or_else(attention)?)?;
        }
        (
            Some(ReturnPhase::Claimed),
            ReturnPhase::Consumed,
            Creation::Consuming {
                root,
                file,
                version,
            },
        ) => {
            root.current(next.holding.as_ref().ok_or_else(attention)?)?;
            origin(after)?.require(file)?;
            let named = root.payload()?.ok_or_else(attention)?;
            origin(after)?.require(&named)?;
            if !version.matches_opened(file, false)? || !version.matches_opened(&named, false)? {
                return Err(attention());
            }
            match target(after, &root.parent) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                _ => return Err(attention()),
            }
        }
        (
            Some(ReturnPhase::Consumed),
            ReturnPhase::Returned,
            Creation::Returned { parent, file },
        ) => {
            require_target(after, parent, file)?;
            if let Some(root) = Root::open(next.holding.as_ref().ok_or_else(attention)?)?
                && root.payload()?.is_some()
            {
                return Err(attention());
            }
        }
        _ => return Err(attention()),
    }
    if let Some(old) = &before.returned {
        let mut normalized = next.clone();
        normalized.phase = old.phase;
        if encode(&normalized)? != encode(old)? {
            return Err(attention());
        }
    }
    Ok(())
}
fn save(
    state: &mut Store,
    disposal: &mut Disposal,
    next: Return,
    proof: Creation,
) -> io::Result<Creation> {
    let mut after = disposal.clone();
    after.returned = Some(next);
    let transition = Transition {
        before: disposal.pending(),
        after: after.pending(),
        created: None,
        returning: Some(proof),
        empty: false,
    };
    if let Err(error) = state.disposal_transition(&transition) {
        if let Some(Creation::Reserved { opened, .. }) = &transition.returning {
            let _cleanup = opened.clean_empty();
        }
        return Err(error);
    }
    *disposal = after;
    Ok(transition.returning.unwrap())
}
fn phase(
    state: &mut Store,
    disposal: &mut Disposal,
    next: ReturnPhase,
    proof: Creation,
) -> io::Result<Creation> {
    let mut returned = disposal.returned.clone().ok_or_else(attention)?;
    returned.phase = next;
    save(state, disposal, returned, proof)
}
pub(super) fn recover(
    state: &mut Store,
    disposal: &mut Disposal,
    cancellation: &Cancellation,
) -> io::Result<()> {
    check_cancel(cancellation)?;
    let old = record(disposal)?.retained.stage.clone();
    // These namespaces have consumed copy rights, including after this phase.
    for stage in [&disposal.original, &disposal.holding] {
        if disposal.empty_copy && (stage == &old || std::ptr::eq(stage, &disposal.holding)) {
            continue;
        }
        if let Some(root) = Root::open(stage)? {
            root.empty(stage)?;
        }
    }
    if disposal.returned.is_none() {
        let source = Root::open(&old)?;
        let file = source.as_ref().map(previous).transpose()?.flatten();
        if let Some(file) = file {
            let source = source.unwrap();
            source.only(&[b"previous"])?;
            origin(disposal)?.require(&file)?;
            let (opened, mut holding, temporary) =
                Opened::prepare(old.directory.parent().ok_or_else(attention)?)?;
            let _retained = temporary.keep();
            holding.payload_identity = Some(Identity::from_metadata(&file.metadata()?));
            holding.ownership.as_mut().ok_or_else(attention)?.payload = Some(origin(disposal)?);
            save(
                state,
                disposal,
                Return {
                    holding: Some(holding),
                    phase: ReturnPhase::Reserved,
                },
                Creation::Reserved {
                    opened,
                    source,
                    file,
                },
            )?;
            #[cfg(test)]
            transfer::stage_boundary::disposal_point("return-reserved", disposal);
        } else {
            let parent = parent(disposal)?;
            let file = target(disposal, &parent)?;
            save(
                state,
                disposal,
                Return {
                    holding: None,
                    phase: ReturnPhase::Skipped,
                },
                Creation::Skipped {
                    source,
                    parent,
                    file,
                },
            )?;
            #[cfg(test)]
            transfer::stage_boundary::disposal_point("return-skipped", disposal);
        }
    }
    if disposal.returned.as_ref().unwrap().phase == ReturnPhase::Reserved {
        let stage = disposal
            .returned
            .as_ref()
            .unwrap()
            .holding
            .as_ref()
            .unwrap()
            .clone();
        let source = Root::open(&old)?;
        let holding = Root::open(&stage)?.ok_or_else(attention)?;
        holding.only(&[b"folder"])?;
        let selected = source.as_ref().map(previous).transpose()?.flatten();
        match (selected, holding.payload()?) {
            (Some(file), None) => {
                let source = source.as_ref().unwrap();
                source.only(&[b"previous"])?;
                origin(disposal)?.require(&file)?;
                source.current(&old)?;
                holding.current(&stage)?;
                rustix::fs::renameat_with(
                    &source.directory,
                    c"previous",
                    &holding.directory,
                    c"folder",
                    RenameFlags::NOREPLACE,
                )?;
                source.directory.sync_all()?;
                holding.directory.sync_all()?;
                #[cfg(test)]
                transfer::stage_boundary::disposal_point("return-after-claim", disposal);
            }
            (None, Some(file)) => origin(disposal)?.require(&file)?,
            _ => return Err(attention()),
        }
        phase(state, disposal, ReturnPhase::Claimed, Creation::Claimed)?;
        #[cfg(test)]
        transfer::stage_boundary::disposal_point("return-claimed", disposal);
    }
    if disposal.returned.as_ref().unwrap().phase == ReturnPhase::Claimed {
        let stage = disposal
            .returned
            .as_ref()
            .unwrap()
            .holding
            .as_ref()
            .unwrap()
            .clone();
        let root = Root::open(&stage)?.ok_or_else(attention)?;
        root.only(&[b"folder"])?;
        let file = root.payload()?.ok_or_else(attention)?;
        origin(disposal)?.require(&file)?;
        let version = fingerprint::Version::from_metadata(&file.metadata()?);
        let proof = phase(
            state,
            disposal,
            ReturnPhase::Consumed,
            Creation::Consuming {
                root,
                file,
                version,
            },
        )?;
        #[cfg(test)]
        transfer::stage_boundary::disposal_point("return-consumed", disposal);
        let Creation::Consuming {
            root,
            file,
            version,
        } = proof
        else {
            unreachable!();
        };
        check_cancel(cancellation)?;
        root.current(&stage)?;
        origin(disposal)?.require(&file)?;
        let named = root.payload()?.ok_or_else(attention)?;
        origin(disposal)?.require(&named)?;
        if !version.matches_opened(&file, false)? || !version.matches_opened(&named, false)? {
            return Err(attention());
        }
        rustix::fs::renameat_with(
            &root.directory,
            c"folder",
            &root.parent,
            disposal.destination.file_name().ok_or_else(attention)?,
            RenameFlags::NOREPLACE,
        )?;
        root.directory.sync_all()?;
        fs::File::from(openat(
            &root.parent,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?)
        .sync_all()?;
        #[cfg(test)]
        transfer::stage_boundary::disposal_point("return-after-rename", disposal);
        #[cfg(test)]
        undo::abort_boundary::disposal_boundary::after_raw_return();
    }
    if disposal.returned.as_ref().unwrap().phase == ReturnPhase::Consumed {
        let parent = parent(disposal)?;
        let file = target(disposal, &parent)?;
        phase(
            state,
            disposal,
            ReturnPhase::Returned,
            Creation::Returned { parent, file },
        )?;
        #[cfg(test)]
        transfer::stage_boundary::disposal_point("return-returned", disposal);
    }
    // Returned/Skipped receipts permit only empty, original-root retirement.
    if let Some(root) = Root::open(&old)? {
        root.empty(&old)?;
    }
    if let Some(stage) = disposal.returned.as_ref().unwrap().holding.as_ref()
        && let Some(root) = Root::open(stage)?
    {
        root.empty(stage)?;
    }
    #[cfg(test)]
    transfer::stage_boundary::disposal_point("return-roots-gone", disposal);
    state.clear_intent()
}
