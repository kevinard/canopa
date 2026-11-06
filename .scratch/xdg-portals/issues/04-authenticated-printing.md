# Complete authenticated printing through Iced

Status: in-progress

Finish the authentication behavior required for Canopa Print. Phase53's worker
substrate is necessary work, not acceptance of the complete feature. Keep the
full portal objective and the user's Print-first/Qt-deferred priority intact.

The worker now has a separate credential stream, typed scheduler/queue challenges,
refusal/cancellation handling, explicit non-console CUPS callbacks, peer/protection
checks, ordered queue requirements and literal IPP authentication fields. Its fourteen actual-worker cases pass against the current build. This provides
no Iced or protected recovery acceptance. ADR0094 and PHASE53-LIVE locate
source/API evidence and the currently active validation handle.

The Iced provider, request cache and private sealed-memory prepared handoff now
pass scoped phase54 native checks in both appearances, including actual GTK and
explicit Qt callers. See [Iced evidence](../print/iced-authentication-evidence.json).
Complete authentication still requires the remaining account/recovery/held-job/
Digest/length and resource gates below. Use
request-owned credentials with explicit consent and a private retained handoff for
prepared printing. Do not place secrets in Grant JSON, public settings, argv,
ordinary stdout/stderr or recovery records. Bind credentials to the live request,
configured endpoint/protection space or queue/ordered requirement set; do not
replay them to a changed endpoint/realm/queue or downgrade. Rejected credentials
must not create an endless prompt loop. File selection must not retain unrelated
printer secrets. Preserve all current file, layout, copy and driver authority.

Include HTTP Digest alongside Basic/TLS, and distinguish bounded interactive
prompts from bounded credential exchanges for multi-record recovery. A cached
credential reply is not another user prompt. Verify authentication required only
at Create-Job or Send-Document, not just metadata. Measure how a callback changing
the CUPS account affects the job owner and durable intent. Record the correct
owner before an uncertain creation can occur, preserve it after creation, and
revalidate protected recovery without silently switching accounts or resending
an uncertain job. The phase53 native worker fixture has no recovery registration;
it cannot establish those guarantees.

The ordinary worker deadline is60s; authenticated work has a300s budget, the Iced
prompt90s and each credential exchange120s. Preserve these finite budgets and
prompt cancellation and resource retirement. Test direct and prepared flows,
metadata and submission authentication, original caller loss, Request.Close,
Lockdown, shutdown, wrong values, malformed/private-channel replies and changed
requirements before Create. Native acceptance must use actual Iced controls in
both appearances and actual frontend callers, including the existing GTK and
explicit Qt probes. Automatic Qt routing remains deferred.

CUPS2.4.20 Basic auth truncates a190+byte username:password value; the prototype
proves189 preserved/190,191,192 rejected. Current worker explicitly limits the
combined value rather than changing its secret. This is not full length parity.
Investigate a public authenticated transport path or maintained runtime correction
and verify long ASCII/UTF-8 credentials end to end. Never patch/install system
CUPS implicitly, bypass selected drivers or retry an uncertain print operation.

Per-job auth-info must preserve the server's ordered domain/username/password
fields, including punctuation and single-field passwords. Actual protected device
rejection may occur after a scheduler accepts a job. Implement identity-checked
CUPS-Authenticate-Job for the existing held job, with current credentials and
fresh source/server identity. Do not resubmit or duplicate its document. Verify
protected cancellation/recovery and preserve uncertain/accepted records correctly.

Keep validation storage bounded and reuse the unchanged workspace selection,
root incremental cache and checksum native stores. Use disposable credentials and
private services; keep concise nonsecret comparisons and terminal-process receipts.
Broader driverless/physical/sandbox, failure, packaging and full release gates,
six experimental interfaces and the ten existing Files failures remain open.

Synchronous partial-job retirement now uses exact approved credentials without
new prompts; issue05 is scoped verified. The full feature remains in progress.
Actual PAM namespace probes reproduce late Create owner mismatch after a lost
acknowledgement and Send-only explicit401 without an authenticated send. Preserve
those fixtures and the Create boundary is now scoped verified. Send-only
continuation is scoped verified by phase57. Finish protected recovery acceptance
and remaining connection compatibility;
see [issue06](06-authenticated-job-ownership.md) and
[measured investigation](../print/principal-binding-investigation.json).

## Phase58 scoped protected-recovery result

Explicit Check and clean up now authenticates through Iced with a fresh broker for
that action, separate from unattended cache-only retirement. Approved HTTP account
and immutable job owner remain distinct; fixed job proofs survive privacy changes
and are revalidated before mutation. Wrong-once, third-correct, declined values,
TLS, known/unknown IDs, legacy metadata, lost Cancel acknowledgement and a distinct
scheduler namespace control pass. A real sixteen-record batch uses one approval.
Accepted and live jobs remain protected. No recovery operation creates a job or
resends a document. Manual recovery preserves the selected appearance in its sign-
in helper. Both appearances, actual frontend callers and prior Print regressions
pass; see [ADR0098](../../../docs/adr/0098-explicit-protected-printer-recovery.md)
and [evidence](../print/protected-recovery-evidence.json).

416 Portal library checks (including 198 Print and 12 Lockdown),55 integration,PPD runtime,
strict workspace Clippy and original compile pass. The index records 189 native
groups and 989 observed terminal identities with explicit source-continuity and binary
pins, rather than rerunning the unchanged worker matrix after a main-only theme
fix. Failed and interrupted receipts remain. Early TLS observability issue08,
phase57 handoff timing and ten Files failures prevent a broad all-green claim.

This issue stays in progress. Next is actual post-acceptance device rejection and
identity-checked CUPS-Authenticate-Job, without another document send. Preparation
is [held-job-next.md](../print/held-job-next.md); no implementation is claimed.
Digest, long credentials, resource/timeouts/file-output credential lifetime,
connection compatibility and broader device/runtime/release gates remain. Print
stays opt-in and automatic Qt integration remains deferred.

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

## Phase61 scoped client-library compatibility result

The optional pinned CUPS client overlay corrects measured Digest default-algorithm,
legacy opaque, negotiated qop and ignored DigestOptions defects. All 1301 sources,
LICENSE/NOTICE and the library checksum are verified. Thirteen actual-worker
invocations cover 52 cases; 54 separate public-API capacity comparisons reproduce
baseline limits and preserve candidate 127-byte usernames/1023-byte passwords.
All 120 observed identities are terminal; private auth data removed. See
[ADR0101](../../../docs/adr/0101-process-private-print-authentication-runtime.md)
and [evidence](../print/authentication-runtime-evidence.json). Normal backend
selection and Rust's 189-byte combined cap remain unchanged. This is a verified
client candidate, not production long-credential or Iced authentication acceptance.

The workspace/default/all-target/no-fail-fast run passes Portal425/55 and other
executables except ten top-level Files integration failures. Tooling, strict
Clippy, settings/configuration and vendor runtime checks pass. The generated
inventory needed only its stale lockfile-hash header regenerated (650 rows same).
No Rust source/manifests/lockfile changed in this phase, no new Cargo graph, about
43 MB net storage growth. Failed/invalid fixture and baseline receipts are kept.

This issue stays in progress. Next is worker-only runtime selection with a
verified capability, typed private limits and cache/prepared binding; legacy
usernames above 64 bytes must be rejected before truncation. Continue actual
Rust long credentials, Iced/frontend both appearances, protected metadata/Create/
Send/recovery/held and broader Print acceptance using the selected runtime.
Print remains opt-in; automatic Qt remains deferred by the user.

## Phase62 scoped runtime and credential-capacity acceptance

The backend freezes one optional sealed CUPS client image and passes it to its
metadata UI and CUPS workers. Only standalone workers load it; credential helpers
receive no image descriptor. The canonical client exports a versioned capability.
Recognized version1 supports 127-byte users/1023-byte passwords (1151 combined),
with exact UTF-8 byte validation and no truncation. System fallback now refuses
usernames above 64 bytes as well as combined values above 189 bytes. Private policy
is part of exact server-scope equality and sealed prepared handoff; public Print
options cannot increase it. Missing/unknown capabilities fail before authentication.

435 serial Portal library/55 integration/PPD checks, strict workspace Clippy and the
original compilation graph pass. Actual worker120 cases cover selected Basic/MD5/
SHA256 and conservative fallback. Four complete Iced/public runs cover Basic/SHA256
in both appearances, GTK/explicitQt six-page prepared jobs, direct/prepared cache,
wrong-once and cancellation/withdrawal; two reduced captures verify masked long
fields after paste redraw. Seventy isolation observations and a separate actual
parent/source-replacement handoff check pass. All808 observed identities terminal;
private auth data removed. See [ADR0101](../../../docs/adr/0101-process-private-print-authentication-runtime.md)
and [integrated evidence](../print/selected-authentication-runtime-evidence.json).

The original full workspace/no-fail-fast run still fails only the same exact ten
Files integration cases as phase61; other executables and independent post-Cargo
settings/configuration/CLI/vendor-runtime checks pass. Compiler/Clippy setup errors,
restricted-socket test run and long-keyboard injector timeout remain recorded.
The latter was a fixture-input failure; owned single-paste sources on private
nested desktops verify the actual long-password flow. No new Cargo dependency
or feature graph/cache cleanup; binaries archived once by lossless checksum.

This issue remains in progress. Next exercise selected-runtime protected Create/
Send-only, explicit recovery, same-UUID held jobs and lost-acknowledgement/source
regressions. Then finish resource/timeouts/file-output credential lifetime and
remaining connection, driver/device, Flatpak, runtime/package/release gates.
The private Digest gateway is not actual Digest/PAM or physical acceptance.
Print stays opt-in; automatic Qt integration remains deferred by the user.

## Phase63 selected protected-operation acceptance

The unchanged phase62 Rust binary/client now pass49 additional native invocations:
Create-only/Send-only, retries/TLS/lost replies, protected recovery with a distinct
approved actor, sameUUID received held-job release, cancellation/denial and changed
source/owner/queue/server/requirements/replacement controls.122 raw workers map the
explicit sealed candidate;73 private challenges verify its capacity. Ten actual
Iced invocations cover both appearances and140 held-review command steps, including
caller/Request.Close/window loss, Lockdown, shutdown/restart and wrong/correct
fields.24 production observations keep CUPS out of parent/credential UI and its
image out of credential helpers.431 observed identities terminal/private cleanup;
both recovery and held captures visually inspected. See
[evidence](../print/selected-protected-print-evidence.json) and
[ADR0101](../../../docs/adr/0101-process-private-print-authentication-runtime.md).

Digest MD5/SHA256 is independently verified before a valid actor is forwarded to
actual CUPS Basic/PAM; native cupsd Digest/PAM is not claimed. The device executor
adapter remains namespace/protocol-only. Original fixture backend-path mismatch
and Basic-only negative PAM assertion failures are retained; neither required a
product change. No Rust/vendor edit, build, new graph/ELF/binary copy/cache purge or
workspace repeat. The435/55/PPD/strictClippy/compile and exact ten Files failures
remain the unchanged source-verified baseline. Net storage growth about12MB.

This issue stays in progress. Next is actual prompt/deadline/resource and file-
output credential lifetime, then broader connection, driver/device, Flatpak,
runtime/package/NOTICE/release gates. See [the resource continuation](../print/authentication-resources-next.md).
The earlier early-TLS and parallel-handoff issues remain. Full18 active, Print
opt-in and automatic Qt integration deferred by the user.


## Phase64 scoped prompt, channel and file-output lifetime result

The real idle Iced helper retires within its90-second budget. Seven private reply
cases cover empty/partial stalls at120 seconds, partial EOF and cancellation;
workers terminate while uncertain and unrelated jobs and records remain intact.
No document is resent. Actual public frontend/Iced transitions in both appearances
withdraw an in-flight metadata prompt, require new consent after file selection
clears approved printer credentials, and deliver a prepared four-page PDF without
another printer request. Existing compiled storage/admission checks pass with the
selected runtime retained; a controlled exit-confirmation barrier retains both
slots until release. Seven observed non-CUPS workers get no CUPS image or mapping.

See [resource evidence](../print/authentication-resource-evidence.json) and
[phase64](../PHASE64-LIVE.md).23 selected raw workers,15 private challenges, seven
production isolation observations and114 unique observed terminal PIDs are recorded.
Private auth data is removed. Interrupted receipts and a thread-aware observer
continuation are retained; no product fix was needed.1305 inputs and the prior
435/55/PPD/Clippy/default compile remain unchanged. About7.55MB growth, no rebuild.

This issue remains in progress. Outer60/300-second supervision, a post-approval
incomplete private reply investigation, broader connection/credential compatibility
and repeated authenticated descriptor retirement remain in the
[next map](../print/authentication-connection-next.md). The latter private-reply
scope is not a reproduced defect. File tests prove fresh-consent and I/O behavior,
not allocator-wide erasure; prompt-budget/epoch checks are existing unit evidence.
Driverless/device, Flatpak, optional runtime packaging/NOTICE, earlier handoff/TLS,
ten Files failures and release gates remain. Print stays opt-in and automatic Qt
routing remains deferred.


## Phase65 private reply lifetime correction

A controlled helper reproduces a timeout gap after ordinary approval and helper
exit: a retained incomplete private reply keeps the real parent pending beyond
90 seconds. The prompt now bounds approval, helper exit, private length/body and
EOF with one90-second deadline and caller lifetime. The same reproducer completes
at90.2 seconds while the controlled writer is still live. Two new regression tests
cover six postapproval withdrawal/caller-loss cases and successful full reply
validation. This is a faulty-helper boundary result, not a claim that normal Iced
forks or normally emits incomplete credentials.

437 Portal library/55 integration/PPD1, strict workspace Clippy and compilation
with the original default workspace graph pass. Four real Iced invocations cover
file transitions/prepared4pagePDF delivery and wrong-once TLS/PAM protected recovery
in both appearances. All94 unique observed candidate PIDs are terminal; private
auth data is removed. Full workspace execution was not repeated; the exact ten
Files failures remain open. Net storage at seal is about78MB lower than at start,
with no new feature graph, per-evidence binary copies or cache purge.

See [ADR0102](../../../docs/adr/0102-print-authentication-reply-lifetime.md),
[issue09](09-print-authentication-reply-lifetime.md) and
[evidence](../print/authentication-reply-lifetime-evidence.json). The parent issue
and full18 goal remain active. Continue the unfinished worker-supervision,
connection and runtime/package gates in the [next map](../print/authentication-connection-next.md).

## Phase66 quoted authentication compatibility

The previous CUPS image fails 30 of 48 independently checked positive quoted-field
cases. A canonical seven-file overlay fixes challenge decoding and Digest header
escaping while preserving original hash bytes and bounded capacities. The same
48 cases pass. The maintained tool passes 77 cases over loopback HTTP and 77 over
private TLS; actual quoted SHA-256 Iced file flows pass in both appearances.
Quoted-account Create and TLS Send-only wrong-password correction pass through
the independent Digest-to-Basic/PAM gateway. All 343 observed candidate PIDs are
terminal and private authentication data is removed.

The required original-graph workspace check passes Portal 437/55 but retains the
ten existing Files integration failures and one video-playback library failure.
That video case passes its isolated compiled retry; no cause or fix is claimed.
Settings/CLI/configuration/vendor post-stages and authentication self-tests pass
separately. See [ADR0103](../../../docs/adr/0103-quoted-printer-authentication-fields.md),
[issue10](10-quoted-print-authentication.md) and
[evidence](../print/quoted-authentication-evidence.json). Print remains opt-in;
the parent issue and full portal goal remain active. Continue the existing
supervision, resource, packaging and device gates in the
[next map](../print/authentication-connection-next.md).

## Phase67 private runtime packaging

The release packager now accepts the verified CUPS build receipt and includes the
private client with original LICENSE/NOTICE, complete patch, modification summary
and checksum manifest. Extracted bundles are checked by the ordinary smoke script.
Stale/incomplete builds, altered files and partial/symlink runtime contents refuse
packaging or checking. Twelve actual metadata-worker cases pass on the staged
image; a full archive of all eight existing debug executables passes smoke checks.

The relocated normal backend discovers the bundled client without an environment
override. Three actual frontend/Iced quoted SHA-256 consent/file groups pass, with
30 observed process identities terminal and private data removed. No executable
data copy is needed for that prefix check, and the duplicate full test archive is
removed after checksum verification. Rust/client/Cargo inputs are unchanged.

See [ADR0104](../../../docs/adr/0104-packaged-private-print-runtime.md) and
[evidence](../print/packaged-runtime-evidence.json). Runtime packaging/NOTICE is
locally verified; the configured Ubuntu release workflow and deployed desktop
remain unexecuted here. Outer supervision/resource, driverless/device/Flatpak and
release acceptance remain. Print stays opt-in, Qt routing stays deferred, and the
parent issue and full portal goal stay active.

## Phase68 parent supervision and authenticated descriptor retirement

The real public Client/Broker/Dialog supervisor passes controlled native checks
with its unchanged production timers. Ordinary cooperative/uncooperative replies
arrive at60.03/70.03seconds; authenticated replies at300.03/310.04seconds. The
children observe cancellation EOF at the appropriate60/300-second boundary. The
uncooperative cases exercise the actual10-second stop grace and forced retirement.
No prompt is active in those deadline cases, so90/120-second private-channel
budgets cannot explain their completion.

One retained parent passes42 rounds across success/two cached credential exchanges,
pending-prompt cancellation, dropped futures, malformed private replies and helper
failure, followed by ordinary work. Every round returns to11descriptors, one
retained image and no children. The baseline is specific to this process. All114
unique candidate observed identities are terminal and private data is removed.
Controlled children verify sealed-image bytes but do not load CUPS or mutate jobs;
actual scheduler/Iced scopes remain separate. An initial sandbox socket denial is
retained as failed environment evidence; the unchanged fixture passes with native
socket access.

No application Rust, Cargo graph or canonical runtime changes were needed. One
17MB native fixture links the existing original workspace artifacts; its obsolete
raw executable is losslessly archived by checksum. See
[the supervision guide](../../../docs/print-supervision.md) and
[evidence](../print/supervision-evidence.json). Parent deadline/FD checks are
resolved; existing handoff/TLS, driverless/device/Flatpak and release/deployment
gates remain. Print stays opt-in and the full portal goal remains active.

## Phase69 staging stream completion

A controlled CLOEXEC fork copy reproduces staging WouldBlock through the actual
normal file setup worker. The closed-copy control passes; explicit completion of
the unchanged producer stream also passes with copies still live. The maintained
caller-seam regression is red before the fix. PrepareFile now shuts down its write
half after sending the complete descriptor bundle. Receiver validation remains
strict, with no waits or sleeps. Unit fixtures retain a live CLOEXEC clone and
assert specific errors for open writers, extra packets and another allocation.

The regression and closed-copy native control each pass six groups after the fix;
the original debugger perturbation also passes. Default-parallel Portal437,
integration55/PPD1, strict workspace Clippy/original graph build pass. Public
frontend/Iced file output passes three groups per appearance, prepared four-page
PDF delivery and no printer job. All96 observed candidate identities are terminal
and private data is removed. The exact historical phase57 fork remains unknown.

See [ADR0105](../../../docs/adr/0105-print-staging-stream-completion.md),
[issue07 resolution](07-staging-handoff-test-timing.md) and
[evidence](../print/staging-completion-evidence.json). One old unique harness is
archived by checksum; no native executable/cache graph or canonical runtime change
is added. Full workspace execution is not repeated for this local Portal change;
prior Files failures remain. Early TLS, device/driverless/Flatpak and deployment
gates remain open, as does the full portal goal. Print stays opt-in/Qt deferred.
