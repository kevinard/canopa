# Iced transfer service checkpoint

Status: verified scoped progress, 2026-10-18. The full PRD goal remains active.
Final proof: `iced-transfer-verification.json`. All processes in this checkpoint
are terminal; both read-only review agents are complete with no unresolved
findings. No compiler, native fixture or metadata script remains live.

Progress toward the entire PRD; no completion boundary or requirement is removed.
Iced and `canopa-ui` remain mandatory. The previous verified native candidate is
`4f62a086329a7aa34f4c45a1a7505841e04dd18fa58f8acb9bb72693dafa5f93`.
Its preserved executable is recorded in `iced-transfer-before-binary.json`.
The source has changed since that candidate; do not describe it as current proof.

## Current implementation

ADR0067 adds Copy/Move to the renderer-independent serial operation service.
Source placement captured at Copy/Cut and destination placement at Paste remain
independent. Typed, generation-bound questions reject stale, wrong-kind and
unoffered answers. Progress has one coalesced wake-up and shared immutable report
and path data. Stopped results retain actual reports and opaque exact failed and
remaining continuations, without reconstructing their ownership from paths.
Finished results release executable plans and placements.

Fatal execution/finalization errors now retain confirmed engine outcomes and
become explicit interrupted records. They preserve known plans, never save the
original request as an ordinary failed batch, and block further changes pending
reconciliation. The core error carries accumulated facts through final cleanup
failure. Validating hydration rejects variant/phase/version/context mismatches;
invalid records remain byte-for-byte in the database.

No Iced Copy/Paste invocation or transfer question controls are connected yet.
Stopped plans are review-only pending durable source ownership, journal
association and exact handoff. This interim interruption handling is not the
finished Recovery/reconciliation feature.

## Verification in this goal turn

The initial desktop all-target check (`iced-transfer-desktop-check-01.log`,
session28401) explicitly exited0 before later review fixes. It is historical
typecheck evidence, not verification of the final review changes.

`iced-transfer-review-green-01.log` explicitly exited0:13 public transfer tests
and12 previous operation-worker tests, plus the parent-invoked crash helper.
Tests exercise actual private files, raw Unix names, symlinks, shared Undo,
distinct folder/file conflicts, cancellation, original media observations,
destination loss, unavailable-folder Retry, failed stopped-result writes,
observable copying, and a forced cross-filesystem Move on devices2050 and35.
A real SQLite final-trim failure preserves a committed prefix and its exact
remaining continuation across orderly reopening without replay. Eight corrupt
record shapes are rejected without rewriting their bytes, phases or files.

Red/green proofs are in `iced-transfer-finalization-{red,green}-01.log` and
`iced-transfer-validation-red-01.log` / `iced-transfer-review-green-01.log`.
`iced-transfer-waiting-progress-red-01.log` exposes a real throttled count0
while the conflict follows a confirmed count1 prefix. The source now publishes
latest counts before asking; the full core run is still validating that change.

The earlier immutable manifest is `iced-transfer-source-01.json`:500 inputs,
nine changed/new paths against `iced-operations-ui-source-03.json` (496 inputs).
Final review fixes are captured in `iced-transfer-source-02.json`, also500
inputs/nine changed paths against the original baseline. Never overwrite either.

## Follow-up fixes and live work

The first full no-default-features Files suite (session82412,
`iced-transfer-full-core-01.log`) explicitly exited0:43 test binaries,402 parent
tests passed and22 helper tests ignored by direct discovery (their parents invoke
them in isolated processes). It verified the earlier Rust inputs, before the
following cancellation/folder-ID fixes. The waiting-progress assertion passes.

Both review axes resolved the snapshot-copy and original-batch findings.
The follow-up found two additional edges, both now fixed:

- Engine-entry cancellation now returns exact safe remaining intent. The
  preflight path recognizes known cancellation before changes. An actual
  admission/dispatch/Cancel race runs16 times and later mutations remain usable.
- A confirmed destination-folder commit records its operation ID immediately.
  A DELETE-pending trigger after an empty-folder prefix creates a real fatal
  journal failure; the confirmed report and ID reopen without repeating work.

The16 transfer tests and12 prior worker tests pass in
`iced-transfer-entry-folder-green-01.log` (session86209, terminal0). Three genuine
red failures appear in `iced-transfer-entry-folder-red-01.log`: fresh/decoded
entry cancellation, actual dispatch cancellation, and folder-only ID retention.
Known before-execution refusals now have an explicit persisted marker; ambiguous
failed originals cannot hydrate. A later test extension covers that marker's
validity and reopening a refused transfer while the original uncertainty stays.
Iced interrupted terminal results refresh the source and notify; a controller
regression covers stopped/interrupted partial results and avoids duplicate
refresh. Interrupted details identify the report as the confirmed portion.

Both review axes now report zero unresolved findings. Reviewers are
`/root/operation_standards_review` and `/root/operation_spec_review`, read-only
and idle. The applicable
implement/code-review skill authorizes their parallel review follow-ups. They
must not compile or edit. All-target Clippy01 found two simplifiable Boolean
expressions and two large enums; the follow-up simplifies the guards and boxes
private transfer results without changing their serialized shape. Clippy02
(session91555) explicitly exited0. The final lint edits were independently
reviewed by both axes with no regression; see `iced-transfer-review.md`.
The571-package attribution inventory (59371), acceptance-matrix parse, tracked
whitespace and workspace format check (49056) also explicitly exited0.

Iced library/controller tests (24160) explicitly exited0 with51 passed, including
the new stopped/interrupted source refresh and explanation regression.
The normal build (56805) explicitly exited0. Candidate
`5996a732f0acc376a72707e5298b792cbe62db724834bc84fa59494aadb5f22d`
is115210728 bytes and binds all500 unchanged inputs from source02; see
`iced-transfer-binary-01.json`. Linked libraries resolve without libgtk.
The final full core suite (14222) explicitly exited0 with405 parent tests and22
parent-invoked helper definitions across43 binaries; doc tests also pass.
Counting excludes the doc-test block rather than replacing the final test
binary's9 passing tests with that zero. All500 source inputs and the candidate
binary were rechecked unchanged before native execution.
The first native dark wrapper (4535) explicitly exited1 before launching the app:
the sandbox denied creation of its private D-Bus Unix socket. This is runner
setup failure, not a failed application workflow. The isolated rerun received
the required socket permission through the execution tool and explicitly exited0
in session18015, output `iced-transfer-native-dark-02`,11 declared checks, empty
application log. The light/125% fixture (17985) explicitly exited0 with11 checks
and an empty application log. Eight captures across both themes were inspected:
narrow New Folder, Undo question, source-closure pending work and observer loss.
All500 source inputs and the normal executable remained unchanged through the
final native checks. Full current counts are405 core tests,51 Iced controller/
library tests and22 native regression checks.
No native transfer acceptance is claimed.

The tested after-binary is preserved as `canopa-files-5996a732.gz` in this
directory (28202611 compressed bytes,115210728 original bytes). Its decompressed
SHA256 matches the candidate; see `iced-transfer-after-binary.json`. A temporary
uncompressed copy hit EDQUOT after93192192 bytes; that owned partial copy and its
private directory were removed before making the verified archive. Restore the
gzip archive to a private executable path if needed; do not treat the archive as
an executable. The earlier4f62 before-binary remains in its recorded temporary
directory. Workspace free space is limited; inspect generated-package cleanup
before another large rebuild, with every compiler/native process terminal.

## Full goal continuation

Complete durable Retry/Continue with atomic chosen-plan handoff, saved-source
ownership, journal association and descriptor authority. Then wire Iced transfer
questions/progress/controls and real Wayland clipboard/DnD. Continue Trash and
Recovery, device management, media previews, FileManager1, Downloads acceptance,
accessibility/performance, full workspace and release gates, independent final
review and commit. Preserve the rest of the ledger's Next work list.

The Git checkout has extensive pre-existing changes; `.git` is read-only. Keep
the unrelated shell research document untouched. Compiler/native runs remain
sequential, with no source mutation during either. Use offline Cargo with `-j1`.
All mutating tests use private temporary files and no real user mounts/services.
