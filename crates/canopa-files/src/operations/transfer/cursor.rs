//! Incremental traversal persistence. Ordinary progress never rewrites the tail.
use super::*;
use std::{collections::VecDeque, ops::Deref, os::unix::ffi::OsStrExt};
pub(in crate::operations) mod expansion;

pub(in crate::operations) const QUEUE: u8 = 0;
const DIRECTORY: u8 = 1;
const FAILED: u8 = 2;
const SKIPPED: u8 = 3;
const DESTINATION: u8 = 4;
const SAVED: u8 = 5;
const COPIED: u8 = 6;
const REMAINDER: u8 = 7;
pub(in crate::operations) const EXPANSION: u8 = 8;
pub(in crate::operations) const CHILD: u8 = 9;
type Task = (PathBuf, PathBuf, Option<DirectoryFinish>);

#[derive(Clone)]
pub(in crate::operations) struct Row {
    pub role: u8,
    pub key: Vec<u8>,
    pub body: String,
}
pub(in crate::operations) enum Edit {
    Put(Row),
    Remove { role: u8, key: Vec<u8> },
}
pub(in crate::operations) struct Update {
    pub progress: String,
    pub edits: Vec<Edit>,
}
#[derive(Clone)]
pub(in crate::operations) struct Recorded {
    pub spec: String,
    pub progress: String,
    pub rows: Vec<Row>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    kind: TransferKind,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    restart: RestartObservation,
}

thread_local! {
    static SPEC: std::cell::RefCell<serialized::Last<Spec>> =
        const { std::cell::RefCell::new(serialized::Last::new()) };
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Progress {
    operation: Option<u64>,
    completed: usize,
    bytes: u64,
    root: restart::Evidence<original::Original>,
    counts: Vec<usize>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    #[serde(with = "crate::path_serde")]
    source: PathBuf,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    finish: Option<DirectoryFinish>,
}
pub(in crate::operations) fn stage_scope(row: &Row) -> io::Result<(PathBuf, PathBuf)> {
    if row.role != QUEUE || row.key.len() != 8 {
        return Err(invalid("Missing exact staged-work task."));
    }
    let item: Item = decoded(&row.body)?;
    Ok((item.source, item.destination))
}
impl From<Task> for Item {
    fn from((source, destination, finish): Task) -> Self {
        Self {
            source,
            destination,
            finish,
        }
    }
}
impl From<Item> for Task {
    fn from(item: Item) -> Self {
        (item.source, item.destination, item.finish)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    item: Item,
    report: TransferReportFailure,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    #[serde(with = "crate::path_serde")]
    source: PathBuf,
    receipt: saved_stage::Receipt,
}
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct Name(#[serde(with = "crate::path_serde")] PathBuf);
fn encoded<T: Serialize>(value: &T) -> io::Result<String> {
    serde_json::to_string(value).map_err(io::Error::other)
}
fn decoded<T: serde::de::DeserializeOwned>(value: &str) -> io::Result<T> {
    serde_json::from_str(value).map_err(io::Error::other)
}
fn number(value: usize) -> Vec<u8> {
    (value as u64).to_be_bytes().to_vec()
}
fn position(value: i64) -> Vec<u8> {
    ((value as u64) ^ (1 << 63)).to_be_bytes().to_vec()
}
fn put<T: Serialize>(role: u8, key: Vec<u8>, value: &T) -> io::Result<Edit> {
    Ok(Edit::Put(Row {
        role,
        key,
        body: encoded(value)?,
    }))
}

pub(super) struct Queue {
    entries: VecDeque<Task>,
    positions: VecDeque<i64>,
    front: i64,
    edits: Vec<Edit>,
    current: Option<Vec<u8>>,
}
impl Queue {
    pub(super) fn new(entries: Vec<Task>) -> Self {
        Self {
            positions: (0..entries.len() as i64).collect(),
            entries: entries.into(),
            front: -1,
            edits: Vec::new(),
            current: None,
        }
    }
    pub(super) fn pop_front(&mut self) -> Option<Task> {
        let task = self.entries.pop_front()?;
        let position = self.positions.pop_front().unwrap();
        self.current = Some(self::position(position));
        self.edits.push(Edit::Remove {
            role: QUEUE,
            key: self::position(position),
        });
        Some(task)
    }
    pub(super) fn push_front(&mut self, task: Task) {
        let key = self::position(self.front);
        self.positions.push_front(self.front);
        self.front = self
            .front
            .checked_sub(1)
            .expect("transfer task positions exhausted");
        self.edits.push(Edit::Put(Row {
            role: QUEUE,
            key,
            body: encoded(&Item::from(task.clone())).expect("transfer task serialization"),
        }));
        self.entries.push_front(task);
    }
    pub(super) fn into_tasks(self) -> impl Iterator<Item = Task> {
        self.entries.into_iter()
    }
}
impl Deref for Queue {
    type Target = VecDeque<Task>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

pub(super) struct Tracker {
    scope: store::ActiveCursorScope,
    progress: Progress,
    saved: BTreeMap<PathBuf, String>,
    copied: BTreeMap<PathBuf, String>,
    expansion: Option<(Vec<u8>, usize)>,
}
impl Tracker {
    pub(super) fn begin(
        state: &mut Store,
        execution: &requests::RequestRecord,
        id: u64,
        plan: &TransferContinuation,
        observed: &ObservedTransfer,
        root: &DestinationObservation,
    ) -> io::Result<Self> {
        let progress = Progress {
            operation: None,
            completed: 0,
            bytes: 0,
            root: root.restart_original(),
            counts: vec![
                plan.pending.len(),
                plan.directories.len(),
                0,
                0,
                0,
                plan.saved.len(),
                plan.copied.len(),
                0,
                0,
                0,
            ],
        };
        let spec = Spec {
            kind: plan.kind,
            destination: plan.destination.clone(),
            restart: observed.restart_observation(),
        };
        let mut edits = Vec::new();
        for (index, task) in plan.pending.iter().enumerate() {
            edits.push(put(
                QUEUE,
                position(index as i64),
                &Item::from(task.clone()),
            )?);
        }
        for (source, (destination, finish)) in &plan.directories {
            edits.push(put(
                DIRECTORY,
                source.as_os_str().as_bytes().to_vec(),
                &Item::from((source.clone(), destination.clone(), Some(*finish))),
            )?);
        }
        for (source, receipt) in &plan.saved {
            edits.push(put(
                SAVED,
                source.as_os_str().as_bytes().to_vec(),
                &Saved {
                    source: source.clone(),
                    receipt: *receipt,
                },
            )?);
        }
        for (source, proof) in &plan.copied {
            edits.push(put(COPIED, source.as_os_str().as_bytes().to_vec(), proof)?);
        }
        let copied = plan
            .copied
            .iter()
            .map(|(source, proof)| Ok((source.clone(), encoded(proof)?)))
            .collect::<io::Result<_>>()?;
        let scope = state.begin_active_transfer(
            execution,
            id,
            &encoded(&spec)?,
            Update {
                progress: encoded(&progress)?,
                edits,
            },
        )?;
        let saved = plan
            .saved
            .iter()
            .map(|(source, receipt)| {
                Ok((
                    source.clone(),
                    encoded(&Saved {
                        source: source.clone(),
                        receipt: *receipt,
                    })?,
                ))
            })
            .collect::<io::Result<_>>()?;
        Ok(Self {
            scope,
            progress,
            saved,
            copied,
            expansion: None,
        })
    }
    /// Binding a previously unavailable root is durable before the first write.
    /// Keep the unconsumed head and its previous history watermark intact here.
    pub(super) fn bind_root(
        &mut self,
        state: &mut Store,
        root: &DestinationObservation,
    ) -> io::Result<()> {
        let current = root.restart_original();
        if encoded(&current)? != encoded(&self.progress.root)? {
            self.progress.root = current;
            state.update_active_transfer(
                &self.scope,
                Update {
                    progress: encoded(&self.progress)?,
                    edits: Vec::new(),
                },
            )?;
        }
        Ok(())
    }
    pub(super) fn checkpoint(
        &mut self,
        state: &mut Store,
        checkpoint: Checkpoint<'_>,
    ) -> io::Result<()> {
        let Checkpoint {
            queue,
            directories,
            changed_directories,
            saved,
            copied,
            failed,
            result,
            root,
        } = checkpoint;
        let mut edits = std::mem::take(&mut queue.edits);
        if let Some((key, children)) = self.expansion.take() {
            edits.push(Edit::Remove {
                role: EXPANSION,
                key: key.clone(),
            });
            for index in 0..children {
                edits.push(Edit::Remove {
                    role: CHILD,
                    key: expansion::child_key(&key, index),
                });
            }
        }
        for (source, destination, finish) in changed_directories.drain(..) {
            edits.push(put(
                DIRECTORY,
                source.as_os_str().as_bytes().to_vec(),
                &Item::from((source, destination, finish)),
            )?);
        }
        for (index, task) in failed
            .iter()
            .enumerate()
            .skip(self.progress.counts[FAILED as usize])
        {
            let failure = &result.failures[index];
            edits.push(put(
                FAILED,
                number(index),
                &Failure {
                    item: Item::from(task.clone()),
                    report: TransferReportFailure {
                        source: failure.source.clone(),
                        destination: failure.destination.clone(),
                        message: failure.message.clone(),
                    },
                },
            )?);
        }
        for (role, names) in [
            (SKIPPED, &result.skipped),
            (DESTINATION, &result.destinations),
            (REMAINDER, &result.source_remainders),
        ] {
            for (index, name) in names
                .iter()
                .enumerate()
                .skip(self.progress.counts[role as usize])
            {
                edits.push(put(role, number(index), &Name(name.clone()))?);
            }
        }
        let mut next_saved = BTreeMap::new();
        for (source, receipt) in saved {
            let body = encoded(&Saved {
                source: source.clone(),
                receipt: *receipt,
            })?;
            if self.saved.get(source) != Some(&body) {
                edits.push(Edit::Put(Row {
                    role: SAVED,
                    key: source.as_os_str().as_bytes().to_vec(),
                    body: body.clone(),
                }));
            }
            next_saved.insert(source.clone(), body);
        }
        for source in self.saved.keys() {
            if !next_saved.contains_key(source) {
                edits.push(Edit::Remove {
                    role: SAVED,
                    key: source.as_os_str().as_bytes().to_vec(),
                });
            }
        }
        let mut next_copied = BTreeMap::new();
        for (source, proof) in copied {
            let body = encoded(proof)?;
            if self.copied.get(source) != Some(&body) {
                edits.push(Edit::Put(Row {
                    role: COPIED,
                    key: source.as_os_str().as_bytes().to_vec(),
                    body: body.clone(),
                }));
            }
            next_copied.insert(source.clone(), body);
        }
        for source in self.copied.keys() {
            if !next_copied.contains_key(source) {
                edits.push(Edit::Remove {
                    role: COPIED,
                    key: source.as_os_str().as_bytes().to_vec(),
                });
            }
        }
        self.copied = next_copied;
        self.saved = next_saved;
        self.progress = Progress {
            operation: result.id,
            completed: result.completed,
            bytes: result.bytes,
            root: root.restart_original(),
            counts: vec![
                queue.len(),
                directories.len(),
                failed.len(),
                result.skipped.len(),
                result.destinations.len(),
                saved.len(),
                copied.len(),
                result.source_remainders.len(),
                0,
                0,
            ],
        };
        state.update_active_transfer(
            &self.scope,
            Update {
                progress: encoded(&self.progress)?,
                edits,
            },
        )
    }
}
pub(super) struct Checkpoint<'a> {
    pub queue: &'a mut Queue,
    pub directories: &'a BTreeMap<PathBuf, (PathBuf, DirectoryFinish)>,
    pub changed_directories: &'a mut Vec<Task>,
    pub saved: &'a BTreeMap<PathBuf, saved_stage::Receipt>,
    pub copied: &'a BTreeMap<PathBuf, copied_leaf::Cleanup>,
    pub failed: &'a [Task],
    pub result: &'a TransferOutcome,
    pub root: &'a DestinationObservation,
}

pub(in crate::operations) enum Recovered {
    Stopped(Box<StoppedTransfer>),
    Completed(execution::CompletedTransfer),
}

pub(in crate::operations) fn recover(record: Recorded) -> io::Result<Recovered> {
    let spec: Spec = decoded(&record.spec)?;
    let mut progress: Progress = decoded(&record.progress)?;
    if (6..10).contains(&progress.counts.len()) {
        progress.counts.resize(10, 0);
    }
    let restart = spec.restart.with_current_root(progress.root)?;
    let mut counts = vec![0; 10];
    let mut queue = Vec::new();
    let mut directories = BTreeMap::new();
    let mut failed = Vec::new();
    let mut failures = Vec::new();
    let mut skipped = Vec::new();
    let mut destinations = Vec::new();
    let mut source_remainders = Vec::new();
    let mut saved = BTreeMap::new();
    let mut copied = BTreeMap::new();
    for row in record.rows {
        let role = usize::from(row.role);
        if role >= counts.len() {
            return Err(invalid("Unknown active transfer row."));
        }
        match row.role {
            QUEUE => {
                if row.key.len() != 8 {
                    return Err(invalid("Invalid transfer queue position."));
                }
                queue.push(Task::from(decoded::<Item>(&row.body)?));
            }
            DIRECTORY => {
                let item: Item = decoded(&row.body)?;
                if row.key != item.source.as_os_str().as_bytes() {
                    return Err(invalid("The traversal folder key changed."));
                }
                let finish = item
                    .finish
                    .ok_or_else(|| invalid("The traversal folder completion is unavailable."))?;
                if directories
                    .insert(item.source, (item.destination, finish))
                    .is_some()
                {
                    return Err(invalid("Duplicate traversal folder."));
                }
            }
            FAILED => {
                if row.key != number(counts[role]) {
                    return Err(invalid("The failed task order changed."));
                }
                let failure: Failure = decoded(&row.body)?;
                if failure.item.source != failure.report.source
                    || failure.item.destination != failure.report.destination
                {
                    return Err(invalid("The failed task changed."));
                }
                failed.push(failure.item.into());
                failures.push(failure.report);
            }
            SKIPPED | DESTINATION | REMAINDER => {
                if row.key != number(counts[role]) {
                    return Err(invalid("The transfer result order changed."));
                }
                let name: Name = decoded(&row.body)?;
                if !name.0.is_absolute() {
                    return Err(invalid("Transfer paths must remain absolute."));
                }
                if row.role == SKIPPED {
                    skipped.push(name.0);
                } else if row.role == DESTINATION {
                    destinations.push(name.0);
                } else {
                    source_remainders.push(name.0);
                }
            }
            SAVED => {
                let item: Saved = decoded(&row.body)?;
                if row.key != item.source.as_os_str().as_bytes()
                    || saved.insert(item.source, item.receipt).is_some()
                {
                    return Err(invalid("The saved task changed."));
                }
            }
            COPIED => {
                let proof: copied_leaf::Cleanup = decoded(&row.body)?;
                if row.key != proof.source.path.as_os_str().as_bytes()
                    || copied.insert(proof.source.path.clone(), proof).is_some()
                {
                    return Err(invalid("The copied source cleanup changed."));
                }
            }
            _ => {
                return Err(invalid(
                    "The directory expansion has not been independently reconciled.",
                ));
            }
        }
        counts[role] += 1;
    }
    if counts != progress.counts {
        return Err(invalid("The active transfer lost or gained a task."));
    }
    if copied.values().any(|proof| {
        spec.kind != TransferKind::Move
            || !queue
                .iter()
                .chain(&failed)
                .any(|(source, destination, finish)| {
                    finish.is_none() && proof.valid_for(source, destination)
                })
    }) {
        return Err(invalid(
            "A copied-source cleanup has no exact unfinished task.",
        ));
    }
    if queue.is_empty() && failed.is_empty() {
        if progress.operation.is_none() || !saved.is_empty() {
            return Err(invalid(
                "The active transfer cannot prove its complete end.",
            ));
        }
        let completion = execution::CompletedTransfer {
            kind: spec.kind,
            destination: spec.destination,
            operation: progress.operation,
            completed: progress.completed,
            bytes: progress.bytes,
            skipped,
            destinations,
            source_remainders,
            cancelled: false,
            needs_attention: false,
            attention: None,
        };
        if !completion.valid() {
            return Err(invalid("The active transfer completion is invalid."));
        }
        return Ok(Recovered::Completed(completion));
    }
    let plan = |pending: Vec<Task>| -> io::Result<TransferContinuation> {
        let plan = TransferContinuation {
            kind: spec.kind,
            destination: spec.destination.clone(),
            copied: copied_for(&pending, &copied),
            pending,
            directories: directories.clone(),
            saved: saved.clone(),
            restart: Some(restart.clone()),
            observed: None,
            destination_observation: None,
        };
        // Apply the checkpoint codec's path, traversal and original-scope rules.
        serde_json::from_value(serde_json::to_value(plan).map_err(io::Error::other)?)
            .map_err(io::Error::other)
    };
    let retry = if failed.is_empty() {
        None
    } else {
        Some(plan(retry_tasks(spec.kind, failed, &directories))?)
    };
    let remaining_paths = queue.iter().map(|(source, _, _)| source.clone()).collect();
    let remaining = if queue.is_empty() {
        None
    } else {
        Some(plan(queue)?)
    };
    let stop = StoppedTransfer {
        report: TransferReport {
            kind: spec.kind,
            destination: spec.destination,
            operation: progress.operation,
            completed: progress.completed,
            bytes: progress.bytes,
            failures,
            skipped,
            remaining: remaining_paths,
            source_remainders,
            destinations,
            cancelled: false,
            needs_attention: true,
            attention: Some(
                "Files closed before this transfer finished. Continue its unfinished work.".into(),
            ),
            failed_resolution: TransferResolution::Available,
            remaining_resolution: TransferResolution::Available,
            attributes_omitted: Vec::new(),
        },
        retry,
        remaining,
        interrupted: None,
        rejected_before_execution: false,
    };
    if !stop.valid() || (stop.retry.is_none() && stop.remaining.is_none()) {
        return Err(invalid("The active transfer has no exact unfinished part."));
    }
    Ok(Recovered::Stopped(Box::new(stop)))
}

/// This receipt is restricted to the still-durable queue head. A Keep Both
/// reply can change its leaf name, but never its source or destination parent.
pub(in crate::operations) fn binds(row: &Row, change: &Change) -> io::Result<bool> {
    let task: Item = decoded(&row.body)?;
    if row.role != QUEUE || row.key.len() != 8 || task.finish.is_some() {
        return Ok(false);
    }
    let (source, destination) = match change {
        Change::Copied {
            source: Some(source),
            publication: Some(_),
            path,
            ..
        } => (&source.path, path),
        Change::SourceRemoved {
            original,
            current,
            originals: Some(_),
            ..
        } => (original, current),
        _ => return Ok(false),
    };
    Ok(source == &task.source && destination.parent() == task.destination.parent())
}

pub(in crate::operations) fn binds_entry_move(row: &Row, change: &Change) -> io::Result<bool> {
    let task: Item = decoded(&row.body)?;
    let Change::Rename {
        move_facts: Some(facts),
        original,
        current,
        identity,
        retained: None,
        reviewed: None,
        saved: None,
    } = change
    else {
        return Ok(false);
    };
    Ok(row.role == QUEUE
        && row.key.len() == 8
        && task.finish.is_none()
        && task.source == *original
        && original != current
        && task.destination.parent() == current.parent()
        && facts.matches_identity(*identity))
}

pub(in crate::operations) fn binds_reviewed_replacement(
    row: &Row,
    change: &Change,
) -> io::Result<bool> {
    let task: Item = decoded(&row.body)?;
    let (source, destination, retained) = match change {
        Change::Copied {
            source: Some(source),
            publication: Some(_),
            path,
            retained: Some(retained),
            ..
        } => (&source.path, path, retained),
        Change::Rename {
            original,
            current,
            identity,
            move_facts: Some(facts),
            retained: Some(retained),
            reviewed: None,
            saved: None,
        } if facts.matches_identity(*identity) => (original, current, retained),
        _ => return Ok(false),
    };
    Ok(row.role == QUEUE
        && row.key.len() == 8
        && task.finish.is_none()
        && task.source == *source
        && task.destination == *destination
        && retained
            .replacement_facts
            .as_ref()
            .is_some_and(|facts| facts.matches_retained(retained, destination)))
}
pub(in crate::operations) fn binds_saved_move(row: &Row, change: &Change) -> io::Result<bool> {
    let task: Item = decoded(&row.body)?;
    let Change::Rename {
        original,
        current,
        identity,
        move_facts: Some(facts),
        retained: None,
        reviewed: None,
        saved: Some(saved),
    } = change
    else {
        return Ok(false);
    };
    Ok(row.role == QUEUE
        && row.key.len() == 8
        && task.finish.is_none()
        && task.source == *original
        && task.destination.parent() == current.parent()
        && facts.matches_identity(*identity)
        && saved.matches_move(facts)
        && saved.counted().is_some())
}

pub(in crate::operations) fn binds_saved_copy(row: &Row, change: &Change) -> io::Result<bool> {
    let task: Item = decoded(&row.body)?;
    let Change::Rename {
        original,
        current,
        move_facts: None,
        retained: None,
        reviewed: None,
        saved: Some(saved),
        ..
    } = change
    else {
        return Ok(false);
    };
    Ok(row.role == QUEUE
        && row.key.len() == 8
        && task.finish.is_none()
        && task.source == *original
        && task.destination.parent() == current.parent()
        && saved.copied_bytes().is_some()
        && saved.counted().is_some())
}

/// Consume only filesystem steps independently bound before their mutation.
/// Completed outputs need no new authority; a Move's unfinished unlink does.
pub(in crate::operations) fn advance(
    record: &mut Recorded,
    operation: u64,
    steps: &[Change],
) -> io::Result<()> {
    let spec: Spec = decoded(&record.spec)?;
    let mut progress: Progress = decoded(&record.progress)?;
    if (6..10).contains(&progress.counts.len()) {
        progress.counts.resize(10, 0);
    }
    if progress.counts.len() != 10 {
        return Err(invalid("Invalid active transfer counts."));
    }
    // A Copy task consumes exactly one step, so several steps of a Copy are a
    // copy batch: one plain copy per consecutive queued task (ADR0180).
    if spec.kind == TransferKind::Copy
        && steps.len() > 1
        && steps
            .iter()
            .all(|change| matches!(change, Change::Copied { retained: None, .. }))
    {
        for step in steps {
            advance(record, operation, std::slice::from_ref(step))?;
        }
        return Ok(());
    }
    let index = record
        .rows
        .iter()
        .position(|row| row.role == QUEUE)
        .ok_or_else(|| invalid("The completed step has no current task."))?;
    let mut task: Item = decoded(&record.rows[index].body)?;
    if matches!(
        steps.first(),
        Some(Change::Rename {
            move_facts: Some(_),
            ..
        })
    ) || steps.first().is_some_and(
        |c| matches!(c,Change::Rename {saved:Some(s),..} if s.copied_bytes().is_some()),
    ) {
        return advance_entry_move(record, operation, steps);
    }
    if matches!(steps.first(), Some(Change::RemovedDirectory { .. })) {
        return advance_directory_finish(record, operation, steps);
    }
    if matches!(steps.first(), Some(Change::CreatedFolder { .. })) {
        if steps.len() != 1 {
            return Err(invalid(
                "A directory publication has one exact expansion step.",
            ));
        }
        return expansion::advance(record, operation, steps.first());
    }
    if steps.is_empty()
        || steps.len() > 2
        || steps.iter().any(|change| {
            if change
                .retained()
                .is_some_and(|r| r.replacement_facts.is_some())
            {
                !binds_reviewed_replacement(&record.rows[index], change).unwrap_or(false)
            } else {
                !binds(&record.rows[index], change).unwrap_or(false)
            }
        })
    {
        return Err(invalid(
            "The completed filesystem steps differ from their active task.",
        ));
    }
    let existing = record
        .rows
        .iter()
        .position(|row| row.role == COPIED && row.key == task.source.as_os_str().as_bytes());
    let (proof, removal) = if let Some(copied) = existing {
        if spec.kind != TransferKind::Move || steps.len() != 1 {
            return Err(invalid("The copied-source cleanup changed."));
        }
        (
            decoded::<copied_leaf::Cleanup>(&record.rows[copied].body)?,
            Some(&steps[0]),
        )
    } else {
        let proof = copied_leaf::Cleanup::from_change(&steps[0])?;
        progress.bytes = progress
            .bytes
            .checked_add(proof.source.bytes)
            .ok_or_else(|| invalid("Transfer byte count overflow."))?;
        let removal = steps.get(1);
        if spec.kind == TransferKind::Copy && removal.is_some() {
            return Err(invalid("A Copy cannot consume a source-removal step."));
        }
        (proof, removal)
    };
    if let Some(change) = removal
        && !proof.matches_removal(change)
    {
        return Err(invalid(
            "The source-removal step differs from its committed copy.",
        ));
    }
    task.destination = proof.destination.clone();
    progress.operation = Some(operation);
    let complete = spec.kind == TransferKind::Copy || removal.is_some();
    if complete {
        progress.completed = progress
            .completed
            .checked_add(1)
            .ok_or_else(|| invalid("Transfer item count overflow."))?;
        progress.counts[QUEUE as usize] = progress.counts[QUEUE as usize]
            .checked_sub(1)
            .ok_or_else(|| invalid("The active queue count changed."))?;
        if task.destination.parent() == Some(spec.destination.as_path()) {
            let count = &mut progress.counts[DESTINATION as usize];
            let Edit::Put(row) = put(DESTINATION, number(*count), &Name(task.destination))? else {
                unreachable!()
            };
            record.rows.push(row);
            *count += 1;
        }
        record.rows.remove(index);
        if let Some(copied) = record
            .rows
            .iter()
            .position(|row| row.role == COPIED && row.key == task.source.as_os_str().as_bytes())
        {
            record.rows.remove(copied);
            progress.counts[COPIED as usize] = progress.counts[COPIED as usize]
                .checked_sub(1)
                .ok_or_else(|| invalid("The copied-source count changed."))?;
        }
    } else {
        record.rows[index].body = encoded(&task)?;
        let Edit::Put(row) = put(COPIED, task.source.as_os_str().as_bytes().to_vec(), &proof)?
        else {
            unreachable!()
        };
        record.rows.push(row);
        progress.counts[COPIED as usize] += 1;
    }
    record.progress = encoded(&progress)?;
    // The decoder also checks exact counts and the canonical row order.
    record
        .rows
        .sort_by(|a, b| (a.role, &a.key).cmp(&(b.role, &b.key)));
    Ok(())
}

pub(in crate::operations) fn scope(spec: &str) -> io::Result<(TransferKind, PathBuf)> {
    SPEC.with(|cached| {
        let spec = cached.borrow_mut().read(spec, decoded)?;
        Ok((spec.kind, spec.destination.clone()))
    })
}

fn advance_entry_move(record: &mut Recorded, operation: u64, steps: &[Change]) -> io::Result<()> {
    let spec: Spec = decoded(&record.spec)?;
    let mut progress: Progress = decoded(&record.progress)?;
    if (6..10).contains(&progress.counts.len()) {
        progress.counts.resize(10, 0);
    }
    if spec.kind != TransferKind::Move || progress.counts.len() != 10 || steps.len() != 1 {
        return Err(invalid("The whole-entry Move differs from its exact task."));
    }
    let index = record
        .rows
        .iter()
        .position(|row| row.role == QUEUE)
        .ok_or_else(|| invalid("The completed Move has no active task."))?;
    if !(binds_entry_move(&record.rows[index], &steps[0])?
        || binds_reviewed_replacement(&record.rows[index], &steps[0])?
        || binds_saved_move(&record.rows[index], &steps[0])?
        || binds_saved_copy(&record.rows[index], &steps[0])?)
    {
        return Err(invalid(
            "The completed Move differs from its original task.",
        ));
    }
    let Change::Rename { current, saved, .. } = &steps[0] else {
        unreachable!()
    };
    let counted = saved
        .as_ref()
        .map_or(Some(true), |saved| saved.counted())
        .ok_or_else(|| invalid("The saved move lost its completion facts."))?;
    if let Some(bytes) = saved.as_ref().and_then(|s| s.copied_bytes()) {
        progress.bytes = progress
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid("Saved transfer byte count overflow."))?;
    }
    if counted {
        progress.completed = progress
            .completed
            .checked_add(1)
            .ok_or_else(|| invalid("Transfer item count overflow."))?;
    }
    progress.operation = Some(operation);
    progress.counts[QUEUE as usize] = progress.counts[QUEUE as usize]
        .checked_sub(1)
        .ok_or_else(|| invalid("The active queue count changed."))?;
    if counted && current.parent() == Some(spec.destination.as_path()) {
        let count = &mut progress.counts[DESTINATION as usize];
        let Edit::Put(row) = put(DESTINATION, number(*count), &Name(current.clone()))? else {
            unreachable!()
        };
        *count = count
            .checked_add(1)
            .ok_or_else(|| invalid("Transfer destination count overflow."))?;
        record.rows.push(row);
    }
    record.rows.remove(index);
    record
        .rows
        .sort_by(|a, b| (a.role, &a.key).cmp(&(b.role, &b.key)));
    record.progress = encoded(&progress)?;
    Ok(())
}

/// A postorder directory step has exact original folders and an actual captured
/// range. A resumed traversal can start a new range, never claim a new creation.
pub(in crate::operations) fn binds_directory_finish(
    row: &Row,
    operation: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<bool> {
    let task: Item = decoded(&row.body)?;
    let Some(finish) = task.finish else {
        return Ok(false);
    };
    let Change::RemovedDirectory {
        path,
        identity,
        parent_identity,
        mode,
        moved: Some(moved),
        removal: Some(facts),
    } = change
    else {
        return Ok(false);
    };
    Ok(row.role == QUEUE
        && row.key.len() == 8
        && task.source == *path
        && task.destination == moved.current
        && finish.identity == *identity
        && finish.parent_identity == *parent_identity
        && finish.destination_identity == moved.identity
        && finish.mode == *mode
        && moved.first <= ordinal
        && facts.matches(finish.original, *identity, *parent_identity, moved.identity)
        && if finish.operation == operation {
            moved.first == finish.first && moved.created == finish.created
        } else {
            moved.created.is_none()
        })
}

fn advance_directory_finish(
    record: &mut Recorded,
    operation: u64,
    steps: &[Change],
) -> io::Result<()> {
    let spec: Spec = decoded(&record.spec)?;
    let mut progress: Progress = decoded(&record.progress)?;
    if (6..10).contains(&progress.counts.len()) {
        progress.counts.resize(10, 0);
    }
    if spec.kind != TransferKind::Move || progress.counts.len() != 10 || steps.len() != 1 {
        return Err(invalid(
            "The directory removal differs from its exact task.",
        ));
    }
    let index = record
        .rows
        .iter()
        .position(|row| row.role == QUEUE)
        .ok_or_else(|| invalid("The completed directory has no active task."))?;
    let task: Item = decoded(&record.rows[index].body)?;
    let finish = task
        .finish
        .ok_or_else(|| invalid("The directory lost its postorder facts."))?;
    let Change::RemovedDirectory {
        moved: Some(moved), ..
    } = &steps[0]
    else {
        return Err(invalid("The directory has a different completed change."));
    };
    // The store verified the real ordinal/range before entering this decoder.
    if !binds_directory_finish(&record.rows[index], operation, moved.first, &steps[0])? {
        return Err(invalid(
            "The completed directory differs from its original folders.",
        ));
    }
    if progress.completed == finish.completed_before
        || (finish.was_empty && finish.created.is_none())
    {
        progress.completed = progress
            .completed
            .checked_add(1)
            .ok_or_else(|| invalid("Directory completion count overflow."))?;
    }
    progress.operation = Some(operation);
    progress.counts[QUEUE as usize] = progress.counts[QUEUE as usize]
        .checked_sub(1)
        .ok_or_else(|| invalid("The directory queue count changed."))?;
    record.rows.remove(index);
    record.progress = encoded(&progress)?;
    Ok(())
}
