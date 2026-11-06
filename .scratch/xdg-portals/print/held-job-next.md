# Authenticate an accepted job through backend-owned Iced review

Status: phase60 scoped live-job implementation and verification complete.

Read [ADR0100](../../../docs/adr/0100-live-printer-job-authentication.md) and
[sealed evidence](live-printer-job-evidence.json). The normal opt-in backend owns
the Iced review, lifetime/policy and fixed live-job proofs. Both appearances,
release-only account changes, source/requirement/namespace changes, lost replies
and relevant prior Print regressions pass. The original plan below is retained
as design rationale, not an instruction to implement the same feature again.

Full issue04 and the eighteen-interface goal remain active. Next are Digest,
long credentials, resource/timeout and connection compatibility, then remaining
driver/device/Flatpak/runtime/package/release acceptance. Print stays opt-in and
automatic Qt integration remains deferred. The namespace executor adapter remains
protocol-only; current-user listing is constrained by effective account visibility.

## Measured boundary

Phase58 protected recovery is sealed. The phase59 raw held controls06/07 accept
one document and let the worker exit before the backend gate opens. The backend
then exits2 and CUPS holds the same job for authentication. Wrong0x400e fields
rehold it; correct fields complete the same UUID without another Create or Send.
Canopa's current capabilities can still report authentication none after the hold.
The protocol fixture uses an explicit namespace-only executor adapter; it is not
unmodified executor, Iced or physical-printer acceptance. The separate receipt-
failure regression reproduces with original unprivileged CUPS and cups-exec.
See [investigation](held-job-investigation.json) and [checkpoint](../PHASE59-LIVE.md).

Scheduler acknowledgement ends document delivery. CUPS owns the accepted job;
the uncertainty catalog must not become a duplicate database of accepted jobs.
No short post-ack polling window can handle a later device challenge. Automatic
cleanup must preserve received jobs, including after a local receipt write fails.

## Chosen ownership seam for implementation

Use a separate explicit Iced printer-job review owned by the running backend,
independent of the completed application's Print request. Bind its lifetime to a
new review request, its original caller, Request.Close, backend shutdown and live
Lockdown Printing policy. Reuse the controller's in-process revision-bearing
Permit and cancellation Scope. A standalone UI that reads one policy property is
insufficient: an earlier denial must remain revoking even if later re-enabled.
The current --recover-print command cancels incomplete jobs and exports files;
it must not gain an offline printing-policy bypass to release accepted jobs.

Expose a Canopa-private review entry only with opt-in Print, and a normal user
command or launcher that invokes it. Keep it separate from the standardized XDG
Print wire interface and its advertisement. The backend owns the Iced child and
a private request/reply socket for bounded list and authenticate actions. The UI
receives job metadata, never credentials. Authentication uses the existing
separate worker credential stream and fresh action-owned Broker/Iced prompt.
Backend child retirement and admission remain owned until confirmed exit.

Reuse Print's request authority for direct callers rather than trusting a caller-
supplied identity. Reuse the existing appearance, window parenting, prompt budgets,
process exchange and cancellation conventions. No permanent credential store,
new native dependency or additional document copy is needed. Existing recovery
can remain available independently; accepted-job release is a different action.

## Worker contract

List live jobs from configured CUPS queues under explicit review. Capture the
source's immutable owner, name, queue, UUID and configured server/protection before
authentication. Cap lists and frames. A UUID or equally strong unique Canopa-tag
proof is required across later connections; generic title plus numeric ID is not
enough. Unavailable or contradictory metadata cannot authorize a mutation.
The approved HTTP actor may differ from the immutable job owner.

Only state4 with cups-held-for-authentication and a received document is eligible
for device sign-in. Keep queue's ordered domain/username/password requirement
separate from HTTP realm/resource credentials. CUPS can omit auth-info-required
after changing internal none to username,password; implement this precise late-
held default deliberately, without interpreting missing requirements for arbitrary
jobs as permission to send passwords. Preserve unsupported requirements explicitly.

Before credentials and again before mutation, inspect the fixed job on the
current configured connection, check protection, UUID/visible identity, received
state and current ordered requirements. Reuse public HTTP authentication deferral
and the checked-connection supervisor; an HTTP401 can authenticate outside the
mutation and then revalidate. Reconnection or missing acknowledgement cannot
silently repeat an uncertain Authenticate-Job. Issue only0x400e for the selected
job: never Create, Send, Cancel, Release-Job or restart another job on this path.

An acknowledged Authenticate-Job means credentials were submitted and release
requested. It does not prove device completion. Refresh live status; wrong values
may cause a later hold again. Keep each attempt explicit and bounded, and report
uncertainty without an automatic mutation retry.

## Implementation and acceptance

Add one live-job domain/binding type, isolated worker list/authenticate operations,
a backend review adapter with policy lifetime, and an Iced list/action surface.
Wire a concrete user entry through the normal backend. Keep validation sources
stable, root incremental graph unchanged, and native binaries checksum-addressed.

First reuse the measured late-held source-only backend control with the new real
worker action. Then test actual Iced controls in both appearances, including
wrong values, decline, close, original caller loss, Request.Close, Lockdown and
shutdown. Change UUID/owner/queue/requirements and replace scheduler namespaces;
verify fixed identity and no mutation of unrelated jobs. Drop authentication
acknowledgements and record exactly one document upload for the original job.
Do not call the namespace executor adapter physical/runtime acceptance.

Retain original acknowledgement/receipt-fault, empty-only recovery, signal/HUP,
cached automatic credentials, protected recovery, reconnect, public GTK/explicitQt,
layout/copy/driver and file-output checks. Digest, long credentials, broader
resource/device/runtime/release gates and existing failures remain separate.

Primary references checked against CUPS2.4.20:
- [CUPS IPP extension](https://www.cups.org/doc/spec-ipp.html):0x400e accepts the
  existing job URI and ordered literal auth-info job attributes.
- [Scheduler IPP implementation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/scheduler/ipp.c):authenticate_job handles that existing job.
- [Backend exit handling](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/scheduler/job.c):exit2 holds and can replace internal none with username,password.
