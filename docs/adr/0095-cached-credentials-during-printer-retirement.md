# Cached credentials during automatic printer retirement

Status: scoped verified; complete protected authentication remains open.

A failed submission can still own a scheduler job. Its synchronous RAII retirement
opens fresh CUPS sessions before the worker's credential channel is dropped.
A real worker reproduction lost the final Send-Document reply after the scheduler
accepted it. The next Get-Jobs lookup asked for a new authentication realm through
that live channel. This was cleanup, not a document retry. Supplying synthetic
credentials caused the known tagged job to be canceled; an unrelated job remained
held. A failure path must not solicit a new user decision implicitly.

Retirement now enters a thread-bound, nestable cache-only scope. It marks private
challenges noninteractive while preserving approved cache access. The broker can
reuse only an exact endpoint/protection/realm/resource or queue/ordered-field
entry. For server authentication, the cached username must also equal the session
account being checked against the recorded job. A normal explicit operation can
still adopt an account the user selected earlier for that request.

A missing or rejected cache entry returns no credentials without opening a helper
or consuming another prompt. This is unavailable authentication, not user
cancellation. It must leave a lost-acknowledgement failure classified as uncertain,
with the record available for explicit recovery. A cache miss also cannot poison
other approved scopes in a recovery batch. Interactive refusals and malformed
channel replies retain their existing final handling. The standalone Iced helper
rejects noninteractive requests, and retained handoff entries represent interactive
consent.

This keeps successful protected cleanup possible with the same approved account;
it does not disable authentication wholesale. It preserves exact tag/user/queue
matching, final identity revalidation, cancellation's reconnect guard, accepted-job
protection, finite worker ownership and the rule against document resubmission.
The outer supervisor already carries no interactive provider. Explicit protected
recovery remains a separate user action, as do held-job credential correction and
authentication required first at Create/Send.

The final candidate passes190 Print tests,12 Lockdown tests,408 Portal library
and55 integration tests, plus the explicit PPD runtime test, formatting, strict
workspace Clippy and the original workspace compile graph. Twelve native groups
of fixtures contain91 passing cases and480 observed terminal processes. Both
appearances cover real public failure handling, authentication/withdrawal and
ordinary printer controls; the five reconnect regressions remain green.

Lost Create/final acknowledgements, rejected Send, new realms and rejected cached
passwords preserve uncertainty without another prompt or document retry. The
same approved account/scope cancels the exact owned job. Accepted and unrelated
jobs remain protected. Source snapshots, failed baseline and intentionally stopped
validation receipts are preserved in the [scoped evidence](../../.scratch/xdg-portals/print/retirement-authentication-evidence.json).
No new native executable, binary copy, dependency or routing change was needed;
about184MB of free space was gained against this phase's baseline.

The retirement matrix uses a private Basic gateway. A separate actual PAM
namespace investigation now measures late account binding and reproduces a
Create-time durable-owner mismatch and Send-only401 failure. The former is now
scoped verified in [ADR0096](0096-durable-owner-before-authenticated-creation.md);
Send-only authentication remains open. See [the measured investigation](../../.scratch/xdg-portals/print/principal-binding-investigation.json).
Durable late-account binding, manual protected recovery,
CUPS-Authenticate-Job, Digest, longer credentials and remaining resource/runtime
acceptance stay required by [authentication work](../../.scratch/xdg-portals/issues/04-authenticated-printing.md).
Print remains opt-in. See [the reproduction and task](../../.scratch/xdg-portals/issues/05-unattended-authentication-retirement.md)
and [request credential ownership](0094-request-owned-printer-authentication.md).
