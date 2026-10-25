# Recovery access beneath restricted created folders

Status: scoped schema49 verification.
Continues ADR0138 and ADR0139. The complete five-area Files goal remains active.

A stopped folder copy can continue after the user removes search access from its
created destination folder. Continue temporarily borrows owner access and saves the
original mode. If that worker dies while writing a partial, explicit Reconcile
preserves edited partial bytes and restores the destination's original mode. A real
queued-worker fixture reproduced the resulting Recovery failure twice: all four
rows were unavailable beneath the now non-searchable folder. The journal retained
the unique bytes, but the public catalog could not inspect or copy them.

Record original directory and parent generations when borrowing an owned created
folder. Save a separate permission context with that same original mode. Recovery
looks up only recorded ancestors of each retained work root. It may borrow owner
read/search access through those original objects, after saving a durable grant.
It adds no write permission. Missing original proofs, replacement folders/devices,
unrecorded changed modes, and failed or ignored metadata saves grant no access.
An externally searchable mode can be inspected without claiming its permissions.

A live catalog item holds its permission leases through inspection, previews,
Recovery Copy and Keep. Clones share the same leases. Serialization drops them;
a saved description is not permission or execution authority. The last live
selection enqueues restoration on a dedicated worker. It uses the opened original
folder, preserving replacement paths and differing external mode edits. The same
history lock stays alive through asynchronous restoration. Temporary write access
and read selections share a registry so writer completion cannot remove access
still needed by a live reader, or capture the reader's borrowed mode as original.
Startup restores abandoned read grants through recorded generations. Failed
restoration retains its durable record for a later attempt.

Keep also reconciles the original write-access records after atomically archiving
the disposal roots and retiring the journal. Leaving those records behind prevented
the accepted transfer cursor from regaining Continue. Archived retained files keep
their independent permission context and remain readable through a fresh catalog
after a second restart. Retention data still supplies no transfer execution owner.

Schema49 prevents older executables from interpreting these permission contexts
and grants using their earlier cleanup policy. Legacy access rows without original
proofs are not upgraded from current paths to obtain read permission authority.

Four focused public acceptance parents pass: edited partial Copy/Keep/Continue and
a second restart; actual process death while borrowing read/search access;
replacement-folder and external-mode preservation; and rejected/ignored grant
saves plus cancellation before inspection. On 881 unchanged build inputs, final
checks pass 511 backend tests across 20 targets (44 ignored helpers) and 277 normal
isolated desktop-feature library tests (24 ignored), with zero failures or filters.
Both required active/stopped batch restart gates, all-target default Clippy with
warnings denied, formatting and fixture schema/syntax checks pass. Exact attempts,
source relations and protected binaries are in
`.scratch/file-manager/restricted-access-verification-01.json`. Seven private failed
fixtures retain 134 decoded and live-verified entries. Their bytes, modes, mtime and
objects match; directory ctime changes from controlled archive search borrowing are
explicitly recorded. The application/Niri binaries and required restart gate remain
preserved. No main application binary was rebuilt for this slice.

These tests cover one restricted created parent. Nested restricted ancestors and
interruption between ancestor/descendant permission restoration still need direct
acceptance. Complete native/workspace,
physical-device and release acceptance remains open.
