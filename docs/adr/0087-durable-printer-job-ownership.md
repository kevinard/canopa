# Durable printer job ownership and restart cleanup

Status: scoped stable-endpoint restart cleanup verified in both appearances.
Print remains explicit opt-in and absent from default packaged routing. The full
portal goal and the remaining gates below stay active.

The previous actual main-death audit left an incoming CUPS job after restart.
The main process intentionally kills its helpers when it dies, so worker RAII and
the former parent's in-memory job ID cannot provide restart cleanup. A lost
Create-Job reply also leaves the backend without an acknowledged numeric ID.
Hard worker retirement must remain intact.

Each production printer submission now persists a 128-bit random intent before
Create-Job. The queued job name begins with its complete tag; only the display
title is truncated at a UTF-8 boundary to stay below IPP's name limit. The intent
also records the configured scheduler endpoint/port/encryption, user and approved
queue/instance. The Iced consent title is unchanged. A numeric job ID alone never
selects a recovery candidate: the worker queries this user's queue and matches the
exact tagged name, user and decoded queue, then revalidates before cancellation.

The private recovery directory contains a separate `printer-jobs.json` ledger,
limited to sixteen records and64KiB. The file publication catalog is unchanged.
The existing directory ownership/mode, no-follow, regular-file and single-link
rules apply. Updates use a fixed scratch name, atomic rename and file/directory/
ancestor fsync; no document body or authentication secret is stored. Intent is
durable before Create-Job, acknowledged ID before upload, and accepted state before
the worker reports success. Accepted metadata can be discarded after confirmation;
failed metadata removal preserves an accepted record for later disposal.

Startup cleanup runs in the existing bounded worker transport after D-Bus
readiness. Normal failed submissions also run cleanup after their worker is
actually reaped. A durable claim protects each entry while a cleanup worker is
alive. Claims use boot/PID/start identity and whole-process pidfd termination:
a zombie leader with a live thread is still busy. The native lifetime regression
reproduces this edge case without contacting any scheduler. Process pidfds without
`PIDFD_THREAD` become ready only after the thread group exits; see the
[Linux interface documentation](https://man7.org/linux/man-pages/man2/pidfd_open.2.html).

An unaccepted record is retired only after matching jobs are confirmed terminal.
Accepted records are removed without cancelling or resubmitting the acknowledged
job. A changed configured endpoint/user, unreadable state, missing match or
unconfirmed cancellation retains the record and its bounded admission slot. There
is no automatic replacement submission. Report counts refer to ledger records,
including accepted metadata removal, rather than physical printer completion.

The implementation preserves existing parent-death KILL and request/worker/storage
lifetime ownership. No GTK, Cargo dependency, vendor or default routing change is
introduced. The internal worker protocol adds optional registration on Submit and
a bounded recovery report; stateless developer clients keep their prior behavior.

Formatting, strict workspace/all-target Clippy and original-graph compilation pass,
with140 Print/12 Lockdown and complete Portal358 library/55 integration tests.
The public/native matrix passes13 groups per appearance: lost creation/final
replies, actual main death at three CUPS transaction points, fresh printing after
restart, live-worker isolation, exact-tag retirement, unrelated held-job isolation,
active accepted-job preservation and refusal before Create-Job when the ledger is
full. Its worker regression checks actual protocol frames too. Fifteen ordinary
prepared PDF/PS/SVG copy/duplex jobs also pass, including exact content, raster
blank backs, intrinsic geometry and one completed scheduler copy. All201 observed
final desktop/CUPS processes are terminal, and the separate thread-group fixture
also exits cleanly. Guarded sources and binary pins match. The exact earlier
process-lifetime source is preserved by checksum alongside the red regression.

First checks exposed three test fixtures using directories that did not meet the
existing0700 recovery requirement; the fixtures now use private child directories.
The first native control delivered the exact PDF but compared an escaped receiver
environment value with the raw job name. The final fixture decodes only the known
tag prefix while retaining the raw receipt. These failed runs remain failed
evidence, separate from final acceptance. No permission rule was relaxed.

Net phase disk growth is57,696,256bytes, leaving7,167,328,256bytes free at the seal;
no application binary copies, new native executable or alternate build graph were
created. See the [durable evidence](../../.scratch/xdg-portals/print/printer-recovery-evidence.json)
and [active checkpoint](../../.scratch/xdg-portals/CONTINUE.md). No new full workspace
execution is claimed; ten recorded Files integration failures remain open.

This does not establish atomic cancellation across a live scheduler namespace
replacement/internal libcups reconnect. That race still needs policy and evidence;
matching a tag immediately before the library call is not an atomic conditional
Cancel-Job. Missing/unidentified records also need a user-facing review/absence
policy. Physical/Flatpak/authenticated-device acceptance, advanced printer options,
publication crash, actual remote/kernel waits and global cross-restart resource
accounting remain separate gates. Print is not ready for default advertisement.
