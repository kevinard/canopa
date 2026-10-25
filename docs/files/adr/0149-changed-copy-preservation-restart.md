# Changed-copy preservation binds its original Undo history

Status: schema57 source13 diagnostic checkpoint (899 captured inputs).
All40 backend targets ran:743 passed,2 failed,65 ignored. Default desktop-library
349 passed/32 ignored, default all-target Clippy with warnings denied, formatting,
fixture syntax/hash and both batch restart gates pass. The two additional-pass
failures are real public Undo Replace gaps, not waived acceptance. The full Files
goal and restart-recovery work remain active.

Two real worker deaths before displacement reproduced a replacement history file
reconciling the original changed-copy preparation and removing its empty work root.
Changed-copy preservation used a generic Undo intent without the independent
original-history binding already used for other displaced Undo work.

Extend that existing binding to Copy without a saved previous destination. The
reviewed selection retains its actual parent and entry descriptors across the
question. Its preparation stores the selected original generation and snapshot in
the actual created root. Registration atomically binds that preparation, unchanged
canonical history and exact pending body to the original history-file generation.
Failed admission cleans only the verified empty created root. Once registered,
even a refused live rename leaves its journal owning the preparation.

Restart finishes metadata when the preserved original entry can be proved. An
unused preparation uses the existing empty-copy compensation protocol, retaining
canonical Undo for a fresh decision. Its durable Skipped/Returned states grant
only original empty-root cleanup. It never falls back to the legacy rollback rename.
Replacement roots, entries, history files and missing bindings remain protected.
Schema57 prevents older executables from treating the expanded binding and
empty-copy compensation as ordinary generic work.

Recovery exposes the original preservation root. A fresh private Keep review binds
the exact journal, independent binding, directory-access state, root version and
current saved-entry snapshot. It can retain uncertain contents within that original
root without acquiring operation execution rights. Stale reviews refuse; refreshed
Keep archives the whole root atomically before retiring its journal and binding.
Original Undo remains available for a fresh decision. Unknown extra children stay
inside the retained whole root. A replacement root is never adopted.

A refused-write test then twice reproduced ignored DELETE changes committing a
preservation handoff while leaving its original Undo step active. The shared
reversal transaction now verifies the exact step is absent before clearing its
journal. Ignored or refused saves roll back the entire handoff.

The first broad matrix passed618 tests on source04, but a separate public-API
probe twice exposed a further ignored-insert gap when older Recovery archives
already existed. SQLite retained an older last_insert_rowid after ignoring the
new archive insertion. The selected edit's bytes survived in its work root while
both journal and Undo history retired and its Recovery row was absent. A permanent
regression reproduced this before the fix. Shared archive insertion now checks
that one row was inserted, reads back its exact body at the new id and verifies
both namespace and root ownership indexes before reversal can commit. The
regression proves retry after the refusal ends retains all four protected versions.

Expanded checks exposed two older payload-only Recovery expectations and a repeatable
500-item batch Undo timing failure: 22.64–23.55 seconds against its existing20-second
debug acceptance bound. Interrupted Copy Undo now exposes its whole original root,
including completed empty-only cleanup receipts with preserved contents. The updated
public tests copy that root, kill an actual recovery export, refuse unavailable
storage, Keep the original root explicitly and undo exports while preserving both
retained work and the newer visible occupant. They no longer request a consumed
payload name be returned automatically.

Scoped profiling found repeated serialization dominated the older Copy Undo.
Registration still proves the selected task against canonical history; subsequent
checks can verify exact stored canonical bytes against that existing digest. Legacy
JSON formatting uses the original normalized check. The independent pending digest
continues to bind the complete selected task. A process-death regression accepts
reformatted canonical JSON and refuses changed history.

Transitions now carry only a bound predecessor, phase and captured post-rename
snapshot. Their next task is derived from that predecessor, so another Change,
root or selection cannot be substituted. Physical checks still run before and
after phase storage. This removes repeated serialization of identical immutable
fields. Measured Undo phases fell from22.61 to18.48 seconds with the same63,000
identity captures; the unchanged20-second gate also passes without instrumentation.
Temporary profiling code and page traces are removed. These measurements cover
this debug batch acceptance, not optimized startup, idle resources or search.

Focused public acceptance covers two restarts before/after preservation, fresh
Undo after compensation, history replacement, parent/entry replacement during
review, original-root refusal and reappearance, uncertain-entry Keep, stale Keep,
initial refused/ignored registration and completion writes, and nonempty archive
handoffs. Seven retained failure fixtures (99 entries) have decoded and separately live-verified archives under
`.scratch/file-manager/changed-copy-*`.

The next broader batch reproduced an older Copy Undo refusing its saved version
following a directory Merge. The child return moved the original saved container
but its owners retained the removed source parent's generation. Merge preparation
now selects catalogued containers through their existing ownership and opened
parent/entry descriptors. Their return records preserve both parent generations,
the container and saved entry; only the direction of these recorded roles changes.
The existing return commit updates every indexed owner to the verified destination
parent. It does not recapture a replacement container or weaken identity checks.
The original public integration case and a separately retained public probe
reproduced the refusal before this fix. Its 15-entry fixture is archived and
independently verified without changing any source bytes or modes.

The source12 matrix passed619 tests and failed the occupied saved-container return
case, which a separately retained public probe repeated. A cross-filesystem Keep
both return already rebinds its owners to the newly created container; applying the
old root's direct-rename proof to that created target correctly refused. Source13
records direct parent movement only for same-filesystem children. Cross-filesystem
returns retain their existing created-copy migration and owner rebind. Four fixture
roots (42 entries) preserve both failures, decoded and independently live verified.
The same-filesystem Merge and full saved-container suites pass together:34 tests,
with one direct child fixture ignored. Source13 broader validation is complete and retains the two additional-pass failures described below.

Unbound legacy disposition, remaining generic Forward/Undo/cleanup, Trash/Restore,
broader Recovery/expiry/queue/report acceptance, backend navigation/search/preview
separation, product and physical checks, startup/focus/performance, full native and
workspace checks, Shell/Notify regressions, Niri distribution, release packaging
and GTK retirement remain open.

A separate next-work probe twice reproduced generic same-filesystem Move Undo
reclassifying an already returned saved destination as RetainedOnly after its old
previous name was recreated through an existing hardlink. Later edits survive.
The control with those same edits but no recreated name retires the completed
step. This is unfinished generic Move return work; no repeated native rename or
deletion was observed. Sources, retained fixtures and the control are recorded in
`.scratch/file-manager/changed-copy-rename-next-*` and CHANGED-COPY-NEXT.md.

The final expanded run reproduces two public additional-pass failures:
`pending_additions_and_recovery_follow_the_actual_merged_destination_folder` and
`additional_passes_keep_previous_and_new_recovery_namespaces_separate`.
Their live Undo Replace still creates a legacy Stage without original root/entry
proof. A later Merge correctly refuses to turn that catalog path into a current
execution owner. Two retained public probes repeat both failures, with32 entries
archived and independently verified. These cases require a creator-bound generic
Undo displacement upgrade; do not weaken saved-container admission to pass them.

Independent work in a private source copy made the next Move regression
deterministic: a one-item worker dies after its actual saved return; a no-recreated-
name control passes, while an old hardlink recreating previous leaves a redundant
RetainedOnly row. A schema58 candidate binds the selected saved return, consumes its
source name before one live rename, recognizes completion by original generation
and retains uncertain original roots through fresh Keep. Six private crash parents
pass, including original-root/history replacement and recorded-source identity.
One new parent first reproduced older Copy Undo deleting a replacement because
completion reread an already returned source path. The candidate now carries the
recorded returned identity through that metadata handoff. This candidate remains
outside source13; its passing private tests are not production acceptance. Generic
Undo Replace root creation and other unfinished goal areas remain open.
