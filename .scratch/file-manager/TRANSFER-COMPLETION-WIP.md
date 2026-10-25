# Copy/Move completion after process death

Started 2026-10-21. Goal priority remains complete restart recovery, followed by
backend separation, product acceptance, reliability/performance and release.
This is a new scoped step after schema26 ordinary mutation completion.

The new public killed-process regression reproduced a completed Copy restoring
as Interrupted, with its confirmed Undo still present. The real child commits
work, SQL refuses its result checkpoint, and the child kills itself with SIGKILL.
The parent edits the completed target and creates a new original occupant before
reopening. No filesystem replay can safely repair this missing historical result.

Implemented schema27 typed transfer start/end evidence, exact revision/body and
result validation, backend startup restoration, metadata-only live/end/result
Retry save, and request-budget accounting. ADR0111 records the scope and the
variable-size result budget limitation. Ordinary Rename/Create Folder evidence
and saved reports remain separate from executable continuation capabilities.

Focused verification so far: request_execution passes14 parents,1 private helper
ignored in the parent inventory but explicitly executed by crash parents. Cases
include Copy/Move of files/nested folders, two restarts preserving newer bytes and
inodes, end-write refusal, startup result-write refusal, activity/removal/shutdown
and dispatch holds, corrupt/unended/stale/legacy refusal, normal Undo expiry, and
existing ordinary mutation/partial-transfer regressions. The initial public
regression failed as expected before implementation. Initial compilation also
caught missing Job fields and the generic begin_request helper; these were fixed.

A broader local-runtime library/queue/transfer/source-retention run is underway.
No native, complete workspace or release validation is claimed. Unfinished
transfers remain review-only after restart; durable original/media/traversal,
remaining-step and interrupted-cleanup authority are the next implementation work.
No other source/proof archives or running desktop processes were changed.


## Verified checkpoint

Final selected backend run:162 parents across8 targets pass,6 ignored entries in
the parent inventory with private helpers invoked by their parents. The default
library passes107/2 ignored in the existing private HOME/XDG/bus/no-display/no-
system-bus runner (receipt continuation-core-1792628031). Default all-target
Clippy with warnings denied, formatting and default build pass. No830 recorded
Cargo/crate/vendor input changed through these checks and native verification.

The final backend cases include actual Copy/Move death for files, nested folders,
batches and raw Unix names; real cross-filesystem directory Move with original
source and destination-root replacement; failed end/result saves and holds;
legacy/stale/corrupt/unended refusal; Undo expiry; and atomic retirement of the
duplicate receipt while preserving the existing finished-result budget. Primary
review found and fixed that potential budget regression before final validation.
No independent review or complete backend/workspace run is claimed.

Native02 passes the existing continuation and dismissal cases in both themes,
and the new transfer case in dark. Its light transfer run passes Copy result
restoration assertions then times out in grim screenshot capture at the existing
5-second deadline. Native03 passes both new transfer themes without source,
fixture or timeout changes after concurrent compilation ended. Six distinct cases
therefore pass16 checks across02/03 on ELF b379372fb3f1e15448c490e0630ff15b7f70a57bf873a1a6f28661d9f94cc5c5,
with pinned Niri63e39bae3473. Four final Copy/Move captures inspected; no clipping
of their result controls/status. The screenshot failure remains unresolved.

Native01 fails before app launch because the sandbox forbids private D-Bus socket
binding;02/03 use the authorized isolated harness with socket access. The first
build fails with ENOSPC. Nine exact regenerable test ELFs (312230184B) and23
unused vendor incremental-cache directories (466985761 logical bytes) were
reclaimed under Cargo locks, with audits; source, application/Niri binaries and
prior proof archives remain intact. The first cache-audit write itself hit ENOSPC
before deletion, leaving its incomplete audit; the second audit records actual
reclamation. All failures and later successful logs remain separate.

## Next required authority

Completion restoration is not executable continuation. Public paused/interrupted
Copy/Move must next persist original selected-entry, parent, destination, traversal,
stage and cleanup proof before accepting/releasing their last live ownership.
Exact request/journal/step checkpoints and descriptor-relative mutation must
prevent a replacement entry or device from becoming execution authority. Preserve
independent completed-prefix Undo, source retention, opposite Retry/Continue
orders and new-process clipboard intent. Then finish the other operation families
and the original remaining phases; the full goal remains active.

Private probes (transfer-durable-handle-probe.log and
transfer-durable-handle-return-probe.json) establish that this host's temporary
and workspace filesystems provide kernel file handles plus unique mount IDs.
Eight separate-process replacement/original-return cases pass for files,
directories, dangling links and FIFOs. This is investigation evidence only;
these tokens are not yet persisted or used to authorize any saved plan. Missing
or unsupported proof must remain a truthful refusal rather than adopting a fresh
pathname, inode number or device observation.
