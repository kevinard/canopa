# Files resident host and local activation

Status: scoped hosting and activation verified, 2026-10-19. The full product,
session integration, performance and release gates remain incomplete.
Extends Files ADR0074/0076 and root ADR0017.

Files uses one owning process for a state directory. Configuration coordination,
local-volume observation, operation execution and action attention start before
Iced. Optional `--daemon` starts without a browser or GUI initialization. The
first ordinary launch into that owner initializes Iced on its main thread;
subsequent launches open windows on the same handles. Once initialized, Iced
retains its session connection while resident. This is display-free initial
hosting, not a separate frontend process or isolation from a frontend crash.
To open windows later, start the resident with its graphical session's display
environment. A start without display variables remains headless and refuses
browser requests without stopping its backend; it still accepts orderly Quit.
Display variables are never copied from a caller into a multithreaded process.
The core workflows remain usable without the desktop feature. Separate-process
hosting remains viable if crash isolation justifies the additional transport.

Use a private Unix socket for launch intent only, following Shell's local-control
precedent. This is not a filesystem command bus. Transfer execution, opaque
clipboard authority and generation-bound answers stay on their existing typed
interfaces in the owning process. Acquire a permanent advisory lock in the state
directory before starting any owner. Derive the endpoint in XDG_RUNTIME_DIR from
the canonical state-directory bytes; another session unable to reach that
endpoint must fail rather than start a second writer. Require an absolute,
owner-only runtime directory. Recover stale socket names only under the state
lock, never remove a non-socket, and remove an endpoint on orderly teardown only
if its filesystem identity still matches.

The bounded, versioned activation frame preserves Unix path bytes. Resolve
relative launch paths in the caller before forwarding. Unspecified configuration
inherits the running owner's selection; an explicit conflicting selection is
refused. A launch acknowledgement means the owner accepted the request, not that
the compositor mapped the window. Accepted openings retain a lifetime hold
through presentation handoff. Shutdown admission and pending openings share a
lock so a last-window close cannot lose an accepted launch. Only an explicit
stopping refusal may be retried automatically; a lost acknowledgement is
ambiguous and must not duplicate a request.

`--daemon` also makes an already running owner resident. `--quit` releases that
policy and requests exit after windows, accepted work, decisions, clipboard
ownership and unsaved results finish. It does not answer questions or cancel
work. An idle owner that has never initialized Iced can exit without connecting
to a display. Initial resident waiting blocks; draining before first GUI startup
may inspect active backend holds at a bounded interval. Normal resident idle
must not poll for launch requests.

Live executable stopped transfers retain application and activity holds until
explicit Resume or Abandon; closing windows or requesting Quit does not discard
their only plans. This also keeps managed device removal excluded while those
owners remain. Abandon saves its historical disposition before releasing owners
and never deletes files or reverses completed work. See
[Files ADR0102](../files/adr/0102-live-application-transfer-continuation.md).

Orderly shutdown acknowledges history teardown and awaits the exact volume
helper's exit and final observation-reader updates before releasing the instance
lock. Frontend detachment still never blocks on that helper. The activation
listener uses a private socketpair to wake shutdown independently of its public
pathname; connection establishment and frame I/O share a deadline.

Keep the desktop entry's normal Exec contract. The initial exclusion of D-Bus
activation is superseded for FileManager1 by [ADR0025](0025-files-file-manager-interface.md):
optional session-bus metadata starts the same resident owner on demand. It does
not add automatic startup or a resident systemd unit. An updater may replace the executable
while the old owner runs: compatible protocol launches continue to use that
owner until it exits. An incompatible protocol refuses activation; never replace
or kill a live owner to upgrade it. Packaging must document this behavior.

Public verification seams are launch/ownership with real private filesystem
locks and sockets, and the existing application workflows. Native acceptance
must cover initial headless residency, later windows in the same process,
last-window closure during work and decisions, reactivation, orderly shutdown,
and measured idle/warm behavior. This decision does not complete those gates,
safe crash continuation or the full product brief.

## Scoped verification

Six activation tests use actual private locks and sockets, with three subprocess
fixtures. They cover raw filenames, handoff holds, bounded admission, conflicting
configuration, headless refusal, endpoint replacement, a full accept backlog and
reclaiming an idle crashed owner's stale endpoint. The first implementation
failed the replacement and connection regressions; independent wakeup and bounded
connection establishment resolve them. An actual paused-helper shutdown failure
was corrected by awaiting the final observation reader and child reaping.

The local-runtime library builds without Iced. The default build passes 47
unfiltered Iced library tests. Native acceptance passes 49 checks: headless
startup and teardown, graphical residency and same-process reopening, filesystem
operations in both themes and light-theme custom actions. The reopened runtime
preserves raw-byte Rename/Undo and an unanswered generation-bound review while
every window is closed. Its explicit answer resumes queued work without changing
later file contents. The review screenshot was inspected; both scoped review
axes have no unresolved code findings.

The private debug fixture measured a first opening of 5.37 seconds and three
warm openings of 1.02–1.21 seconds. Three-second observations of the owner and
volume helper recorded about 60–61 MiB summed RSS before GUI initialization and
190 MiB after closing the first window. Initial intervals used about 1% of one
CPU core; the post-window interval recorded no CPU ticks. Summed RSS includes
shared pages, and these short observations include the fixture provider. They
do not establish release performance, a steady-state resource budget or cold-cache
latency. The independent first-window timeout remains unresolved. One new native
attempt also timed out starting Niri before Files launched; the unchanged retry
passed and is not a Files reliability fix.

Exact sources, attempts, limits and evidence are recorded in
[the resident verification](../../.scratch/file-manager/resident-verification.json).

Later device acceptance also covers actual private-provider Power off/Eject
callbacks completing after every window closes. A subsequent launch opens the
same resident owner and inspects its result; explicit Quit then retires it.
Both appearances pass. This extends window-lifetime evidence without changing
the process layout or establishing physical-device or production performance
acceptance. See [the device evidence](../../.scratch/file-manager/device-drives-verification.json).

Actual Files drag acceptance also verifies a foreign Move waiting on a conflict
while every Files window is closed. Reactivation reaches the same owner, the
explicit answer resumes the retained request, and shared Undo restores both
versions. An outgoing native Move waits for the foreign receiver's acknowledgement
with zero Files windows and then releases the ordinary owner. Both appearances
pass. This extends lifetime evidence without adding crash isolation or restart
execution authority. See [the external drag evidence](../../.scratch/file-manager/dnd-exchange-verification.json).
