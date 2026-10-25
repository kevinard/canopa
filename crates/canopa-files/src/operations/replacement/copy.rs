use super::*;

pub(in crate::operations) struct PreparedCopy<'a> {
    pub reviewed: Reviewed,
    pub opened: &'a staging::Prepared,
    pub stage: Stage,
    pub temporary: tempfile::TempDir,
    pub change: Change,
}

pub(in crate::operations) fn commit(
    state: &mut Store,
    id: u64,
    prepared: PreparedCopy<'_>,
    cancellation: &Cancellation,
    mut checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let PreparedCopy {
        reviewed,
        opened,
        mut stage,
        temporary,
        mut change,
    } = prepared;
    let retained = reviewed.attach(opened.parent(), &mut stage)?;
    let Change::Copied {
        identity,
        fingerprint,
        publication,
        retained: slot,
        ..
    } = &mut change
    else {
        return Err(invalid("A copied replacement needs its Copy step."));
    };
    *slot = Some(retained);
    *publication = opened.expected_publication().map(Box::new);
    let identity = *identity;
    let fingerprint = fingerprint.clone();
    stage.replacement.as_mut().unwrap().copy = Some(Copy {
        phase: CopyPhase::Ready,
        identity,
        fingerprint: fingerprint.clone(),
        publication: publication.as_deref().cloned(),
    });
    let ordinal = state.next_ordinal(id)?;
    save(state, id, ordinal, &change, &stage)?;
    let _retained = temporary.keep();
    let before = (|| -> io::Result<()> {
        checkpoint()?;
        opened.require_current()?;
        check_cancel(cancellation)?;
        reviewed.claim(
            opened.directory(),
            &mut stage,
            |stage| save(state, id, ordinal, &change, stage),
            cancellation,
        )
    })();
    if let Err(error) = before {
        if transfer::placement_failure(&error).is_some() {
            return Err(protect(error));
        }
        return abort(
            state,
            Step {
                id,
                ordinal,
                change: &change,
            },
            &mut stage,
            &reviewed,
            opened,
            &fingerprint,
            error,
        );
    }
    if let Err(error) = (|| -> io::Result<()> {
        checkpoint()?;
        reviewed.require_saved(opened.directory())?;
        opened.require_current()?;
        check_cancel(cancellation)
    })() {
        if transfer::placement_failure(&error).is_some() {
            return Err(protect(error));
        }
        return abort(
            state,
            Step {
                id,
                ordinal,
                change: &change,
            },
            &mut stage,
            &reviewed,
            opened,
            &fingerprint,
            error,
        );
    }
    stage
        .replacement
        .as_mut()
        .unwrap()
        .copy
        .as_mut()
        .unwrap()
        .phase = CopyPhase::Publishing;
    save(state, id, ordinal, &change, &stage).map_err(protect)?;
    #[cfg(test)]
    super::super::replacement_boundary::at_phase(
        "publishing",
        change.forward_path(),
        &stage.payload(),
    );
    let publication = (|| -> io::Result<()> {
        opened.require_current()?;
        reviewed.require_saved(opened.directory())?;
        check_cancel(cancellation)?;
        opened.publish(change.forward_path())
    })();
    if let Err(error) = publication {
        if transfer::placement_failure(&error).is_some() {
            return Err(protect(error));
        }
        opened.unpublish(change.forward_path()).map_err(protect)?;
        return abort(
            state,
            Step {
                id,
                ordinal,
                change: &change,
            },
            &mut stage,
            &reviewed,
            opened,
            &fingerprint,
            error,
        );
    }
    #[cfg(test)]
    super::super::replacement_boundary::at_phase(
        "published",
        change.forward_path(),
        &stage.payload(),
    );
    opened
        .require_published(change.forward_path())
        .map_err(protect)?;
    let refreshed = reviewed
        .refreshed(opened.directory(), &change)
        .map_err(protect)?;
    let normal = state.append_replacement_with_cleanup(id, ordinal, &refreshed)?;
    if normal.ownership.is_some() {
        clean_stage(&normal).map_err(protect)?;
    }
    state.clear_intent()?;
    #[cfg(test)]
    super::super::replacement_boundary::at_phase(
        "finished",
        change.forward_path(),
        &stage.payload(),
    );
    Ok(())
}
fn save(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: &Stage,
) -> io::Result<()> {
    state.intent(&Pending::Forward {
        id,
        ordinal,
        change: change.clone(),
        stage: Some(stage.clone()),
    })
}
fn abort(
    state: &mut Store,
    step: Step<'_>,
    stage: &mut Stage,
    reviewed: &Reviewed,
    opened: &staging::Prepared,
    fingerprint: &Fingerprint,
    error: io::Error,
) -> io::Result<()> {
    let Step {
        id,
        ordinal,
        change,
    } = step;
    stage
        .replacement
        .as_mut()
        .unwrap()
        .copy
        .as_mut()
        .unwrap()
        .phase = CopyPhase::Returning;
    save(state, id, ordinal, change, stage).map_err(protect)?;
    reviewed
        .restore(opened.directory(), stage, |stage| {
            save(state, id, ordinal, change, stage)
        })
        .map_err(protect)?;
    if opened.fingerprint(&Cancellation::default())? != *fingerprint {
        return Err(protect(changed()));
    }
    opened.discard_owned().map_err(protect)?;
    let normal = normalized(stage);
    state.returned_source_intent(&Pending::Preparing {
        id,
        destination: change.forward_path().to_owned(),
        stage: normal.clone(),
    })?;
    #[cfg(test)]
    super::super::replacement_boundary::at_phase(
        "return-preparing",
        change.forward_path(),
        &normal.payload(),
    );
    clean_stage(&normal).map_err(protect)?;
    state.clear_intent()?;
    Err(error)
}
