# 0078: Core-owned custom-action queue

Status: scoped implementation verified, 2026-10-19. Continues ADR0003/0060/0074/0076.
The entire product brief remains active; no resident/headless host is added here.

## Workflow ownership

The Iced adapter previously owned custom-action admission, queue advancement,
cancellation and completion bookkeeping. Running a command on a worker thread
alone did not make that workflow independent of the renderer: Iced Ready events
dispatched subsequent commands. ActionJobs now owns the complete workflow,
with typed captured ActionRequest, bounded latest snapshots and explicit commands.
The existing public action execution seam remains its domain implementation.
Public runtime tests use real private scripts and files under the application's
pre-agreed action/operation seams (ADR0001), without UI or database side channels.

The core retains the captured catalog action, Unix paths and original working
folder binding. It validates applicability and rechecks that binding on the
command worker before preparation/execution. Preserve the existing four running
workers and 100 completed output summaries, process-group
cancellation and bounded output. Worker completion advances the queue even when
no observer consumes snapshots. Dropping a frontend's clone does not cancel
accepted commands while the owning runtime keeps its handle. Orderly shutdown
refuses active work and completion receipts not yet handed to the host; dropping the final owner cancels it as a stop request.

Compact typed completion receipts remain available independently of the 100-row
output history. Share 1024 admission slots between unfinished commands and
completion receipts not yet handed to the host. All accepted commands advance
without an observer; a stalled host eventually receives admission refusal instead
of silent completion loss or unbounded memory growth. Taking the receipts hands
notification responsibility to the host. Until that handoff the core retains an
application hold and refuses orderly shutdown. Queued cancellation produces no
completion notification, preserving the existing behavior.

Iced maps invocation to admission, mirrors snapshots, presents output and sends
explicit Cancel. Output editor state, displayed pages, focus, window opening
and native clipboard output copying remain in the adapter. Desktop notifications
and their pending-delivery holds remain a separate host-extraction gate. No
universal command bus or renderer-dependent identity enters ActionJobs.

## Limits and verification

Commands remain process-owned and volatile in this slice. No automatic replay,
fabricated file Undo, durable scheduling, source lease or original-media authority
is inferred. ADR0056's durable scheduling and ADR0076's headless host, single-owner
activation, frontend reconnect and performance acceptance remain incomplete.

Verify through the public core interface that accepted work advances with no
renderer or live observer, preserves captured requests and output, handles queued
and running cancellation, refuses a retargeted working folder and bounds retained
summaries/admission. Run the existing native custom-action workflows in both
themes after the adapter is migrated, including completion attention, output,
cancellation and accepted-command lifetime after every Browser closes. Record
actual results in the implementation ledger and action-runtime verification;
this decision does not itself claim they pass.

Scoped verification uses production source07, fixture09 and binary6b46a987. The
full no-desktop run87573 passes454 parent tests plus one isolated descriptor
helper. Focused runtime39000 passes4, Iced56583 passes50, and default all-target
Clippy91764, formatting35961 and normal build51420 pass. Clippy retains three
existing unused-helper warning groups. Both scoped review axes report0 findings.

Native47605 passes30 checks across dark100 and light125, including real argument
bytes, narrow paged output, cancellation and accepted-command lifetime after
Browser closure. Native62086 is retained as a failed fixture run: its old menu
index selected Copy after Copy/Cut were added. Fixture09 selects the observed
configured-action row and captures focus; all filesystem/output/lifetime
assertions are unchanged. Each passing application log contains four expected
missing GIO remote-monitor messages on the private bus, with no panic, CRITICAL
or GTK widget warnings. Source10 records documentation only; all547 input hashes
and the preserved binary/archive are verified. These results do not establish
resident hosting, crash isolation or any whole-product completion gate.
