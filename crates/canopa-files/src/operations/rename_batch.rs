//! Bulk Rename is one operation whose steps are ordinary journalled Renames
//! inside one folder. Each step claims and publishes its entry exactly as a
//! single Rename does, so Undo reverses the whole batch, newest step first.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// One requested name change. Both paths share their folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRename {
    #[serde(with = "crate::path_serde")]
    pub source: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub destination: PathBuf,
}

/// What a batch actually changed. Steps after a cancellation or an
/// interruption stay in `remaining`, untouched.
#[derive(Clone, Debug, Default)]
pub struct BatchRenameOutcome {
    /// The history entry holding every completed step, when any completed.
    pub id: Option<u64>,
    /// New paths of the renamed items, in the order they were renamed.
    pub renamed: Vec<PathBuf>,
    /// Items left with their name, with the reason.
    pub failures: Vec<(PathBuf, String)>,
    pub remaining: Vec<PathBuf>,
    pub cancelled: bool,
    /// A step stopped where Recovery must review it; nothing else ran after.
    pub interrupted: Option<String>,
}

/// Order the steps so each name is vacated before another step takes it:
/// renaming `1`→`2` and `2`→`3` renames `2` first. Refuses duplicates, steps
/// that leave their folder or keep their name, and names that swap in a cycle,
/// which would need a temporary name the person never chose.
pub fn order(renames: &[BatchRename]) -> io::Result<Vec<BatchRename>> {
    if renames.is_empty() {
        return Err(invalid("A batch rename needs at least one item."));
    }
    let folder = renames[0].source.parent();
    let mut sources = BTreeMap::new();
    let mut destinations = BTreeSet::new();
    for (index, step) in renames.iter().enumerate() {
        validate_target(&step.source)?;
        validate_target(&step.destination)?;
        if step.source.parent() != folder || step.destination.parent() != folder {
            return Err(invalid("Every renamed item must stay in its folder."));
        }
        if step.source == step.destination {
            return Err(invalid("A renamed item must change its name."));
        }
        if sources.insert(step.source.clone(), index).is_some() {
            return Err(invalid("An item can only be renamed once in a batch."));
        }
        if !destinations.insert(step.destination.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Two items would be named “{}”.",
                    display_name(&step.destination)
                ),
            ));
        }
    }
    // Each destination names at most one other step's source, so the
    // dependencies form chains, or cycles when names swap.
    let blocker = |step: &BatchRename| sources.get(&step.destination).copied();
    let mut done = vec![false; renames.len()];
    let mut ordered = Vec::with_capacity(renames.len());
    while ordered.len() < renames.len() {
        let before = ordered.len();
        for (index, step) in renames.iter().enumerate() {
            if !done[index] && blocker(step).is_none_or(|other| done[other]) {
                done[index] = true;
                ordered.push(step.clone());
            }
        }
        if ordered.len() == before {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Some items would swap names. Rename them in two steps.",
            ));
        }
    }
    Ok(ordered)
}

impl OperationEngine {
    /// Rename items of one folder as one undoable operation. Prefer the queued
    /// request; this public seam serves headless tests.
    pub fn rename_batch(
        &self,
        renames: &[BatchRename],
        cancellation: &Cancellation,
    ) -> io::Result<BatchRenameOutcome> {
        self.rename_batch_observed(renames, cancellation, None)
    }

    /// Like Trash, the history entry is not bound to the request: request
    /// bindings describe Transfer cursors and single-step receipts. An
    /// interrupted step is still reconciled from its journal intent.
    pub(super) fn rename_batch_observed(
        &self,
        renames: &[BatchRename],
        cancellation: &Cancellation,
        observed: Option<&transfer::ObservedSources>,
    ) -> io::Result<BatchRenameOutcome> {
        let steps = order(renames)?;
        check_cancel(cancellation)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        check_cancel(cancellation)?;
        recover(&mut state, cancellation)?;
        let label = format!(
            "rename {} {}",
            steps.len(),
            if steps.len() == 1 { "item" } else { "items" }
        );
        let id = state.begin(&label)?;
        let mut outcome = BatchRenameOutcome::default();
        let mut pending: std::collections::VecDeque<_> = steps.into();
        while let Some(step) = pending.front().cloned() {
            if cancellation.is_cancelled() {
                outcome.cancelled = true;
                break;
            }
            match rename_step(&mut state, id, &step, observed) {
                Ok(()) => outcome.renamed.push(step.destination.clone()),
                Err(error) if state.pending()?.is_some() => {
                    // The claimed entry is protected by the journal. Stop here
                    // so Recovery reviews it before any other step runs.
                    outcome.interrupted = Some(error.to_string());
                    break;
                }
                Err(error) => outcome
                    .failures
                    .push((step.source.clone(), error.to_string())),
            }
            pending.pop_front();
        }
        outcome.remaining = pending.into_iter().map(|step| step.source).collect();
        if outcome.interrupted.is_none() {
            state.discard_empty(id)?;
            state.trim()?;
        }
        if !outcome.renamed.is_empty() {
            outcome.id = Some(id);
        }
        Ok(outcome)
    }
}

/// One step, through the same claim and publication as a single Rename.
fn rename_step(
    state: &mut Store,
    id: u64,
    step: &BatchRename,
    observed: Option<&transfer::ObservedSources>,
) -> io::Result<()> {
    let BatchRename {
        source,
        destination,
    } = step;
    let bindings = observed
        .map(|observed| {
            observed.require(source)?;
            observed.rename_bindings(source, destination)
        })
        .transpose()?;
    let identity = bindings
        .as_ref()
        .map_or_else(|| Identity::read(source), |bindings| Ok(bindings.source))?;
    let saved = state
        .owned_saved_stage(source, identity)?
        .map(|record| {
            saved_stage::Movement::capture(
                source,
                destination,
                record.retained.stage.parent_access,
                StageParentAccess::Existing,
                None,
            )
        })
        .transpose()?;
    let change = Change::Rename {
        move_facts: None,
        saved,
        identity,
        original: source.clone(),
        current: destination.clone(),
        retained: None,
        reviewed: None,
    };
    validate_change(&change)?;
    rename::Prepared::open(source, destination, identity, bindings.as_ref())?
        .commit(state, id, &change)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(source: &str, destination: &str) -> BatchRename {
        BatchRename {
            source: PathBuf::from("/folder").join(source),
            destination: PathBuf::from("/folder").join(destination),
        }
    }

    #[test]
    fn chains_vacate_each_name_first() {
        let ordered = order(&[step("1", "2"), step("2", "3"), step("3", "4")]).unwrap();
        let names: Vec<_> = ordered
            .iter()
            .map(|step| step.source.file_name().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["3", "2", "1"]);
    }

    #[test]
    fn swaps_duplicates_and_other_folders_are_refused() {
        assert!(order(&[step("a", "b"), step("b", "a")]).is_err());
        assert!(order(&[step("a", "c"), step("b", "c")]).is_err());
        assert!(order(&[step("a", "a")]).is_err());
        assert!(
            order(&[BatchRename {
                source: "/folder/a".into(),
                destination: "/other/a".into(),
            }])
            .is_err()
        );
        assert!(order(&[]).is_err());
    }
}
