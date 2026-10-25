# Keep interrupted operations inspectable and recovery copies journaled

Status: accepted under delegated implementation authority, 2026-10-16.

An unresolved filesystem journal must not hide the history needed to recover its
files. Previously, a newer occupant at a copied file's path could prevent startup
reconciliation of its Undo quarantine. Opening the whole engine failed, leaving
the Recovery window unavailable. A regression kills a real process after the
file reaches quarantine and reproduces this failure through the public engine.

Opening the engine now retains an unresolved pending journal and exposes its
operation label, recovery error and paths. Ordinary mutations still reconcile
pending work first and refuse to proceed while that fails. Browsing remains
available. Startup presents Recovery when an interrupted operation needs
attention. Recovery shows the reason independently of selection and offers an
explicit Retry interrupted operation button. Retry rechecks actual files; it
does not authorize overwriting an occupied path or deleting unexpected data.

The catalog includes owned work payloads, previous destinations, displaced newer
files and source quarantines referenced by pending work. These entries keep the
original filename for Quick Look and save-chooser suggestions. Availability is
checked against the owned stage and observed file version. Pending records are
held for recovery rather than eligible for ordinary history cleanup. Invalid
stage descriptions do not become preview/copy sources. Unsupported directory or
special-file payloads retain the existing inspection-only policy.

Users can recover a non-consuming copy while the original operation remains
unresolved. Schema version 3 adds a separate recovery-copy intent slot. All
intent reads and completion/cleanup transactions are scoped to the active slot;
finishing a rescue copy cannot erase the original intent. Startup reconciles the
recovery-copy slot before the original slot. Only preparation or a new copied
file is valid in that slot. A failed reconciliation of this slot pauses further
copies as well as ordinary mutations, while catalog inspection remains available.
The normal operation lock serializes all filesystem work across both slots.

Recovery copies become ordinary Undo entries. Undo waits until pending recovery
is reconciled, then can reverse the copies independently. History expiry and
garbage collection wait while either slot is pending, even if this temporarily
retains more than the usual 100 entries. The original pending record and its
sources remain available after copy-out and another process restart.

A rescue destination must be fresh and outside all paths owned or referenced by
pending work, as well as its source's retained-data folder. Its parent is resolved
before copying; directory checks use resolved parent locations too, so a symlink
alias cannot put new files inside a journal-owned folder. The final filename
retains its Unix bytes and is not followed as a symlink. Full pinned-directory
race auditing remains separate work.

The public regression also kills a second process after actual rescue-copy bytes
are written. Reopening cleans the partial rescue copy and preserves the original
pending Undo. It exercises failed mutation/retry, alias rejection, successful
copy-out, another restart, manual clearing of the occupied path, explicit retry
and both independent Undo entries. Its two ignored helpers are executed and
killed by the parent test.

All 83 core tests, desktop build/typechecking and all-target Clippy pass. All
seven ignored crash helpers are executed and killed by their parent tests.
`native-interrupted-08/summary.json` (dark) and
`native-interrupted-09/summary.json` (light, 1.25×) pass eight checks through real
native copy/Undo, actual process termination, startup Recovery, rendered saved
contents, save-chooser copy-out, a second restart, 360-pixel visible controls,
explicit retry and both Undo entries. Rendered inspection confirms the original
filename/content and readable narrow layout. `native-recovery-09/summary.json`
passes the existing ordinary retained-version Recovery workflow. Application
logs have no GTK warnings. The full existing desktop regression passes in light
appearance at 1.25× in `native-acceptance-24/summary.json`, covering browsing,
previews, clipboard, background jobs, notifications and Trash.

The native fixture monitors its expected process exits independently of the
desktop harness's normal unexpected-exit guard and restores live processes to
cleanup if an earlier step fails. Its first attempts correctly killed Files but
tripped that guard. Concurrent CPU-heavy checks also caused first-window fixture
timeouts; isolated runs then reached the full workflow. Preview/toolbar checks
now inspect foreground pixels rather than inferring completed rendering from a
window title or width. An initial visual reading wrongly classified the dark
toolbar as blank; exact pixel comparison corrected that record. Temporary
geometry diagnostics are removed, with no production redraw workaround added.

This provides inspection and non-consuming salvage. Rich choices for every
interrupted phase, directory recovery, explicit protected-data disposal and
persisted unfinished batch continuations remain in the implementation ledger.
Cleanup failures after an intent has already committed/cleared need their own
orphan-data audit. Readers still share the engine lock with active work; making
catalog inspection responsive while a live conflict waits is separate work.

[ADR 0050](0050-directory-recovery-copies.md) extends the separate Recovery lane
to recursive directory copies on schema 17, including created-folder intents,
partial completed work and typed in-memory continuations. It keeps the original
unresolved journal intact and does not add durable unfinished-job resume.
