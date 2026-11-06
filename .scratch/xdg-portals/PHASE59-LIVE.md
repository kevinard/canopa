# Phase59: acceptance commit and later held-job authentication

Full18 goal active (11 packaged/six experimental/Print opt-in). User chose finish
Canopa Print features first; automatic Qt integration deferred. Previous turn was
PROGRESS: phase58 protected recovery sealed189 native groups/989 observed terminal.
No question/external blocker/new agent/commit/PR. Native fixture limitations below
are not a blocked goal: implementation and meaningful validation can continue.

## Final scoped result; next work remains active

All current tool handles TERMINAL: worker03/40442, native matrix67093 and sealer28599
exit0. No live build/native fixture or pending question. Candidate03 FROZEN6files/
127636bytes with baseline preimages verified against phase58 worker05. Sources,
review patch and replay sources retained in target59. No commit/PR/new agents.

Worker03 PASS: format3.0s, strictworkspaceClippy117.05s, original compile484.18s;
200Print/12ignored,12Lockdown,418serialPortal/20ignored,55integration/2ignored,
PPDruntime1. Guards unchanged. Binary SHA256:
8f9e3d1576055e1c90868055d680202431051b9c276bd34cf502721f7f6bea57.
Selected Portal execution is not all-workspace execution or release acceptance.

`acceptance-01.json` and print/accepted-job-preservation-evidence.json are sealed:
18 native replay steps,70groups,244 observed terminal identities. Final binary
has native ACK/catalog-repair automatic+explicit preservation; original signal
and HUP withdrawal;14worker-auth/10retirement/5reconnect; public lost final ACK in
same/new protection spaces, protected wrong-once/Cancel/window-close and existing
recovery review, all in both appearances. Original CUPS/cups-exec used for ACK and
withdrawal. No held-device feature acceptance is claimed.

Parent policy now refuses a numeric-only CancelJob after worker exit without
durable recovery. Its eight subprocess controls pass. Raw unregistered worker
cases are separately named no-registration; they never establish parent policy.
Worker01 intentionally stopped after passed lint when review found missing
capability loading in automatic Context preflight; validation-stop-01.json stays.
Retirement-worker01 failed an obsolete expectation that lost-final/doc1 cancels;
retirement02/03 correct that expectation and add three explicit empty-job controls.
No failed receipt was overwritten. Candidate02 earlier passes retain their pins.

Storage at seal:6712233984baseline bytes free,6576279552observed free (~136MB net
phase growth before final small metadata writes). Reserve6GB/growthcap3GB maintained.
No executable copies, native fixture builds, dependencies, alternate target paths
or feature/profile changes. Reuse root incremental and checksum native stores.

NEXT implement held-job sign-in through backend-owned Iced review. Read issue04,
print/held-job-next.md and the investigation below. Chosen implementation seam is
concrete but NO held production code is written. Review must live independently
of the completed Print request while honoring in-process Lockdown revisions,
original caller/Request.Close and shutdown. UI lists live accepted held jobs;
fresh action-owned credentials authenticate only the fixed UUID/owner/queue on a
checked configured connection using0x400e, never Create/Send/implicit Cancel.
No arbitrary post-ack polling window or accepted-job shadow database. Add a real
user entry through opt-in backend; standalone cleanup cannot bypass printing policy.
CUPS late-none requires deliberate default username/password handling despite
current metadata none; raw protocol evidence has an explicit namespace-only
executor adapter. Other authentication/compatibility/runtime/release gates and
six experimental interfaces remain. Full18 goal active; Print-first, Qt deferred.

## Verified source changes

Baseline4files verified against phase58 worker05 source guards before editing;
sources/baseline and baseline-sources.json. Candidate01 sources/patch captured
AFTER cargo fmt. No Cargo/vendor/dependency/profile/route/API-advertisement change.
- worker.rs Cancellation::withdrawn recognizes actual IN/HUP only; poll failure
  does not grant original-request withdrawal authority.
- cups.rs PartialJob records ephemeral withdrawn bool only when send fails and
  actual cancel channel is signalled. Complete send HTTP/IPP success commits in
  memory BEFORE journal.accepted. Bookkeeping error warns/keeps metadata, still
  returns Accepted. No automatic cancellation or false Failed after known success.
- cups/recovery.rs new CancellationAuthority: EmptyIncoming for ordinary failure
  and all recovery, WithdrawnRequest only for live failed submission with true
  pipe withdrawal. Before cancellation on EmptyIncoming, inspect exact job again
  on current session and verify empty incoming with bound UUID when available.
  Received or unreadable metadata remains unresolved. Terminal states can retire
  metadata. Accepted receipts keep existing metadata-only handling. Cache-only
  automatic credentials and connection supervision remain.
- cups/identity.rs verify_incomplete checks receipt not acknowledged, unchanged
  identity, state4/documents0/job-incoming. New regression distinguishes matching
  received jobs from cancellation authority; hidden UUID-bound fields still work.
- client.rs no longer turns a Created job number into a separate CancelJob after
  worker exit. Without a durable recovery binding, preserve uncertainty. Original
  live worker withdrawal still has its existing authority. A new subprocess test
  checks eight registered/unregistered terminal paths (not scheduler acceptance).
ADR0099, context map, Print guide and issues04/06 now record scoped validation.

## Actual red acknowledgement regression

`native-acknowledgement-probe01/summary.json` is real unmodified unprivileged CUPS
AND original checksum-store cups-exec (no adapter). Final HTTP200/IPP0 fully
forwarded to current old worker; a second hard link on private catalog makes its
post-ack write fail. Worker emits Created2 then Failed "Invalid file destination";
its error destructor issues Cancel8; actual job2 becomes7/documents1. Unrelated
incoming job1 preserved; no second Create/Send. All observed processes terminal,
private auth/spool removed. This is the pre-edit regression, not acceptance.
`acknowledgement-probe01` also reproduces under namespace-root test executor;
unmodified native control independently establishes product bug. Actual stdout,
source scripts and relay operation/status receipts retained. New corrected test
adds recovery-after-repair checks. withdrawal.py is positive control, not generic
retry. New operation still requires known ack to report Accepted; lost reply stays
uncertain and no document resubmission is permitted.

## Held-device investigation (no implementation yet)

`held-probe06/07` both reach intended baseline red AFTER raw controlPASS. Current
phase58 worker Created/Accepted exits, normal journal empty; backend is gated until
AFTER worker exit, then exits2; real CUPS job4 reasoncups-held-for-authentication,
documents1. Raw0x400e wrong user/password fields reaches backend and reholds; correct
fields completes SAME UUID/state9/documents1. Exactly1workerCreate,1workerSend,
1backenddocumentread/acceptance,7248bytes/sourceSHA. Unrelated incoming job1 stays4.
07 also runs existing Canopa capabilities AFTER hold: authentication.kind=none.
GetPrinterAttributes and GetPrinters omit auth-info-required, yet backend's real
AUTH_INFO_REQUIRED changes fromnone to username,password. Future held UI cannot
skip this case merely because initial/current printer metadata says none.

Raw-control fidelity is explicit: actual unmodified CUPS scheduler runs UID0 only
inside unprivileged single-ID user namespace, host root read-only, only private
temp writable; namespace DAC/FOWNER/CHOWN caps needed for updating CUPS's0400 cache.
No host root/config change. Native cups-exec cannot setgroups in this namespace
(kernel denies it; no subuid/subgid mappings or setuid map helpers available).
Source-only namespace-exec.py adapter only execs owned device-backend.py as already
selected UID/GID0, validates exact single-ID map/backend path/perms; cannot run as
normal host user. This is protocol evidence, NOT unmodified-executor/runtime/Iced/
physical acceptance. The acknowledgement regression needs no adapter at all.

Earlier held probe failures retained:
01 expected literal none but CUPS omits attribute (primary printers.c confirms).
02 rootless CUPS acknowledges auth yet intentionally skips credential cache when
RunUser!=0 (primary ipp.c save_auth_info); backend receives no fields.
03 namespace-root Group/SystemGroup conflict; no product/native acceptance.
04 kernel-denied setgroups in native cups-exec exits101 before backend.
05 adapter works but namespace lacks DAC override: CUPS cannot rewrite0400 auth
cache after wrong password, so correct retry still sees old password. Actual
error confirms; scoped namespace caps correct this in06/07. All private data removed.
Do not rewrite these as passing or call them Canopa authentication bugs.

## Next held-job design constraints

CUPS owns accepted queued jobs after a Print request finishes. A local recovery
journal is for uncertain outcomes; its empty baseline assertion LOCATES missing
follow-up, not a requirement to duplicate all accepted jobs in a new database.
Prefer explicit Iced review of live CUPS held jobs with fixed pre-auth identity,
independent of the completed Print request; keep existing accepted-receipt cleanup
semantics apart from received-job protection above. Do not solve late rejection
with an arbitrary short polling window. Need actual Iced entry/action, not just a
raw helper or manual workaround; full issue04 stays active until integrated.

Owner/backend/Lockdown lifetime needs a concrete interface before adding UI:
releasing a held job is printing, unlike recovery cancellation. A standalone CLI
must not bypass live disable-printing policy or withdrawal. Backend-owned review
or equivalent propagated live policy should own new action and prompt. User
selection must bind source owner/name/queue/UUID, configured peer/protection and
fresh same-connection proof before mutation; HTTP actor never rewrites job owner.
A generic title+numeric ID is insufficient across scheduler replacement. Require
UUID or equally strong unique Canopa-tag proof; hidden legacy identity remains
unavailable. Credentials fresh per action, private channel, finite attempts;
actual CUPS late-none -> defaultusername/password behavior needs deliberate handling
and tests. HTTP credentials and ordered device fields are separate scopes.

New authenticate path must only issue0x400e on same accepted job, NEVER Create,
Send or implicit Cancel. HTTP401 can use deferral/fresh protected session outside
mutation guard (phase58 cached-password fix), then repeat identity/requirements
checks. Generic lost ack/reconnect cannot authorize a repeat. Authentication ACK
means credentials submitted/release requested, not physical completion. UI can
refresh live status; wrong/Cancel/Close/caller/Lockdown/shutdown, changed identities/
requirements, cached scopes, namespace and lost ack need real tests. No production
held-job design or code selected/implemented yet. Read issue04 and held-job-next.md.

## Evidence, tools and storage

Authoritative `target/portal-print-59/investigation-01.json` and
`.scratch/xdg-portals/print/held-job-investigation.json` index facts/limits with hashes.
/tmp/canopa-print-59 has held.py, acknowledgement.py, acknowledgement-native.py,
withdrawal.py, device-backend.py, namespace-exec.py and validate.py. Scripts copied
into individual receipts; no executable copies, native builds or new graph. For
rootless ACK/withdrawal scripts backend mode755; held root-namespace script mode500;
DO NOT run these modes concurrently (shared stable backend path). Native run needs
escalated private sockets/proc; already authorized via task, no user question.

Phase baseline6712233984bytes free; reserve6GB/growthcap3GB. Latest free~6.709GB
before build. No cleanup/new dependency/native compiled fixture/binary copy. Reuse
root debug incremental/cache and checksum compositor/runtime stores. Main user
goal18 remains active; Print-first, Qt deferred. Phase58 earlyTLS issue08 and
orchestration143/phase57 handoff/tenFiles failures remain; no workspacegreenclaim.
