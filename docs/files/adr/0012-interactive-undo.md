# Interactive Undo and restoring under a new name

Status: accepted under delegated implementation authority, 2026-10-16;
implementation in progress.

Undo still targets the latest concrete history entry. The worker can now ask the
native adapter to resolve an occupied restoration path. Keep both restores the
incoming item under a new name using the transfer engine's naming policy, such
as `report (2).txt`. The existing newer item keeps its name and contents. Skip
leaves that change available to Undo and proceeds through other independent
changes in the same operation. Cancel stops remaining work while retaining
completed reversals and unresolved history. Operations distinguishes completed,
partly undone and cancelled work. Stale notification actions still cannot undo a
newer operation.

The noninteractive core APIs keep their previous refusal behavior. The native
adapter uses the interactive API, with metadata for the incoming and existing
items, explicit conflict choices and Keep both as the safe default.
Decide later leaves the worker waiting under its application hold. The job
captures its invoking native window weakly, including Quick Look and Trash, so
foreground questions appear for those sources too. Background questions still
notify and can be reviewed in Operations. Trash sources use the original name
in the dialog rather than the opaque storage basename.

Restoring an item under a new name must not strand older history. The Undo intent
records a path relocation before the filesystem rename. Committing the reversal
rewrites affected older and remaining history paths, retained-stage locations,
identity bindings and protected cleanup records in the same SQLite transaction
that removes the reversed change and clears its intent. Startup reconciliation
performs the same transaction when filesystem identity proves the rename
completed. If a replacement reversal already returned its source but still has
an older destination to restore, conversion to the retained-only remainder also
commits the relocation atomically.

Directory relocations include descendants. Symbolic links also carry a namespace:
a directory link may be broken at its current location but become traversable
again at the original parent. History through that link follows its restored
name. Paths retain their Unix bytes. Relocation never moves or relabels the newer
object occupying the original path.

The current interactive implementation covers occupied paths when reversing a
same-filesystem rename/move or Trash. Public filesystem tests cover raw paths,
older history after restart, directory symlinks, retained replacement stages,
partial batch Skip/retry, cross-filesystem identity bindings and a process killed
after its actual Keep both rename and before the API returned. Recovery then
successfully undoes the older 4,000-file copy while preserving newer work.
Native acceptance exercises actual dialogs from Quick Look and Trash, metadata,
Skip, Escape and the safe Return default in dark and light/fractional appearance
(`native-undo-06` and `native-undo-07`). All 64 core tests and the existing full
native regression (`native-acceptance-19`) pass. The metadata remains selectable
without starting selected; default focus is applied after activation.

Replace now preserves file/symlink occupants in protected Recovery when reversing
same-filesystem rename/move or Trash; [ADR 0014](0014-undo-replace.md) records its
transaction and verification. The complete Undo requirement remains open.
Changed copied replacements and retained-only conflict choices are now covered by
[ADR 0015](0015-undo-previous-destinations.md). Changed copies without a previous
destination are covered by [ADR 0018](0018-undo-changed-copies.md). Next work
on available changed saved versions is recorded in
[ADR 0026](0026-undo-changed-saved-versions.md). Remaining work includes
unavailable retained versions and folder
Merge, cross-filesystem restoration conflicts, dependent-step Skip, remaining
batch choices, and accurate presentation of partial progress. Replacement choices
must retain displaced newer work for explicit recovery; they must not silently
release it as ordinary history expires. A Recovery copy should use a user-chosen
name/location without consuming data still required by an Undo entry. Pending
history that needs manual recovery must remain inspectable rather than making
the entire operation engine inaccessible. These requirements also need restart
and native acceptance before completion.
Uninspectable same-filesystem moves now use
[ADR 0029](0029-undo-unavailable-moved-items.md).

[ADR 0028](0028-undo-replaced-moved-items.md) adds explicit Return/Keep/Skip
choices when a different item occupies the moved or renamed item's path.
