//! Batched new-file Copy (ADR0180).
//!
//! Consecutive plain new files of one source folder going to one destination
//! folder share one private stage and one journal intent. The protocol keeps
//! the single-item order, per batch instead of per file:
//!
//! 1. create the stage, then register the batch (`Preparing`);
//! 2. create every payload, then register their identities;
//! 3. copy and fingerprint each payload (syncs deferred to the ADR0165 barrier);
//! 4. save `Forward` with one `Change::Copied` per copied file and the ordinals
//!    `first..first+N-1`;
//! 5. publish each payload with a no-replace rename, in order, stopping at the
//!    first failure;
//! 6. append the published prefix and keep `Cleanup{published}` in one
//!    transaction;
//! 7. remove the unpublished payloads and the stage, then clear.
//!
//! Recovery finishes any phase: `Preparing` removes registered payloads,
//! `Forward` appends the published prefix it can prove, `Cleanup` removes what
//! is left. Unknown entries keep the stage for Recovery review.
use super::*;
use original::Original;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, openat};
use std::ffi::OsString;

/// At most this many files, and this many content bytes, share one batch.
pub(super) const MAX_ITEMS: usize = 64;
pub(super) const MAX_BYTES: u64 = 4 * 1024 * 1024;
/// Fewer consecutive files keep single-item steps: batching only pays for
/// many small files, and a few files keep their exact per-file visibility.
pub(super) const MIN_ITEMS: usize = 8;
/// The encoded intent must stay inside the journal's 128 KiB step budget.
/// Gathering estimates each file's share; Forward then shrinks to fit exactly.
const JOURNAL_BUDGET: usize = 96 * 1024;

#[cfg(test)]
thread_local! {
    /// Files published through batches on this thread, for tests.
    pub(super) static PUBLISHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A conservative estimate of one file's encoded share of a Forward intent:
/// its proofs and fingerprint, plus each path four times.
fn estimate(source: &Path, destination: &Path) -> usize {
    2048 + 4 * (source.as_os_str().len() + destination.as_os_str().len())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::operations) enum Phase {
    Preparing,
    Forward { first: u64 },
    Cleanup { first: u64, published: usize },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct Payload {
    identity: Identity,
    original: Original,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct Item {
    #[serde(with = "crate::path_serde")]
    pub source: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub destination: PathBuf,
    pub payload: Option<Payload>,
    /// Set in the Forward phase for each file whose copy completed.
    pub change: Option<Change>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct CopyBatch {
    pub id: u64,
    pub phase: Phase,
    pub stage: Stage,
    pub items: Vec<Item>,
}

fn payload_name(index: usize) -> String {
    format!("{index}")
}

fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The batched copies or their destination folder changed; existing contents are preserved.",
    )
}

fn attention() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Interrupted copies are preserved. Keep their work folder in Recovery.",
    )
}

impl CopyBatch {
    pub(in crate::operations) fn validate(&self) -> io::Result<()> {
        if self.items.is_empty() || self.items.len() > MAX_ITEMS {
            return Err(invalid("A copy batch holds 1 to 64 files."));
        }
        if self.stage.ownership.is_none()
            || self.stage.payload_identity.is_some()
            || self.stage.replacement.is_some()
            || self.stage.entry_move.is_some()
            || self.stage.source_claim.is_some()
            || self.stage.directory_claim.is_some()
        {
            return Err(invalid("A copy batch needs its own plain stage."));
        }
        let folder = self.stage.directory.parent();
        for item in &self.items {
            validate_target(&item.source)?;
            validate_target(&item.destination)?;
            validate_stage(Some(&self.stage), &item.destination)?;
            if item.destination.parent() != folder
                || item.source.parent() != self.items[0].source.parent()
            {
                return Err(invalid(
                    "A copy batch stays in one source and one destination folder.",
                ));
            }
            if let Some(change) = &item.change {
                let Change::Copied {
                    path,
                    retained: None,
                    publication: Some(_),
                    ..
                } = change
                else {
                    return Err(invalid("A batched copy records a plain new copy."));
                };
                if path != &item.destination || item.payload.is_none() {
                    return Err(invalid("A batched copy differs from its file."));
                }
            }
        }
        let mut names: Vec<_> = self.items.iter().map(|item| &item.destination).collect();
        names.sort();
        names.dedup();
        if names.len() != self.items.len() {
            return Err(invalid("A copy batch names each destination once."));
        }
        match self.phase {
            Phase::Preparing => {
                if self.items.iter().any(|item| item.change.is_some()) {
                    return Err(invalid("A preparing copy batch has no copies yet."));
                }
            }
            Phase::Forward { .. } | Phase::Cleanup { .. } => {
                // Copied files form a prefix: a failed file ends the batch.
                let copied = self
                    .items
                    .iter()
                    .take_while(|item| item.change.is_some())
                    .count();
                if copied == 0
                    || self.items[copied..]
                        .iter()
                        .any(|item| item.change.is_some())
                {
                    return Err(invalid("A copy batch publishes a prefix of its files."));
                }
                if let Phase::Cleanup { published, .. } = self.phase
                    && published > copied
                {
                    return Err(invalid("A copy batch cannot publish more than it copied."));
                }
            }
        }
        Ok(())
    }

    /// The only saved transitions: payloads registered while preparing, and
    /// preparing to Forward with the same files and payloads.
    pub(in crate::operations) fn transition(&self, after: &Self) -> io::Result<()> {
        let refused = || invalid("The copy batch changed outside its own phases.");
        if self.id != after.id || self.stage != after.stage || self.items.len() != after.items.len()
        {
            return Err(refused());
        }
        for (before, next) in self.items.iter().zip(&after.items) {
            if before.source != next.source || before.destination != next.destination {
                return Err(refused());
            }
        }
        match (&self.phase, &after.phase) {
            (Phase::Preparing, Phase::Preparing) => {
                for (before, next) in self.items.iter().zip(&after.items) {
                    if before.payload.is_some() && before.payload != next.payload {
                        return Err(refused());
                    }
                }
                Ok(())
            }
            (Phase::Preparing, Phase::Forward { .. }) => {
                for (before, next) in self.items.iter().zip(&after.items) {
                    if before.payload != next.payload {
                        return Err(refused());
                    }
                }
                Ok(())
            }
            _ => Err(refused()),
        }
    }

    fn changes(&self) -> impl Iterator<Item = (usize, &Item, &Change)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| item.change.as_ref().map(|change| (index, item, change)))
    }
}

/// The batch stage, opened through its recorded parent and ownership proof.
struct Root {
    parent: fs::File,
    directory: Option<fs::File>,
}

impl Root {
    fn open(stage: &Stage) -> io::Result<Self> {
        let ownership = stage.ownership.as_ref().ok_or_else(attention)?;
        let parent = fs::File::from(rustix::fs::open(
            stage.directory.parent().ok_or_else(attention)?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        ownership.require_parent(&parent)?;
        let name = stage.directory.file_name().ok_or_else(attention)?;
        let directory = match openat(
            &parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => Some(fs::File::from(fd)),
            Err(rustix::io::Errno::NOENT) => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(directory) = &directory {
            ownership.require_directory(directory)?;
            if Identity::from_metadata(&directory.metadata()?) != stage.identity {
                return Err(changed());
            }
        }
        Ok(Self { parent, directory })
    }

    /// The payload at `index` if it is still the recorded object.
    fn payload(&self, index: usize, payload: &Payload) -> io::Result<Option<OsString>> {
        let Some(directory) = &self.directory else {
            return Ok(None);
        };
        let name = payload_name(index);
        let file = match openat(
            directory,
            name.as_str(),
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fs::File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if Identity::from_metadata(&file.metadata()?) != payload.identity {
            return Err(changed());
        }
        payload.original.require(&file)?;
        Ok(Some(OsString::from(name)))
    }

    /// Remove the recorded payloads still in the stage, then the stage when
    /// nothing else is inside.
    fn clean(
        &self,
        stage: &Stage,
        payloads: impl Iterator<Item = (usize, Payload)>,
    ) -> io::Result<()> {
        let Some(directory) = &self.directory else {
            return Ok(());
        };
        for (index, payload) in payloads {
            if let Some(name) = self.payload(index, &payload)? {
                rustix::fs::unlinkat(directory, name.as_os_str(), AtFlags::empty())?;
            }
        }
        let name = stage.directory.file_name().ok_or_else(attention)?;
        match rustix::fs::unlinkat(&self.parent, name, AtFlags::REMOVEDIR) {
            Ok(()) => {}
            Err(rustix::io::Errno::NOTEMPTY | rustix::io::Errno::EXIST) => return Err(attention()),
            Err(error) => return Err(error.into()),
        }
        durability::sync(&fs::File::from(openat(
            &self.parent,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?))
    }
}

/// Finish the journal's copy batch from whatever phase it reached.
pub(super) fn recover(state: &mut Store, batch: CopyBatch) -> io::Result<()> {
    batch.validate()?;
    state.require_copy_batch(&batch)?;
    let root = Root::open(&batch.stage)?;
    let batch = match batch.phase {
        Phase::Preparing => {
            let payloads: Vec<_> = batch
                .items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| item.payload.clone().map(|payload| (index, payload)))
                .collect();
            root.clean(&batch.stage, payloads.into_iter())?;
            return state.clear_intent();
        }
        Phase::Forward { .. } => {
            // Publication ran in order and stopped at the first failure, so
            // the published files are a prefix of the copied ones.
            let mut published = 0;
            let mut gap = false;
            for (index, item, change) in batch.changes() {
                let payload = item.payload.as_ref().ok_or_else(attention)?;
                let at_stage = root.payload(index, payload)?.is_some();
                let at_destination = match change {
                    Change::Copied {
                        publication: Some(publication),
                        identity,
                        ..
                    } => match publication.require(&item.destination) {
                        Ok(file) => Identity::from_metadata(&file.metadata()?) == *identity,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                        Err(error) => return Err(error),
                    },
                    _ => return Err(attention()),
                };
                match (at_stage, at_destination) {
                    (false, true) if !gap => published += 1,
                    (true, false) => gap = true,
                    _ => return Err(attention()),
                }
            }
            state.complete_copy_batch(&batch, published)?
        }
        Phase::Cleanup { .. } => batch,
    };
    finish(state, &batch, &root)
}

/// Remove what the Cleanup phase left in the stage, then clear the intent.
fn finish(state: &mut Store, batch: &CopyBatch, root: &Root) -> io::Result<()> {
    let Phase::Cleanup { published, .. } = batch.phase else {
        return Err(attention());
    };
    let copied = batch
        .items
        .iter()
        .take_while(|item| item.change.is_some())
        .count();
    let published_end = published.min(copied);
    let leftovers: Vec<_> = batch
        .items
        .iter()
        .enumerate()
        .skip(published_end)
        .filter_map(|(index, item)| item.payload.clone().map(|payload| (index, payload)))
        .collect();
    root.clean(&batch.stage, leftovers.into_iter())?;
    state.clear_intent()
}

/// One file of a batch, as the transfer gathered it.
pub(super) struct Candidate {
    pub source: PathBuf,
    pub destination: PathBuf,
}

/// A published file, for the transfer's report and accounting.
pub(super) struct Copied {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub bytes: u64,
    pub attributes: attributes::Kept,
}

pub(super) struct Outcome {
    pub copied: Vec<Copied>,
    /// The file whose copy failed, which ended the batch, and why.
    pub failed: Option<(Candidate, io::Error)>,
    /// Files the batch did not publish, in order, for the single-item path.
    pub returned: Vec<Candidate>,
    /// Content bytes reported for files that were not published.
    pub unpublished_bytes: u64,
}

struct Prepared {
    source: fingerprint::source::Opened,
    output: Option<fs::File>,
}

/// Copy `candidates` as one batch. Every candidate shares one source folder
/// and one destination folder, and had no name at its destination when it was
/// gathered. Files the batch did not publish come back in `returned`.
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    state: &mut Store,
    id: u64,
    candidates: Vec<Candidate>,
    access_parent: Option<Identity>,
    cancellation: &Cancellation,
    progress: &mut impl FnMut(&Path, u64),
    checkpoint: &mut impl FnMut(&Path, &Path) -> io::Result<()>,
) -> io::Result<Outcome> {
    let folder = candidates[0]
        .destination
        .parent()
        .ok_or_else(|| invalid("A copied item needs its parent."))?
        .to_owned();
    // 1. The stage, then its registration.
    let (opened, mut stage, mut temporary) = staging::Opened::prepare(&folder)?;
    stage.record_parent_access(access_parent);
    let mut batch = CopyBatch {
        id,
        phase: Phase::Preparing,
        stage: stage.clone(),
        items: candidates
            .iter()
            .map(|candidate| Item {
                source: candidate.source.clone(),
                destination: candidate.destination.clone(),
                payload: None,
                change: None,
            })
            .collect(),
    };
    if let Err(error) = state.begin_copy_batch(&batch) {
        let _ = opened.clean_empty();
        return Err(error);
    }
    // The intent owns the stage from registration onward.
    temporary.disable_cleanup(true);
    #[cfg(test)]
    super::transfer::copy_batch_boundary::point("registered", &batch);
    // 2. Every payload, then their registration.
    let mut prepared = Vec::with_capacity(candidates.len());
    for (index, candidate) in candidates.iter().enumerate() {
        let made = (|| -> io::Result<(Prepared, Payload)> {
            check_cancel(cancellation)?;
            let source = fingerprint::source::Opened::open(&candidate.source)?;
            source.require_current()?;
            opened.require_current()?;
            let name = payload_name(index);
            let (file, output) = if source.metadata().is_symlink() {
                rustix::fs::symlinkat(source.link()?, opened.directory(), name.as_str())?;
                let file = fs::File::from(openat(
                    opened.directory(),
                    name.as_str(),
                    OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )?);
                (file, None)
            } else if source.metadata().is_file() {
                let file = fs::File::from(openat(
                    opened.directory(),
                    name.as_str(),
                    OFlags::RDWR
                        | OFlags::CREATE
                        | OFlags::EXCL
                        | OFlags::NOFOLLOW
                        | OFlags::NONBLOCK
                        | OFlags::CLOEXEC,
                    Mode::RUSR | Mode::WUSR,
                )?);
                let output = file.try_clone()?;
                (file, Some(output))
            } else {
                return Err(invalid(
                    "only regular files and symbolic links can be copied",
                ));
            };
            let payload = Payload {
                identity: Identity::from_metadata(&file.metadata()?),
                original: Original::capture(&file)?,
            };
            Ok((Prepared { source, output }, payload))
        })();
        match made {
            Ok((ready, payload)) => {
                batch.items[index].payload = Some(payload);
                prepared.push(ready);
            }
            // A file that cannot start ends the batch; it and the rest return
            // to the single-item path, which reports or retries them.
            Err(_) => break,
        }
    }
    let registered = batch.clone();
    let before = CopyBatch {
        items: batch
            .items
            .iter()
            .map(|item| Item {
                payload: None,
                ..item.clone()
            })
            .collect(),
        ..batch.clone()
    };
    state.advance_copy_batch(&before, &registered)?;
    #[cfg(test)]
    super::transfer::copy_batch_boundary::point("payloads", &batch);
    // 3. Content, metadata and fingerprints.
    let mut sizes = Vec::with_capacity(prepared.len());
    let mut kept = Vec::with_capacity(prepared.len());
    let mut changes = Vec::with_capacity(prepared.len());
    let mut failure = None;
    for (index, ready) in prepared.iter_mut().enumerate() {
        let item = &batch.items[index];
        let payload = item.payload.as_ref().unwrap();
        let mut written = 0;
        let copied = (|| -> io::Result<(Change, attributes::Kept)> {
            check_cancel(cancellation)?;
            checkpoint(&item.source, &item.destination)?;
            let before = ready.source.metadata().clone();
            let name = payload_name(index);
            let require = || -> io::Result<()> {
                opened.require_current()?;
                if staging::named_identity(opened.directory(), name.as_str())? != payload.identity {
                    return Err(changed());
                }
                Ok(())
            };
            let (digest, attributes, entry) = match ready.output.take() {
                Some(output) => {
                    let input = ready.source.regular()?;
                    let entry = output.try_clone()?;
                    let (digest, attributes) = prepared_copy::fill_regular(
                        &ready.source,
                        input,
                        output,
                        &before,
                        prepared_copy::Times::Source,
                        cancellation,
                        &mut |count| {
                            written += count;
                            progress(&item.source, count);
                        },
                        &mut || checkpoint(&item.source, &item.destination),
                        require,
                        |_| {},
                    )?;
                    (digest, attributes, entry)
                }
                None => {
                    let target = ready.source.link()?;
                    require()?;
                    let entry = fs::File::from(openat(
                        opened.directory(),
                        name.as_str(),
                        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )?);
                    (
                        fingerprint::content_digest(target.as_os_str().as_encoded_bytes()),
                        attributes::Kept::All,
                        entry,
                    )
                }
            };
            ready.source.require_current()?;
            let metadata = entry.metadata()?;
            let identity = Identity::from_metadata(&metadata);
            if identity != payload.identity {
                return Err(changed());
            }
            let fingerprint = Fingerprint::from_metadata(&metadata, digest);
            let source_fingerprint = Fingerprint::from_metadata(&before, digest);
            let publication = Original::capture(opened.parent())
                .ok()
                .map(|parent| Box::new(staging::Published::new(parent, payload.original)));
            Ok((
                Change::Copied {
                    source: copied_leaf::Source::capture(&ready.source, &source_fingerprint),
                    publication,
                    access_parent,
                    path: item.destination.clone(),
                    identity,
                    fingerprint,
                    retained: None,
                },
                attributes,
            ))
        })();
        sizes.push(written);
        match copied {
            Ok((change, attributes)) => {
                changes.push(change);
                kept.push(attributes);
            }
            Err(error) => {
                failure = Some((index, error));
                break;
            }
        }
    }
    let unpublished_bytes = |published: usize| sizes.iter().skip(published).sum::<u64>();
    // The failed file is reported as the single-item step would; it and the
    // files after it are not published.
    let mut candidates = candidates;
    let failed = failure.map(|(index, error)| (candidates.remove(index), error));
    if changes.is_empty() {
        // Nothing to publish: retire the batch like recovery of `Preparing`.
        let saved = registered;
        recover(state, saved)?;
        return Ok(Outcome {
            copied: Vec::new(),
            failed,
            returned: candidates,
            unpublished_bytes: unpublished_bytes(0),
        });
    }
    // 4. One Forward intent for every copied file.
    let mut forward = registered.clone();
    forward.phase = Phase::Forward {
        first: state.next_ordinal(id)?,
    };
    for (item, change) in forward.items.iter_mut().zip(changes) {
        item.change = Some(change);
    }
    // Keep the intent inside the journal's step budget: copies at the end that
    // do not fit are not published by this batch.
    while store::encode(&Pending::CopyBatch {
        batch: Box::new(forward.clone()),
    })
    .is_err()
    {
        let Some(last) = forward
            .items
            .iter_mut()
            .rev()
            .find(|item| item.change.is_some())
        else {
            break;
        };
        last.change = None;
    }
    if forward.items.iter().all(|item| item.change.is_none()) {
        recover(state, registered)?;
        return Ok(Outcome {
            copied: Vec::new(),
            failed,
            returned: candidates,
            unpublished_bytes: unpublished_bytes(0),
        });
    }
    state.advance_copy_batch(&registered, &forward)?;
    #[cfg(test)]
    super::transfer::copy_batch_boundary::point("forward", &forward);
    // 5. Publish in order; the first failure ends publication.
    let mut published = 0;
    for (index, item, _) in forward.changes() {
        let done = (|| -> io::Result<()> {
            checkpoint(&item.source, &item.destination)?;
            opened.require_current()?;
            let payload = item.payload.as_ref().ok_or_else(attention)?;
            let name = payload_name(index);
            if staging::named_identity(opened.directory(), name.as_str())? != payload.identity {
                return Err(changed());
            }
            let target = item
                .destination
                .file_name()
                .ok_or_else(|| invalid("A copied item needs its name."))?;
            rustix::fs::renameat_with(
                opened.directory(),
                name.as_str(),
                opened.parent(),
                target,
                RenameFlags::NOREPLACE,
            )?;
            if staging::named_identity(opened.parent(), target)? != payload.identity {
                return Err(changed());
            }
            Ok(())
        })();
        if done.is_err() {
            break;
        }
        published += 1;
        #[cfg(test)]
        super::transfer::copy_batch_boundary::published(&forward, published);
    }
    if published > 0 {
        durability::sync(&fs::File::from(openat(
            opened.parent(),
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?))?;
        durability::sync(opened.directory())?;
    }
    #[cfg(test)]
    PUBLISHED.set(PUBLISHED.get() + published);
    // 6. The published prefix and the cleanup, together.
    let cleanup = state.complete_copy_batch(&forward, published)?;
    #[cfg(test)]
    super::transfer::copy_batch_boundary::point("appended", &cleanup);
    // 7. The rest of the stage, then the intent.
    let root = Root::open(&cleanup.stage)?;
    finish(state, &cleanup, &root)?;
    let mut copied = Vec::with_capacity(published);
    let mut returned = Vec::new();
    for (index, candidate) in candidates.into_iter().enumerate() {
        if index < published {
            copied.push(Copied {
                source: candidate.source,
                destination: candidate.destination,
                bytes: sizes[index],
                attributes: kept[index],
            });
        } else {
            returned.push(candidate);
        }
    }
    Ok(Outcome {
        copied,
        failed,
        returned,
        unpublished_bytes: unpublished_bytes(published),
    })
}

/// The batch that starts with `first`: it and the consecutive queued files of
/// the same source and destination folders that are plain new files, up to
/// `MAX_ITEMS` files and `MAX_BYTES` of content. `None` below `MIN_ITEMS`.
pub(super) fn gather<'a>(
    first: (&Path, &Path),
    queued: impl Iterator<Item = (&'a Path, &'a Path, bool)>,
    owned: impl Fn(&Path) -> bool,
) -> Option<Vec<Candidate>> {
    let eligible = |source: &Path, destination: &Path| -> Option<u64> {
        if source == destination
            || owned(source)
            || validate_target(source).is_err()
            || validate_target(destination).is_err()
        {
            return None;
        }
        let metadata = fs::symlink_metadata(source).ok()?;
        let size = if metadata.is_file() {
            metadata.len()
        } else if metadata.is_symlink() {
            0
        } else {
            return None;
        };
        match fs::symlink_metadata(destination) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Some(size),
            _ => None,
        }
    };
    let (source, destination) = first;
    let mut total = eligible(source, destination)?;
    if total > MAX_BYTES {
        return None;
    }
    let mut batch = vec![Candidate {
        source: source.to_owned(),
        destination: destination.to_owned(),
    }];
    let mut journal = estimate(source, destination);
    for (next_source, next_destination, finishing) in queued {
        if batch.len() == MAX_ITEMS
            || finishing
            || next_source.parent() != source.parent()
            || next_destination.parent() != destination.parent()
        {
            break;
        }
        let Some(size) = eligible(next_source, next_destination) else {
            break;
        };
        let share = estimate(next_source, next_destination);
        if total + size > MAX_BYTES || journal + share > JOURNAL_BUDGET {
            break;
        }
        total += size;
        journal += share;
        batch.push(Candidate {
            source: next_source.to_owned(),
            destination: next_destination.to_owned(),
        });
    }
    (batch.len() >= MIN_ITEMS).then_some(batch)
}
