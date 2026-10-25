# 0081: Backend-owned volume and operation startup

Status: accepted, scoped verification, 2026-10-19. Continues ADR0066/0074/0076.

The platform monitor and operation queue belong to the runtime owner. Start them
before Iced initialization and retain their owning handles outside its state.
The queue worker waits for the monitor's first fully prepared storage observation
before opening history or processing requests. Startup errors and disconnection
remain observable; an unprepared volume catalog never becomes execution authority.
Dropping a frontend or notice receiver cannot gate initialization. Monitor loss
continues to invalidate the original Storage and refuse unsafe work.

Use the existing public Storage and OperationJobs seams under ADR0001's delegated
test-seam authority, with real private files and actual prepared observations.
One bounded readiness handoff connects the platform worker to the core queue.
Final-owner drop must release a waiting worker without joining on the caller.
Check ownership again after readiness and before opening history: a successful
observation can arrive after the final owner has already requested shutdown.
Native acceptance continues to use the private GIO provider and real signals.
No fake filesystem, private database inspection or renderer-driven ready callback
is needed to verify the workflow.

Iced attaches to existing handles, renders startup/error snapshots and maps the
coalesced notice streams. It no longer creates the monitor or starts the queue
in a StorageReady message handler. Its browser refresh, dialogs and focus remain
presentation responsibilities.

This extraction does not choose process layout or activation transport. Resident
startup, notifications, primary-instance activation and orderly host shutdown
remain required by ADR0076. It does not grant durable replay or Retry/Continue
authority to restored presentation records.

## Scoped verification

Public startup tests cover successful preparation without a frontend, admission
and history gating, failed/disconnected readiness, original Storage invalidation,
waiting-worker release and successful readiness after final-owner drop. The last
case reproduced unwanted history creation before the post-preparation shutdown
check. Broader unchanged-core verification passes 76 parent tests and 2 isolated
helpers across six queue/storage/clipboard suites. Iced library tests pass 47;
all-target Clippy and formatting pass, and both scoped reviews have no remaining
findings.

The clean candidate passes original operation workflows in dark/100% and
light/125%, plus rapid location-entry and rebound text-editing regressions:
5 workflows, 43 declared checks, all application logs empty. Actual file changes,
shared Undo, decisions and private GIO monitor loss are checked. A presentation
transition bug discovered during acceptance is recorded in root ADR0020.

The intermittent first-window timeout is still unexplained. The passing startup
probes and a slow font-initialization observation do not establish a startup fix.
Resident/headless hosting, activation, notifications, idle/warm measurements and
full product/workspace/release validation remain separate work. See the
[immutable scoped evidence](../../../.scratch/file-manager/operation-startup-verification.json).
