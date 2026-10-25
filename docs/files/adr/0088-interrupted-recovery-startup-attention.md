# 0088: Interrupted Recovery startup attention

Status: startup attention and interrupted Undo workflow scoped-verified,
2026-10-19. Continues ADR0017/0074/0076/0086/0087. The complete product and
migration gates remain open.

The first Iced native interruption fixture kills its own private Files process
after an actual Undo quarantine, creates a newer occupant and reopens the same
history. The backend starts and browsing works, but Recovery never appears.
The adapter had no startup presentation hook. This is distinct from the existing
first-window timeout: the restarted Browser is mapped and focused in this case.

OperationJobs exposes a read-only `needs_recovery_review` request. The owner
checks committed unresolved journals and uncertain result reports on its worker;
the caller does not enumerate retained versions or infer review requirements
from displayed historical phases. Older journals without request rows are
included. Ordinary retained versions do not need startup attention. The query
does not reconcile, acknowledge reports, answer questions or grant execution.

Iced asks once when it attaches to the existing runtime. A required review opens
Recovery once a Browser exists; a result arriving with no Browser waits for a
later opening. An already-open Recovery satisfies presentation without another
focus request. Dismissing the window consumes this startup presentation, so
ordinary queue updates cannot reopen it. Inspection errors are shown through
Operations. Headless startup does not initialize Iced or a catalog observer.

Verify the core query through actual interrupted runtime and direct-engine
histories, including salvage, refused/successful reconciliation and uncertain
reports. Native acceptance must cover original-name preview bytes, non-consuming
copy-out, repeated process restart, blocked ordinary mutations, respected
dismissal, narrow controls and explicit reconciliation preserving newer work.
This does not establish general durable continuation or frontend crash isolation.

Scoped verification passes twelve Recovery core parents, including actual killed
runtime and legacy direct-engine Undo histories, and 52 Iced controller tests.
A controller regression first reproduced the late-reply dismissal race. The
separate consumed flag fixes that ordering and covers deferred presentation
without a Browser. Default all-target Clippy passes with warnings denied.

The same clean executable passes five native check groups in each theme
(dark100/light125). Actual quarantined text is previewed and copied out while
the primary journal identity and newer occupant remain unchanged. A second
restart presents Recovery again. At 340 pixels, explicit reconciliation refuses
the occupied original name, then succeeds after it is cleared. Salvage and the
original Copy retain independent Undo. No debugger, diagnostic pause or modifier
reset is used by the final fixture.

The large-text fixture exposed blocking editor construction, followed by
quadratic multiline insertion in the first correction. A private parent-debugger
stack identified the latter. The adapter first lays out an empty text editor,
then appends groups of 128 complete lines using the renderer's line iterator.
This avoids unbounded initial shaping and quadratic insertion while preserving
Unicode, paired line endings and the existing text limit. Pending text is
consumed once under the window/request revision guard; stale or repeated layout
callbacks cannot populate another selection or reset its cursor. File reads
remain on the worker and user edit actions stay ignored. Long-line and broader
preview performance acceptance remain open.
See the [verification receipt](../../../.scratch/file-manager/iced-interrupted-verification.json).
Separate initial-Browser timeouts remain unresolved; these successful scoped
runs do not establish startup reliability or production performance.
