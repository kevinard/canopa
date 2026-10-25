# Restricted created-parent Recovery access

Schema49 source is restricted-access-inputs-09.json, with 881 build inputs.
The complete five-area goal remains active. Do not mark it complete for this slice.

New production module operations/store/directory_reads.rs records independent
permission context plus durable temporary owner read/search grants. Access has
optional descriptor-derived directory/parent Original proofs captured during the
live owned-folder borrow. Catalog items hold nonserialized leases and the history
lock; final release only sends to a dedicated worker. Permission mutations run off
the renderer. Shared reader/writer state preserves original modes. Keep now restores
its lane's write access after clearing the journal, allowing the original accepted
cursor to regain Continue. Indexed ancestor lookups avoid scanning all contexts per
catalog row. External readable modes are not claimed as grants.

New acceptance module transfer/stage_boundary/access_checks.rs controls an actual
queued Copy: first stop midway through b-current, change the already-created target
folder to 0400, Continue, and SIGKILL during a new 64KiB partial. The parent edits that
partial, then uses public Reconcile and Recovery. Input02 red02/red03 fail an early
test assumption (startup intentionally defers permission compensation until review).
Input03 probe01/red04 reproduce the actual catalog failure after Reconcile: mode 0400
is restored but all four rows have unavailable entries. Input04 green01 passes
Copy/Keep but fails immediate Continue; input05 probe02 identifies the leftover
directory_access row blocking cursor restoration. Input06 green02 passes the whole
basic sequence after Keep restores that row. Input07 green03 passes 3 parents but
the replacement fixture's cross-parent rename fails at its own permissions setup.
Input08 green04 passes all 4 parents and 2 ignored subprocess helpers. Input09 adds
indexed ancestor lookup and cancellation-before-inspection assertions. Final scoped
checks pass: 511 backend tests across 20 targets, 44 ignored helpers; 277 isolated
normal default-feature library tests, 24 ignored. Zero failures/filters. Both required
batch gates, default all-target Clippy01 with warnings denied, formatting01 and the
schema49 fixture syntax check pass. The default receipt is
continuation-core-1792805716-receipt.json. All tool handles are terminal.

restricted-access-verification-01.json records exact results/source relations.
The failed fixture archive SHA is
f38d11748b159de4cf7ed00e736a0282695d2e69f0bcf3882f42af72269207c1.
Seven roots retain 134 decoded/live-verified entries. Archiving/verification borrows
search permission in private failed folders and restores their exact modes; the
resulting directory ctime changes are explicitly recorded. Bytes, modes, mtime,
device and inode match. Source evidence is archived separately with final source
and preserved attempt manifests/logs. No main application or Niri binary has been
rebuilt by this slice. Do not delete the required operation_transfer_resume test
ELF or other protected binaries. Never rebuild a current_exe test ELF while one of
its children is still alive. Keep build-input hashes frozen during validation.

Next acceptance must cover nested restricted ancestors, including a process death
between their asynchronous permission releases. Current startup restores grants in
descending path depth but does not borrow a previously restored nonsearchable
ancestor just to reach an abandoned descendant grant. Build a public fixture before
changing this. Also check nested Continue parent-access traversal. Do not claim those
cases are already handled. Then complete remaining generic Forward/Undo abort
cleanup, broader Undo/expiry/queued lineage/large reports and Trash/Restore.

All backend separation, product/media/drag/device/accessibility/physical acceptance,
startup/focus/optimized measurements, native/workspace/Shell/Notify/Niri distribution,
release package/docs and GTK reference retirement remain open.
