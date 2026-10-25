# 0103: Preserve queued Rename's selected entry and parent folders

Status: scoped backend and native verification, 2026-10-20. Phase2 of the continuation
handoff; extends ADR0075. The full execution and restart contract remains open.

## Evidence and decision

A public OperationJobs regression queues Rename behind an actual Undo question,
parks the selected file, and creates an unrelated replacement at its pathname.
The previous backend renamed the replacement and published Finished. The same
source reproduced this twice (execution-authority-core-02/03, receipts51/52).

Capture Rename's original selected entry on the metadata worker before durable
admission is acknowledged. Retain its O_PATH/NOFOLLOW pin through queued work and
validate it before entering the mutation engine. A missing original does not
authorize an entry that appears later. Original content edits are allowed;
final symlinks are selected entries, without pinning their referents. A later
deliberate request may select a replacement.

A second regression retained the selected inode through a hard link in a
replacement source parent. The browsing binding still resolved to the same path
string, so leaf observation alone allowed the rename (core07, receipt54).
Retain and validate both original parent folders too. Parent aliases follow the
original folder; returning that folder is allowed. Parent pins use the ordinary
shared source budget, deduplicate equal paths, and release with the live owner.
Missing or unreadable parents refuse admission. Source observation failures keep
their original meaning; partial budget captures release before refusal.

## Boundaries

This guards queued selection and location changes. It does not make subsequent
pathname syscalls atomic or descriptor-relative, persist pins across restart, or
freeze contents. No saved row becomes executable through fresh observations.
Existing storage requirements and source browsing bindings still apply.

Retained-data admission needs a separate execution-owner decision: blindly adding
Rename's own request_sources reservation makes the existing mutation guard refuse
its own retained source. This observation slice added no such reservation.
[ADR0105](0105-retained-rename-ownership-and-dismissal.md) subsequently connects
Rename admission retention and its narrowly linked Forward owner, with explicit
ordinary-request review/dismissal. Exact
request/journal ownership, retention across supported restarts and the general
check/use mutation gap remain Phase2 work. Managed custom actions retain ADR0092's
explicit contract; these changes do not pin script referents or sandbox effects.

## Validation

Frozen source18 passes 577 backend parent tests across67 standard targets plus the
custom device harness,83 default Iced library tests (one ignored), formatting,
all-target Clippy with warnings denied and the default build. The isolated
RLIMIT_NOFILE128 child verifies bounded two-pin same-parent Rename admission,
partial-capture cleanup, queued cancellation and release from completed rows.

ELF613871c85757 passes six selected native cases/40 grouped checks: Create/Rename/
Undo, live continuation and interrupted Recovery in dark100% and light125%.
Critical narrow prompts, Undo, independent continuation Undo and Recovery captures
were inspected. The interruption batch used a runner-only signal diagnostic after
an unexplained parent SIGTERM; it is not a fix claim for that external condition.
New leaf/parent replacement assertions exercise the public backend queue; existing
native workflows establish current Iced compatibility, not the full product.
See EXECUTION-AUTHORITY-WIP.md and execution-authority-verification.json for exact
sources, terminal receipts, all failed attempts and remaining limits.
