# Iced name and operation interface checkpoint

The full file-manager goal remains active. This is a verified migration slice;
no PRD completion box is accepted. The supported GUI uses Iced and `canopa-ui`.
GTK source remains historical reference while the complete brief is ported.

## Current candidate and processes

Normal executable: `target/debug/canopa-files`, SHA
`4f62a086329a7aa34f4c45a1a7505841e04dd18fa58f8acb9bb72693dafa5f93`,
113470152 bytes. `iced-operations-ui-source-03.json` binds 496 unchanged inputs;
`iced-operations-ui-binary-03.json` and `iced-operations-ui-verification.json`
record the candidate. No compiler or native workflow is live. Final test session
34621, build 94319 and all native handles explicitly terminated successfully.
Preserve this executable before a future scoped Cargo clean.

## Implemented scope

Iced modules `file_operations`, `file_operation_rows`, `name_dialog` and
`undo_dialog` connect the existing core worker to actual windows. Production
startup observes local volumes before starting the service; unit controller boot
never starts external platform services. Admission keeps name drafts until a
durable reply. Original raw paths and resolved Browser bindings remain captured;
unchanged lossy Rename is a byte-preserving no-op. Shared global Undo captures an
exact history head. Questions retain job IDs/generations and capability-dependent
choices, defaulting to Cancel. Source closure hides presentation without
answering the independent operation; Operations Review reopens it.

Completion refreshes affected active views and marks inactive tabs dirty.
Resolved bindings handle aliases. Pending navigation stays intact and its
eventual destination is refreshed after loading. StartSaveFailed distinguishes a
failed start checkpoint from an unsaved stopped result, preserving source
tracking through Retry. Each notification retains its own pending count. Both
save failures hold the process and keep the last Operations window reachable.

`local_volumes.rs` hosts GIO on the actual main thread/default context of a
private helper. It pins `/proc/self/exe`, uses bounded versioned plain-data frames
with raw paths and mount incarnations/revisions, and exits on parent stdin EOF.
The reader owns reaping; UI drop never joins. Initial observations prepare the
shared Storage before admission. Observer loss invalidates every pending check,
including unknown/local roots; Undo checks placement again after a question.
GIO/GLib are platform libraries. No GTK widget dependency is linked.

## Verification

Final `iced-operations-ui-final-tests-01.log`: 50 controller/library plus 12 public
worker tests pass. One child helper is ignored at the top level and invoked by a
passing crash/reopen parent. This includes raw names, aliases, queued cancellation,
real killed workers, observed-volume replacement, observer loss, post-question
placement and an actual database failure at the Running checkpoint.

Five sequential native workflows pass 52 declared checks on this exact binary:
- `iced-operations-expanded-dark-03`: 11, dark100.
- `iced-operations-expanded-light-01`: 11, light125.
- `iced-operations-actions-regression-01`: 15, dark100.
- `iced-operations-browsing-regression-01`: 13, light125.
- `iced-operations-preferences-regression-01`: two focused toolbar-fast checks.

The operation workflows verify real names/Undo across windows, raw-name no-op,
field correction, collision preservation, private GIO volume observation and
unplug, waiting Undo/queued work after the last Browser closes, Review and loss
of only the exact pidfd-pinned helper. All filesystem mutations are private;
no actual mount or user folder is changed. Both operation logs are empty. Three
regression logs contain only four unavailable GVfs services on their private bus.
Ten selected captures were inspected; paths are in the verification receipt.

All-target Clippy lints (`-D clippy::all`), normal build, workspace formatting,
tracked whitespace, 571-package attribution inventory, matrix parsing and staging
24 release-documentation files/39 offline links pass. Three pre-existing unused
transfer/recovery helper warning groups remain unsuppressed. These scoped checks
do not replace strict full-workspace/release acceptance.

Two-axis review has no remaining scoped findings after fixes for navigation,
aliases, failed-start tracking, overlapping notifications and pidfd ordering.
Two P3 maintainability findings were also resolved. See the review report.

Earlier dark attempt01 received external SIGTERM before its final lifetime
checks. Attempt02 exposed a fixture error: a new Operations window has no initial
focus, so Review requires five Tabs. Four activated Cancel and legitimately let
queued work proceed. Attempt03 corrects that and asserts the target remains
absent immediately after Browser closure. Navigation red01 selected zero tests
and is not a functional red; red02 reproduces both navigation and alias defects.

## Remaining full-goal work

Continue the full PRD through Iced transfers, complete Trash/Recovery, device
browsing/actions, external clipboard/DnD/FileManager1/primary activation, media
and remaining preview behavior. Exact durable Retry/Continue, source owners,
journal association, descriptor authority and restart safety gates remain open.
Full native/accessibility/performance/workspace/release validation, final review
and commit remain required. Routine product decisions stay delegated and recorded
in Files ADRs; the goal must not be narrowed to this checkpoint.
