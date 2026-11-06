# CUPS final HTTP acknowledgement

Status: historical HTTP acknowledgement fix and initial hard-main-death failure.
[ADR0087](0087-durable-printer-job-ownership.md) now records the scoped restart
cleanup fix. Print remains explicit opt-in and outside packaged routing; the full
portal goal and wider Print/release gates remain open.

Print must acknowledge a printer submission only after receiving the scheduler's
final success response. A completed printer job and a received acknowledgement
are separate facts: a transport failure can occur between them. The application
must not be told that an unacknowledged submission succeeded, and the backend must
not automatically submit a replacement that could print twice.

A private Unix-socket relay reproduces the failure through actual normal-service
Print, the standard frontend, Iced consent, private CUPS and a loopback PDF printer.
The relay receives the scheduler's complete HTTP200/IPP success reply to
Send-Document, then drops that reply instead of forwarding it. The printer receives
the exact approved bytes once and the scheduler completes the job. Before the fix,
public Print nevertheless reports success. An independent invocation of the actual
isolated worker likewise emits Created then Accepted and exits successfully after
the final reply is dropped. Positive forwarding and a dropped Create-Job reply
are controls: the first succeeds; the second returns portal failure without a
Send-Document or automatic replacement job.

The CUPS convenience function `cupsFinishDestDocument` discards the response
object and returns the thread's last IPP error value. In the installed CUPS
implementation, an HTTP read error can leave that value at an earlier success.
The Rust worker now also reads the public `httpGetStatus` result on that same live
connection. Only HTTP200 together with an IPP success class permits Accepted and
marks its owned partial job committed. Existing RAII and controller cleanup remain
responsible for unacknowledged known jobs; no resubmission is added. The existing
native stall shim forwards this additional public function.

This is a worker-boundary correction, with no GTK, parser, Cargo dependency or
default portal routing change. The regression uses actual transport loss. The
retained [worker regression](../../scripts/test-print-cups-replies.py) accepts an
explicit private scheduler socket/fixture queue and verifies actual Created,
Accepted and Failed frames. An operation failure is a Failed frame delivered by
a normally exiting helper. Fixture cleanup retires only IDs recorded from the
owned scheduler's replies; it does not grant this knowledge to the product.

Formatting, strict workspace/all-target Clippy and original-graph compilation
pass, together with135 Print/12 Lockdown and complete Portal353 library/55
integration tests. Four public acknowledgement/control cases and three isolated
worker cases pass in each appearance (dark100/light125). Lost Create replies
produce failure/no upload/no replacement; lost final replies produce failure and
owned cancellation. Both positive public calls deliver exact bytes once. Fifteen
normal prepared PDF/PS/SVG copy/duplex cases also complete as single scheduler
copies, with preserved content/geometry. All133 observed final native processes
are terminal. Source/binary guards match, with no executable copies/new native
driver; sealed disk use is6,709,248bytes below the phase baseline. See
[evidence](../../.scratch/xdg-portals/print/cups-acknowledgement-evidence.json).
No new full workspace execution is claimed; ten recorded Files failures remain.

The actual main-death audit identifies a separate unresolved failure. The shared
helper spawner intentionally uses parent-death KILL, so abrupt service death
retires the CUPS worker without running its RAII cancellation. After acknowledged
Create-Job and before the final Send-Document HTTP chunk reaches the scheduler,
killing only the main service retires the identified worker. The restarted backend
leaves that job incoming/held (state4), confirmed in the retained CUPS job record,
with no receiver output. This fails in both appearances. A lost Create reply also
leaves an incoming job whose ID the product never received. These outcomes are
preserved as failed receipts, not accepted crash recovery. The next policy needs
durable, precise printer-job ownership and restart cleanup while preserving hard
worker retirement; simply disabling parent-death KILL would weaken existing
resource guarantees. No cancellation of unidentified or unrelated jobs is added.

Earlier fixture failures are also retained: requiring physical completion before
releasing a dropped reply conflicts with timely owned cancellation; requiring a
nonzero helper exit for a delivered Failed frame misreads the worker protocol.
Neither is counted as final acceptance. Publication crash, cross-restart resource
limits, remote/kernel waits, automatic Qt/live toolkit printer emission, advanced
device controls, real Flatpak/physical printing and runtime packaging remain.

Primary implementation references: pinned
[CUPS destination job functions](https://github.com/OpenPrinting/cups/blob/v2.4.20/cups/dest-job.c#L230-L269)
and [CUPS response handling](https://github.com/OpenPrinting/cups/blob/v2.4.20/cups/request.c#L289-L440).
The observed stale-success behavior is confirmed by the local native regression;
it is not a claim about every CUPS version or every transport error.
