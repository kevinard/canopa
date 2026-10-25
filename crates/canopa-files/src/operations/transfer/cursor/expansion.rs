//! One original folder expansion, persisted before publication or child work.
use super::*;
use sha2::{Digest, Sha256};
use std::ffi::OsString;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    item: Item,
    operation: u64,
    first: u64,
    stage: Option<Stage>,
    change: Option<Change>,
    children: usize,
    names_sha256: [u8; 32],
}
pub(super) fn child_key(head: &[u8], index: usize) -> Vec<u8> {
    [head, &number(index)].concat()
}
fn names_digest(names: &[OsString]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for name in names {
        let bytes = name.as_bytes();
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize().into()
}
fn header(head: &Row, row: &Row, operation: u64, first: u64) -> io::Result<Header> {
    let task: Item = decoded(&head.body)?;
    let header: Header = decoded(&row.body)?;
    let finish = header
        .item
        .finish
        .ok_or_else(|| invalid("Missing original folder expansion."))?;
    if head.role != QUEUE
        || head.key.len() != 8
        || row.role != EXPANSION
        || row.key != head.key
        || task.finish.is_some()
        || task.source != header.item.source
        || task.destination.parent() != header.item.destination.parent()
        || header.operation != operation
        || header.first != first
        || !header.item.destination.is_absolute()
        || finish.mode & !0o777 != 0
        || finish.was_empty != (header.children == 0)
    {
        return Err(invalid(
            "The folder expansion differs from its exact active task.",
        ));
    }
    match (&header.stage, &header.change) {
        (None, None) => {}
        (
            Some(_),
            Some(Change::CreatedFolder {
                path,
                identity,
                mode,
                ..
            }),
        ) if path == &header.item.destination
            && *identity == finish.destination_identity
            && *mode == Some(finish.mode) => {}
        _ => return Err(invalid("The expansion has a different publication role.")),
    }
    if let Some(stage) = &header.stage
        && (finish.operation != operation
            || finish.first != first
            || finish.created != Some(first)
            || stage.payload_identity != Some(finish.destination_identity)
            || stage.directory.parent() != header.item.destination.parent()
            || stage.replacement.is_some()
            || stage.entry_move.is_some()
            || stage.source_claim.is_some()
            || stage.directory_claim.is_some()
            || stage.sibling_payload
            || finish.original.is_some_and(|original| {
                stage
                    .ownership
                    .as_ref()
                    .and_then(|proof| proof.payload_origins())
                    != Some(original.destination_origins())
            }))
    {
        return Err(invalid(
            "The prepared folder differs from its original expansion.",
        ));
    }
    Ok(header)
}
pub(in crate::operations) fn validate_prepared(
    head: &Row,
    row: &Row,
    operation: u64,
    first: u64,
    preparing: Option<&Pending>,
) -> io::Result<()> {
    let header = header(head, row, operation, first)?;
    match (&header.stage, preparing) {
        (None, None) => Ok(()),
        (
            Some(stage),
            Some(Pending::Preparing {
                id,
                destination,
                stage: actual,
            }),
        ) if *id == operation
            && destination == &header.item.destination
            && encoded(stage)? == encoded(actual)? =>
        {
            Ok(())
        }
        _ => Err(invalid(
            "The original expansion and Preparing intent differ.",
        )),
    }
}
pub(in crate::operations) fn binds(
    head: &Row,
    row: &Row,
    operation: u64,
    first: u64,
    change: &Change,
    stage: Option<&Stage>,
) -> io::Result<bool> {
    let header = header(head, row, operation, first)?;
    let finish = header.item.finish.unwrap();
    let Change::CreatedFolder {
        path,
        identity,
        mode,
        ..
    } = change
    else {
        return Ok(false);
    };
    Ok(header.stage.is_some()
        && path == &header.item.destination
        && *identity == finish.destination_identity
        && *mode == Some(finish.mode)
        && header.change.as_ref().map(encoded).transpose()? == Some(encoded(change)?)
        && stage.map(encoded).transpose()? == header.stage.as_ref().map(encoded).transpose()?)
}
pub(in crate::operations) fn committed(
    head: &Row,
    row: &Row,
    operation: u64,
    first: u64,
    change: &Change,
) -> io::Result<bool> {
    let header = header(head, row, operation, first)?;
    binds(head, row, operation, first, change, header.stage.as_ref())
}
pub(in crate::operations::transfer) struct Preparation<'a> {
    pub task: Task,
    pub stage: Option<&'a Stage>,
    pub change: Option<&'a Change>,
    pub names: &'a [OsString],
    pub operation: u64,
}
impl Tracker {
    pub(in crate::operations::transfer) fn prepare_directory(
        &mut self,
        state: &mut Store,
        queue: &Queue,
        preparation: Preparation<'_>,
    ) -> io::Result<()> {
        let Preparation {
            task,
            stage,
            change,
            names,
            operation,
        } = preparation;
        if self.expansion.is_some() {
            return Err(invalid("An earlier directory expansion is still active."));
        }
        let key = queue
            .current
            .clone()
            .ok_or_else(|| invalid("The directory has no active queue head."))?;
        let header = Header {
            operation,
            first: state.next_ordinal(operation)?,
            item: Item::from(task),
            stage: stage.cloned(),
            change: change.cloned(),
            children: names.len(),
            names_sha256: names_digest(names),
        };
        let preparing = header.stage.as_ref().map(|stage| Pending::Preparing {
            id: header.operation,
            destination: header.item.destination.clone(),
            stage: stage.clone(),
        });
        let mut edits = vec![put(EXPANSION, key.clone(), &header)?];
        for (index, name) in names.iter().enumerate() {
            edits.push(put(
                CHILD,
                child_key(&key, index),
                &Name(PathBuf::from(name)),
            )?);
        }
        let mut progress = self.progress.clone();
        progress.counts[EXPANSION as usize] = 1;
        progress.counts[CHILD as usize] = names.len();
        state.prepare_active_directory(
            &self.scope,
            Update {
                progress: encoded(&progress)?,
                edits,
            },
            preparing.as_ref(),
        )?;
        self.progress = progress;
        self.expansion = Some((key, names.len()));
        Ok(())
    }
}
pub(in crate::operations) fn advance(
    record: &mut Recorded,
    operation: u64,
    change: Option<&Change>,
) -> io::Result<()> {
    let spec: Spec = decoded(&record.spec)?;
    let mut progress: Progress = decoded(&record.progress)?;
    if (6..10).contains(&progress.counts.len()) {
        progress.counts.resize(10, 0);
    }
    if progress.counts.len() != 10 || progress.counts[EXPANSION as usize] != 1 {
        return Err(invalid("The original directory expansion is unavailable."));
    }
    let head = record
        .rows
        .iter()
        .find(|row| row.role == QUEUE)
        .ok_or_else(|| invalid("The expansion lost its active head."))?;
    let row = record
        .rows
        .iter()
        .find(|row| row.role == EXPANSION)
        .ok_or_else(|| invalid("The expansion lost its original descriptor."))?;
    let descriptor: Header = decoded(&row.body)?;
    let first = descriptor.first;
    let header = header(head, row, operation, first)?;
    let created = match (header.stage.as_ref(), change) {
        (Some(_), Some(change)) if committed(head, row, operation, first, change)? => true,
        (None, None) => false,
        _ => {
            return Err(invalid(
                "The folder publication differs from its exact expansion.",
            ));
        }
    };
    let children: Vec<_> = record.rows.iter().filter(|row| row.role == CHILD).collect();
    if children.len() != header.children || children.len() != progress.counts[CHILD as usize] {
        return Err(invalid("The expansion lost or gained a child."));
    }
    let mut names = Vec::new();
    for (index, row) in children.iter().enumerate() {
        let Name(path) = decoded(&row.body)?;
        let mut components = path.components();
        let bytes = path.as_os_str().as_bytes();
        if bytes.contains(&b'/')
            || bytes.contains(&0)
            || row.key != child_key(&head.key, index)
            || !matches!(components.next(), Some(std::path::Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(invalid("Invalid original directory child name."));
        }
        names.push(path.into_os_string());
    }
    if names_digest(&names) != header.names_sha256
        || names.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("The original child enumeration changed."));
    }
    let head_key = head.key.clone();
    let number = i64::from_be_bytes(
        (u64::from_be_bytes(head_key.as_slice().try_into().unwrap()) ^ (1 << 63)).to_be_bytes(),
    );
    let first_child = number
        .checked_sub(
            i64::try_from(names.len()).map_err(|_| invalid("Too many directory children."))?,
        )
        .ok_or_else(|| invalid("Directory queue positions exhausted."))?;
    record.rows.retain(|row| {
        row.role != EXPANSION && row.role != CHILD && !(row.role == QUEUE && row.key == head_key)
    });
    for (index, name) in names.iter().enumerate() {
        let Edit::Put(row) = put(
            QUEUE,
            position(first_child + index as i64),
            &Item::from((
                header.item.source.join(name),
                header.item.destination.join(name),
                None,
            )),
        )?
        else {
            unreachable!()
        };
        record.rows.push(row);
    }
    if spec.kind == TransferKind::Move {
        let Edit::Put(row) = put(QUEUE, head_key, &header.item)? else {
            unreachable!()
        };
        record.rows.push(row);
    }
    let directory_key = header.item.source.as_os_str().as_bytes().to_vec();
    if !record
        .rows
        .iter()
        .any(|row| row.role == DIRECTORY && row.key == directory_key)
    {
        progress.counts[DIRECTORY as usize] += 1;
    }
    record
        .rows
        .retain(|row| row.role != DIRECTORY || row.key != directory_key);
    let Edit::Put(row) = put(DIRECTORY, directory_key, &header.item)? else {
        unreachable!()
    };
    record.rows.push(row);
    progress.operation = Some(operation);
    progress.counts[QUEUE as usize] = progress.counts[QUEUE as usize]
        .checked_sub(1)
        .and_then(|n| n.checked_add(names.len() + usize::from(spec.kind == TransferKind::Move)))
        .ok_or_else(|| invalid("Invalid expanded queue count."))?;
    progress.counts[EXPANSION as usize] = 0;
    progress.counts[CHILD as usize] = 0;
    if created {
        progress.completed = progress
            .completed
            .checked_add(1)
            .ok_or_else(|| invalid("Directory count overflow."))?;
        if header.item.destination.parent() == Some(spec.destination.as_path()) {
            let count = &mut progress.counts[DESTINATION as usize];
            let Edit::Put(row) = put(
                DESTINATION,
                super::number(*count),
                &Name(header.item.destination),
            )?
            else {
                unreachable!()
            };
            record.rows.push(row);
            *count += 1;
        }
    }
    record.progress = encoded(&progress)?;
    record
        .rows
        .sort_by(|a, b| (a.role, &a.key).cmp(&(b.role, &b.key)));
    Ok(())
}

/// Merge accepts an existing original object and has no creation history step.
pub(in crate::operations) fn advance_merge(
    record: &mut Recorded,
    operation: u64,
) -> io::Result<()> {
    let Some(row) = record.rows.iter().find(|row| row.role == EXPANSION) else {
        return Ok(());
    };
    let header: Header = decoded(&row.body)?;
    if header.stage.is_some() {
        return Err(invalid(
            "Review the prepared directory publication before continuing its children.",
        ));
    }
    if header.change.is_some() {
        return Err(invalid("A Merge cannot borrow a creation step."));
    }
    advance(record, operation, None)
}

/// A verified cursor can promote only its own exact prepared publication. This
/// checks the complete proposed graph; it does not record an end or grant parts.
pub(in crate::operations) fn publication_intent(
    record: &Recorded,
    operation: u64,
    first: u64,
    pending: &Pending,
) -> io::Result<Pending> {
    let head = record
        .rows
        .iter()
        .find(|row| row.role == QUEUE)
        .ok_or_else(|| invalid("The publication lost its active head."))?;
    let row = record
        .rows
        .iter()
        .find(|row| row.role == EXPANSION)
        .ok_or_else(|| invalid("The publication lost its exact expansion."))?;
    let header = header(head, row, operation, first)?;
    let stage = header
        .stage
        .as_ref()
        .ok_or_else(|| invalid("A Merge has no prepared publication."))?;
    let change = header
        .change
        .as_ref()
        .ok_or_else(|| invalid("The directory lost its intended creation."))?;
    match pending {
        Pending::Preparing { .. } => validate_prepared(head, row, operation, first, Some(pending))?,
        Pending::Forward {
            id,
            ordinal,
            change: actual,
            stage: actual_stage,
        } if *id == operation
            && *ordinal == first
            && encoded(actual)? == encoded(change)?
            && actual_stage.as_ref().map(encoded).transpose()? == Some(encoded(stage)?) => {}
        _ => {
            return Err(invalid(
                "The captured journal differs from its original folder expansion.",
            ));
        }
    }
    if !committed(head, row, operation, first, change)? {
        return Err(invalid("The folder's intended creation changed."));
    }
    let mut projected = record.clone();
    advance(&mut projected, operation, Some(change))?;
    super::recover(projected)?;
    header
        .item
        .finish
        .unwrap()
        .original
        .ok_or_else(|| invalid("The original source folder cannot be verified after restart."))?
        .require_source(&header.item.source)?;
    Ok(Pending::Forward {
        id: operation,
        ordinal: first,
        change: change.clone(),
        stage: Some(stage.clone()),
    })
}

pub(in crate::operations) fn present(record: &Recorded) -> io::Result<bool> {
    let mut progress: Progress = decoded(&record.progress)?;
    if (6..10).contains(&progress.counts.len()) {
        progress.counts.resize(10, 0);
    }
    if progress.counts.len() != 10 {
        return Err(invalid("Invalid active directory counters."));
    }
    Ok(progress.counts[EXPANSION as usize] != 0
        || progress.counts[CHILD as usize] != 0
        || record
            .rows
            .iter()
            .any(|row| matches!(row.role, EXPANSION | CHILD)))
}
