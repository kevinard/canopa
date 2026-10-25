# Native file clipboard and shared Operations

Status: accepted for implementation; broader desktop acceptance remains tracked
in the [implementation ledger](../../../.scratch/file-manager/IMPLEMENTATION.md).

Canopa Files publishes `x-special/gnome-copied-files`, `text/uri-list`, and
`application/x-kde-cutselection`. File URIs preserve original Unix filename bytes;
only local file locations are accepted. Reading an external clipboard is
asynchronous, bounded to 16 MiB and five seconds, and checks that the clipboard
did not change between reading its formats. Cut completion updates the remaining
source selection only while its cut intent is still current. Content revisions
reject stale asynchronous reads; a separate cut-intent identifier survives the
application's own partial selection updates and subsequent Retry/Continue jobs.
A newer Copy or Cut, or an external clipboard change, invalidates that intent.
Unavailable and unfinished sources remain selected. See
[ADR 0045](0045-directory-retry-completion-plans.md) for the native regression
evidence and directory retry contract.

Ctrl+C, Ctrl+X and Ctrl+V operate on files when the listing owns input. Editable
fields retain their normal text clipboard behavior. Paste captures its destination
at invocation, so later navigation does not redirect the transfer. Closing the
originating tab while clipboard data is arriving does not discard a valid paste.

File clipboard ownership holds the application alive so it can serve Wayland
requests after its last browser closes. Losing ownership releases that hold.
This is separate from operation holds: queued, running and waiting jobs each keep
the application alive independently of windows or clipboard ownership.

One shared queue serializes mutations against the operation history. Workers
perform filesystem access; the GTK thread observes coalesced progress at 80 ms
intervals. Cancellation reaches copy chunks and waiting prompts. Cancelling a
queued request removes it immediately. Initial history loading can accept queued
requests; failure releases them with an error instead of leaving invisible holds.

Operations is available from the header and Ctrl+J. Its window can close without
cancelling jobs. It shows recent jobs, completed counts, failures, remaining source
paths, Cancel, Retry failed items, Continue remaining items, and Undo completed
changes. Completed rows hide their progress indicator; undone jobs say Undone.
The in-memory presentation retains up to 100 finished jobs plus active jobs.

Forward conflicts use ordinary native windows. Both items show their paths,
sizes or folder kind, and modification times. Files offer Replace, Keep both and
Skip; folders offer Merge, Keep both and Skip. A file/folder type mismatch offers
Keep both and Skip. Keep both is the initial default, and applying a choice to
remaining conflicts is opt-in. Unavailable destinations offer Retry or Cancel.
Decide later hides the prompt while retaining its reply channel and job hold;
Review or reopening Operations presents it again. A prompt reopened without its
original browser is transient for Operations, so it receives focus above it.

Retry failed items retains each exact source/destination pair. Cancelled or paused
work has an opaque continuation containing exact pending targets and directory
postorder work. In particular, resuming a partially copied Keep both folder uses
the already chosen folder name. A resumed/retried request starts a new Undo entry
to keep history chronological if other operations occurred meanwhile. Ordinary
Ctrl+Z waits for the active operation to finish or be cancelled. Targeted Undo
buttons and notifications recheck the requested entry atomically under the
history lock; an older action cannot reverse newer work.

Desktop notifications report completion, cancellation, failures and pending
decisions. A new prompt opens automatically only over the active originating
browser or active Operations window; background work asks for attention through
a notification until the user opens Operations. They open Operations and offer Undo for the concrete completed entry
when it is latest. A short application hold lets the final notification leave the
process before its last job hold is released. Activation of an already running
application is covered by the private desktop fixture; activation after process
exit still needs desktop/D-Bus installation integration.

The durable filesystem history remains authoritative. The presentation queue and
continuations are currently in memory: restoring unfinished jobs after a process
restart, graphical Undo conflict choices, retained-data recovery, removable-drive
unplug acceptance, and drag and drop are still required. Startup crash recovery
already preserves the completed filesystem prefix and cleans owned partial files;
that is distinct from restoring the unfinished job presentation.

Native clipboard acceptance uses `grim` captures. Niri's screenshot action itself
replaces the clipboard with an image, so using it between Copy and Paste would
invalidate the workflow under test. A private notification receiver records the
actual D-Bus notifications and emits their real action signals; it never accesses
the live desktop's notification service.
