//! One preparation lifecycle for Copy, Move, Recovery and copied Undo returns.
//! Content/metadata transfer has no publication or history authority.
use super::fingerprint::{Fingerprint, changed, same_snapshot, source};
use super::*;

pub(super) struct PreparedCopy {
    pub source: source::Opened,
    pub opened: staging::Prepared,
    pub stage: Stage,
    pub temporary: tempfile::TempDir,
    pub identity: Identity,
    pub fingerprint: Fingerprint,
    pub source_identity: Identity,
    /// The output digest with the selected source metadata. This is the content
    /// the source must still match before removal; Copy does not reread it.
    pub source_fingerprint: Fingerprint,
}

pub(super) fn prepare_copy(
    source: &Path,
    parent: &Path,
    cancellation: &Cancellation,
    register: impl FnMut(&staging::Registration<'_>) -> io::Result<()>,
    progress: impl FnMut(u64),
    checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<PreparedCopy> {
    prepare_using(
        || source::Opened::open(source),
        Output {
            parent,
            pinned: None,
            times: Times::Source,
        },
        cancellation,
        register,
        progress,
        checkpoint,
    )
}

/// A new document from a template: the template's contents and permissions,
/// dated now, since it is a new file rather than a copy of an old one.
pub(super) fn prepare_new(
    source: &Path,
    parent: &Path,
    cancellation: &Cancellation,
    register: impl FnMut(&staging::Registration<'_>) -> io::Result<()>,
) -> io::Result<PreparedCopy> {
    prepare_using(
        || source::Opened::open(source),
        Output {
            parent,
            pinned: None,
            times: Times::Now,
        },
        cancellation,
        register,
        |_| {},
        || Ok(()),
    )
}

/// The times the prepared output keeps.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Times {
    Source,
    Now,
}

/// Where the output is prepared, and what it keeps of its source.
struct Output<'a> {
    parent: &'a Path,
    /// The destination folder already retained by the transfer selection.
    pinned: Option<&'a fs::File>,
    times: Times,
}

/// Prepare through the source folder and destination parent already retained
/// by the actual transfer selection.
pub(super) fn prepare_pinned(
    source: source::Opened,
    parent: &Path,
    pinned: &fs::File,
    cancellation: &Cancellation,
    register: impl FnMut(&staging::Registration<'_>) -> io::Result<()>,
    progress: impl FnMut(u64),
    checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<PreparedCopy> {
    prepare_using(
        || Ok(source),
        Output {
            parent,
            pinned: Some(pinned),
            times: Times::Source,
        },
        cancellation,
        register,
        progress,
        checkpoint,
    )
}

fn prepare_using(
    source: impl FnOnce() -> io::Result<source::Opened>,
    Output {
        parent,
        pinned,
        times,
    }: Output<'_>,
    cancellation: &Cancellation,
    mut register: impl FnMut(&staging::Registration<'_>) -> io::Result<()>,
    mut progress: impl FnMut(u64),
    mut checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<PreparedCopy> {
    check_cancel(cancellation)?;
    checkpoint()?;
    let source = source()?;
    source.require_current()?;
    let before = source.metadata();
    let (opened, mut stage, mut temporary) = match pinned {
        Some(pinned) => staging::Opened::prepare_in(parent, pinned)?,
        None => staging::Opened::prepare(parent)?,
    };
    register(&opened.registration(&source, &stage)?)?;
    #[cfg(test)]
    super::transfer::stage_boundary::preparing("empty", source.path(), &stage);
    // The durable intent owns this stage from registration onward. Cleanup must
    // validate its placement/identity instead of running through TempDir::drop.
    temporary.disable_cleanup(true);
    checkpoint()?;
    source.require_current()?;
    let (payload, digest) = if before.is_symlink() {
        let target = source.link()?;
        let file = opened.link(&target)?;
        let payload = opened.payload(file, &mut stage)?;
        register(&payload.registration(&source, &stage)?)?;
        #[cfg(test)]
        super::transfer::stage_boundary::preparing("payload", source.path(), &stage);
        payload.require_current()?;
        (
            payload,
            fingerprint::content_digest(target.as_os_str().as_encoded_bytes()),
        )
    } else {
        let mut input = source.regular()?;
        let file = opened.regular()?;
        let mut payload = opened.payload(file, &mut stage)?;
        // Record the actual new leaf before the first data write. A crash
        // between creation and this save leaves an unknown leaf for review.
        register(&payload.registration(&source, &stage)?)?;
        #[cfg(test)]
        super::transfer::stage_boundary::preparing("payload", source.path(), &stage);
        payload.require_current()?;
        // A duplicate of the owned output descriptor lets the callback validate
        // its namespace without borrowing the writer. No user path is reopened.
        let mut output = payload.file().try_clone()?;
        let mut behind = copy_backend::WriteBehind::new(&output)?;
        copy_backend::copy_regular(&mut input, &mut output, |count| {
            check_cancel(cancellation)?;
            checkpoint()?;
            source.require_current()?;
            payload.require_current()?;
            if count > 0 {
                #[cfg(test)]
                super::transfer::stage_boundary::preparing("written", source.path(), &stage);
                #[cfg(test)]
                super::undo::abort_boundary::preparing(source.path(), &stage);
                #[cfg(test)]
                super::transfer::stage_boundary::restricted_preparing(
                    source.path(),
                    &stage,
                    cancellation,
                );
                progress(count);
                behind.written(count)?;
                check_cancel(cancellation)?;
                source.require_current()?;
                payload.require_current()?;
            }
            Ok(())
        })?;
        #[cfg(test)]
        super::transfer::stage_boundary::preparing("transferred", source.path(), &stage);
        if !same_snapshot(before, &input.metadata()?) {
            return Err(changed());
        }
        checkpoint()?;
        payload.require_current()?;
        // Trust upstream copying. Hash only the actual output for Undo and later
        // destructive checks; source removal separately verifies both contents.
        let digest = fingerprint::copied_output_digest(&output, cancellation)?;
        if !same_snapshot(before, &input.metadata()?) {
            return Err(changed());
        }
        // The source is unchanged, so the destination must hold all of it. On
        // a network share or phone this is the end-to-end check that the
        // server accepted the whole file (ADR0168).
        let kept = output.metadata()?.len();
        if kept != before.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "The destination kept {kept} of {} bytes. The copy was not completed.",
                    before.len()
                ),
            ));
        }
        source.require_current()?;
        payload.require_current()?;
        copy_backend::restore_metadata(&output, before)?;
        if times == Times::Now {
            let now = std::time::SystemTime::now();
            output.set_times(fs::FileTimes::new().set_modified(now).set_accessed(now))?;
        }
        // Sync is a host durability boundary, after backend content/metadata.
        // A batched Copy span defers it to the batch barrier (ADR0165).
        durability::sync(&output)?;
        #[cfg(test)]
        super::transfer::stage_boundary::preparing("completed", source.path(), &stage);
        (payload, digest)
    };
    source.require_current()?;
    check_cancel(cancellation)?;
    checkpoint()?;
    source.require_current()?;
    payload.require_current()?;
    let metadata = payload.metadata()?;
    let identity = Identity::from_metadata(&metadata);
    stage.payload_identity = Some(identity);
    Ok(PreparedCopy {
        opened: payload,
        stage,
        temporary,
        identity,
        fingerprint: Fingerprint::from_metadata(&metadata, digest),
        source_identity: Identity::from_metadata(before),
        source_fingerprint: Fingerprint::from_metadata(before, digest),
        source,
    })
}
