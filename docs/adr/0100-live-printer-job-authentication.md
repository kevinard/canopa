# Backend-owned Iced sign-in for live printer jobs

Status: scoped native verification complete; full Print acceptance remains open.

A scheduler can accept the complete document before a device later rejects its
credentials. The successful Print request and its uncertainty record have already
finished. Phase59 measured this boundary and a same-UUID CUPS-Authenticate-Job
control. Extending the document upload or polling briefly after acceptance cannot
own that later action. Accepted jobs remain CUPS's responsibility; Canopa does not
add a duplicate accepted-job database.

The opt-in backend now has a separate Canopa-private PrintJobs review interface.
`xdg-desktop-portal-canopa --printer-jobs` invokes that running backend. The XDG
Print interface and its packaged advertisement stay unchanged. The existing
standalone recovery command remains for uncertain records and retained files;
it cannot release accepted jobs while bypassing live printing policy.

The review reuses Print's original-caller authority, request admission and Close,
and the in-process Lockdown permit with latched denial revisions. Backend shutdown,
caller loss and a printing denial revoke the review. A later enable cannot revive
that earlier approval. The backend owns the Iced child through a private command
socket and retains its single review admission until the child and its bounded
helpers have confirmed exit, even after the method future disappears.

The Iced child gets display entries and selection keys. The backend retains the
actual source proofs. Each explicit sign-in consumes the cached selections; a
fresh listing and another activation are needed for another attempt. A fresh
credential broker belongs to each action. Credentials use the existing private
worker channel and separate Iced prompt; they do not enter review JSON, arguments,
application settings or a persistent store.

Listing reads current-user active jobs for the selected configured queue. A sign-in
binding records the UUID, immutable owner/name/queue, configured server and actual
connection protection, and ordered credential requirements. Missing source identity
leaves the action unavailable. A different approved HTTP actor cannot rewrite the
job's original owner. Before and after the prompt, the worker verifies the same
received document held for authentication. It requests current requirements from
CUPS directly; the final job-identity response is the last network query before
mutation. That action uses the checked connection and issues only0x400e. It cannot
create another job, upload the document or cancel a job.

CUPS can omit its updated requirement after changing internal none to username,
password on backend exit2. Canopa applies that precise default only to a confirmed
held-for-authentication job with a received document. Explicit ordered requirements
and unsupported negotiate behavior remain distinct. Field order must match the
binding, including at the parent credential-channel boundary.

HTTP401 approval occurs outside the mutation guard, on a fresh protected session,
then repeats source and requirement checks. Generic lost acknowledgement or a
connection failure does not authorize another mutation. An acknowledged action
means sign-in was submitted and release requested; it does not establish physical
printing. The UI refreshes live state and can show a later hold again after wrong
values, without resubmitting the document.

The final candidate passes format, strict workspace Clippy and the original
workspace compilation graph, plus 425 serial Portal library checks (207 Print),
55 integration, 12 Lockdown and the PPD runtime check. The actual Iced flow passes
in both appearances, including review Close, caller loss, Request.Close, Lockdown
and backend shutdown/restart. Cancel preserves the received job; wrong fields hold
it again; correct fields complete the same UUID and document without another upload.

The sealed evidence contains 64 native invocations, 116 groups and 706 observed
terminal identities. Both Iced appearance sequences contain 70 command/assertion
steps. Thirteen negative worker and nineteen source cases distinguish explicit
metadata-reply faults from actual scheduler title changes and a separate scheduler
namespace with reused numeric identity. Eleven release-only HTTP cases cover
actual private PAM, different actor/original owner, metadata privacy, rejection,
decline, policy denial, loopback and TLS. A lost IPP body after HTTP200 with an
unrelated authentication hint still cannot authorize another release. Existing
Print, protected recovery, reconnect and actual GTK/explicit Qt caller regressions
also pass. This is selected execution after workspace compilation, not an all-
workspace or release acceptance claim.

The wrong-once PAM case exposed a real gap: metadata200 resets libcups's cached-
password state and a guarded reconnect can overwrite401 without another callback.
The correction recognizes only the operation's fresh challenge, cleared before
that action, together with the refused reconnect, as existing recovery does. It
still requires fresh approval/session and source checks. The original failing
receipt and the corrected loopback/TLS regression remain in the evidence.

The device fixture uses a namespace-only executor adapter; it does not establish
unmodified cups-exec runtime or physical printing. Held Iced runs exercise device
fields; release-only PAM/TLS controls exercise actual workers in separate runs.
TLS encrypts worker to private gateway, with a loopback hop to private CUPS.
Current-user listing follows the effective CUPS account; arbitrary account switches
can hide original-owner jobs. Digest, long credentials and broader connection,
resource, device and packaging acceptance remain in progress.
See [sealed evidence](../../.scratch/xdg-portals/print/live-printer-job-evidence.json),
[current checkpoint](../../.scratch/xdg-portals/PHASE60-LIVE.md) and
[the full authentication issue](../../.scratch/xdg-portals/issues/04-authenticated-printing.md).

Primary protocol references are the [CUPS IPP extension](https://www.cups.org/doc/spec-ipp.html),
the [2.4.20 scheduler operation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/scheduler/ipp.c)
and [backend exit handling](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/scheduler/job.c).
Print remains opt-in. Digest, long credentials, connection/resource/device/runtime/
release gates, existing failures and the six experimental interfaces remain open.
The user prioritizes finishing Canopa Print before automatic Qt integration.
