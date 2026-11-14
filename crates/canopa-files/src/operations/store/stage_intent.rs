//! Independent preparation and empty-cleanup bindings. Facts are installed from
//! actual creators or a verified Forward transition, never from recovery paths.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    history: Identity,
    original: Option<original::Original>,
    step: u64,
    task: Option<transfer_cursor::StageTask>,
    #[serde(with = "crate::path_serde::optional")]
    source: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reproduction: Option<fingerprint::source::Reproduction>,
    canonical: Option<[u8; 32]>,
    receipt: Option<Vec<u8>>,
    digest: [u8; 32],
}
pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS stage_intents (
        lane INTEGER PRIMARY KEY CHECK(lane IN (0,1)),
        body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=262144));",
        )
        .map_err(db_error)
}
fn digest(
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
    binding: &Binding,
) -> io::Result<[u8; 32]> {
    let mut hash = Sha256::new();
    hash.update(b"canopa-files-stage-intent-v1\0");
    // Keep Binding's raw-Unix-path serializer for its optional source too.
    let mut original = binding.clone();
    original.digest = [0; 32];
    hash.update(encode(&(history, lane.index(), pending, original))?.as_bytes());
    Ok(hash.finalize().into())
}
fn operation(pending: &Pending) -> io::Result<u64> {
    match pending {
        Pending::Preparing { id, .. } | Pending::Cleanup { id, .. } => Ok(*id),
        Pending::Disposing { disposal } => Ok(disposal.id),
        _ => Err(invalid("Expected original staged work.")),
    }
}
fn next(connection: &Connection, id: u64) -> io::Result<u64> {
    let exists: bool = connection
        .query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM operations WHERE id=?1)",
            [id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if !exists {
        return Err(invalid(
            "The original staged-work operation is unavailable.",
        ));
    }
    connection
        .query_row_cached(
            "SELECT coalesce(max(ordinal)+1,0) FROM changes WHERE operation=?1",
            [id],
            |r| r.get(0),
        )
        .map_err(db_error)
}
fn pending_body(connection: &Connection, lane: IntentLane) -> io::Result<Option<String>> {
    connection
        .query_row_cached(
            match lane {
                IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                IntentLane::RecoveryCopy => "SELECT body FROM recovery_pending WHERE singleton=1",
            },
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)
}
fn read(connection: &Connection, lane: IntentLane) -> io::Result<Option<Binding>> {
    let body: Option<String> = connection
        .query_row_cached(
            "SELECT body FROM stage_intents WHERE lane=?1",
            [lane.index()],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    body.map(|body| decode(&body)).transpose()
}
fn install(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
    mut binding: Binding,
) -> io::Result<()> {
    binding.digest = digest(history, lane, pending, &binding)?;
    connection.execute_cached("INSERT INTO stage_intents(lane,body) VALUES(?1,?2) ON CONFLICT(lane) DO UPDATE SET body=excluded.body", params![lane.index(),encode(&binding)?]).map_err(db_error)?;
    Ok(())
}
pub(super) fn clear(connection: &Connection, lane: IntentLane) -> io::Result<()> {
    connection
        .execute_cached("DELETE FROM stage_intents WHERE lane=?1", [lane.index()])
        .map_err(db_error)?;
    Ok(())
}
/// Runs before directory access. Missing records never authorize legacy cleanup.
pub(super) fn require(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
    problems: Problems,
) -> io::Result<()> {
    if matches!(pending, Pending::Disposing { disposal } if disposal.undo.is_some()) {
        return if matches!(lane, IntentLane::Primary) {
            undo_work::require(connection, history, pending)
        } else {
            Err(invalid("Undo disposal belongs to its primary journal."))
        };
    }
    if !matches!(
        pending,
        Pending::Preparing { .. } | Pending::Disposing { .. } | Pending::Cleanup { .. }
    ) {
        return Ok(());
    }
    let Some(binding) = read(connection, lane)? else {
        if matches!(lane, IntentLane::Primary)
            && transfer_cursor::prepared_expansion(connection, history, problems, pending)?
                .is_some()
        {
            return Ok(());
        }
        return Err(invalid(
            "The staged work lost its original preparation or cleanup binding. Its files are preserved for Recovery.",
        ));
    };
    let id = operation(pending)?;
    let stage = match pending {
        Pending::Preparing { stage, .. } | Pending::Cleanup { stage, .. } => stage,
        Pending::Disposing { disposal } => {
            disposal.validate()?;
            &disposal.original
        }
        _ => unreachable!(),
    };
    if stage.ownership.is_none() {
        return Err(invalid(
            "The staged work has no original object-generation proof.",
        ));
    }
    if binding.history != history
        || binding.digest != digest(history, lane, pending, &binding)?
        || binding.task != transfer_cursor::stage_task(connection, history, lane, id)?
    {
        return Err(invalid(
            "The staged work differs from its original operation or task. Its files are preserved for Recovery.",
        ));
    }
    binding
        .original
        .ok_or_else(|| invalid("The staged work has no original history-file generation proof."))?
        .require(&transfer_completion::history_file(connection, history)?)?;
    match pending {
        Pending::Preparing { .. } | Pending::Disposing { .. } => {
            if binding.canonical.is_some()
                || binding.receipt.is_some()
                || next(connection, id)? != binding.step
            {
                return Err(invalid("The preparation crossed another filesystem step."));
            }
        }
        Pending::Cleanup { ordinal, .. } => {
            let body: String = connection
                .query_row_cached(
                    "SELECT body FROM changes WHERE operation=?1 AND ordinal=?2",
                    params![id, ordinal],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            if binding.step != *ordinal
                || next(connection, id)?
                    != ordinal
                        .checked_add(1)
                        .ok_or_else(|| invalid("Invalid cleanup ordinal."))?
                || binding.canonical != Some(Sha256::digest(body.as_bytes()).into())
                || binding.receipt
                    != binding
                        .task
                        .as_ref()
                        .map(|task| transfer_cursor::stage_step(connection, task, *ordinal, &body))
                        .transpose()?
            {
                return Err(invalid(
                    "The cleanup differs from its original committed step.",
                ));
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}
pub(super) fn prepare(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
    created: &staging::Registration<'_>,
    task: Option<transfer_cursor::StageTask>,
    problems: Problems,
) -> io::Result<()> {
    let Pending::Preparing {
        id,
        destination,
        stage,
    } = pending
    else {
        return Err(invalid("Expected new copy preparation."));
    };
    created.require()?;
    let source = created.source();
    let reproduction = created.reproduction();
    if task != transfer_cursor::stage_task(connection, history, lane, *id)? {
        return Err(invalid(
            "The live preparation's original task changed before its save.",
        ));
    }
    if let Some(body) = pending_body(connection, lane)? {
        let previous: Pending = decode(&body)?;
        require(connection, history, lane, &previous, problems)?;
        let Pending::Preparing {
            id: owner,
            destination: target,
            stage: before,
        } = previous
        else {
            return Err(invalid(
                "An existing journal cannot become new preparation.",
            ));
        };
        let mut expected = stage.clone();
        expected.payload_identity = before.payload_identity;
        expected.ownership = before.ownership.clone();
        let binding = read(connection, lane)?
            .ok_or_else(|| invalid("The preparation lost its original binding."))?;
        if owner != *id
            || target != *destination
            || expected != before
            || before.payload_identity.is_some()
            || binding.source.as_deref() != Some(source)
            || binding.task != task
            || binding.reproduction != reproduction
        {
            return Err(invalid(
                "A new payload differs from its original empty preparation.",
            ));
        }
    } else if read(connection, lane)?.is_some() {
        return Err(invalid(
            "The original staged-work journal is missing; its binding is preserved.",
        ));
    }
    let binding = Binding {
        history,
        original: match read(connection, lane)? {
            Some(binding) => binding.original,
            None => transfer_completion::history_file(connection, history)
                .ok()
                .and_then(|file| original::Original::capture(&file).ok()),
        },
        step: next(connection, *id)?,
        task,
        source: Some(source.to_owned()),
        reproduction,
        canonical: None,
        receipt: None,
        digest: [0; 32],
    };
    install(connection, history, lane, pending, binding)
}
pub(super) fn leaving(preparing: &Pending, next: &Pending) -> io::Result<()> {
    let Pending::Preparing { stage: before, .. } = preparing else {
        return Ok(());
    };
    let after = match next {
        Pending::Forward {
            stage: Some(stage), ..
        }
        | Pending::Undo {
            stage: Some(stage), ..
        } => stage,
        Pending::StageMigration { migration } => &migration.stage,
        _ => {
            return Err(invalid(
                "A preparation must retain its actual created work when it advances.",
            ));
        }
    };
    if before.directory != after.directory
        || before.identity != after.identity
        || before.parent_identity != after.parent_identity
        || before.payload_identity != after.payload_identity
        || before.ownership != after.ownership
    {
        return Err(invalid(
            "The filesystem step differs from its originally registered work.",
        ));
    }
    Ok(())
}
/// The caller has verified the exact Forward and retired only its returned step.
pub(super) fn returned(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
    original: Option<original::Original>,
) -> io::Result<()> {
    let Pending::Preparing { id, stage, .. } = pending else {
        return Err(invalid("Expected returned empty work."));
    };
    if stage.payload_identity.is_some()
        || stage
            .ownership
            .as_ref()
            .is_none_or(|p| p.payload_origins().is_some())
    {
        return Err(invalid(
            "A returned preparation grants empty-directory rights only.",
        ));
    }
    let binding = Binding {
        history,
        original,
        step: next(connection, *id)?,
        task: transfer_cursor::stage_task(connection, history, lane, *id)?,
        source: None,
        reproduction: None,
        canonical: None,
        receipt: None,
        digest: [0; 32],
    };
    install(connection, history, lane, pending, binding)
}
/// Lives only across the verified append transaction, including a descriptor-
/// verified replacement refresh. It cannot be decoded from a saved journal.
pub(super) struct Completion {
    history: Identity,
    original: Option<original::Original>,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
    stage: Stage,
    task: Option<transfer_cursor::StageTask>,
}
pub(super) fn completion(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: &Stage,
) -> io::Result<Completion> {
    let body = pending_body(connection, lane)?
        .ok_or_else(|| invalid("A cleanup needs its original Forward intent."))?;
    let pending: Pending = decode(&body)?;
    context::require_forward(connection, history, lane, &pending)?;
    transfer_cursor::require_forward(connection, history, lane, &pending)?;
    let history_original = context::history_original(connection, history, lane, &pending)?;
    let Pending::Forward {
        id: owner,
        ordinal: step,
        change: before,
        stage: Some(original),
    } = pending
    else {
        return Err(invalid("A cleanup needs its exact original work."));
    };
    let mut normalized = replacement::normalized(&original);
    normalized.directory_claim = None;
    let expected = staging::completed_cleanup(&normalized)?;
    if owner != id
        || step != ordinal
        || encode(&before)? != encode(change)?
        || next(connection, id)? != ordinal
        || expected != *stage
    {
        return Err(invalid("A cleanup differs from its original Forward work."));
    }
    Ok(Completion {
        history,
        original: history_original,
        lane,
        id,
        ordinal,
        stage: stage.clone(),
        task: transfer_cursor::stage_task(connection, history, lane, id)?,
    })
}
impl Completion {
    pub(super) fn install(self, connection: &Connection, change: &Change) -> io::Result<()> {
        let pending = Pending::Cleanup {
            id: self.id,
            ordinal: self.ordinal,
            destination: change.forward_path().to_owned(),
            stage: self.stage,
        };
        let body = encode(change)?;
        let receipt = self
            .task
            .as_ref()
            .map(|task| transfer_cursor::stage_step(connection, task, self.ordinal, &body))
            .transpose()?;
        let binding = Binding {
            history: self.history,
            original: self.original,
            step: self.ordinal,
            task: self.task,
            source: None,
            reproduction: None,
            canonical: Some(Sha256::digest(body.as_bytes()).into()),
            receipt,
            digest: [0; 32],
        };
        install(connection, self.history, self.lane, &pending, binding)
    }
}

pub(super) fn disposal_transition(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    transition: &staging::disposal::Transition,
    problems: Problems,
) -> io::Result<()> {
    let current: Pending = decode(
        &pending_body(connection, lane)?
            .ok_or_else(|| invalid("The original disposal preparation is missing."))?,
    )?;
    if encode(&current)? != encode(transition.before())? {
        return Err(invalid("The original disposal journal changed."));
    }
    require(connection, history, lane, &current, problems)?;
    transition.require()?;
    let binding = read(connection, lane)?
        .ok_or_else(|| invalid("The original disposal task binding is missing."))?;
    install(connection, history, lane, transition.after(), binding)
}

impl Store {
    pub(in crate::operations) fn reproduce_partial(
        &self,
        payload: &fs::File,
        cancellation: &Cancellation,
    ) -> io::Result<Option<fingerprint::source::Reproduced>> {
        // Directory preparations can only be removed while empty. rmdir itself
        // refuses any children, including ones arriving after this observation.
        if payload.metadata()?.is_dir() {
            return Ok(None);
        }
        self.require_bound_forward()?;
        if let Some(reproduced) = undo_work::reproduce(self, payload, cancellation)? {
            return Ok(Some(reproduced));
        }
        let binding = read(&self.connection, self.lane)?
            .ok_or_else(|| invalid("The partial copy lost its original source binding."))?;
        let source = binding.source.as_deref().ok_or_else(|| {
            invalid(
                "The partial copy has no original source path. Its data is preserved for Recovery.",
            )
        })?;
        let proof = binding.reproduction.ok_or_else(|| invalid("The partial copy has no original source generation proof. Its data is preserved for Recovery."))?;
        proof
            .reproduce(source, payload, cancellation)
            .map(Some)
            .map_err(fingerprint::source::reproduction_problem)
    }
}

/// Re-encode a private live fixture with the exact pre-schema48 binding shape.
/// This creates historical test data, never a production upgrade capability.
#[cfg(test)]
pub(in crate::operations) fn legacy_reproduction_fixture(path: &Path) {
    let connection = Connection::open(path).unwrap();
    let lane = IntentLane::Primary;
    let pending: Pending = decode(&pending_body(&connection, lane).unwrap().unwrap()).unwrap();
    let mut binding = read(&connection, lane).unwrap().unwrap();
    binding.reproduction = None;
    binding.digest = digest(binding.history, lane, &pending, &binding).unwrap();
    connection
        .execute(
            "UPDATE stage_intents SET body=?1 WHERE lane=0",
            [encode(&binding).unwrap()],
        )
        .unwrap();
    connection.pragma_update(None, "user_version", 47).unwrap();
}
