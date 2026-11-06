# Bind authenticated printer accounts and handle definitive Send rejection

Status: in-progress

The actual CUPS2.4.20 scheduler now runs with a disposable PAM verifier and passwd
alias in private read-only namespace overlays. Correct synthetic passwords pass;
wrong passwords return401. No host authentication files or real credentials are
used. The source-only fixtures reuse the phase55 binary and runtime stores.
See [measured investigation](../print/principal-binding-investigation.json).

Three real-worker cases distinguish account/owner behavior. Login required at
metadata or Create-Job makes the approved alias the job owner. Login first
required at Send-Document leaves the original job owner. A successful Create-only
job hides the durable mismatch because normal completion removes its record.
Dropping only that Create acknowledgement exposes it: one alias-owned held job,
one original-user intent with unknown job ID, no Send, no cleanup lookup because
the account equality guard refuses. The metadata-auth control finds and cancels
its exact tagged job using approved credentials. Do not weaken identity checks.

Keep configured endpoint/protection, authenticated account and immutable job owner
distinct. Persist the correct possible owner before a retried authenticated
Create can reach the scheduler. Fail closed if that persistence fails. After a
job exists, a Send-time login must preserve its owner. A process death or lost
reply must still leave exact tag/queue/owner authority without resubmitting an
uncertain creation or document. Legacy records must remain readable safely.

Send-only authentication has a second reproduced boundary. The worker issues one
Send and receives an explicit401. Its callback approves the alias, but no
authenticated Send follows, and the job stays incoming. The public CUPS
`cupsFinishDestDocument` calls `cupsGetResponse`; that authenticates/reconnects
on401 and returns without replaying the document. The direct raw IPP control
authenticates and succeeds, so this is neither an owner rejection nor a malformed
fixture document. An explicit rejection is different from lost acknowledgement;
any continuation must prove the rejected operation did not accept the document,
retain the same job/endpoint/owner authority and enforce finite authentication.
Never add a general retry on failure or a new Create.

Use failing regression assertions at the real worker/journal/HTTP seam before
editing. Cover metadata/Create/Send authentication, wrong/declined values,
durability failure and main death, accepted/lost/rejected replies and unrelated
jobs. Verify explicit protected review, same-account cleanup and cache-only
automatic cleanup. Keep reconnect cancellation checks, source snapshots, document
seals, driver checks and request cancellation. Add no permanent credential store.

Principal probes03/04/05 are measured evidence, not feature acceptance; the two
failed setup receipts01/02 remain failed. Source lookup confirms scheduler
`add_job` prefers HTTP principal at creation and the public response helper does
not replay its stream. These probes do not implement held-job Authenticate-Job,
Digest, longer credentials or complete protected recovery, which issue04 still
requires. Keep Print opt-in and automatic Qt integration deferred.

Replay sources: `target/portal-print-55/replay/principal.py` and
`principal-fault.py`. Pass the validated worker02 receipt through
CANOPA_PRINT_VALIDATION; select CANOPA_PRINCIPAL_FAULT=lose-create for the lost
creation case. Every observed service wrapper/worker is terminal and private PAM,
password/spool data is removed. No new native binary or Cargo graph was needed.

## Phase56 scoped result

The late Create durable-owner mismatch is corrected and verified. Guarded callback
persistence precedes authenticated Create; it disarms beforeID/Send, and failed
writes refuse authentication. The existing schema and exact cancellation checks
remain. Four actual PAM variants/12cases, the full worker/Iced/GTK/explicitQt and
reconnect regression matrix total103 native cases/503 observed terminal processes.
192Print/410Portal library/55integration/12Lockdown, PPDruntime1, strict workspace
Clippy and original compile graph pass. See [ADR0096](../../../docs/adr/0096-durable-owner-before-authenticated-creation.md)
and [evidence](../print/creation-owner-evidence.json).

Issue remains in-progress for the Send-only401 gap and protected authority after
creation. No general retry has been added. Complete protected recovery, held-job
correction and the rest of issue04 remain required. Print stays opt-in.

## Phase57 scoped result

Send-only definitive HTTP 401 continuation is corrected. Proof binds the same
exact empty incoming job before credentials; an optional UUID is durable before
continuation and permits default privacy to hide owner/name after login. The
creation owner stays immutable, visible identity conflicts fail, attempts are
finite, and lost acceptance never triggers document resubmission. Wrong-once
initially reproduced CUPS cache reuse after metadata success; fresh protected
public HTTP authentication contexts correct it.

The frozen candidate passes 195 Print checks, 413 Portal library checks serially,
55 integration checks, 12 Lockdown checks and the explicit PPD runtime case.
Strict workspace Clippy and original compilation pass. All 140 native groups
pass, with 721 observed process identities terminal. These include actual PAM
Create/Send faults, real Iced and GTK/explicit Qt callers in both appearances,
and prior driver/copy/retirement/reconnect regressions. See
[ADR0097](../../../docs/adr/0097-rejected-print-document-continuation.md) and
[scoped evidence](../print/rejected-send-evidence.json).

The original parallel handoff EAGAIN and interrupted dark UI receipts remain
failed. Serial checks and repeated native passes do not erase them. The signal
sender remains unknown. No complete workspace execution green result is claimed.

Issue stays in progress for explicit protected recovery and account/owner
authority after creation. Nonreusable connections, unavailable identity metadata
and actual Send namespace replacement need compatibility evidence before default
routing. Held-job Authenticate-Job, Digest/long credentials and the rest of issue04
remain. Print stays opt-in; automatic Qt integration remains deferred.

## Phase58 scoped protected-recovery result

Explicit Iced review now separates its approved account from the original job
owner. Unknown IDs bind exact tag/owner/queue matches and optional UUIDs before
cancellation; known durable UUIDs support default privacy after another account
logs in. Every later mutation rechecks that fixed identity. Visible conflicts,
unprovable legacy metadata and namespace changes retain uncertainty. Accepted
receipts only retire metadata, live workers remain protected, and opening review
or unattended cleanup cannot prompt. Sixteen real unknown-ID jobs retire from one
Iced approval. Wrong/declined credentials, lost Cancel acknowledgement and both
appearances pass with the earlier Print and owner regressions.

See [ADR0098](../../../docs/adr/0098-explicit-protected-printer-recovery.md) and
[scoped evidence](../print/protected-recovery-evidence.json). The full authentication
issue04 remains active. This issue remains in progress for nonreusable connections,
unavailable identity metadata and actual Send namespace compatibility. The early-
TLS abrupt-close test failed safely before login; passing repeats do not resolve
[issue08](08-early-tls-rejection-observability.md). No document-retry policy changed.
Print stays opt-in. Automatic Qt integration remains deferred.

## Phase59 scoped accepted-job preservation

A complete HTTP/IPP acknowledgement now commits before local receipt writes. An
actual catalog-fault regression with original native CUPS/cups-exec reports
Accepted and preserves the received job; after catalog repair, both automatic
and explicit recovery preserve it. Ordinary error/restart cleanup requires a
freshly proven empty incoming job. The original live request's signal/HUP can
still cancel its own received job before acknowledgement. Parent cleanup cannot
fall back to a numeric-only cancellation without durable recovery authority.

The final frozen candidate passes200 Print checks within418 serial Portal library
checks,55 integration,12 Lockdown and PPD runtime, strict workspace Clippy and the
original compilation graph. Eighteen native replay steps/70groups/244 observed
terminal identities pass, including public lost replies, cached scopes, protected
recovery and both Iced appearances. See [ADR0099](../../../docs/adr/0099-preserve-accepted-printer-jobs-after-bookkeeping-failure.md)
and [evidence](../print/accepted-job-preservation-evidence.json). This is scoped
preservation acceptance, not full authentication or all-workspace execution.

Actual late backend rejection is now measured: a raw control authenticates the
same UUID with one original document send. It uses an explicitly limited namespace
executor adapter for device credentials, and is not physical/runtime/Iced feature
acceptance. Current Canopa metadata can still report no required fields after
that hold. Implement the backend-owned Iced review and precise late-held default
requirements described in [the next implementation map](../print/held-job-next.md).
The held-job UI/action is not implemented yet. Full issue04, connection/Digest/
length/resource/device/runtime/release gates remain active; Print stays opt-in,
and automatic Qt integration remains deferred.

## Phase60 scoped live-job sign-in

The running opt-in backend now owns an Iced live printer-job review through
`--printer-jobs`, under original-caller/Request.Close/Lockdown/shutdown authority.
It lists current-user active jobs, retains source proofs privately, and sends only
CUPS-Authenticate-Job for a freshly checked received authentication-held document.
Wrong fields can hold the same job again; Cancel and withdrawal preserve it.
No accepted-job shadow database, another document upload or implicit cancellation
is added. Actual release-only PAM permits a distinct approved actor while retaining
the original job owner and handling default privacy.

The final candidate passes207 Print checks within425 serial Portal library checks,
55 integration,12 Lockdown,PPD runtime, format, strict workspace Clippy and the
original compilation graph. Sealed64 native invocations/116groups/706observed
terminal identities include both Iced appearances and previous Print/GTK/explicit
Qt/recovery/reconnect regressions. The original wrong-once HTTP regression is
retained; fresh challenge recognition now passes both loopback and TLS. Lost
acknowledgements, including HTTP200 with an unrelated authentication header,
cannot trigger another release. See [ADR0100](../../../docs/adr/0100-live-printer-job-authentication.md)
and [evidence](../print/live-printer-job-evidence.json).

This issue remains in progress. The namespace-only executor adapter establishes
protocol behavior, not physical or unmodified executor/runtime acceptance. Held
Iced device-field runs and actual-worker PAM/TLS runs have distinct scopes.
Listing follows the effective account and can omit original-owner jobs after an
arbitrary account change. Digest, longer credentials, prompt/timeout/resource and
broader connection compatibility, driver/device precision, Flatpak, runtime and
release gates remain. Existing Files/handoff/early-TLS failures stay open. Print
remains opt-in; automatic Qt integration remains deferred by the user.
