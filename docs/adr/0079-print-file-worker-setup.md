# Print file verification and setup workers

Status: accepted; scoped native file-worker verification; Print unregistered

## Context

The retained Print file consent and publication journal preserve live object
authority and uncertain commits. The initial implementation still verified file
metadata in controller futures and prepared staging on `spawn_blocking`. A stalled
filesystem could block runtime threads or keep a blocking task, its storage and
its operation permit alive indefinitely. Its failed-settlement fallback also
consulted that same filesystem in the backend process.

## Decision

File consent transport is separate from filesystem verification. After its own
Iced helper exits, the backend receives the approved descriptors on a nonblocking,
one-shot Unix socket. It checks the grant's shape and receives actual descriptors;
it does not reopen the destination or perform metadata validation there.
Before issuing a prepared token, and again before delivering a retained approval,
the controller asks the production backend to verify the file. `VerifyFile` uses
the shared bounded worker client and revalidates the held parent, optional entry
and selected name inside its worker. Cancellation still owns and retires that
worker when the initiating D-Bus future disappears.

The backend chooses a random staging name without filesystem I/O, then sends the
retained consent to `PrepareFile`. That worker verifies the consent and creates
and syncs the private staging directory, empty payload and journal. It returns
the publication identity and four or five live O_PATH descriptors with SCM_RIGHTS.
The backend checks the allocation, framing, descriptor count, O_PATH/CLOEXEC flags
and end of stream after the worker exits. A leaked writer or extra packet fails
immediately. `PublishFile` and `SettleFile` check object type and identity in
their workers, reopen access through those held proc-FD objects, and revalidate
the publication before acting. A replaced pathname cannot redirect that reopen.

One independently owned publication task retains the same operation permit,
frozen body, original consent and helper lifetimes across setup, publication and
settlement. Losing the initiating future signals the active stage, then permits
independent settlement once that stage has exited. Even cancellation after setup
returns a descriptor receipt can use it for settlement instead of abandoning the
known stage. A failure before that receipt can leave private setup material; its
already-chosen name supplies a recovery hint. Setup cannot claim or replace the
destination.

After failed settlement, the backend uses only its positive publication receipt
to establish success. Without that receipt it returns an uncertain failure and
retains recovery; it does not stat a target, read a journal, restore or retry.
With a positive receipt it can report committed output while retaining any
uncleaned original and stage. The recovery hint uses the originally approved
parent location; a moved parent can make it stale. Neither a hint nor a journal
authorizes replay after restart. Startup discovery and recovery presentation are
still required.

## Validation and remaining lifetime boundary

Formatting, strict whole-workspace/all-target Clippy and the original workspace's
all-target compilation pass. The harness passes 97 Print tests and 12 Lockdown
tests; complete Portal regressions pass 315 library and 55 integration tests
(16/2 ignored). The three new handoff tests cover live object authority for new
and replacement output, O_PATH/CLOEXEC flags, another allocation, extra packets,
an unclosed writer and conservative original-location hints.

Six actual file-worker groups pass. Normal publication retains its complete
new/replacement/format/late-file/cancellation/future-loss behavior. Five stall
cases observe real FIFO opens in `wait_for_partner`: setup cancellation, setup
future loss and verification cancellation retire their helpers after about ten
seconds. Failed settlement reaches its deadline after about seventy seconds.
With a positive publication receipt it reports committed output and preserves
recovery; with an actual commit whose reply is discarded it returns uncertain
failure without retrying, preserving the new document and previous file.
All 19 observed filesystem-fixture processes are terminal.

The existing 24-group native production renderer/publication/storage/lifetime
matrix passes (126 observed processes), as do 36 public canonical-frontend/
Production/Iced/file/private-CUPS groups across dark/100% and light/125%
(176 observed processes). Their 22 actual printer deliveries, live GTK and
explicit Qt DBus/PDF clients remain correct. All 321 observed processes are
terminal, with no cleanup kills. Guarded inputs and executable pins match.
Startup linkage contains no GTK, CUPS or qpdf.

The authoritative receipt is `target/portal-print-38/acceptance-01.json`; its
concise index is `.scratch/xdg-portals/print/setup-evidence.json`. The original
incremental target/cache is reused. Measured phase net storage growth is negative
by about 4 MB. No new Rust native driver or binary copies were added. No new full
workspace test gate ran; the ten recorded Files integration failures remain.

An intermediate test caught the validating receiver waiting for EOF before
rejecting swapped descriptors. The exact blocked harness was identified and
terminated; validation-before-EOF was restored for that receiver, while the
nonblocking presentation receiver remains transport-only. The final regression
passes. An earlier Clippy style correction and a sandbox-restricted probe failure
are retained in the execution records. They are not full acceptance passes.

This checkpoint removes explicit metadata, staging and failed-settlement filesystem
calls from backend request handling. It does not establish bounded termination
for a worker stuck in uninterruptible kernel I/O. At this checkpoint the task runner still
awaits SIGKILL and reaping, and shutdown drains the retained operation permits.
That separate retirement/quarantine policy remains a gate before normal Print
registration. A killable FIFO-open test must not be presented as proof about
uninterruptible remote-filesystem waits. The
[wait contract](https://man7.org/linux/man-pages/man2/waitpid.2.html) distinguishes
sending a signal from observing that the child has actually exited.

The O_PATH boundary avoids writable staging descriptions in the backend.
Closing ordinary writable descriptors can dispatch filesystem flush work: Linux's
[close implementation](https://github.com/torvalds/linux/blob/master/fs/open.c)
calls the file operation's flush method, and
[NFS file flush](https://github.com/torvalds/linux/blob/master/fs/nfs/file.c)
flushes pending writes. A local Linux probe passes O_PATH handles by SCM_RIGHTS
and reopens the exact held object after its original pathname is replaced; the
foreign entry remains untouched. This supports the chosen access mechanism and
does not establish NFS or uninterruptible-wait acceptance. Reaping/quarantine
and shutdown remain the unfinished lifetime boundary.

The subsequent [retirement decision](0080-print-pending-worker-retirement.md)
bounds caller completion and shutdown while retaining worker ownership and
deferring settlement until confirmed exit. Its new evidence supersedes the
pending software-lifetime work above; physical/remote and broader recovery
acceptance remain separate.

The broader recovery, power-loss, device/toolkit/Flatpak, ambiguous CUPS and
packaging gates remain. The accepted eighteen-interface goal and installed
services, routing and queues do not change.
