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
    /// Whether the source's extended attributes and ACLs all reached the copy.
    pub attributes: attributes::Kept,
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
pub(super) enum Times {
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
    let (payload, digest, kept) = if before.is_symlink() {
        let target = source.link()?;
        let file = opened.link(&target)?;
        let payload = opened.payload(file, &mut stage)?;
        register(&payload.registration(&source, &stage)?)?;
        #[cfg(test)]
        super::transfer::stage_boundary::preparing("payload", source.path(), &stage);
        payload.require_current()?;
        // Linux keeps no user attributes or ACLs on symbolic links.
        (
            payload,
            fingerprint::content_digest(target.as_os_str().as_encoded_bytes()),
            attributes::Kept::All,
        )
    } else {
        let input = source.regular()?;
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
        let output = payload.file().try_clone()?;
        let (digest, kept) = fill_regular(
            &source,
            input,
            output,
            before,
            times,
            cancellation,
            &mut progress,
            &mut checkpoint,
            || payload.require_current(),
            |_point| {
                #[cfg(test)]
                match _point {
                    "written" => {
                        super::transfer::stage_boundary::preparing(
                            "written",
                            source.path(),
                            &stage,
                        );
                        super::undo::abort_boundary::preparing(source.path(), &stage);
                        super::transfer::stage_boundary::restricted_preparing(
                            source.path(),
                            &stage,
                            cancellation,
                        );
                    }
                    point => {
                        super::transfer::stage_boundary::preparing(point, source.path(), &stage)
                    }
                }
            },
        )?;
        (payload, digest, kept)
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
        attributes: kept,
    })
}

/// Copy a regular source's content and metadata into a new owned `output`
/// and fingerprint the result. `require` proves the output's namespace is
/// still the owned one; `boundary` names the test cut points. Shared by single
/// copies and batched copies (ADR0180).
#[allow(clippy::too_many_arguments)]
pub(super) fn fill_regular(
    source: &source::Opened,
    mut input: fs::File,
    mut output: fs::File,
    before: &fs::Metadata,
    times: Times,
    cancellation: &Cancellation,
    progress: &mut impl FnMut(u64),
    checkpoint: &mut impl FnMut() -> io::Result<()>,
    require: impl Fn() -> io::Result<()>,
    mut boundary: impl FnMut(&'static str),
) -> io::Result<([u8; 16], attributes::Kept)> {
    let mut behind = copy_backend::WriteBehind::new(&output)?;
    let mut holes = copy_backend::Holes::find(&input, &output)?;
    copy_backend::copy_regular(&mut input, &mut output, |count| {
        check_cancel(cancellation)?;
        checkpoint()?;
        source.require_current()?;
        require()?;
        if count > 0 {
            boundary("written");
            progress(count);
            // Punch passed holes before the window sync writes them.
            holes.written(count)?;
            behind.written(count)?;
            check_cancel(cancellation)?;
            source.require_current()?;
            require()?;
        }
        Ok(())
    })?;
    boundary("transferred");
    if !same_snapshot(before, &input.metadata()?) {
        return Err(changed());
    }
    checkpoint()?;
    require()?;
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
    require()?;
    // ACLs precede the permission restore, which then keeps their mask.
    let kept = attributes::Attributes::of_file(&input).apply(&output);
    copy_backend::restore_metadata(&output, before)?;
    if times == Times::Now {
        let now = std::time::SystemTime::now();
        output.set_times(fs::FileTimes::new().set_modified(now).set_accessed(now))?;
    }
    // Sync is a host durability boundary, after backend content/metadata.
    // A batched Copy span defers it to the batch barrier (ADR0165).
    durability::sync(&output)?;
    boundary("completed");
    Ok((digest, kept))
}
