# Suppress interactive prompts during synchronous printer retirement

Status: done (scoped retirement; full authentication remains in issue04)

Source inspection during phase54 found an important remaining authentication
boundary. Client's post-worker cleanup deliberately passes no credential provider,
but synchronous PartialJob::drop calls Journal::retire or cups::cancel while the
submission worker's authentication::Scope is still installed. Fresh Sessions in
that path can inherit the live channel through Authentication::install. Therefore
outer-cleanup suppression does not prove that partial-job retirement cannot ask
for another password. This source finding was reproduced and corrected in phase55. The original
failed native receipt is preserved; the final scoped matrix passes.

Use the validated private scheduler/relay and synthetic credentials. Fail or lose
a Create/Send/final acknowledgement after a job exists, then require a new or
rejected authentication scope during identity lookup/retirement. Prove that no
new interactive prompt appears during automatic retirement, no document is sent
again, and an unconfirmed record remains available for explicit protected review.
Preserve already accepted jobs, exact tag/owner/queue identity, checked connection
cancellation and bounded retirement/admission. Request.Close, caller loss,
Lockdown and shutdown must still withdraw any already active normal prompt.

A synchronous guard suppressing interactive channel access is one possible seam;
a cache-only retirement mode could preserve already approved exact scopes. Neither
approach may silently change the authenticated account or relax the job identity
check. Measure real principal binding and integrate explicit protected recovery
as required by issue04. Keep this work required before complete authentication or
default Print advertisement. Reuse the same workspace graph/cache and bounded
native evidence; do not patch installed CUPS or retain credentials on disk.

Source locations: print/worker/cups.rs PartialJob::drop; cups/recovery.rs
Journal::retire and retire; cups.rs Session::open; cups/authentication.rs
Authentication::install; print/authentication/channel.rs current/Scope.

The phase55 actual-worker reproduction confirms the new realm was requested at
Get-Jobs after a lost final reply, not during submission retry. A cache-only
protocol, thread-bound guard, same-account cache restriction, refusal/cancellation
distinction and helper/handoff checks entered validation and passed the scoped
acceptance below. See
[ADR0095](../../../docs/adr/0095-cached-credentials-during-printer-retirement.md).
Corrected-worker/public-failure acceptance is recorded below.

Phase55 acceptance:190 Print/12 Lockdown/408 Portal library/55 integration plus
PPD runtime1, formatting, strict workspace Clippy and original compile graph pass.
91 native cases/480 observed terminal processes include seven retirement faults,
four public failure cases across appearances, full authentication/ordinary controls
and reconnect guards. Exact approved scope/account can cancel; new/rejected values
retain a listable unconfirmed record without UI, false user-cancel classification
or document retry. See [evidence](../print/retirement-authentication-evidence.json)
and [ADR0095](../../../docs/adr/0095-cached-credentials-during-printer-retirement.md).
Actual PAM principal and late Create/Send gaps were separately measured and remain
required in [issue06](06-authenticated-job-ownership.md). Full auth is not done.
