# Files review against the product brief and repository standards

Baseline: `65e7cc245dc52607196bae6330f4a92da49918e2` (HEAD). No commits follow it.
The review covers `git diff HEAD --` and the new, untracked Files crate and its
integration scripts/docs. The full product goal remains active. Two read-only
reviewers inspected source; runtime reproduction of their findings is separate.

## Standards

- **P2, documented standard: shared control styling is ignored.**
  `crates/canopa-files/src/desktop/appearance.rs:45` fixes control geometry and
  colors instead of resolving `Appearance::style(UiKind)`. Shared button radius,
  input and menu settings therefore have no effect. Root ADR0015:28 requires
  semantic roles and sizes. Map applicable styles and state overrides to GTK.
- **P3, optional heuristic: possible Data Clumps / Primitive Obsession.**
  `scripts/verify-files.py:51` carries scenario choice as many positional booleans,
  repeated at line1567. Contradictory scenario flags silently select the first
  branch. One validated scenario and structured options would reduce this risk.
  This is a judgment call, not a documented-standard violation.

Standards: one documented violation and one optional smell; worst issue within
this axis is ignored shared control appearance.

## Spec

- **P1, failed/paused requests can disappear after natural process exit.**
  The PRD asks Operations to show "what finished and what remains" (line196)
  and provide "Retry failed items" (line203). Requests/continuations are in
  `desktop/jobs.rs` memory; completion retains retry work around line1337 but
  releases its application hold around line1495. With all windows closed,
  failure can terminate Files after its brief notification hold. A restarted
  process initializes an empty queue. Persist exact unresolved requests and
  original placement requirements before releasing their lifetime.
- **P1, Copy/Cut loses the selected source's media binding before Paste.**
  The PRD includes "Cut, Copy, and Paste" (line140) and "local files and removable
  drives" (line210). `desktop/clipboard.rs:18` retains paths and an intent counter;
  the storage snapshot is captured later at Paste enqueue (`desktop/jobs.rs:477`),
  with the destination tab's browsing binding. Replacing the source media before
  Paste can therefore authorize different files at the same path. Capture source
  placement and alias bindings with the selection and retain them through Retry.

No material scope creep was identified. Spec: two partial requirements; the
worst issues within this axis are unresolved-job loss and source-media confusion.

## Follow-through

Parent source inspection confirms the appearance adapter omission. The clipboard
finding was then reproduced twice with real native Cut and a distinct replacement
UUID; the wrong replacement bytes moved while the original source stayed intact.
[ADR0055](../../docs/files/adr/0055-clipboard-source-placement.md) records the fix,
verified by 76 final native checks and 41 inspected captures, including actual
cross-filesystem Retry/Undo in both themes. See its
[proof](clipboard-placement-verification.json). Durable jobs and shared control
appearance remain open. The initial axis totals above describe the review
snapshot, not an unchanged final defect count or a completion claim.
