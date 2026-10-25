# 0100: Selected sources survive live transfer continuation

Status: scoped core and native verification, 2026-10-20. Extends ADR0045,
ADR0056 and ADR0075. ADR0102 subsequently adds the live application workflow;
wider acceptance and restart authority remain open.

## Evidence and decision

A public engine regression reproduced Continue copying an unrelated replacement
at an explicitly selected file's pathname. The initial transfer had observed
the original entry, but its returned continuation retained only traversal data.
Resuming no longer checked the selected-entry observation.

Keep the original ObservedTransfer with each in-memory remaining/failed plan.
Clones share the existing bounded O_PATH pins; never observe the current pathname
again to make an old plan executable. Preserve this observation through initial
cancellation, unavailable-placement pauses and successive failed-item outcomes.
The original Copy/Move kind and destination must remain unchanged. Changing the
public descriptive fields of a live plan cannot change the captured intent.

An explicitly selected replacement is a new request. Restoring the actual
original object permits retry. A selected folder still includes its ordinary
descendant repair workflow: this change does not freeze contents or make each
descendant a separately selected root.

Retry and Continue may share postorder directory completion. Seed a resumed
run's previously verified directory set only from its captured DirectoryFinish
identity matching the still-held selected pin. An already removed source can
then reach the existing idempotent finish handling. This permits no mutation of
a replacement and does not replace journal, destination or cleanup checks.

## Ownership and limits

An unfinished live plan retains its selected pins until its final owner releases
it. Plans and clones share the process budget; there is no additional descriptor
allowance. A completed transfer with no remaining plans releases its observation
normally. This extends ADR0075's lifetime beyond one execution for live plans.

The application's descriptive durable Stopped result has no live execution owner.
Convert engine plans to descriptive saved plans explicitly and release their
observations there. Keeping pins in an otherwise review-only history row would
exhaust clipboard replacement capacity after that clipboard owner is gone.
The existing reduced-descriptor clipboard test reproduced this regression in
the first full-suite run. Actual application Resume must retain its executable
plan in a separate process-owned workflow, alongside original placement and
atomic request handoff; a saved result must not acquire that role implicitly.
ADR0102 now provides that separate live owner and explicit abandonment.

The portable checkpoint format remains version 1. Encoding omits process-local
observations, and decoding creates none. A saved payload remains insufficient
application execution authority. Restored application rows remain review-only;
their request/journal association, stable original media, source proof and exact
cross-process ownership handoff still need implementation. The low-level checkpoint round-trip tests do
not prove that wider contract. Descriptor-relative filesystem operations,
cross-process inode reuse and stale-plan replay also retain their separate gates.

## Scoped verification

The original replacement regression fails before the change and passes for Copy
and Move afterward. It also restores the actual source, retries it and checks
separate Undo for the resumed work and completed prefix. Additional public tests
cover a raw dangling link through two pre-execution pauses and refusal of changed
kind/destination before filesystem work.

The focused suite passes transfer, directory Retry/Continue, checkpoint and
handoff tests. The directory regression initially exposed duplicate postorder
cleanup, corrected by the recorded-directory rule above. Its selected-file test
now parks and returns the original parent instead of replacing a FIFO with a
different file; unselected-parent and Undo assertions remain intact.

The unchanged reduced-descriptor clipboard regression passes after saved results
release their observations. The complete private local-runtime suite then passes
552 parent tests across 65 targets, plus its custom device-runtime harness.
There are 36 ignored entries: some are helpers invoked by their parent tests,
and others require maintained private fixtures. This is not a claim that every
ignored entry ran. Default Iced library tests pass 78 tests with one ignored;
all-target Clippy with warnings denied, formatting and the default build pass.

Eight native cases pass 24 checks across dark/100% and light/125%: transfer
progress, retained questions, completed-prefix Undo, result inspection, clipboard
replacement under the existing 128-descriptor limit, parent closure/reopening
and compact inspectors. They use private files, session buses and storage
providers under the pinned Niri compatibility executable. No host service,
mount, session default or user file is changed.

The first native attempt also exposed an outdated keyboard fixture: its four
Tabs focused Cancel rather than Review because Recovery adds a keyboard stop.
The preserved previous application fails at the same step. Diagnostic captures
show the question still pending before that input and Cancel focused afterward.
The fixture now includes Recovery in Review and older-job traversal; the GUI,
timeouts and filesystem assertions are unchanged. All affected paths pass in
both appearances. The separate first-window timeout in that initial attempt
remains unexplained; successful later openings do not establish a reliability fix.

Primary standards/spec review has no unresolved findings in this change; no
independent review is claimed. Sources, failed attempts, executable hashes and
verification limits are recorded in
[the continuation evidence](../../../.scratch/file-manager/continuation-verification.json).
Application Retry/Continue was outside this scope and is subsequently connected
by [ADR0102](0102-live-application-transfer-continuation.md). Restart placement and
media proof, request/journal association, descriptor-relative access and restart
execution remain open, as
do full workspace, accessibility, performance and release acceptance.
