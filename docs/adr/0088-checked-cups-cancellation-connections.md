# Checked CUPS connections during printer-job retirement

Status: scoped reconnect regression verified; Print remains opt-in.

The durable exact-tag ledger from [ADR0087](0087-durable-printer-job-ownership.md)
protects recovery selection, but a real two-scheduler test exposed another race.
After the final ownership query, libcups can reconnect inside cancellation. A
replacement scheduler can have the same numeric ID for an unrelated job. The
pre-fix worker cancelled that unrelated job even though its own tagged job remained
on the original scheduler. The failed receipt remains preserved.

Recovery now keeps the synchronous libcups thread, HTTP object and thread-local
authentication state. A per-worker Linux seccomp notification supervisor permits
ordinary connect calls while querying identity, then rejects every connect during
the cancellation call. If the library attempts to reconnect, recovery performs a
complete fresh lookup before another cancellation attempt. Three changing
connections leave the record unresolved rather than looping indefinitely. One
supervisor serves all receipts in a recovery operation. Normal failed submissions
use the same guard through journal retirement.

This follows the actual reconnect behavior in CUPS
[request handling](https://github.com/OpenPrinting/cups/blob/v2.4.20/cups/request.c)
and [destination cancellation](https://github.com/OpenPrinting/cups/blob/v2.4.20/cups/dest-job.c).
It avoids transferring opaque CUPS objects or authentication state to another
thread. The supervisor handles only kernel notifications; CUPS stays on its
original worker thread.

The guard checks the syscall architecture, rejects the x32 ABI, verifies kernel
notification sizes and installs only inside the isolated worker. Unsupported
kernel/ABI, supervisor failure and unconfirmed cancellation retain recovery state.
The stop pipe and an invalid-fd connect wake notification retirement; the latter
cannot contact an endpoint. Process pidfds continue protecting the entire worker
thread group, including its supervisor. Parent-death termination is unchanged.
The Linux [seccomp notification interface](https://docs.kernel.org/userspace-api/seccomp_filter.html)
provides the listener and continuation API used here.

This is a sequencing guard for trusted synchronous libcups. Lookup continuation
does not inspect or authorize sockaddr memory. It assumes a server namespace
remains continuous within an established connection. It does not establish an
atomic conditional IPP Cancel-Job against an adversarial server, a proxy that
retargets an existing connection or arbitrary same-server identifier reuse.
The stateless developer CancelJob operation retains its existing explicit-ID
contract; production retirement uses the durable intent path.

The retained Python regression takes two explicitly owned Unix schedulers and a
disposable queue. It creates incomplete jobs with matching IDs, without uploading
a document, and closes the checked HTTP connection at the measured cancellation
boundary. Stable and same-scheduler reconnect cases cancel only the tagged job.
Replacement and three repeated reconnects leave both jobs untouched and preserve
the unresolved record. Two receipts in one worker confirm supervisor reuse. All
five actual worker processes and both schedulers exit; the harness also exits.

Formatting, strict workspace/all-target Clippy and original-graph compilation
pass, with140 Print/12 Lockdown and complete Portal358 library/55 integration
tests. The separate zombie-leader/live-thread regression still reports busy
until whole-process termination. Thirteen normal-service dark regression groups also pass, including lost
replies and three actual main-death/restart points; all51 observed desktop/CUPS
processes are terminal. UI sources are unchanged; broader appearance/device
acceptance is not re-established by this targeted run.

See [the retained regression](../../scripts/test-print-cups-reconnect.py),
[reconnect evidence](../../.scratch/xdg-portals/print/printer-connection-evidence.json)
and [checkpoint](../../.scratch/xdg-portals/CONTINUE.md). Root target outputs and
checksum-addressed native fixtures were reused. No alternate build graph or
application executable copies were created. These checks do not establish full
workspace execution, authenticated/physical devices, Flatpak or release acceptance.
Unresolved printer records still need their Iced review and absence policy; the
remaining Print and six experimental-interface gates remain active.
