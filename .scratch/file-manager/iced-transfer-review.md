# Shared Iced transfer service review

Scope: ADR0067's transfer-service foundation, against the entire active PRD and
ADR0056/0065/0066. Baseline is `iced-operations-ui-source-03.json` (496 inputs);
final inputs are `iced-transfer-source-02.json` (500 inputs). Files are untracked,
so reviewers inspected complete source files and the manifest-identified changed
paths rather than treating an empty Git diff as a review. Reviews were read-only;
reviewers did not compile, edit or run native fixtures.

## Standards

Final report: no unresolved findings or new documented-standard violations/P3
smells. Private boxing preserves serialization shape, and the simplified Boolean
checks preserve validation behavior.

Resolved findings:

- P2: repeated UI snapshot reads deep-copied retained path/report vectors under
  the shared lock. Immutable paths and reports now use shared ownership.
- P2: fatal transfer errors fell back to an executable original failed batch.
  Confirmed outcomes survive errors and persist as explicit interrupted work;
  only proven before-execution refusals may retain the original plan, with a
  validated marker.
- P2: folder creation counted a confirmed completion before assigning its
  operation ID. The commit now records that ID immediately. A real journal
  failure after a folder-only prefix verifies persistence and reopening.

Reviewer: `/root/operation_standards_review`.

## Spec

Final report: no unresolved findings or spec regression in the final edits.
Boxing preserves serialized behavior; simplified checks are logically equivalent.

Resolved findings:

- P1: a fatal post-commit/finalization error discarded actual outcomes and left
  the original batch as an ordinary failed plan. The interrupted record now
  preserves confirmed facts/known exact plans and refuses replay pending review.
- P2: hydration accepted Transfer plans without original source requirements and
  stopped results without plans. Variant/context/phase validation now rejects
  those and other malformed records while preserving their saved bytes.
- P2: cancellation after dispatch but before execution became uncertainty and
  blocked later work. Fresh/decoded entry cancellation retains safe exact intent;
  the cancellation-sensitive preflight path distinguishes known cancellation.
  Actual rapid admission/Cancel trials confirm that later mutations remain usable.

Reviewer: `/root/operation_spec_review`.

Final totals: Standards0 unresolved; Spec0 unresolved. Full Iced transfer
invocation, durable Retry/Continue, saved-source ownership, journal association,
descriptor authority and other PRD gates remain implementation work. This review
does not certify those unfinished requirements or full product acceptance.
