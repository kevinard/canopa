# Durable owner before authenticated creation

Status: scoped verified; Send-only authentication and complete protected recovery remain open.

Actual CUPS2.4.20 PAM tests use a synthetic alias/password in private namespace
overlays. An HTTP principal takes precedence over requesting-user-name when the
scheduler creates a job. Authentication required first at Create can therefore
change its owner after Canopa has written the initial intent. A lost reply leaves
an alias-owned job but an original-user record. The account guard correctly
refuses cleanup; weakening it would hide the wrong durable authority.

Only the synchronous Create call now arms a scoped journal binding on the retained
authentication object. After a challenge is approved and before returning its
password to CUPS, the callback persists that username as the provisional owner.
The existing private ledger mutation checks the owning worker, unknown job ID and
unaccepted state, writes a fresh file and synchronizes it and its parents. Failure
returns refusal before changing CUPS's user or permitting the authenticated retry.
No password enters the journal.

The journal's in-memory intent changes only after persistence succeeds. A guard
owns shared state independently of the callback allocation; no guard borrow is
held across the foreign call. It disarms before the returned ID is recorded or
Send starts, including errors and unwinding. A later Send login cannot change the
owner of an existing job. The record schema, tag, queue, endpoint/protection,
worker authority, accepted-job protection and checked cancellation stay intact.

The real failing regression precedes this change. Four actual PAM fixture variants
now pass12 cases: normal metadata/Create/Send controls, lost creation reply, worker
death before forwarding, and refused durable mutation. The fixture inspects the
ledger before forwarding Create. A lost reply now cancels only the exactly owned
job; death leaves the correct owner with no forwarded creation/document; a refused
write prevents authenticated Create. Send-only login preserves the original owner.

192 Print tests,12 Lockdown tests,410 Portal library and55 integration tests, the
explicit PPD runtime case, formatting, strict workspace Clippy and the original
workspace compilation graph pass. All16 native fixture receipts pass103 cases,
with503 observed terminal processes. Both appearances cover Iced authentication,
public failures and ordinary printer controls, plus real GTK/explicit Qt prepared
callers and reconnect regressions. Actual masked credentials and ordinary controls
were visually inspected. The [scoped evidence](../../.scratch/xdg-portals/print/creation-owner-evidence.json)
preserves the failed baseline, four-file source snapshot and review patch. About
37MB net storage growth; no dependency, native executable or binary copy added.
No complete workspace execution is established; ten existing Files failures remain.

Send-only authentication still returns
after explicit401 without a second authenticated Send; a continuation requires a
separate design that never retries ambiguous acceptance. Explicit protected
recovery and the account/owner distinction beyond creation remain required.

See [the investigation](../../.scratch/xdg-portals/print/principal-binding-investigation.json),
[issue06](../../.scratch/xdg-portals/issues/06-authenticated-job-ownership.md),
[credential ownership](0094-request-owned-printer-authentication.md) and
[automatic cleanup](0095-cached-credentials-during-printer-retirement.md).
The primary behavior is in CUPS's [scheduler creation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/scheduler/ipp.c)
and [stream response handling](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/request.c).
Print remains opt-in and automatic Qt integration deferred.
