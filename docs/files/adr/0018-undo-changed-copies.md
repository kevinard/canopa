# Finish Undo of changed copies while preserving newer work

Status: accepted under delegated implementation authority, 2026-10-16.

A copied file may be edited, replaced by another file or link, or replaced by a
folder. If the copy did not replace an older destination, there is no incoming
previous version for the ordinary Undo restoration dialog. Interactive Undo now
shows the current item's name, type, size and modification time and explains
these choices:

- Keep item leaves it in place and finishes this part of Undo. It is Return's
  default. The finished history step no longer asks about the kept item.
- Move to Recovery removes a regular file or link from this folder while
  preserving its selected version in protected Recovery. Links remain links;
  their targets are untouched. Folders and special objects cannot use this
  command and may be kept or skipped instead.
- Skip leaves the change retryable and processes other items. Cancel preserves
  completed decisions and leaves remaining work available to Undo.

Noninteractive Undo still refuses changed copies. Copies with a previous
destination retain ADR 0015's restoration choices. Keep item and Move to Recovery
are distinct core choices, rather than overloading Keep both or Replace. The
native adapter uses the appropriate labels and reply mapping for each conflict.
Outcomes report kept paths separately from reversed and skipped changes, and
Operations states how many items were kept.

Move to Recovery snapshots the observed version before the question and uses
the protected displacement transaction from ADR 0014. It refuses a changed
observation before/after extraction. A durable Undo intent owns the protected
stage before moving the file; archiving that version, deleting its copy history
step and clearing the intent commit together. Startup recognizes extraction by
the expected identity in the saved location, even if another newer file has
since arrived at the original path. It keeps that arrival and finishes the
protected archive. An uncompleted extraction instead rolls back without
overwriting another occupant. Writes through an existing open file handle stay
protected, including after ordinary history expiry and restart.

Keeping an edited child also keeps copy-created ancestor folders from the same
operation. Their history is consumed atomically with the child's keep decision,
so cancellation or termination cannot leave a later Undo trying to remove its
container. Ancestors created by a separate earlier operation remain in that
operation's history and refuse removal while they contain the kept work.
Skipped children leave copy-created ancestors retryable rather than failing
with a nonempty-folder error. Broader dependency handling for moves and folder
restoration remains separate work.

Schema version 4 adds a derived exact-path index of created-folder changes. The
keep transaction queries component-aware ancestor keys in bounded pages instead
of decoding an entire batch for every kept file. Creation, replacement,
relocation, deletion and migration maintain the index with the change rows.
Upgrading older stores populates the derived indexes and advances the version
atomically. Older binaries refuse the new schema.

Public regressions cover Skip/Cancel/restart, keeping one edited item in a
partly reversed batch, older parent-folder history, modified data through an
open handle and expiry, changes during the question, a folder at the copy path,
raw-name links and target preservation, containing-folder Skip/Keep, and a keep
followed by cancellation/restart. The container case first failed with a
nonempty-directory error; its cancellation case then exposed the need for an
atomic container decision. A child process is killed after an actual removal
within a 500-file batch and before the API returns. Reopening and finishing Undo
keeps all 500 newer versions in paginated Recovery and all original sources.

All 92 core tests pass (`core-92.log`); all eight ignored crash helpers are
executed/killed by their parent tests. Desktop build, all-target Clippy,
formatting, Python compilation and whitespace checks pass.
`native-undo-16/summary.json` (dark) and `native-undo-17/summary.json` (light,
1.25×) pass all 22 checks, including current-only metadata, Skip, Escape, safe
Keep item, separate kept-item reporting, Move to Recovery and preview/copy-out
of the removed edits. Rendered inspection confirms the labels, metadata and
newer contents. The full existing desktop regression passes in
`native-acceptance-25/summary.json`.

The dark run logged an intermittent burst of GTK `gtk_tree_store_remove`
assertions around a native save chooser during the earlier replacement-recovery
steps, before the new changed-copy prompt. Functional checks completed, but this
is not a clean-log result. The light run and full regression have no GTK
warnings. Replaying all 22 Undo checks serially under GDB with fatal criticals
(`native-undo-chooser-debug-01`) did not reproduce the assertion. The bounded
`native-chooser-stress-debug-02` run then repeated all 22 checks plus 30 serial
native recovery saves under GDB/fatal criticals, preserving every file's exact
contents with no GTK assertion. Debug artifacts are clearly named. This has not
established the intermittent assertion's cause. Two concurrent private-desktop
runs (`native-chooser-parallel-debug-01`, dark, and
`native-chooser-parallel-debug-02`, light at 1.25×) subsequently each passed the
22 Undo checks plus 30 repeated native recovery saves under GDB/fatal criticals.
Every saved file matched its expected contents, with no GTK assertion or trapped
stack. The initial burst remains unexplained; no application workaround is claimed.

Changed available previous destinations now use the explicit review in
[ADR 0026](0026-undo-changed-saved-versions.md). Available replacements of moved
items use [ADR 0028](0028-undo-replaced-moved-items.md). Remaining Undo work
includes unavailable saved versions and cross-filesystem move destinations;
uninspectable same-filesystem moves use
[ADR 0029](0029-undo-unavailable-moved-items.md). Further work includes
folder Merge, cross-filesystem reversal conflicts, broader
dependent-step choices and batch decisions. Interrupted-phase choices,
directory recovery, explicit protected-data disposal and accurate overall
progress/persisted continuations remain in the implementation ledger.
