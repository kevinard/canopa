# 0065: Iced filesystem operation interfaces

Status: implementation in progress, 2026-10-18. This does not claim completion of
the file manager or parity with the historical GTK adapter.

Use a renderer-independent operation service around the existing operation engine.
The Iced interface owns drafts and presentation; the engine owns filesystem
transactions, conflict capabilities, retained versions and actual shared Undo.
Do not introduce a second filesystem implementation in the renderer.

Admission persists a versioned request in the existing history database before
filesystem work starts. A metadata coordinator stays available while a single
filesystem worker executes or waits for a decision. Custom commands keep their
separate bounded execution policy and never create fabricated Undo entries.
Use asynchronous, coalesced change notifications instead of idle UI polling.

Capture the invoking Browser's raw directory path and resolved browsing binding.
Name dialogs keep that source while other windows or tabs navigate. A name is
one basename; an unchanged lossy display of a raw filename is a no-op, never an
instruction to replace its original bytes. Successful work refreshes affected
views without redirecting an unrelated Browser or stealing its selection.

Finished scheduling records contain a result, without an executable original
request or source placement lease. A failed checkpoint retains the actual result
in memory and holds the application until saving succeeds or the user explicitly
resolves it. Reopening queued or Running records presents them for review and
never automatically executes them. Corrupt or incompatible records remain on
disk with a visible error. Uncertain filesystem journals block ordinary new
mutations until Recovery can establish their authority; mere record visibility
does not establish crash continuation.

Public verification uses the established real-temporary-filesystem seam: submit
a request through the operation service, await durable admission and observable
completion, inspect actual files and shared Undo, then reopen the service. Native
verification exercises Iced name prompts, keyboard/focus, narrow tiles, independent
Browser state and Operations lifetime. These follow the repository's existing
public-test contracts and delegated engineering choices.

The full operation port still includes transfers, interactive Undo, Trash,
Recovery, device observation, external clipboard and drag-and-drop, completion
notifications, durable Retry/Continue and exact source/journal ownership. The
descriptor-authority and durability gates in ADR0056 remain open. Keep the full
PRD and workspace/release acceptance gates active.

## Worker foundation

The first implementation exposes durable mutation/Undo admission, serial work,
typed generation-bound Undo questions, queued/running cancellation, retained
result-save Retry, coalesced renderer notices and orderly idle shutdown. Completed
rows release their executable request and placement snapshot. Reopened Queued and
Running records become Awaiting review and Interrupted, respectively; no current
API authorizes replay or continuation of these records.

Public tests include actual filename bytes, a changed browsing alias, cancellation
behind an Undo question, a temporarily moved history file during result saving,
orderly reopening, and a real killed worker with queued/running records. This
foundation is now connected to Iced name dialogs and the shared Operations
window, with live platform volume observation (ADR0066). Exact journal
association, source-owner continuation and the broader native operation
lifetime/notification matrix still require work.
