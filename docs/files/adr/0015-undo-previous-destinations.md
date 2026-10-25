# Undo restores previous destinations without discarding newer files

Status: accepted under delegated implementation authority, 2026-10-16;
implementation in progress.

After a copy replaces a destination, that copied file may be edited or replaced
by another file. Undo now treats its previous destination as the incoming
version and asks how to handle the newer occupant. The same choices apply when
a retained-only remainder needs to restore its previous destination to an
occupied path. The worker reports the source as Current, Trash or Previous
Destination; the native dialog shows the original path with a previous-
destination label instead of an opaque staging name, alongside both versions'
sizes and modification times.

Keep both restores the previous destination with a new name and preserves the
newer occupant at its existing path. Completing the reversal records the durable
name relocation in older/remaining history and identity bindings, so an earlier
operation can still undo that restored item after restart. Replace preserves the
newer occupant in protected Recovery and restores the previous destination at
the original path. Skip keeps the unresolved change retryable while processing
other changes; Cancel retains completed reversals and unresolved history. Keep
both remains Return's default. File/folder collisions keep the existing folder
policy; Replace applies to files and links.

Name relocation covers the historical namespace even when the immediate saved
version is a regular file. An older version may be a directory link with Undo
entries beneath it. Those entries must follow the restored name so they become
usable again when that older link is restored. Startup reconciliation applies
this rule to older exact-path relocation intents as well. Native conflict
metadata identifies links as links, and replacing a directory link affects the
link object while preserving its target directory.

An unchanged copied replacement still uses ordinary Undo without a question.
Interactive conflict resolution checks its identity and fingerprint before
classifying it as newer work. Unreadable contents are also treated as work to
preserve rather than permission to delete. The retained incoming version must
pass the existing stage, identity and version checks. An available changed
retained version now has a separate explicit review under
[ADR 0026](0026-undo-changed-saved-versions.md); unavailable data still needs
recovery. These destination choices do not bypass that check. Plain copied
files with no previous destination use the separate
changed-copy interaction recorded in [ADR 0018](0018-undo-changed-copies.md).

When a changed copy has a previous destination, its restoration runs through a
retained-only Undo intent while the recorded copy remains unchanged until the
reversal commits. Skip and Cancel therefore keep the original change available
for retry. Replace uses ADR 0014's durable displacement and protected Recovery
transaction. Completion also requires the retained file to have left its owned
stage, so a matching inode at the destination alone does not establish a
completed restoration.

Startup reconciliation now cleans the consumed retained stage as well as any
explicit Undo work stage. A killed-process regression first exposed an abandoned
empty folder in this path. Completed-stage cleanup removes only an empty owned
directory; it preserves unexpected contents instead of unlinking them. Cleanup
of an unfinished preparation remains a separate operation.

Five public regressions cover Keep both with older history after restart,
Replace after another file takes the copy's path, partial Skip followed by
cancellation/restart/retry, and a process killed after an actual previous-
destination restoration, plus a directory-link namespace behind intermediate
regular-file versions. The crash fixture uses two 500-file copy operations.
After recovery and reversal, all 500 newer files remain and no staging folders
or retained versions are abandoned. The five ignored crash helpers are executed
by their parent tests.

All 81 core tests and focused Clippy pass. Native Undo runs
`native-undo-12/summary.json` (dark) and `native-undo-14/summary.json` (light,
1.25×) pass all 16 checks, including previous-destination metadata, Keep both,
Replace, Recovery preview/copy-out and replacing/restoring a directory link.
Rendered inspection confirms the preserved newest edit in Recovery and link
metadata with Replace available. Both application logs have no GTK warnings.
The full existing native regression passes in `native-acceptance-22/summary.json`.
The first light attempt exposed a fixture focus race when reopening Recovery;
the fixture now waits for actual compositor focus before sending keys.

Available changed/replaced moved items now use the explicit review in
[ADR 0028](0028-undo-replaced-moved-items.md), including partial-return and
batch-interruption coverage. The full Undo requirement remains open. Remaining
cases include unavailable retained versions, folder Merge, cross-filesystem
reversal conflicts, dependency-aware
Skip and remaining batch choices. A replacement move interrupted between returning
its source and restoring its old destination needs further fault coverage. Pending
recovery must remain inspectable. The initial 500-file crash/reversal case took
about 50 seconds because each relocation scanned history;
[ADR 0016](0016-undo-namespace-index.md) records its subsequent indexed-path fix.
The remaining filesystem/cleanup work stays in the implementation ledger.
Uninspectable same-filesystem moved items now use
[ADR 0029](0029-undo-unavailable-moved-items.md).
