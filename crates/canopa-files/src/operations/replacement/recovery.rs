use super::*;

pub(in crate::operations) struct Reopened {
    parent: fs::File,
    directory: Option<fs::File>,
    target: PathBuf,
    facts: Facts,
}
impl Reopened {
    pub(in crate::operations) fn open(stage: &Stage, change: &Change) -> io::Result<Self> {
        let claim = stage.replacement.as_ref().ok_or_else(interrupted)?;
        if claim.copy.is_some() != stage.entry_move.is_none() {
            return Err(invalid(
                "The replacement has inconsistent Copy/Move ownership.",
            ));
        }
        let retained = change
            .retained()
            .ok_or_else(|| invalid("A replacement claim needs its saved version."))?;
        if retained.replacement_facts.as_deref() != Some(&claim.facts)
            || retained.stage != normalized(stage)
            || retained.original_identity() != claim.facts.version.identity
            || stage.parent_identity != Some(claim.facts.parent)
        {
            return Err(invalid(
                "The replacement claim differs from its saved step.",
            ));
        }
        let originals = claim.facts.originals.as_ref().ok_or_else(interrupted)?;
        let parent = fs::File::from(open(
            change.forward_path().parent().unwrap(),
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        originals.parent.require(&parent)?;
        if Identity::from_metadata(&parent.metadata()?) != claim.facts.parent {
            return Err(changed());
        }
        let ownership = stage.ownership.as_ref().ok_or_else(interrupted)?;
        ownership.require_parent(&parent)?;
        let directory = match openat(
            &parent,
            stage.directory.file_name().unwrap(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(file) => {
                let file = fs::File::from(file);
                ownership.require_directory(&file)?;
                if Identity::from_metadata(&file.metadata()?) != stage.identity {
                    return Err(changed());
                }
                Some(file)
            }
            Err(rustix::io::Errno::NOENT) => None,
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            parent,
            directory,
            target: change.forward_path().to_owned(),
            facts: claim.facts.clone(),
        })
    }
    fn previous(&self) -> io::Result<Option<fs::File>> {
        self.directory
            .as_ref()
            .map(|directory| optional(directory, c"previous"))
            .transpose()
            .map(Option::flatten)
    }
    pub(in crate::operations) fn require_saved(&self, stage: &Stage) -> io::Result<fs::File> {
        if stage.replacement.as_ref().ok_or_else(interrupted)?.phase != Phase::Saved {
            return Err(interrupted());
        }
        let previous = self.previous()?.ok_or_else(interrupted)?;
        self.facts
            .originals
            .as_ref()
            .ok_or_else(interrupted)?
            .entry
            .require(&previous)?;
        if Identity::from_metadata(&previous.metadata()?) != self.facts.version.identity {
            return Err(changed());
        }
        Ok(previous)
    }
    pub(in crate::operations) fn restore(
        &self,
        state: &mut Store,
        id: u64,
        ordinal: u64,
        change: &Change,
        stage: &mut Stage,
    ) -> io::Result<()> {
        let claim = stage.replacement.as_ref().ok_or_else(interrupted)?;
        if claim.phase == Phase::Unclaimed {
            return if self.previous()?.is_none() {
                Ok(())
            } else {
                Err(changed())
            };
        }
        let subject = if claim.phase == Phase::Returning {
            claim.returning.clone().ok_or_else(interrupted)?
        } else {
            Subject {
                identity: self.facts.version.identity,
                original: Some(self.facts.originals.as_ref().ok_or_else(interrupted)?.entry),
            }
        };
        let original = subject.original.as_ref().ok_or_else(interrupted)?;
        if let Some(previous) = self.previous()? {
            original.require(&previous)?;
            if Identity::from_metadata(&previous.metadata()?) != subject.identity {
                return Err(changed());
            }
            let claim = stage.replacement.as_mut().unwrap();
            claim.phase = Phase::Returning;
            claim.returning = Some(subject.clone());
            state.intent(&Pending::Forward {
                id,
                ordinal,
                change: change.clone(),
                stage: Some(stage.clone()),
            })?;
            let directory = self.directory.as_ref().unwrap();
            #[cfg(test)]
            super::super::replacement_boundary::at_phase(
                "recovery-returning",
                &self.target,
                &stage.directory.join("previous"),
            );
            move_at(
                directory,
                c"previous",
                &self.parent,
                self.target.file_name().unwrap(),
            )?;
            sync(directory, &self.parent)?;
        }
        let returned = named(&self.parent, self.target.file_name().unwrap())?;
        original.require(&returned)?;
        if Identity::from_metadata(&returned.metadata()?) != subject.identity {
            return Err(changed());
        }
        let claim = stage.replacement.as_mut().unwrap();
        if claim.phase != Phase::Returning || claim.returning.is_none() {
            claim.phase = Phase::Returning;
            claim.returning = Some(subject);
            state.intent(&Pending::Forward {
                id,
                ordinal,
                change: change.clone(),
                stage: Some(stage.clone()),
            })?;
        }
        Ok(())
    }
    pub(in crate::operations) fn refresh(
        &self,
        stage: &Stage,
        retained: &mut Retained,
    ) -> io::Result<()> {
        let previous = self.require_saved(stage)?;
        retained.recovery |= !self.facts.version.matches_opened(&previous, true)?;
        retained.identity = Identity::from_metadata(&previous.metadata()?);
        retained.version = Some(fingerprint::Version::from_metadata(&previous.metadata()?));
        Ok(())
    }
    pub(in crate::operations) fn refreshed(
        &self,
        stage: &Stage,
        change: &Change,
    ) -> io::Result<Refreshed> {
        let mut after = change.clone();
        self.refresh(stage, after.retained_mut().ok_or_else(interrupted)?)?;
        Ok(Refreshed {
            before: change.clone(),
            after,
        })
    }
}

pub(in crate::operations) fn recover_copy(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: &Stage,
) -> io::Result<()> {
    validate_stage(Some(stage), change.forward_path())?;
    if stage.entry_move.is_some() || stage.source_claim.is_some() {
        return Err(invalid("A copied replacement has conflicting stage roles."));
    }
    let Change::Copied {
        identity,
        fingerprint,
        publication,
        retained: Some(_),
        ..
    } = change
    else {
        return Err(invalid("A copied replacement needs its exact Copy step."));
    };
    let claim = stage.replacement.as_ref().ok_or_else(interrupted)?;
    let copy = claim.copy.as_ref().ok_or_else(interrupted)?;
    if copy.identity != *identity
        || copy.fingerprint != *fingerprint
        || copy.publication.as_ref() != publication.as_deref()
        || stage.payload_identity != Some(*identity)
    {
        return Err(invalid(
            "The copied replacement differs from its publication intent.",
        ));
    }
    let publication = copy.publication.as_ref().ok_or_else(interrupted)?;
    if stage
        .ownership
        .as_ref()
        .and_then(|ownership| ownership.publication())
        .as_ref()
        != Some(publication)
    {
        return Err(invalid(
            "The copied replacement has inconsistent original output facts.",
        ));
    }
    let reopened = Reopened::open(stage, change)?;
    let payload = reopened
        .directory
        .as_ref()
        .map(|dir| optional(dir, c"folder"))
        .transpose()?
        .flatten();
    if copy.phase != CopyPhase::Publishing {
        return abort_copy(
            state,
            Step {
                id,
                ordinal,
                change,
            },
            stage,
            &reopened,
            payload,
            fingerprint,
        );
    }
    if let Some(payload) = payload {
        stage
            .ownership
            .as_ref()
            .ok_or_else(interrupted)?
            .require_payload(&payload)?;
        if Identity::from_metadata(&payload.metadata()?) != *identity
            || Fingerprint::read_opened(&payload, &Cancellation::default())? != *fingerprint
        {
            return Err(changed());
        }
        reopened.require_saved(stage)?;
        let directory = reopened.directory.as_ref().unwrap();
        move_at(
            directory,
            c"folder",
            &reopened.parent,
            change.forward_path().file_name().unwrap(),
        )?;
        sync(directory, &reopened.parent)?;
    }
    let actual = publication.require(change.forward_path())?;
    if Identity::from_metadata(&actual.metadata()?) != *identity {
        return Err(changed());
    }
    let refreshed = reopened.refreshed(stage, change)?;
    let cleanup = state.append_replacement_with_cleanup(id, ordinal, &refreshed)?;
    clean_stage(&cleanup)?;
    state.clear_intent()
}
fn abort_copy(
    state: &mut Store,
    step: Step<'_>,
    stage: &Stage,
    reopened: &Reopened,
    payload: Option<fs::File>,
    fingerprint: &Fingerprint,
) -> io::Result<()> {
    let Step {
        id,
        ordinal,
        change,
    } = step;
    let mut returning = stage.clone();
    returning
        .replacement
        .as_mut()
        .unwrap()
        .copy
        .as_mut()
        .unwrap()
        .phase = CopyPhase::Returning;
    state.intent(&Pending::Forward {
        id,
        ordinal,
        change: change.clone(),
        stage: Some(returning.clone()),
    })?;
    reopened.restore(state, id, ordinal, change, &mut returning)?;
    if let Some(payload) = payload {
        stage
            .ownership
            .as_ref()
            .ok_or_else(interrupted)?
            .require_payload(&payload)?;
        if Fingerprint::read_opened(&payload, &Cancellation::default())? != *fingerprint {
            return Err(changed());
        }
        let directory = reopened.directory.as_ref().unwrap();
        stage
            .ownership
            .as_ref()
            .unwrap()
            .require_payload(&named(directory, c"folder")?)?;
        rustix::fs::unlinkat(directory, c"folder", rustix::fs::AtFlags::empty())?;
        sync(directory, &reopened.parent)?;
    }
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
    clean_stage(&normal)?;
    state.clear_intent()
}
