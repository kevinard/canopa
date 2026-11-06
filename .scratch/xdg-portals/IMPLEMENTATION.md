## Phase56: durable authenticated Create owner scoped verified

Evidence retention: see [the cleanup record](RETENTION.md) for retained receipts,
compacted source inventories and removed historical local outputs.

Full18 goal ACTIVE:11 packaged/six experimental/Print opt-in. User chose Canopa
Print features first; automatic Qt integration remains deferred. No required input
or external blocker. No live builds/native fixtures remain. All toolhandles from
this phase, including16012/95607/6612, are terminal; never restart old checks merely
because a tool session is unavailable. Full goal/full authentication are NOT done.

Corrected late Create login ownership. Only Create arms a journal binding. Before
CUPS can retry with an approved username, callback persists that provisional owner;
failed writes refuse authentication. Guard disarms beforeID/Send so existing owner
stays fixed. No schema, dependency, driver, runtime profile, route or installed
printer/service change. Four Rust files/70252bytes, source candidate01, complete
300-line review patch with preimages verified against phase55 hashes. No source
changes since that snapshot. Binary SHA256:
d0273396ab98dbcc7b162a51480af69b526eb41529f259ac58f769b6ea54f440.

The original actual-PAM lost-Create regression remains RED in principal-probe01.
Corrected actual private PAM/passwd overlays use a synthetic alias/password,
never host auth files/real credentials. Four variants/12cases verify ownerdurable
and unknownID BEFORE Create forwarding, exact cleanup afterlostreply, retained
correct owner on workerdeath, no authenticatedCreate after refused journal write,
and unchanged original owner for Send-time login. All28observed fixture processes
terminal and private PAM/password/spool data removed. Send-only401 remains a known
failure, explicitly excluded from feature completion; no document retry added.

Scoped evidence: print/creation-owner-evidence.json and
`target/portal-print-56/acceptance-01.json`. Sixteen fixture receipts total103 native
cases/503 observed terminal processes. Includes12actualPAM,7retirement,14authworker,
27controls,5reconnect,4publicIcedfailures,11authentication and6ordinaryprint groups
perappearance. Bothdark100/light125, actualGTK/explicitQt prepared callers, wrong/
retry/Cancel/Close/callerloss/Lockdown/shutdown, driver/raster regressions pass.
Actual masked auth and ordinary controls visually inspected in bothappearances.
No debugger/externalSIGTERM failure in this phase. Historical failed evidence stays.

Validation01 PASS: format3.00s,strictworkspaceClippy189.06s, original workspace/
default/alltarget/offline/incremental/jobs1 graph653.24s.192Print/12ignored,
12Lockdown,410Portal library/20ignored,55integration/2ignored,PPDruntime1 pass.
Source guards unchanged. Same rootcache and checksum native stores; no new native
binary, binary copy or dependency. Netphasegrowth37023744bytes, final6717132800free,
6GBreserve/3GBphasecap retained. Phase55 had gained184041472bytes off its baseline.
No complete workspace execution; ten existing Files integration failures remain.

ADR0096 scopedverified; ADR0095 phase55 retirement scopedverified(91cases/480terminal),
its source/binary evidence preserved. README/devguide/issue04/issue06 updated with
current limits. Source-only replay wrappers, validator, sequence and guarded sealer
are preserved in `target/portal-print-56/replay/`; current /tmp56 copies also exist.
No commit/PR or goal status change. Issue06 remains in-progress for Send and protected
account/owner authority aftercreation. Do not claim full authentication or defaultPrint.

NEXT: actual Send-only authentication and protected recovery. Source confirmed
cupsFinishDestDocument/cupsGetResponse performs authentication/reconnect after401
but never replays stream. Direct authenticated rawIPP Send succeeds; worker sends
one unauthenticatedSend, receives401, approves callback, thenfails and keepsincoming
job/record. This is definitive rejection, distinct from lostfinal acceptance.
No general retry; continuation needs finite attempts, sameexisting job/tag/queue/
immutableowner/endpoint, fresh identity and checkedconnection. Current Server.user
still combines recordedowner/expectedaccount; Create correction does NOT solve
this aftercreation. my_jobs=1 filtersbycurrentauthaccount, so blind reuse of recovery
lookup for differentactor/owner is insufficient. Unattended startup also refuses
aliasowner before UI if configuredOSuser differs; explicit protected review still
needs an authorized account path. Preserve cache-only automatic cleanup, accepted
jobs, caller/Lockdown/Close withdrawal and reconnect cancellation guard.

Suggested next probes: raw actualPAM GetJobAttributes/numberofdocuments afterSend401,
owner vs authactor visibility/refusal, then bounded continuation only afterexplicit
rejection with identity revalidation. TLS handshake may alter final HTTPstatus;
httpReconnect2 preserves401 on successful plain reconnect, not a complete TLS proof.
Capture real protocol facts before a transport redesign. Helddeviceauthentication
occurs afterscheduleracceptance and needs CUPS-Authenticate-Job on samejob, neverSend
again. Digest/longcredentials, native timeout/stalledhelper/fileoutputcache clearing,
physical/driverless/Flatpak/deviceprecision, crash/stalledFS/resources/runtime/default
Printpackaging remain. Sixexperimental companions and workspace/native/release/
tenFiles gates remain. Do not spawn subagents unless an applicable explicit rule asks.

## Phase55: automatic cleanup authentication scoped verified

The full18-interface goal remains active:11 packaged, six experimental, Print
opt-in. The user chose Canopa Print features first; automatic Qt integration is
deferred. No required user input or external blocker.

Automatic synchronous retirement now permits only exact approved credentials for
the checked account. New/rejected scopes cannot open another password dialog, use
prompt budget or become a user cancellation. Same-scope cleanup can still cancel
the exact owned job. Uncertain records stay available for review, with no document
retry and accepted/unrelated jobs preserved. See [ADR0095](../../docs/adr/0095-cached-credentials-during-printer-retirement.md)
and [scoped evidence](print/retirement-authentication-evidence.json).

The final source snapshot is11files/99014bytes; binary SHA256
26939bdff8a14aa8db6f46fb405098d8b1a7c8481ceff20308db91bae8d69e12.
Format, strict workspace Clippy and the original workspace/default/all-target/
offline/incremental/jobs1 compile graph pass.190 Print/12 Lockdown/408 Portal
library/55 integration plus explicit PPD runtime1 pass. Twelve fixture receipts
cover91 native cases and480 observed terminal processes, including both Iced
appearances, real GTK and explicit Qt callers and reconnect guards. Startup has
no GTK/CUPS/qpdf link. No new native binary, binary copy, dependency, runtime
profile or route. Phase free space gained184041472bytes;6GB reserve retained.
No complete workspace execution; ten existing Files integration failures remain.

The corrected real-worker seven-case matrix and four public Iced failure cases
pass. Full authentication11groups and ordinary printer6groups per appearance,
authworker14, controls27 and reconnect5 pass. All native handles are terminal;
no validation is running. The original red failure and intentionally stopped
candidate01 validation remain preserved. Native applications did not require a
debugger in this phase; prior external SIGTERM cause remains unknown.

Next is [actual account/job ownership](issues/06-authenticated-job-ownership.md).
Private read-only PAM/passwd namespace overlays now measure real scheduler
principal binding using a synthetic alias/password, with no host auth changes.
Metadata/Create auth changes job owner; Send-only auth preserves the original.
Lost Create acknowledgement after late login reproduces a wrong-owner durable
record and leaves the job held. Send-only worker gets explicit401, invokes its
credential callback and never issues authenticated Send. Raw authenticated IPP
control succeeds. CUPS source confirms Finish/GetResponse does not replay the
stream; no general retry may be added for uncertain acceptance. See
[measured investigation](print/principal-binding-investigation.json).

No fix for those two gaps is implemented yet. Build regression assertions from
principal-probe03/04/05 before editing. Replay sources are preserved under
`target/portal-print-55/replay/`; no live private services remain. The two earlier
setup failures remain failed. Separate owner from account authority, persist
before authenticated Create, freeze owner after creation, preserve legacy records
and exact checked cancellation. Finish explicit protected recovery and held-job
Authenticate-Job without document resubmission; Digest/long credentials and
native timeout/stall/file-output credential clearing remain required too.

Full Print still needs broader physical/driverless/Flatpak/device precision,
failure/resource/runtime/default packaging. Six experimental companions and final
workspace/native/release gates remain. No commit/PR or goal completion/pause/block.

## Phase54: Iced authentication scoped verified; protected recovery next

Full18 goal ACTIVE: eleven packaged interfaces, six experimental and Print opt-in.
User priority remains Canopa Print features; automatic Qt integration deferred.
Previous goal turn was progress. No live validation/native/debugger handles,
pending input or external blocker. Do not mark the complete goal or complete
printer authentication done.

Implemented one shared Iced authentication helper for metadata and submission,
explicit masked input and endpoint/transport/realm or exact queue/field-order
consent. Credentials use private binary sockets; the ordinary helper reply is a
bool. Request-local cache invalidates rejected credentials, preserves the edited
username, and separates six prompts from64 bounded cached exchanges. File
selection clears the cache and rejects late answers. Prepared printing transfers
a bounded sealed anonymous memfd over the private approval socket. Grant/settings,
argv, ordinary output and recovery records contain no passwords. Existing token
owner/expiry rules retain the private approval. Outer supervisor cleanup has no
provider; synchronous partial-job retirement still needs explicit suppression of
interactive prompts and protected-fault acceptance (issue05).
Interactive operation/prompt/channel bounds are300/90/120s. Completion view and
message-debug regressions cover cleared values and keyboard text redaction.

Validation03 passed:184Print/12ignored,12Lockdown,402Portal library/20ignored,
55integration/2ignored and explicitPPD1. Formatting, strict workspace Clippy152.06s
and original workspace/default/alltarget/offline/incremental/jobs1 build751.55s
passed. No full workspace test execution; ten existing Files failures remain.
Candidate01:22files/261605bytes. Binary SHA256:
b6a6c7b4c9722f8f416110cefeb0f001f8f3a1d1712e65a78e4887e8cf2de68c.

Final native75groups/370observedterminal processes: worker auth14, controls27,
Iced auth11 perappearance, ordinary public Iced6 perappearance. Wrong/retry/Enter,
prepared cache after helper exit, literal device values, direct, actualGTK and
explicitQt six-page prepared output, sign-in/device Cancel beforeCreate, Close,
caller loss, Lockdown and shutdown pass. Both masked forms/appearances inspected.
Ordinary choices/conflicts/driver changes and four600x1200DPI raster pages remain
correct. These use private CUPS and synthetic credentials; scheduler spools/auth
files are temporary and removed after retirement. No physical or real PAM
principal binding is established by the Basic gateway.

Failures retained: unused import lint01; authdark01 externalSIGTERM143 (two prefix
groups,39recorded processes separately retired and two temp roots removed after
PID/start-time/pidfd audit); dark02 and Qt-only01 blocked before PDF rendering.
A Qt-only differential proves QPrinter's native scheduler lookup blocks that PDF
fixture. Only the Qt client's CUPS_SERVER is now a nonexistent private socket;
Canopa still uses the protected real scheduler. ExplicitQt passes; automatic Qt
routing/authentication stays deferred. Light01 externalSIGTERM15 after seven
prefix groups remains failed. Full light02 passes with GDB on Python controller
only; children detached, no SIGTERM reproduced, signalinfo void. GDBexit1 is only
continue after normal exit, not a native test failure. No security setting or
production debug code changed; no debug downloads. External signal cause unknown.

Authoritative: target/portal-print-54/acceptance-01.json and
print/iced-authentication-evidence.json. Same root cache/stores; no dependencies,
vendor/routing/installed queue/service changes, binary copies, new native
executables, fresh build trees or cache cleanup.6GBreserve/3GBphase growth cap;
final observed storage in the sealed receipt.

NEXT: suppress interactive prompts in synchronous partial-job retirement (issue05),
then actual protected scheduler account binding, authentication first required
atCreate/Send, durable owner recorded before uncertain creation; interactive
protected recovery and identity-checked CUPS-Authenticate-Job for the existing
held job, without document resubmission. Digest, longer credentials, timeout/
stalled-helper and native file-output credential-clearing checks remain required.
The6-prompt cache is for an individual print request; full recovery-batch policy
still needs integration and acceptance. Inspect PHASE54-LIVE's explicitly
unverified private-PAM/held-backend investigation ideas before choosing a seam.

Broader Print device/driverless/custom precision/physical/Flatpak/failure/global
resource/runtime/default packaging, six experimental companions and final
workspace/native/package/release/tenFiles gates remain. No commit/PR or goal
completion, pause or block.

## Phase53: authentication worker verified; Iced integration next

The full 18-interface goal remains active: eleven interfaces are packaged, six
are experimental, and Print is opt-in. The user chose to finish Canopa Print
features first; automatic Qt integration remains deferred. No required input or
external blocker is pending. All validation and native processes are terminal.

The worker now has a private inherited credential stream, bounded typed
challenges and binary replies, redacted/cleared secret owners, refusal latching,
and cancellation. CUPS callbacks never fall back to console input. They check the
actual Unix/loopback/TLS peer, preserve TLS policy, and reconnect the same public
HTTP object when CUPS closes its socket before invoking the password callback.
Ordered queue requirements use literal IPP text fields, preserving punctuation
and single-field passwords. Queue credential preparation precedes the journal.

Corrected validation worker05 passed: 173 Print tests (12 ignored), 12 Lockdown,
391 Portal library (20 ignored), 55 integration (2 ignored), and the explicit
PPD runtime regression. Formatting, strict workspace Clippy and the original
workspace/default/all-target/offline/incremental/jobs1 compilation passed.
No complete workspace test execution was established; ten existing Files failures
remain. Sources are frozen in candidate02: 14 files / 170995 bytes. Binary SHA256:
e68dd7d275c0205e419339f14c3dd51e0c68206e80aa2816fcd166819e618462.

Final native acceptance covers 53 groups / 109 observed terminal processes:
14 actual authentication-worker cases, 27 ordinary worker cases, and six public
Iced regression groups in each appearance. Basic/TLS, exact UTF-8/punctuation,
refusal, wrong credentials, malformed replies, caller-pipe loss, ordered queue
fields and refusal before Create are checked. Ordinary Iced controls and four
600x1200 DPI raster pages remain correct; dark/light captures were inspected.
These tests provide no Iced password-prompt or protected recovery acceptance.

The original callback failure and API differential remain evidence. CUPS calls
its password callback with fd=-1; public reconnect repairs the peer check. Later
native05/06 failures were a fixture relay dropping chunked document bodies.
Transport-size observations confirmed an empty forwarded body; native07 passes
after forwarding chunks intact. Production code did not change for that fixture
failure. Failed receipts retain their status. No debugger/security setting was
changed and no production debug logging was added.

NEXT: integrate Iced authentication for metadata and submission, request-owned
reuse and a private retained handoff for prepared printing. Reconcile finite
interactive deadlines and distinguish user prompts from cached recovery-batch
exchanges. Verify late Create/Send authentication, authenticated account versus
durable job owner, protected recovery, and identity-checked held-job correction
without resubmission. HTTP Digest and credentials beyond the current 189-byte
Basic boundary remain required. Client.task still sends authentication=None.
Do not mark authenticated printing complete. See issues/04 and ADR0094.

Authoritative receipt: target/portal-print-53/acceptance-01.json; evidence:
print/authentication-evidence.json. Reused root cache and checksum stores; no
binary copies, native executables, target trees, cache cleanup, dependency/vendor,
installed queue/service or routing changes. A 6 GB reserve and 3 GB phase growth
cap were retained; final storage is recorded in the receipt.

Broader device/driverless/physical/Flatpak, publication-crash/stalled-kernel and
cross-restart resources, Print runtime/default packaging, six experimental
interfaces/companions and workspace/native/release gates remain. No commit or PR
and no goal completion, pause or block.

## Phase52: supported initial application print preferences

Full18 goal active:11 packaged/six experimental/Print opt-in. User priority
remains Canopa Print features; automatic Qt integration deferred.

Worker whitelist resolves current printer controls, GTK/PPD spellings, equivalent
resolution axes, exact DPI/DPCM echo and current paper names/dimensions. Unknown
fields never become CUPS options; supplied margins cannot grant device geometry.
Unavailable/conflicting fields retain queue defaults with a persistent notice.
Missing remembered printer does not redirect. File format must be offered, with
fresh retained picker selection. Initial hints apply once; manual destination
and refresh discard them. Full-option/current-destination/generation and fresh
preCreate driver checks retain authority, as do prepared physical-copy/layout rules.

Worker04 continuation TERMINAL0 (reuses worker03 format/Clippy/build): 166 Print/12 ignored,
384 Portal library/20 ignored,
55 integration/2 ignored,
12 Lockdown and explicit PPD runtime pass. Strict workspace
Clippy and original workspace/default/all-target/offline/incremental/jobs1 compile
pass; no complete workspace execution. Ten Files failures remain. Initial lint01
failed type inference, fixed; lint02 passed before final equivalent-PPD and wire
identity follow-up. Failed receipt retained.

Final native 51 groups/146 observed processes terminal:
- target/portal-print-52/controls-worker-01/summary.json: 27 groups/34 processes
- target/portal-print-52/application-preferences-dark-02/summary.json: 6 groups/29 processes
- target/portal-print-52/application-preferences-light-02/summary.json: 6 groups/29 processes
- target/portal-print-52/custom-regression-dark-02/summary.json: 6 groups/28 processes
- target/portal-print-52/custom-regression-light-01/summary.json: 6 groups/26 processes
Actual public Iced override of neutral queue defaults, unavailable per-field
fallback, missing printer, conflict correction, accepted source/type/quality and
four600x1200DPI raster pages. Actual GTK/explicitQt A4 overrides saved custom;
prepared copies remain [1,2,3,1,2,3]. Explicit Printer default restores125x225mm;
explicit initial custom paper freshness remains checked. Both appearances visually
inspected. No physical, automatic Qt or broad driverless acceptance claim.

Binary SHA 7dc2fbf1cd6d3154dfc169dff45daabf9e48a58e8979f789bde82c4297742732.14 source files/268423 bytes candidate01 archived; no
new dependency/vendor/profile/route/installed queue or service change. Reused root
cache/checksum stores; zero binary copies/new native executables/new target trees.
Authoritative target/portal-print-52/acceptance-01.json, print/preferences-evidence.json
and ADR0093. No live validation/native handles, pending input or external blocker.

NEXT: authenticated-device controls, broader app/device/driverless/physical and
custom precision, publication-crash/stalled-kernel/cross-restart resources, actual
Flatpak and runtime/default Print packaging. Qt remains deferred. Six experimental
interfaces/companions and full workspace/native/package/release gates remain.
No goal completion/commit/PR.

## Phase51: advanced printer controls and current-option authority

Source16files candidate03 under target/portal-print-51; no Cargo/vendor/routing
change. Public bounded controls, saved IPP/PPD aliases, optional 1MiB pipe/memfd
PPD adapter, fresh objects/mapping/conflicts/conditional sizes, IPP combination
checks, same preCreate path, destination/generation/options-bound Iced confirmation,
transportcopies1 and prepared-client source/type/quality/DPIx/y hints. Lazy library
handle now lives through process exit to protect CUPS pthread destructors.

Final worker06/67446 passes fmt3s, strictworkspaceClippy112.04s, originalgraph
build490.14s,155Print/12ignored,12Lockdown,373Portal library/20ignored,
55integration/2ignored and explicitPPDruntime1. Guards unchanged. Earlier04 built
and regular tests passed but whole receipt failed SIGSEGV in PPDnative test.
GDB + baseline/preload differential confirms early library unload; red retained.
05 only lintfailed obsolete drop(api), corrected to lexical recoverypreflight.

Final controls-worker02/22922:18cases/25processes; reconnect01/81877:5cases/8;
advanced-raster-dark01/34182:4groups/26 and light01/7249:4/25; custom-regression-
dark01/34190:6groups/24 and light01/31679:6/31. All139 observed finalprocesses
terminal. Real dropdown correction, current full-option conflict, default aliases,
custom limits, preCreate driverchange, GTK/explicitQt saved custom geometry and
copies covered. Advanced source/type/quality are in accepted CreateJob bytes
correlated with receiverjobid; all4 raster pages carry600x1200DPI. Native current
binary ca82d1c156254ea33286419f451c86a03ec6a3b9f95af7257c8c9d4b86b1d354.

First advanced PDF native dark01/95401 failed downstreamfield assertion despite
correct Iced response and CUPS jobcontrol attrs. Independent pipeline01/11191
failed duplicate generated media-col-supported in experiment;02/61178 failed PDF
input support;03/85591 failed incompatible receiverflags. All terminal, originals
retained. Raster pipeline01/27329 measured4jobs/22observedterminal; completion
status is not acceptance of its false flags (old3page expectation vs real4 and
receiverjobenv omits media-col). Final native uses actual sourcecount and accepted
wire attrs + bounded public raster header/pixel inspection, not those flags.
Known duplicate MIME attrs only are removed from GetPrinterAttributes; declared
values and raw/forwarded responses retained, no job payload alteration. Installed
gstoraster/gs reused, no nativeexecutable build/copy. Original PDF passthrough and
physical/driverless precision remain open. No new fullworkspace execution;
ten Files failures persist. Full18 goal active. See ADR0092, controls evidence,
PHASE51-LIVE and CONTINUE for authoritative receipts/nextwork.

# XDG portal implementation ledger

Status: implementation and validation in progress, 2026-10-29.

The first implementation extends `canopa-portal` with FileChooser, an Iced helper,
Files Directory/Browser reuse, request lifetimes and per-interface packaging.
The shared Iced/Winit forks gain optional exported Wayland parenting and modal
hints before initial configure. ADR0026 records the decision; the chooser guide
describes setup and follow-ups.

## Focused validation

- `cargo test --locked -p canopa-portal --lib --test file_chooser --test settings`
  passes: two filesystem acceptance tests, the D-Bus lifecycle contract, and the
  existing Settings reload/discovery/restart contract. Private sockets require
  running outside this environment's filesystem-socket sandbox. A denied bind in
  the sandbox is retained as an environment failure, followed by the passing run.
- `cargo check --locked -p canopa-portal --no-default-features --all-targets`
  passes. `cargo tree --locked -p canopa-portal -e normal --prefix none` contains
  no GTK, GIO or GLib packages. The feature-disabled RecoveryPage field needed a
  scoped dead-code allowance; its data and runtime behavior are unchanged.
- Focused portal Clippy passes with warnings denied. Initial wire-argument and
  test-clone lint findings were corrected. Shared workspace validation is below.
- `python3 scripts/test-settings-portal.py --binary target/debug/xdg-desktop-portal-canopa`
  passes real D-Bus activation, public reads and reload signals on its private bus.
- `python3 scripts/test-session-units.py`, `python3 scripts/check-vendor.py`,
  Python syntax compilation and `git diff --check` pass. The systemd verifier
  also requires socket options denied by the sandbox; its outside-sandbox run
  passes without contacting the live service manager.

## Native FileChooser acceptance

`scripts/test-file-chooser-portal.py` exercises the shipped activation file and
descriptor, the real public XDG frontend and the actual Iced helper in private
Niri 26.04 (`8ed0da4`) nested in Weston. It uses no GTK caller or renderer.

The final dark/100% run is `/tmp/canopa-chooser-dark-07`, with 12 recorded checks.
The light/125% run is `/tmp/canopa-chooser-light-04`, also with 12 recorded checks.
Earlier complete runs include `dark-03` through `dark-06` and `light-02/03`.
The final fixture includes expired-export handling, asserts the requested
820-pixel logical width and waits for first-frame decoration geometry before
placing a window for input/capture.

Checks cover filtered range selection without selecting an invisible PNG, combo
choices/current filter round trips, direct row Open with a non-UTF-8 name,
directory selection, SaveFile without file creation, overwrite refusal and
explicit confirmation without modifying original bytes, ordered SaveFiles,
Request.Close, expired parent fallback, imported parenting and backend shutdown.
The installed Niri does not advertise xdg-dialog-v1: optional-protocol absence
is recorded; native modal enforcement is not claimed. The Rust optional path
compiles and xdg-foreign-v2 parenting is exercised.
The fixture calls the public frontend from a host client on its private bus;
actual Flatpak document export/access remains a separate acceptance check.

Dark/light opening/confirmation frames were inspected for readable controls,
fit and shared appearance; the final light confirmation includes the complete
titlebar after the decoration-placement barrier. Early fixture runs
failed because a floating window was assumed centered (`dark-02`) and because
fractional scale reduced the nested output's logical dimensions (`light-01`).
The fixture now explicitly positions an already-parented window and derives
input coordinates from actual output dimensions. `dark-01` was blocked by the
sandbox's socket restriction. These were fixture failures, not native passes.

## Shared validation

`bash scripts/check.sh --workspace` exited 101, with output in
`/tmp/canopa-portal-workspace-check.log`. Vendor/panel/contrast checks, desktop
inventory/runner checks, the updater's 19 tests, companion units, check-script
tests, workspace formatting, workspace all-target Clippy with warnings denied,
and the 601-package attribution inventory pass. Files' library run reported
370 passed, 2 failed and 34 ignored. Both failing parents expected their injected
child to reach a crash boundary and die by SIGKILL; the children instead reached
their 25-second "boundary was not reached" fallback. The failures were:

- `operations::transfer::copy_boundary::recovered_stops_retry_their_end_save_before_gaining_continuation_owners`
- `operations::transfer::directory_boundary::a_completed_source_directory_removal_restores_only_its_result`

Both pass when rerun individually with the same workspace feature graph and
normal incremental debug artifacts:
`dbus-run-session -- cargo test --workspace --locked --lib EXACT_NAME -- --exact --test-threads=1`.
The Copy test passed in 8.37 seconds, the directory test in 2.96 seconds. Logs are
`/tmp/canopa-portal-copy-boundary-recheck.log` and
`/tmp/canopa-portal-directory-boundary-recheck.log`. This establishes isolated
success, not a green full workspace gate or proof that the parallel failures are
harmless. No transfer/Undo implementation was changed for this work.
The full gate stopped before later workspace targets and the remaining script
stages. They remain required in the next shared phase's gate.
Focused/native success does not establish full workspace or companion native
acceptance. No installation, live portal selection or running companion changes
have been performed.

## AppChooser phase

Shell's desktop-entry model/discovery has been moved into `canopa-apps`, with a
Shell re-export preserving its existing call sites. The new chooser metadata
projection retains explicit handlers that would be omitted from a launcher menu;
Hidden user overrides still win. Reads are bounded regular-file reads. The MIT
licence and existing tests move with the implementation; new projection/budget
tests are included. ADR0027 records the integration. Eleven catalog tests and five portal model/framing tests pass. AppChooser,
FileChooser and Settings wire contracts pass; all workspace targets compile after
the catalog/request-host integration. Focused AppChooser/catalog Clippy passes
with warnings denied. These checks precede the new native activation API.

The AppChooser helper now has shared appearance, searchable names, bounded
PNG/SVG icons, incoming startup activation and a fresh input-bound outgoing token
request. ADR0028 records timeout/invalidation policy. A private public OpenURI
fixture is being added; the AppChooser descriptor is still not advertised.
The shared phase requires another full gate after native validation. The earlier
workspace gate remains red as recorded above.


## AppChooser native acceptance

Seven native checks pass in each of `/tmp/canopa-app-chooser-dark-06` and
`/tmp/canopa-app-chooser-light-01` with the shipped descriptor. Public OpenURI
selects fixture Beta, preserves the exact URI, passes a fresh token to its
recording executable and focuses its Wayland surface after token consumption.
Direct backend requests check authoritative live updates, Close and incoming
startup activation. The wire trace confirms the outgoing seat/input serial.
Opening and updated-selection frames were inspected in both themes;
controls fit and selected/unselected text is readable. AppChooser is advertised
alongside Settings and FileChooser. A sixth model/framing test now covers overdue
and superseded token replies; all six pass. Focused Clippy and headless all-target
checks pass after the activation changes.

Earlier fixture failures are retained: `dark-01` hid the standard frontend's
GSettings schemas while isolating application discovery; `dark-02` called before
the frontend acquired its bus name. The fixture now provides only the required
schemas and waits for readiness. In `dark-03/04`, Niri delivered synthetic Return
under a newly connected helper's default keyboard map as Escape. A wire/UI trace
confirmed the cancellation and correct updated selection; an explicit distinct
keyboard-map transition corrects the fixture. `dark-05` passed the seven checks
before the shipped-descriptor runs. Temporary key/control diagnostics were
removed. These failed runs are not counted as passes.

The next shared gate follows the completed catalog, request host and native token
changes. Actual Flatpak document export is still unverified: this host has no
Flatpak command or `/dev/fuse`, while the private host-client frontend checks pass.


FileChooser's latest shared-host/process regression passes all 12 native checks
in `/tmp/canopa-chooser-dark-08`. A new `bash scripts/check.sh --workspace` run is
active; output is `/tmp/canopa-portals-shared-check.log`. It uses normal incremental
debug artifacts and the existing workspace feature/profile selection. The earlier
red gate is retained above; no current gate success is claimed before exit.


The AppChooser shared gate passed all tooling checks, workspace formatting,
all-target Clippy with warnings denied (8m52s), and the 602-package attribution
inventory. Workspace test executables are compiling; the gate has not exited.
Compiled repository sources remain fixed during the run. Access implementation,
common portal choices/icons/one-shot transport and focused/native fixture drafts
are staged separately under `/tmp/canopa-access-stage/`; they have not been
integrated, compiled or advertised. Their Rust syntax formatting and Python
syntax check pass, which is not behavioral verification.


At this handoff, the active full-gate PTY session is `21587`; it is compiling
workspace test executables after successful Clippy. Poll its terminal status
before starting any replacement run. The current compiled sources advertise
Settings, FileChooser and AppChooser only. Access drafts remain under
`/tmp/canopa-access-stage/` (including a prepared `main.rs`/`lib.rs` and sibling
portal refactors); do not overwrite newer sources blindly when integrating them.
The native Access fixture draft imports the repository desktop runner and must
be moved to `scripts/test-access-portal.py` before execution. Its backend/UI/tests
are uncompiled; default Grant is not bound to initial Enter. Root disk free space
is about 1.7 GiB; no caches or retained failure evidence have been removed.


## Shared gate terminal result and Access integration

The AppChooser shared gate session `21587` exited 101. After the filesystem
reached zero available bytes, the Shell test linker terminated with SIGBUS
(`collect2: fatal error: ld terminated with signal 7`). Log:
`/tmp/canopa-portals-shared-check.log`. The shell sandbox also independently failed
to start with an explicit No space left on device error. About 48 MiB became
available after compiler cleanup. Workspace tests and remaining post-test script
stages did not complete; no full-gate success is claimed. No signal was sent to
Cargo: its process had already exited before a guarded interruption attempt.
No cache or retained failure evidence was deleted.

After that terminal result, the prepared Access source, focused wire/model tests
and native fixture were integrated into `canopa-portal`/`scripts`. Common choice
validation, bounded icons and one-shot process transport move within the portal
crate; no shared crate/vendor source changes in this phase. The descriptor still
advertises only Settings, FileChooser and AppChooser. Access requires focused and
native verification before advertisement. Existing FileChooser/AppChooser native
passes predate these portal-local extractions and need regression checks.


Access's `cargo check --locked -p canopa-portal --all-targets` passes in 29.79s,
including its new UI/model and wire test targets. This is compilation only;
behavioral tests and native acceptance remain pending. At that point 41 MiB was
available. A user approval request proposes removing only
`target/release/deps` (4.1 GiB of rebuildable release dependencies); top-level
release executables and the root debug cache are excluded. No cleanup has been
authorized or performed. Do not infer approval from a goal continuation or elapsed
time. The full gate's original handle is terminal 101, so do not poll it as live.


Access's focused Clippy now passes with warnings denied (23.37s), after correcting
one collapsible-if lint. The headless all-target check also passes (14.05s).
Formatting, Python fixture syntax and diff whitespace checks pass. About 23 MiB
is available. No build/test process remains live; `21587`, `65673`, `18335` and
`80559` are terminal. Behavioral/native tests and the incomplete workspace test
stage require disk recovery. Cleanup approval remains unanswered. This is the
first goal turn that encountered actual disk exhaustion; earlier turns were
successful work/verified waits rather than repeated disk blockers.

On recovery, run focused portal library and named Access/AppChooser/FileChooser/
Settings targets, build the helper, then validate Access dark/light and regress
FileChooser/AppChooser after the portal-local extraction. Access is still absent
from the descriptor. Resume the incomplete shared cargo-test/post-test stages
using the same root debug/incremental graph; retain both earlier red gate logs.


Disk recovery audit, second consecutive goal turn: authoritative `df` still shows
23 MiB available. No human cleanup decision or other external capacity change has
arrived. The prior full gate is terminal, and no build is being restarted on this
capacity. Existing source, descriptor and successful compilation/lint evidence
are unchanged. Native/helper linking and required behavioral validation remain
at an impasse pending capacity recovery. The release dependency cleanup request
remains pending; no deletion or alternate build/cache configuration was used.


Disk recovery audit, third consecutive goal turn: `df` again reports 23 MiB
available. No cleanup approval or external capacity change has arrived. Required
behavioral/native verification cannot proceed, and no build remains live to wait
on. The same capacity blocker has now recurred for three consecutive turns.
The goal is marked blocked pending disk recovery. No source/cache deletion,
installation, live portal selection or substitute build profile was performed.


## Resumed after user-managed capacity recovery

The user's choice was to free disk space themselves, not to authorize release
cache deletion. Current `df` reports 34 GiB available. No proposed release-deps
cleanup was performed by this portal task. The current root AGENTS/test guide now
uses one `integration` executable per crate; portal cases are registered in
`crates/canopa-portal/tests/integration.rs`. Preserve the concurrent consolidation
work and select that target for focused checks. The goal is active again, with a
fresh blocked audit if any blocker recurs.

Focused portal library/consolidated integration tests and a normal helper build
are now running. Retained Niri investigation sources/executable have moved under
`.scratch/file-manager/retained-target-artifacts/`; see the user's
`TARGET-CLEANUP.md` relocation table rather than recreating old target directories.


Resumed focused checks pass with the consolidated layout:
`cargo test --locked -p canopa-portal --lib --test integration` reports seven
library/model/framing tests and four wire/lifetime tests passed. The Access wire
case verifies bool/combo choices, custom labels/icon/modal context, grant/deny,
invalid returned controls, malformed options, Close ownership and caller loss.
The normal helper build passes. The first native Access fixture is running in
`/tmp/canopa-access-dark-01`; no native pass is claimed yet.


## Access native acceptance after capacity recovery

The direct backend fixture passes seven checks in dark/100%
(`/tmp/canopa-access-dark-03`) and light/125%
(`/tmp/canopa-access-light-01`). Native input edits the boolean and combo choices,
then explicit Grant returns their exact IDs/values. Opening and initial Enter do
not grant. Deny, Escape, Close and caller loss do not grant. Concurrent questions
keep independent helpers/decisions while Settings remains responsive. Long text
scrolls with both decision actions reachable. `dark-01` passed the same core
checks before edited-choice coverage. `dark-02` left the combo overlay open while
trying keyboard selection; its grant click dismissed the overlay and timed out.
The fixture now selects the visible option explicitly; it does not claim keyboard
navigation support for that widget.

A separate standard Camera frontend probe is running on private PipeWire,
permission-store, bus and compositor instances. This is a consent handoff check,
not camera hardware or media-flow acceptance. Access remains unadvertised until
this additional routing evidence and rendered light frames are reviewed.


## Final Access and helper regression evidence

The maintained Access fixture now includes public Camera consent with private
PipeWire and permission-store services. It passes eight checks per appearance in
`/tmp/canopa-access-dark-04` and `/tmp/canopa-access-light-02`. The helper receives
an actual standard-frontend question and its explicit grant completes the public
request. No camera frames/device access or sandbox/document export are claimed.
The independent Camera window is explicitly floated/sized by the fixture; no
parentless automatic-placement claim is made. The separate earlier prototype
`/tmp/canopa-access-camera-01` also passed and is retained as evidence.

FileChooser regressions pass 12 checks per appearance in
`/tmp/canopa-chooser-dark-09` and `/tmp/canopa-chooser-light-05`; AppChooser passes
seven per appearance in `/tmp/canopa-app-chooser-dark-07` and
`/tmp/canopa-app-chooser-light-02`. These runs cover the portal-local choice/icon/
transport extraction after Access integration. New shared checkbox styling uses
semantic border/focus roles: the previous unchecked light box was too faint.
Focused portal tests (7 library + 4 consolidated wire cases), the helper build
and shared-UI/portal all-target Clippy with warnings denied pass after that change.
Access choice/grant/long-body/public-question captures and FileChooser/AppChooser
regression captures were visually inspected; fit, text and checkbox boundaries
are readable in both appearances. The shipped descriptor advertises Settings,
FileChooser, AppChooser and Access only. The optional Niri floating-window rule in
the Access guide passes the installed compositor's configuration validator.

A fresh required `bash scripts/check.sh --workspace` run is active as PTY session
`6491`, using the current consolidated test layout and normal root incremental
artifacts. Output: `/tmp/canopa-portals-consolidated-workspace-check.log`.
Earlier red gate logs remain retained; this run is not a pass until terminal.


The consolidated gate `6491` is terminal 143 (SIGTERM). Its log ends after all
19 updater tests pass in 210.969s, with no termination diagnostic. No compiler/test
failure was printed. The next stage, `python3 scripts/test-session-units.py`, passes
independently. This is an interrupted/incomplete gate, not a passing workspace
result. A new full invocation is running with output at
`/tmp/canopa-portals-workspace-retry.log`; retain the interrupted log. No build was
restarted based solely on timeout or an unchanged log.


The retry remains live as PTY session `45696`. Tooling checks, workspace all-target
Clippy with warnings denied (5m27s), and the 602-package attribution inventory
pass. Workspace test executables are compiling. Poll this exact handle for its
terminal result; do not restart based on an observation timeout. The separate
normal production dependency graph is retained in
`/tmp/canopa-portal-normal-tree.txt`; an exact package-name scan finds no GTK/GDK,
GIO or GLib packages for standalone default-feature `canopa-portal`.

Notification contract/ownership preparation is saved in `NOTIFICATION.md`.
It is investigation only; its v2 descriptor-resource and typed-action work is
not implemented or advertised. No Rust sources have changed after the current
gate started; only documentation/evidence notes were updated.


## Consolidated shared gate terminal results

The full retry (`45696`, `/tmp/canopa-portals-workspace-retry.log`) exited 101.
Tooling, formatting, workspace all-target Clippy (5m27s), and the 602-package
attribution inventory pass. Files library results are 368 passed, four failed,
34 ignored. Each of those four directory-boundary tests passes when isolated
sequentially in the **same** already-built library executable (16.14s total),
without feature/profile changes; this does not establish a passing parallel run.

The remaining 26 compiled workspace test harnesses were then run directly with
Cargo's package working directories and private buses, without rebuilding.
`/tmp/canopa-portals-continued-tests-01/results.json` records every command/status.
Files integration has the same ten failures as the consolidation baseline
(596 passed, ten failed, 41 ignored, 625.67s): recovery-catalog timeout, a 500-item
Keep-both Undo responsiveness threshold, and eight saved-history/crash-boundary
failures. Notify library has 154 passed and its one known native AT-SPI fixture
failure; Shell binary has 1260 passed and its one known native AT-SPI fixture
failure. Both fixtures fail while establishing their private accessibility bus.
The other 23 harnesses pass, including all four portal wire cases and seven
portal library checks. No transfer/Undo or AT-SPI fixture behavior was changed.

Post-test Settings/CLI/configuration/vendor stages are running separately, with
results retained under `/tmp/canopa-portals-post-stages-01`. The workspace is not
green. Notification implementation begins after this fixed test-artifact run;
subsequent resource/adapter changes require their own focused/shared validation.


The post-test Settings fixture and vendor fixture pass. The first CLI stage was
started while the subsequent shared SVG manifest edit had not yet updated
Cargo.lock, so `--locked` refused it; configuration then had no debug CLI binary.
After the local graph update, the same locked CLI build passes (1m01s) and the
configuration isolation/migration/privacy/no-clobber fixture passes. Original
logs remain under `/tmp/canopa-portals-post-stages-01`, alongside the successful
`cli-retry.log` and `configuration-retry.log`. These do not make the red workspace
test gate green.

Notification source work now exists but is not yet advertised: a Notify-owned
adapter, app/ID registry, bounded owned operation queue, frozen typed action
publications and standard application Activate/ActivateAction calls. New shared
resource work adds owned raster decoding/SVG rasterization, explicit raw-body
format at Notify intake, ordered themed-icon candidates and owned encoded sound
input. Descriptor content is converted into owned pixels/bytes instead of
transport-owned paths or temporary-file leases. Portal notifications request no
automatic expiry, preserving their callbacks through sender exit; the first
version's banners remain until dismissal/action/withdrawal. Optional display
hints/category/button-purpose-specific behavior is not advertised. Focused
compilation and tests are in progress; source existence is not acceptance.


## Notification first wire/native evidence

The first focused adapter run had four passes and two failures: a property
incorrectly named Version instead of version, and the declared serialized (sv)
resource wrapper was not unwrapped. The parser now validates the complete (sv)
signature before tuple conversion (also avoiding generic conversion's unchecked
field count) and unwraps exactly that declared variant. Seven adapter tests pass
in `/tmp/canopa-notification-wire-tests-02.log`; all 38 Notify binary tests pass
in `/tmp/canopa-notification-bin-tests-01.log`. Coverage includes old FDO behavior,
app/ID isolation, dropped replacement targets, frozen publication across
replacement/retirement, typed exported/default activation, failure/token
isolation, sealed descriptors, malformed resource shapes and admitted-operation
reply loss/drain. Intake's 26 focused checks pass in
`/tmp/canopa-notification-intake-tests-01.log`. Six shared raster/SVG tests pass in
`/tmp/canopa-notification-assets-tests-01.log`; core dependency exclusion passes.

The new native frontend fixture passes three checks in dark/100% at
`/tmp/canopa-notification-dark-04`: a notification remains after its public sender
exits and beyond the normal five-second expiry; native banner input D-Bus
reactivates the disposable application with original (si) target and fresh token;
the reactivated application's C Wayland surface consumes that token and is
focused; its native named button sends the original string target and a fresh
token. The descriptor-backed SVG is visible and plain <b> text remains literal.
All application registrations, frontend/backend, bus/compositor and input belong
to the private fixture. No GTK backend or renderer is used.

Earlier native fixture failures remain: dark01 waited for the early socket rather
than the ready backend alias; dark02 proved the default callback but its second
GApplication became remote and could not send; dark03 used a click below the
button. The fixture now uses local NON_UNIQUE sender instances and corrected
coordinates, and proves activation-token consumption explicitly. No pass is
claimed for those entire failed runs. Light/125% and consolidated Notify
integration checks are running. Center input, more native resource/withdrawal
coverage, packaging, documentation and final shared checks remain pending;
Notification is not advertised yet.


Notify's consolidated integration target passes all 276 cases after the adapter
and shared resource/intake changes (`/tmp/canopa-notification-integration-tests-01.log`,
80.72s). The first light fixture attempt proved default reactivation/token
consumption but sampled its second banner before it was drawn; its empty frame
is retained at `/tmp/canopa-notification-light-01`. The fixture now waits for a
rendered banner frame instead of assuming a fixed half-second delay. Expanded
public non-exported-signal and withdrawal coverage is being verified next.


The expanded native Notification fixture now passes five checks in both dark/100%
(`/tmp/canopa-notification-dark-05`) and light/125%
(`/tmp/canopa-notification-light-02`). In addition to exported callbacks and native
activation consumption, it observes the real public non-exported ActionInvoked
signal with original (si) target and platform data, and public withdrawal closes
the intended notification. The final light run waits for actual banner pixels,
resolving the earlier fixed-delay race. Dark's first final banner capture was
inspected; both final appearance sets still need final visual review together.

The first focused Clippy run rejected one nonminimal sound-header predicate. It
is simplified without changing its accepted formats; a callback test type alias
also names the recording tuple. Clippy is being rerun. Registration exhaustion
is now a typed LimitsExceeded path rather than a generic Failed reply. No
Notification descriptor/activation metadata has been installed or advertised.


Final dark/light sender-exit and button captures were inspected together: text,
button boundaries and literal markup are readable; descriptor SVG pixels render
in both appearances. Native Center prototype01 failed waiting for a Shell bar;
no Center input or acceptance is claimed. It used the previous root debug Shell
binary and produced no Shell trace. A current default-feature Shell build is
running (`3858`, `/tmp/canopa-notification-shell-build-01.log`), followed by a
fixture with debug logging, explicit default Notify socket, known bar config and
retained layer inventory. Existing production Shell sources are unchanged.

Focused Notify all-target Clippy passes in
`/tmp/canopa-notification-clippy-02.log` after the predicate/type-alias cleanup.
A subsequent small error-typing change makes portal registry resource exhaustion
report LimitsExceeded. A new offline sound test checks that owned WAV bytes play
after the original file has been removed and release their sound worker. These
last changes still require focused checks. No production Notification descriptor
exists. The current service alias is claimed after its interface is installed;
custom developer bus names never claim that fixed alias. Packaging/readme/native
Center/final shared gate remain pending, along with the broader roadmap.


The current Shell build passes (4m42s). Center prototype02 opens both Shell and
the Center, but the fixture counted the short-lived bar tooltip as Center
readiness and captured/clicked before the reading panel appeared. Its retained
`center.png` contains the tooltip, not Center acceptance. The fixture now moves
the pointer away and waits for actual tall reading-panel pixels. Native Center
callback verification remains in progress. The owned encoded-sound offline test
passes in `/tmp/canopa-notification-owned-sound-01.log`: eight expected PCM samples
play after the source path is removed, then the job/output are released.


Center prototype03 captures the actual reading panel but clicks below its Open
notification button. Prototype04 clicks that verified control and receives the
correct public non-exported action `chosen-center` with original `(si)` target
`('center-target', 27)`, but its platform dictionary is empty. This is the current
native red, not a fixture readiness failure. Source inspection confirms Shell
carries no input activation context to the provider. Notify requests an
uncontextualized token on its separate connection with a 100ms budget; its later
reply is not source/serial-bound and is not a valid fix for the missing click
handoff. Do not weaken this check or claim Center activation acceptance.

[ADR0032](../../docs/adr/0032-panel-action-activation-context.md) records the next
seam: an opt-in generic rich-action capability plus an optional bounded token on
the v5 host event envelope. Preserve frozen v4 invoke events and opaque values;
old actions/events must serialize identically when context is absent. Shell
captures its live panel surface/revision, obtains native input-bound activation
through the existing layer-shell custom operation, bounds/revalidates the reply,
and forwards it through Notify's provider/private socket to the existing
captured-revision domain action. Implementation has not begun; only the decision
and diagnostic fixture exist. The shared protocol currently has nine explicit
HostMessage::Event sites, 33 PanelAction matches/literals (some are the unrelated
Shell config enum), and 17 notification-center command matches/literals.


The first activation-contract slice is implemented in the shared protocol:
`PanelAction.request_activation_token` defaults false and is omitted from legacy
serialization; `v5::HostMessage::Event.activation_token` defaults None and is
omitted when absent. The frozen v4 invoke event is unchanged. The v5 current
contract accepts a token only for the exact opted-in action/revision, validates
4096-byte/nonempty/no-NUL bounds, and reserves the maximum fully escaped token
in advertised action envelopes. Twelve v5 session checks pass in
`/tmp/canopa-panel-activation-tests-01.log`, including exact old-shaped event and
action serialization, unrequested token rejection, replacement and size bounds.
Protocol all-target check passes. Literal constructors in existing consumers
now explicitly use false/None; runtime host/provider/token handoff is **not yet
implemented** and no action opts in. Consumer all-target compilation is running
as PTY session `3146`. Log:
`/tmp/canopa-panel-activation-consumers-check-01.log`.


The shared activation contract now has 13 passing v5 session checks, including an
old strict event decoder and permissive old action decoder. Token-free wire bytes
remain exact. Shell's v5 adapter can request context for current opted-in actions
and revalidates the captured event before writing it; replaced callbacks are
silently dropped. Shell native dispatch captures the active panel surface,
requests a token through the existing layer-shell custom operation, bounds the
reply/age to one second and forwards context separately from opaque action data.
Notify's v5 provider preserves the token in a private flattened socket context;
its core center command accepts an optional bounded token and keeps existing
captured-revision commitment. Open/action/link controls opt in; acknowledgement,
permissions, navigation and other existing actions do not.

`layershellev` now timestamps input, rejects context at two seconds, bounds
pending tokens to 64, reaps expired/retired requests on new admission, and drops
replies after one second or source retirement. A reaped request is not destroyed
a second time by a late Done. Its reviewable patch/provenance were regenerated
against an upstream reconstruction whose exact recorded hash was verified;
`/tmp/canopa-panel-activation-vendor-02.log` passes offline integrity. The recipe
is `/tmp/canopa-regenerate-layershellev.py`. No second fork copy was created.

Consumer all-target checks pass in
`/tmp/canopa-panel-activation-consumers-check-03.log` (2m47s). New host callback
replacement and core token/target tests have been added but not yet run. A joint
normal debug build is in progress (`14948`,
`/tmp/canopa-notification-activation-build-01.log`); its vendor compilation began
before the final late-Done guard edit, so require Cargo to recheck/rebuild current
sources before using it as final native evidence. Native Center remains red
until the new source-bound path is built and verified; metadata is still absent.


The joint activation build completed (4m12s), but current-source assurance still
requires rebuilding its final late-Done guard. Focused Shell/Notify tests are
running as PTY `62253` at `/tmp/canopa-panel-activation-focused-01.log`, selecting
`activation_context` across the two binary and consolidated integration targets
on a private bus. They cover host callback replacement/opaque-value preservation
and captured core center-action token routing. The maintained native fixture now
requires a Center token distinct from prior banner tokens and matches it to
Shell's own Wayland Done/serial/surface trace, so a background token or recycled
value cannot satisfy native acceptance. No post-handoff native pass is claimed
yet; the last native Center red remains dark04.


The focused joint activation run is terminal 0 (`62253`): current sources compile
in 7m57s; the Shell callback/opaque-value/replacement regression and Notify
captured-center-action token regression pass. The other two selected harnesses
have zero matching tests; no additional pass counts are inferred from them.
Protocol's 13 tests remain passing. A locked joint normal-binary build is running
as `82567` at `/tmp/canopa-notification-activation-build-02.log` to establish
current-source native artifacts after the final vendor guard. No post-handoff
native pass is claimed until that build and fixture complete.


The current-source joint normal build passes (`82567`, 54.93s). The expanded
native Center run passes six checks in dark/100% at
`/tmp/canopa-notification-center-dark-05` (`61261`, terminal 0). Its Center token
is distinct from previous banner tokens and matches Shell's Wayland request:
set_serial(40, wl_seat#9), set_surface(wl_surface#52), then the exact returned Done
token. The public action retains the original `(si)` target. This resolves the
actual dark04 missing-context red through the generic host/provider/core handoff.
The existing five public/banner/resource/withdrawal checks also pass on these
current binaries. Light/125% Center verification is running at
`/tmp/canopa-notification-center-light-01`; no light Center pass is claimed yet.
Notification metadata is still absent until this interface acceptance completes.


The complete current-source native suite passes six checks in light/125% at
`/tmp/canopa-notification-center-light-01` (`91439`, terminal 0). Final Center
captures in both appearances were inspected: reading content and explicit action
controls fit, and the light history area has its expected scroll rail. Both
Center tokens are matched to Shell's own native serial/source trace.

Notification metadata is now present: `canopa-notify.portal` and its dedicated
D-Bus activation entry select the existing Notify service. The native fixture
now consumes those exact files and lets the private bus activate the backend
through the real fallback Exec/default configuration discovery. All six checks
pass in `/tmp/canopa-notification-activation-dark-01` (`50844`, terminal 0), with
backend executable identity checked through its private bus PID. Light activation
verification is running at `/tmp/canopa-notification-activation-light-01`.
No live installation/routing or user service manager was changed.

The release documentation staging check passes: 28 files and 52 offline links.
It now ships all four interactive portal guides alongside Settings. The new
Notification guide describes ownership, bounds, activation, routing and first-
version limits; normal Notify provider fallback remains intact when no host
token is supplied. Packaging smoke checks include the new descriptor/activation
entry; a metadata integration case has been added. Final shared/core gates,
updated full focused consumer checks and any required native regressions remain.


Shipped metadata activation also passes all six checks in light/125% at
`/tmp/canopa-notification-activation-light-01` (`33183`, terminal 0). The dark
activated backend PID no longer exists after fixture cleanup, confirming its
private service retired. Both appearance captures and the separate host-surface
traces have been reviewed. The descriptor now advertises only Notification from
Notify; the dialog descriptor still owns its existing four interfaces. Guide,
README, roadmap and ADR status reflect scoped acceptance with shared validation
still pending. Release-documentation staging passes 28 files/52 links. No live
installation, routing or service-manager update was performed.


The final Notification shared workspace gate is running at
`/tmp/canopa-portals-notification-workspace-check.log`; tooling, unit-graph,
formatting and test-layout stages passed, and Clippy is in progress. No terminal
result is claimed yet. The required private shared-fork desktop fixture passes
(`/tmp/canopa-portals-fork-desktop-01`, terminal 0): actual popup grab/outside
dismissal, 125% scaling, all-output retirement/restoration and dismissal on a new
output without a bar. Overlay placement passes on all four edges at 125%
(`/tmp/canopa-portals-placement-01`, terminal 0).

The separate desktop accessibility fixture terminates 134 at
`/tmp/canopa-portals-desktop-01.log`: its private AT-SPI launcher exits with a
usage error; the caller then cannot establish org.a11y.Bus and aborts while
attempting failure-tree collection. This reproduces the recorded host fixture
limitation before compositor/Canopa acceptance. Do not infer native accessibility
acceptance from the passing virtual-input portal tests. The failed fixture's
logs and private state are retained.


The focused-window icon fixture passes on the current Shell binary
(`/tmp/canopa-portals-focused-icons-01`, terminal 0): fresh PNG/SVG and cached
focus switches redraw actual icon/title pixels across left/right/top bars at
125% without unrelated input. Representative placement and icon captures were
inspected and fit. Idle inhibition/retirement passes in private Sway
(`/tmp/canopa-portals-idle-sway-01`, terminal 0); the Niri counterpart is running.
The workspace gate remains active in Clippy with no diagnostic yet.


The workspace gate's original PTY was recovered as `80937` and is terminal 143
(SIGTERM after 548s, no diagnostic). Its compiler child subsequently completed
Clippy successfully in 6m20s; there are no remaining compiler/check processes.
The log stops before attribution/tests. The signal's cause is unknown; do not
call this a green workspace or a test failure. Resume only the unfinished gate
stages, retaining the completed tooling/format/Clippy results.

The resumed attribution check is terminal 1: its generated inventory is stale
(`/tmp/canopa-portals-notification-attributions-01.log`). Regeneration is being
checked against the current locked release graph. All-target workspace tests
are running as PTY `40845` at
`/tmp/canopa-portals-notification-workspace-tests-01.log` using the same normal
incremental debug graph, with no cache cleanup or feature/profile changes.
The private Niri idle-inhibition check also passes
(`/tmp/canopa-portals-idle-niri-01`, terminal 0).


Attribution regeneration/check passes with 602 release dependencies
(`/tmp/canopa-portals-notification-attributions-02.log`, terminal 0); the lockfile
fingerprint now matches the shared SVG manifest extraction. The inventory retains
original license declarations. `git diff --check` passes. A fresh default normal
dependency graph for Portal and Notify (`/tmp/canopa-portals-no-gtk-01.txt`,
terminal 0) contains zero GTK/GDK/GIO/GLib entries. This scoped claim does not
apply to the workspace-wide Shell feature union or the GI test callers.

Account's contract and unresolved profile/avatar lifetime seams are recorded in
`ACCOUNT.md` for the next slice. No Account code, activation or routing is added
by that preparation.


Current rebuilt Portal native regression is underway after the shared SVG
extraction. The initial dark command used the Notification fixture's `--scheme`
flag instead of this fixture's documented `--color-scheme`; argparse rejected it
with terminal 2 before creating a private session
(`/tmp/canopa-chooser-regression-dark-01.log`). The corrected invocation passes
all 12 FileChooser checks (`/tmp/canopa-chooser-regression-dark-02`, terminal 0).
Light/125% verification is now running. Final vendor integrity remains passing
(`/tmp/canopa-portals-notification-vendor-final-01.log`, terminal 0).


The rebuilt Portal passes all 12 FileChooser checks in light/125% as well
(`/tmp/canopa-chooser-regression-light-01`, terminal 0). Shared SVG extraction
therefore retains the complete previous FileChooser acceptance in both
appearances, including raw-byte URIs, no-write save confirmation, independent
requests and helper teardown. AppChooser/Access regression is next. Workspace
all-target compilation remains live as `40845`; no terminal result yet.


AppChooser passes all seven current-source native checks in both appearances
(`/tmp/canopa-app-chooser-regression-dark-01` and
`/tmp/canopa-app-chooser-regression-light-01`, both terminal 0). The real public
OpenURI frontend launches only the selected fixture app, preserves the URI and
provides an input-bound token consumed by its native Wayland surface; live
UpdateChoices and request cancellation remain passing. Access regression is
underway. No original live desktop entry or routing is changed.


Access passes all eight current-source native checks in both appearances
(`/tmp/canopa-access-regression-dark-01` and
`/tmp/canopa-access-regression-light-01`, both terminal 0), including explicit
grant/deny, initial Enter refusal, symbolic SVG, edited bool/combo choices, long
body, Close/caller loss, concurrent decisions and public Camera consent without
media capture. In total all 27 earlier dialog checks per appearance now pass on
the rebuilt Portal after shared asset extraction (FileChooser 12 + AppChooser 7
+ Access 8). AppChooser icons and Access consent captures in both appearances
were inspected. These do not replace the still-active workspace test gate or the
required core/vendor library checks.


The current all-target build has completed and workspace tests have reached the
Files library suite. No additional Cargo build or native compositor fixture is
running alongside its timed worker cases. Account preparation inspected the
locked whoami Linux source via platform-filtered offline metadata. An initial
unfiltered metadata query attempted an uncached non-Linux `wild` download and
failed DNS (optional investigation only); the Linux-filtered offline query
passes, without modifying the lockfile or build features.


Workspace all-target compilation finishes in 28m38s. The shared app catalog
passes 11 tests, Assets passes 11 integration cases (including the extracted SVG
mechanism), and Config passes 12. Files library now passes all 372 tests with
34 expected ignores in 251.77s; the earlier timed directory-boundary reds do not
reproduce in this run. The gate has reached Files integration (647 listed cases);
its terminal result and remaining consumer harnesses are still pending.


The resumed workspace test command is terminal 101 (`40845`). Files integration
finishes in 596.36s: 596 passed, 10 failed, 41 ignored. All ten failure names
match the earlier consolidation/continued-workspace baseline exactly: recovery
catalog timeout, 500-item Keep-both timing gate, and eight saved-history binding/
crash-child cases. The Files library remains fully passing (372 +34 ignored).
The workspace stays red; these are not claimed as portal passes or new portal
failures. Current same-graph Cargo artifacts are being collected without running
or rebuilding already-completed tests, then the remaining built harnesses and
post-stages will continue.


The same-graph artifact collection is terminal 0 in 3.87s: all 745 artifacts are
fresh, zero rebuilt, with 34 compiled test executables. Twelve ran before Cargo
stopped at Files integration. The continuation runner completed the remaining
22 on private buses with package working directories. Its first invocation
stopped at a path guard for two example executables after 12 harnesses; both
hashed examples were verified to contain `test::test_main_static`, and only the
ten unexecuted harnesses resumed. No completed harness was repeated. Both logs
are retained (`/tmp/canopa-portals-notification-continued-tests-01.log` and `-02.log`);
all 22 final results are in
`/tmp/canopa-portals-notification-continued-tests-01/results.json`.

Current focused/full consumer results: Notify binary 38/38, Notify integration
278/278 (including Center context and metadata), Portal integration 4/4 and
library 7/7, Shell integration 29/29, panel protocol integration 21/21 plus
library 14/14, Copy 7/7 and both ordinary Files helper parents pass. Notify
library has 156 passes and its one recorded AT-SPI fixture failure; Shell main
has 1261 passes and its one recorded native Center AT-SPI failure. The same two
fixture names/launcher usage error reproduce; no new failing case appears. All
other 20 remaining executables are terminal 0 (some are zero-case harnesses and
are not counted as extra passing tests). The complete workspace remains red for
the ten baseline Files cases and two baseline native accessibility cases.

The current Settings frontend activation/ReadOne/ReadAll/reload fixture passes
(`/tmp/canopa-portals-notification-settings-01.log`, terminal 0). CLI build,
configuration fixture, vendor library fixture and required core gate remain.


All workspace post-stages now pass: CLI locked build (0.60s), configuration
validation/isolation/migration/privacy/no-clobber, and the current vendor fixture
(`/tmp/canopa-portals-notification-vendor-tests-01.log`, terminal 0). Vendor tests
cover four layer-shell adapter/settings regressions, three event-batch queue
regressions and six native pipe/cancellation/budget cases. Its unused patch
warnings belong to the intentionally narrower temporary graphs; no active
registry version outside the root lockfile was admitted.

The required `bash scripts/check.sh --core` is now running at
`/tmp/canopa-portals-notification-core-01.log`. It is the final pending validation
step for this slice; no Account source implementation begins while it runs. The
current complete workspace outcome remains scoped verification with the same
ten Files failures and two host AT-SPI fixture failures, not a green workspace.


Core validation is still live as PTY `58461` (do not restart). Its dependency
guards pass; Shell no-desktop all-target checking and tests pass (36 library and
13 integration cases). The remaining no-default-feature all-target check passes
in 2m04s. The subsequent Config/File/CLI/Theme/Notify-client/Portal test stage
also passes, including the current Portal's seven model and four wire cases.
It is now compiling the Files no-default-feature test graph; Files tests and
the final Notify/panel-protocol no-default-feature tests have no terminal result
yet. If Files stops the script on baseline failures, run its still-unreached
Notify/panel stage separately and record both outcomes. `git diff --check`
continues to pass. Source implementation remains unchanged during this gate.

Next: finish `58461` and any unreached core stage; update scoped validation/ADR
status while preserving the workspace's 12 baseline failures, then implement
Account from `ACCOUNT.md` with the local picker seam. The broader goal is active,
not complete. Current disk space is sufficient and no cache deletion was made
by this task after the human's cleanup.


The core gate is terminal 101 (`58461`), after its already-passing initial stages.
Files no-default library reports 103 passed, 188 failed, 33 ignored in 46.74s.
These are predominantly ENOENT at self-spawn `.status().unwrap()`, not assertion
failures in resumed file work. Its just-built executable
`canopa_files-645fa06ffe43e95d`, the earlier workspace harness and top-level debug
executables are now absent. Debug directories changed at 16:01; free space rose
from about 13 GiB to 33 GiB. This task did not delete those outputs. The human
confirmed cleanup is finished and authorized continuing rebuilds.

A selected no-default Files self-spawn test is rebuilding/rechecking at
`/tmp/canopa-portals-core-self-spawn-recheck-01.log`. Do not count the interrupted
188-case failure as proof of core behavior, or hide the terminal red. Revalidate
this cause on the rebuilt executable, then continue the unreached Files and
Notify/panel-protocol core stages without repeating completed initial checks.
Account implementation remains pending.


The rebuilt no-default self-spawn recheck is terminal 0 (`96433`): compilation
1m35s, selected copy-boundary test passes in 5.17s while restarting its own
executable repeatedly. This verifies the missing-artifact cause for the earlier
ENOENT failures. The full Files no-default stage is now resuming at
`/tmp/canopa-portals-core-files-retry-01.log`; do not restart the completed
initial core stages. Account's renderer-independent request/result adapter has
been started in `account.rs` while this independent Files graph validates. It
is not registered by the daemon or advertised, and has no tests/GUI acceptance
yet. No shared dependency or Files source was edited by that addition.


Rebuilt Files core library runs through its real assertions: 287 passed, four
failed, 33 ignored in 241.76s (`94898`, terminal 101). The four failures are
child settling/expected SIGKILL gates in replacement_boundary and
saved_cross_boundary, not ENOENT. All four pass sequentially on the exact same
binary in 16.62s (`88390`, terminal 0,
`/tmp/canopa-portals-core-files-isolated-01.log`). Preserve the broader run's red;
collect current core artifacts and continue its still-unreached integration/
helper/doc targets and Notify/panel core stage.

Account now has a renderer-independent typed request/result adapter, not yet
registered or advertised. FileChooser's state/view is being extracted as an
embedded Picker with Selected/Cancelled outcomes; only its standalone Dialog
adapter exits the helper process. Directory cancellation retires on Picker drop.
This local seam remains unverified until focused compilation/tests and native
FileChooser regression run. No shared crate/vendor source or Cargo manifest was
changed during the independent Files core checks.


Core Files artifact collection is terminal 0 (0.64s): 168 fresh artifacts, none
rebuilt. Five unreached test targets ran directly on private buses using their
current no-default executables (`73156`, terminal 1). Integration reports 590
passed, nine failed, 39 ignored in 577.36s; all nine are a subset of the recorded
workspace baseline (the recovery-catalog timeout does not fail in this core run).
The remaining four helper/clipboard/Copy targets pass. Results are in
`/tmp/canopa-portals-core-files-remaining-01/results.json`. The core gate remains
red for the four parallel child timing failures (all pass isolated) and these
nine existing integration cases. File doc-tests and Notify/panel core stage
remain.

Account profile discovery/immutable avatar preparation has begun in its own
module, using bounded AccountsService/NSS fallback, shared raster/SVG decoding
and normalized PNG output. Desktop-only dependencies reuse locked workspace
packages. This source is still unwired/unverified; no Account descriptor is
advertised. Updating the lockfile through host-filtered offline metadata before
resuming locked core commands; do not infer that earlier core checks cover the
new Account code.


Files core doc-tests pass (`10208`, terminal 0; zero documentation cases). The
remaining Notify/panel no-default stage also passes (`49611`, terminal 0); its
full per-harness counts are in `/tmp/canopa-portals-core-notify-01.log`. All core
stages have now been attempted/continued, with the recorded Files reds retained.
The optional Account desktop dependencies add no new registry package versions.
The current source now wires an unadvertised Account endpoint/helper and an Iced
sharing draft with embedded picker, explicit per-field disclosure, bounded local
profile/PNG preparation and a private bounded image cache. This new Account code
and the picker extraction require their own focused/no-default checks and native
acceptance; earlier validation does not establish their completion.


## Core gate completion after external build-output cleanup

The core gate's first Files library execution lost its own running executable
during external build-output cleanup: 188 self-spawn cases returned ENOENT.
The user confirmed cleanup had finished and requested rebuilding. The selected
self-spawn proof passes after rebuilding (`/tmp/canopa-portals-core-self-spawn-recheck-01.log`).
The full core Files library retry reports 287 passed, 4 failed, 33 ignored in
241.76 seconds (`/tmp/canopa-portals-core-files-retry-01.log`). All four remaining
child-boundary timing/expected-kill cases pass sequentially with that exact binary
(`/tmp/canopa-portals-core-files-isolated-01.log`); no ENOENT failures remain.

The five previously unreached core Files test executables were run using the
same Cargo graph/artifacts: integration reports 590 passed, 9 failed, 39 ignored
in 577.36 seconds, with all nine failures a subset of the existing ten Files
integration failures. The recovery-catalog case passes in this run. The other
four executables pass; Copy has seven cases and three feature-gated executables
have zero core cases. Results are in
`/tmp/canopa-portals-core-files-remaining-01/results.json`. Files doctests pass
with zero cases (`/tmp/canopa-portals-core-files-doc-01.log`).

The final core stage, `bash scripts/test-notify.sh --no-default-features`, exits
zero (`/tmp/canopa-portals-core-notify-01.log`): Notify library 53, binary 37,
integration 276; panel protocol library 11, integration 19; Notify compile-fail
doctest 1, all pass. The core gate has now been attempted/continued through all
stages. It remains red for the four parallel Files child-boundary failures and
nine pre-existing integration failures. Isolated successes do not make the full
gate green. Account changes below postdate its earlier portal stage.

## Account implementation and focused acceptance

Account now has a Rust/Iced request-owned sharing draft, optional field/image
disclosure, worker profile lookup and an embedded instance of FileChooser's
browsing component. The standalone adapter owns process exit; the embedded
component returns Selected/Cancelled. Generation checks reject retired picker,
avatar and Share completions. Reviewed owned pixels are normalized to PNG and
published only after Share into a bounded private cache outside the helper's
lifetime. ADR0033 and `docs/account-portal.md` record the behavior and limits.

Initial code generation exposed an Iced restriction not instantiated by
`cargo check`: Subscription::map cannot capture the picker generation. It now
uses Subscription::with. Focused Clippy found a boolean simplification and an
oversized message variant; the picker-start result is boxed.

`cargo test --locked -p canopa-portal --lib --test integration` passes 19 model
and five wire/process tests (`/tmp/canopa-account-tests-06.log`), including
Account ownership, cancellation, caller loss, concurrent drafts, shared capacity,
malformed options/results, fixture AccountsService, bounded image normalization,
source removal, cache publication/reuse/pressure and stale UI completions.
Two initial in-sandbox invocations could not bind their private bus; the
outside-sandbox run passes. `cargo clippy --locked -p canopa-portal --all-targets
-- -D warnings` passes (`/tmp/canopa-account-clippy-02.log`).
The all-target no-default check passes (`/tmp/canopa-account-core-check-01.log`),
and all five wire tests pass without desktop features
(`/tmp/canopa-account-core-tests-01.log`; zero feature-disabled library cases).

Private native development acceptance passes eight checks in
`/tmp/canopa-account-dark-04` and ten in `/tmp/canopa-account-light-01`. It uses a
dummy AccountsService on the private bus, exact fixture identities/images and
the public frontend. Checks cover initial Enter, explicit Share/Cancel, edited
draft retention across picker Cancel, same-window image selection, source
removal, normalized image readability after helper exit, field opt-outs,
owner-authorized Close, independent concurrent edits, responsive Settings and
caller loss. The ten-case fixture additionally checks exact decoded PNG pixels,
Clear without changing the source and window close from the embedded picker.
Dark and light edited/selected/excluded/public captures were inspected.

Earlier native failures are retained: dark-01 hid GLib schemas when isolating
data roots (fixture now sets GSETTINGS_SCHEMA_DIR); dark-02 used an incorrect
fixture signature; dark-03 appended to a focused Location field without selecting
its prior text (fixture now uses Ctrl+A and explicit row Open). These are not
native passes. Development runs used a private augmented descriptor; final
shipped-descriptor runs follow. The live desktop has not been changed.

After the picker extraction, standalone FileChooser passes all twelve dark
checks (`/tmp/canopa-chooser-account-dark-01`). The first light run timed out on
Niri window IPC while another native fixture was running; it is retained as
failed evidence and will be retried in a separate run.
Release documentation stages 29 files with 56 resolving offline links
(`/tmp/canopa-account-docs-01`). Shell syntax and diff whitespace checks pass.


## Account shipped metadata and final regression acceptance

Ten checks per appearance pass with the exact shipped descriptor and activation
files: `/tmp/canopa-account-shipped-dark-01` and
`/tmp/canopa-account-shipped-light-01`. Both result files record
`shipped_descriptor: true`; the fixture verifies the activated backend's actual
executable identity. Exact blue/pink avatar pixels, complete empty opt-out shape,
Clear, window close from the embedded picker, owner Close, caller loss, concurrent
independent names and public Account host responses pass. Edited, embedded-picker,
selected-image, excluded-field and public-frontend captures were inspected in both
appearances. Account is now advertised in the source descriptor. No live routing,
account mutation, grant persistence or service installation was performed.

Standalone FileChooser's isolated light retry passes twelve checks
(`/tmp/canopa-chooser-account-light-02`), alongside twelve dark checks in
`/tmp/canopa-chooser-account-dark-01`. This resolves the outstanding scoped
regression check, while retaining the earlier IPC timeout artifact.

After no-default testing, the ordinary desktop backend was restored with
`cargo build --locked -p canopa-portal` (exit zero, 1.25 seconds;
`/tmp/canopa-account-build-01.log`). The portal's normal dependency graph has 991
entries and no GTK/GDK/GIO/GLib packages
(`/tmp/canopa-account-normal-dependencies-01.txt`). Core/provider feature guards
pass (`/tmp/canopa-account-core-graph-01.log`,
`/tmp/canopa-account-provider-graph-01.log`). Packaged user-unit graph verification
passes (`/tmp/canopa-account-units-01.log`). Attributions were regenerated and
checked: 602 release dependencies (`/tmp/canopa-account-attributions-01.log`).
Documentation staging passes 29 files and 56 offline links
(`/tmp/canopa-account-docs-02`); Python syntax, smoke-script shell syntax, test
discovery and whitespace checks pass. This Portal-only slice did not change shared
crate/vendor source, and does not turn the separately recorded full workspace/core
failures green. Actual Flatpak/FUSE image export remains an acceptance gap.

Background contract investigation is in `BACKGROUND.md`. The current frontend
owns autostart and policy, while a backend must supply trustworthy compositor
application state and decisions. No Background methods are advertised yet.


## Background observation capability

The private native probe passes five checks in `/tmp/canopa-background-niri-02`
(initial successful `-01` retained). It records the installed Niri path, exact
version and executable digest. An initial complete snapshot, exact fixture-client
PIDs with distinct window IDs, focus changes, sibling/final-window closure and
reconnection all pass; the bounded journal contains only the two harmless fixture
clients. No portal, permission change, autostart change or app termination policy
was exercised. `BACKGROUND.md` records the source contract and credential seam.
The next required design/implementation boundary is trustworthy sandbox identity
and a bounded observer which reports unavailable state explicitly. Background
remains unimplemented and unadvertised.


## Background observer and decision implementation in progress

The previous goal turn completed Account and produced the five-case Niri
capability probe. This turn adds an actual Rust observer with bounded initial
state/line/window handling, one coalescing identity worker, pinned process-root
identity, liveness/birth/namespace/metadata validation, freshness and reconnect.
Unknown namespaces fail the whole projection instead of producing an empty map.
Initial numeric-PID association remains limited by Niri's API; ADR0034 records it.

The observer passes 18 model/process/stream tests and seven native checks in
`/tmp/canopa-background-observer-native-01`, using actual Niri and synthetic
bubblewrap roots. Metadata identities deliberately differ from Wayland labels.
Focus aggregation, sibling/final-window close, confirmed-host exclusion, unknown
namespace failure/recovery and shutdown pass. This is actual observer evidence,
not full Flatpak background-policy acceptance. Bubblewrap's unprivileged
namespace capability was checked independently; no Flatpak installation or FUSE
mount was required for this scoped fixture.

Background's exact D-Bus methods/signals and request ownership are implemented.
`/tmp/canopa-background-wire-01.log` passes the combined observation/decisions,
capacity, cancellation/caller-loss and legacy-autostart behavior contract. The
installed frontend 1.22.1 source performs autostart directly; the deprecated
backend method explicitly returns NotSupported. No legacy file writer is added.

A Notify-backed question actor offers typed Block/Allow once/Allow actions. Review
opens a request-owned Iced helper and forwards only its callback's bounded native
activation token. Owned cleanup continues after the request future is dropped and
uses a fixed per-question ID through lost Add replies. The first complete portal
run passes 44 model/actor tests plus six wire tests, with the explicit native
observer helper ignored during ordinary runs (`/tmp/canopa-background-tests-04.log`).
All-target checks pass through `/tmp/canopa-background-check-04.log`; an unused
mutable binding was removed afterward. Focused Clippy before the question adapter
passes (`/tmp/canopa-background-clippy-01.log`).

Background is served for development but is not in the descriptor. Native
question/review/public-frontend acceptance and common dialog transport regression
remain required. Linux helpers now request parent-death termination before exec;
this still needs abrupt-backend native verification.

Before advertising, Notify gains an explicitly negotiated request expiry option
with one bounded weak-record schedule. These question threads are transient and
withdraw without banner display or a surviving portal process. Ordinary portal
notification lifetime remains unchanged. `/tmp/canopa-background-expiry-check-02.log`
passes the combined all-target check (dependency warning: iced_layershell's
StreamExt import is unused under this particular feature union). Focused expiry
tests are running in `/tmp/canopa-background-expiry-tests-01.log`. This cross-app
phase requires the full workspace gate once the relevant edits are complete.

### Background question withdrawal verification

The combined default Notify/Portal all-target check passes in
`/tmp/canopa-background-expiry-check-02.log`. The first focused expiry run
(`/tmp/canopa-background-expiry-tests-01.log`) found two test assertions
incorrectly looking for runtime-only questions in retained history. The tests now
read the public Notify Control Center projection. The corrected run
`/tmp/canopa-background-expiry-tests-02.log` passes all 13 portal adapter tests,
including no-display/sender-loss expiry, replacement binding safety, expired
admission and shutdown withdrawing questions while retaining ordinary items.
The test run after that passes 25 Background library tests and one comprehensive
Background wire test (one native case intentionally ignored) in
`/tmp/canopa-background-tests-05.log`.

The current binaries were built with
`cargo build --locked -p canopa-notify -p canopa-portal --bins` using the same
root incremental debug graph; `/tmp/canopa-background-native-build-01.log` exits
zero. This package feature union emits an existing vendor `StreamExt` unused
import warning; no dependency fork was modified in this phase. Native question
presentation and abrupt backend-loss verification follow.

### Background native presentation and frontend compatibility

The initial dark native run (`/tmp/canopa-background-ui-dark-01`) exposed a real
review-window activation defect: the helper received the input-bound token but
ordinary `window::open` did not consume it. Background now uses the existing
`open_dialog_with_activation` path with no parent/modal claim. The corrected
binary build is `/tmp/canopa-background-native-build-02.log`. Seven native checks
pass in `/tmp/canopa-background-ui-dark-02` and
`/tmp/canopa-background-ui-light-01` (125% scale). They cover all three native
notification decisions, explicit review/Enter/Escape, exact fresh token use,
request owner Close, caller loss, abrupt backend SIGKILL terminating its helper,
and Notify-owned expiry under DND after backend loss. Both review frames were
inspected for readable controls and fit. These use development descriptor copies.

Combined Notify/Portal all-target Clippy passes with warnings denied in
`/tmp/canopa-background-clippy-02.log` (the dependency-only StreamExt warning
remains). This preceded the subsequent shared request-path adjustment.

The first real frontend monitor run
(`/tmp/canopa-background-frontend-dark-01`) found another integration defect:
xdg-desktop-portal 1.22.1 uses `/desktop/notify/backgroundN` handles, while the
shared Requests host only accepted `/desktop/request/…`. Background now opts
into the narrow notification-handle namespace; other dialog path validation is
unchanged. The comprehensive wire test uses actual notification-style handles
and rejects reserved/nested/unrelated paths. The failed native run is retained;
its frontend log shows `Invalid request path`, not an accepted permission result.
The new frontend fixture uses actual bubblewrap roots with synthetic Flatpak
metadata/instance records and a private PermissionStore. It never targets a real
application or changes the live desktop routing.

### Background shared helper regressions and headless checks

`/tmp/canopa-background-regressions-01/results.json` records passing native
FileChooser (12 checks), AppChooser (7), Access (8) and Account (10) runs in both
dark/100% and light/125% appearances after the common helper parent-death change.
Account uses the shipped descriptor. The same sequential runner's Background
frontend light run failed before any question: its five-second service activation
call timed out while Notify's renderer was starting. Its trace is retained.
The fixture now gives explicit service activation 15 seconds. Background itself
also separates cold service setup (at most ten seconds, inside the twenty-second
overall lifetime) from ordinary two-second RPCs. A delayed-capability regression
test and native on-demand cold activation check cover that distinction.

The corrected handle namespace passes 25 Background library tests and the
comprehensive wire test in `/tmp/canopa-background-tests-06.log`. Combined
all-target Clippy passes in `/tmp/canopa-background-clippy-03.log`. Headless
all-target checking passes (`/tmp/canopa-background-core-check-01.log`), followed
by all headless unit suites: 53 Notify library, 43 Notify binary and 23 Portal
library cases pass in `/tmp/canopa-background-core-unit-all-01.log`. These precede
the additional cold-setup regression test. Core/provider dependency guards pass,
and the saved normal graph `/tmp/canopa-background-normal-graph.txt` contains no
GTK/GIO/GLib/GDK crate.

The public dark frontend run `/tmp/canopa-background-frontend-dark-02` passes four
acceptance groups: all three observed-instance decisions and permission effects,
retaining existing Ask, actual namespace-originated RequestBackground Access
grant/cancel, and frontend-owned autostart enable/disable. Its pinned installed
frontend is 1.22.1, SHA-256
`e1429fbe1bf6c2203ba07f478b9e92e9221437b13a92eebafb0982748583a4f8`.
This exercises actual bubblewrap roots and synthetic Flatpak instance metadata;
it does not establish acceptance of a real Flatpak installation or document FUSE.

### Background cold activation and descriptor acceptance

`/tmp/canopa-background-tests-07.log` passes 26 Background library cases and
the comprehensive wire test, including delayed service metadata exceeding an
ordinary RPC deadline. `/tmp/canopa-background-native-build-04.log` builds the
current default binaries. Combined all-target Clippy passes in
`/tmp/canopa-background-clippy-04.log`. The new native fixture starts with Notify
unowned and verifies Background activates it on demand. Seven checks pass in
`/tmp/canopa-background-ui-light-02`. The corrected light frontend passes four
groups in `/tmp/canopa-background-frontend-light-02`; the current actual Rust
observer passes seven checks in `/tmp/canopa-background-observer-native-02`,
recording its compiled harness and compositor hashes.

Following those scoped checks, Background was added to the source portal
descriptor. The setup guide and release documentation list were updated.
Documentation staging passes with 30 files and 60 resolved offline links in
`/tmp/canopa-background-documentation-01`; companion unit verification passes in
`/tmp/canopa-background-session-units-01.log`. Both shipped presentation runs
(`/tmp/canopa-background-shipped-ui-dark-01` and `...-ui-light-01`) pass seven
checks, including cold activation and abrupt backend-loss cleanup. Final shipped
frontend runs and the required shared workspace gate follow.

Both shipped frontend runs now pass four groups:
`/tmp/canopa-background-shipped-frontend-dark-01` and
`/tmp/canopa-background-shipped-frontend-light-01` (125% scale). All final
Background presentation/frontend result files record the actual default binary
hashes and `shipped_descriptor: true`. The frontend result also records its
installed version/hash. Background is now the seventh advertised Canopa backend
interface, counting Notify's separate Notification descriptor. No installed
service files, running session preferences, commits or PRs were changed.

The required post-edit `bash scripts/check.sh --workspace` starts in
`/tmp/canopa-background-workspace-01.log`. Scoped acceptance is complete for this
interface; the broader goal and current shared gate remain active.

The current full workspace gate has passed all pre-test tooling, formatting,
workspace all-target Clippy with warnings denied (5m31s) and the 602-package
attribution inventory. Its workspace test executable build is still running in
the same log. No application source was changed during that build.

The current full test build completes in 27m38s. Files library passes all 372
cases with 34 expected ignores in 264.84s. Files integration is still live as
PTY 2892; recovery-catalog and killed Keep-both failures seen so far also appear
in the previous full workspace log. Its terminal result is not yet available.

During that verified wait, investigation of the earlier AT-SPI fixture aborts
found that installed at-spi2-core 2.62.1 removed the fixtures' --a11y=1 option
while retaining it in the usage string. The release-tagged source confirms it.
A minimized private native launcher/status/registry probe passes in
/tmp/canopa-background-atspi-probe-01 using only --launch-immediately, explicit
IsEnabled through D-Bus, the dbus-daemon implementation, private runtime/config,
memory settings and no display. No renderer check or source fix is claimed yet.
The existing actual assertions must be run under that corrected fixture after
current shared validation has a terminal result.

### Background full workspace outcome and accessibility fixture repair

The full gate is terminal 101 at Files integration: 596 passed, ten failed and
41 ignored in 590.00s. A direct comparison of the terminal failure-name sets
with the previous full workspace log proves all ten names are identical. Files
library passes 372 (+34 ignored). No transfer/Undo code changed.
The identical-graph artifact collection exits zero in 6.70s: all 746 artifacts
are fresh, with no rebuild and 34 test executables. The 22 unexecuted harnesses
all finish in `/tmp/canopa-background-workspace-continued-01`: 20 pass, while
Notify library (156 pass/one native fixture fail) and Shell main (1261 pass/one
native fixture fail) reproduce the launcher usage abort. Notify binary passes
44, Notify integration 278, Portal library 45 and Portal integration six with
one native fixture intentionally ignored. Other consumers pass as recorded in
the results JSON.

All unreached script stages pass in `/tmp/canopa-background-poststages-01`: Settings
activation, CLI build, configuration validation and vendor fixtures.

The unchanged renderer assertions pass under a corrected external fixture
(`/tmp/canopa-background-atspi-renderer-probe-01`, Notify 0.67s, Shell 2.08s).
The two embedded fixture scripts now remove --a11y=1, explicitly enable Status
over D-Bus, select dbus-daemon and use memory settings/local VFS without a display
or system bus. Matching-graph no-run compilation rebuilds only the two changed
test targets in 1m31s. The actual rebuilt parent tests both pass in
`/tmp/canopa-background-atspi-parent-tests-01` (0.87s/2.12s); no accessibility
assertion was removed or weakened. All-target workspace Clippy passes after this
fixture-only change in `/tmp/canopa-background-atspi-clippy-01.log` (50.07s).
The same obsolete option was corrected in the graphical desktop and media
fixture launchers; their Python syntax and embedded shell syntax pass.

The graphical desktop fixture now reaches its real Center keyboard-focus check
and fails there (`/tmp/canopa-background-desktop-a11y-dark-01`), rather than at
launcher startup. Its preserved native tree has no FOCUSED node despite visible
selection. A minimized input-device-lifetime probe is in progress; this is not
claimed as graphical acceptance. The native media fixture setup still requires
its own targeted execution. The broader workspace gate remains red for the
recorded Files failures.

The minimized graphical focus probe succeeds with an input-producing virtual
keyboard kept alive. The previous short-lived input device could clear native
compositor focus while leaving the Center selection highlighted. The desktop
fixture now holds that keyboard during focus observation, retaining its native
FOCUSED assertion. Both final runs pass:
`/tmp/canopa-background-desktop-a11y-dark-02` and
`/tmp/canopa-background-desktop-a11y-light-01` (125%). They exercise real Shell and
Notify, sender actions, native accessibility, Center reading/settings hierarchy,
geometry, source icons, focus, appearance reload, restarts, attention and SVG
pixels. Earlier failure artifacts and the successful minimized probe are retained.

The actual media fixture setup also passes in
`/tmp/canopa-background-media-setup-probe-01`, including generated media, isolated
PipeWire/audio setup, the real private launcher/registry and enabled Status.
This is setup/accessibility verification, not GTK application or playback
acceptance. Python syntax, embedded shell syntax, eight desktop-runner tests,
three test-layout tests and whitespace checks pass after the fixture changes.

All Background shared stages have now been attempted/continued. Its application
consumers, focused/native interface checks and fixed accessibility assertions
pass. The full workspace gate remains red for the exact same ten Files
integration failures; those were not changed or hidden. Only test fixtures and
native validation tooling changed for the accessibility repair; production
accessibility assertions and application behavior remain unchanged.

## DynamicLauncher implementation and initial checks (2026-10-29)

Implemented a version-1 Rust backend and Iced helper. The standard frontend
retains tokens, file publication, launch and uninstall. Review supports application
and web-app types, explicit approval, editable/fixed names and original read-only
icons. The backend denies every unattended policy query; unidentified host
bypass in frontend1.22.1 remains explicitly documented. Launcher admission uses
the existing shared16 request pool. Icon preparation has a separate four-worker
pool whose permits stay in detached synchronous work after request cancellation.
The backend retains <=1MiB encoded bytes and sends <=128-square preview pixels.
PNG/JPEG source dimensions <=512; SVG <=4096, external resources disabled.

New modules: dynamic_launcher.rs, dynamic_launcher_icon.rs and
 dynamic_launcher_ui.rs, with colocated test modules and one consolidated
integration module. No dependencies, shared vendor code or live service/routing
configuration changed. New native fixture uses private desktop/data/service/bus
instances and registered host identities, with a recording executable only.
Guide and ADR0035 are staged; descriptor advertisement is pending native results.

Initial check-01 failed at the nonexistent canopa_ui::text_input_style; the helper
now follows Account's shared theme with Iced's text input. Tests-01 builds in2m14s
and passes all11 new model/decoder/UI tests. The wire executable then cannot start
its D-Bus socket inside the sandbox (invalid empty address). Outside-sandbox
wire-02 reaches a meaningful introspection assertion: a Response type alias hides
its tuple fields from the zbus macro, producing one struct output in XML rather
than named uint32/dict outputs. DynamicLauncher uses the literal tuple return;
wire-03 passes in0.15s after1m11s build. Clippy-01 passes all portal targets with
warnings denied in45.20s before the later metadata/layout edits.

The same return alias existed in seven earlier published dialog methods. Replaced
only those method return spellings with the identical underlying explicit tuple
and added assertions for every method's two output fields in the new private-bus
test. Internal Rust aliases remain. This fixes introspection without changing the
actual response data. Corrected an obsolete Background module status comment.
Portal-all-01 now runs all unit/integration tests for those final source edits.

Native-dark-01 frontend aborts because fixture XDG_DATA_DIRS hid the installed
GSettings schemas; adding explicit GSETTINGS_SCHEMA_DIR fixes setup. Dark-02/03
reach the actual dialog and confirm initial Enter does not accept and preview
pixels are correct. Their empty-name action was aimed at the label; also corrected
fixture Ctrl+A to explicitly BackSpace for an empty replacement. Dark-04 now
confirms the blank name disables Install, but the validation message changes the
vertically centered field position before the second edit. The production review
row now aligns the name column at its top, preserving the input position when
validation text appears. Fixture input points at the captured actual field and
retains blank/edited frames. These failed attempts remain evidence, not acceptance.
All outputs are under /tmp/canopa-dynamic-launcher-*.

Python fixture syntax and three test-layout checks pass. Documentation stage-01
passes31 files/63 offline links. No broad workspace rerun is needed for these
portal-only edits; Background's completed gate remains red for the same ten Files
integration cases, as documented above. Further native and final scoped outcomes
will be appended here.

### DynamicLauncher final scoped/native outcomes

Portal-all-01 is terminal0:56 unit tests pass in2.08s and seven integration tests
pass in0.89s after1m16s compilation; one separate native observer case is intentionally
ignored. Every older published dialog method's introspection now has its two
correct named outputs, and existing wire/cancellation tests pass. A later UI-only
message copy change uses plain user-facing validation text; build-02 passes22.50s.
Current binarySHA256 is
1d5143c6c5a172c72265b2b2ccdfecb4b3d97fccec32f5ff509b65fbc2fdebcb.
Clippy-02 passes all portal targets with warnings denied in1m06s, including final
source. Headless all-target check-01 passes16.39s. Normal graph check finds no
GTK/GIO/GLib/GDK crate. No new Rust dependencies were added.

Native-dark-05 passes the backend/UI groups but its15s synchronous public call
times out during the frontend's separate sandbox icon validator, before any backend
review. A minimized invocation of that same installed validator on a32px PNG
passes0.10s (`validator-probe-01.json`), including benign sandbox NSS/HOME warnings.
The fixture now issues public PrepareInstall asynchronously, pumps its main context
and measures completion with a45s bound. This is a fixture change, not a backend
latency change; the exact cause of the earlier frontend timeout is not established.
Dark-06 completes actual PNG/JPEG/SVG approval, sender scope/replay, install/read/
launch/uninstall and resolved-identity unattended denial. Its last assertion wrongly
expected xdg_toplevel.set_parent in an xdg-foreign path. Retained traces prove the
actual exported handle's import and set_parent_of precede initial configure. The
corrected assertion checks that exact seam. Dark-07 exits0 with all eight groups.

After that scoped native pass, the real descriptor now advertises DynamicLauncher.
Final shipped runs both exit0:
- /tmp/canopa-dynamic-launcher-shipped-dark-01: eight groups,32.93s.
- /tmp/canopa-dynamic-launcher-shipped-light-01: eight groups,34.05s at125%.
Their result.json records the matching current binary hash and shipped_descriptor
true. Captures were visually inspected in both appearances. Each run uses the
shipped descriptor and activation service, real frontend1.22.1, registered host
identities, private bus/nested Niri and disposable data. All three original image
formats have native pixel checks and exact original-byte roundtrips. Public Close
cancels without issuing a token. No real user applications or data directories
were used. Actual Flatpak installation remains a separate acceptance environment.

Guide, ADR0035, README, context map and roadmap are updated. Documentation stage-03
passes31 files/64 offline links. Python AST, three layout checks and whitespace
pass. Five existing private native fixtures are running as regression coverage for
the earlier method introspection correction; results will be appended when terminal.
The full18-interface goal remains active. Lockdown's missing policy-source/write
ownership is the next design seam; investigation notes now explicitly distinguish
an atomic rename from a true concurrent-editor compare-and-swap. No policy or
configuration source was changed in that preparation.

DynamicLauncher regression runner is terminal0. All five existing dark/100%
private fixtures pass against the final current portal binary:
FileChooser12 checks (36.76s), AppChooser7 (12.54s), Access8 (35.79s), Account10
with shipped descriptor (58.48s), Background public frontend4 groups with shipped
metadata (55.43s). Results and per-fixture logs are in
/tmp/canopa-dynamic-launcher-regressions-01/results.json. No live jobs remain.
This completes this slice's scoped verification; Background's earlier shared gate
still has the same ten Files integration reds. No shared schema/vendor/application
changes were made in this slice, so that completed broad gate was not repeated.

## Lockdown policy module (2026-10-29)

The prior turn completed DynamicLauncher; the active18-interface goal remains
intact. This turn applies codebase-design's deep-module vocabulary to Lockdown.
Re-read root AGENTS/context and actual worktree. No Portal AGENTS exists. No
subagents used. Existing human test-consolidation edits preserved.

Verified official backend XML/docs: seven read/write boolean properties, no
version property. Official frontend1.22.1 archive retained at
/tmp/canopa-xdp-1.22.1.tar.gz, extracted source under
/tmp/canopa-xdp-lockdown-source-01/xdg-desktop-portal-1.22.1. Its actual main is
xdp-main.c, not the earlier guessed filenames. xdp-context.c creates a cached
GDBus proxy; FileChooser/OpenURI/Print/Camera/Location consult it. Microphone and
sound-output remain reported properties without corresponding consumption in
this installed source. Current GNOME lockdown.c retained at
/tmp/canopa-xdpg-lockdown.c binds mutable GSettings, including inverted location.
Local pinned zbus5.19 macro always emits on property setters, including no-ops.

ADR0036 defines portal-owned runtime policy in a private SQLite database under
XDG_STATE_HOME/canopa/portal, analogous to mutable DND/runtime state rather than
rewriting shared human KDL. Existing bundled rusqlite is reused; no version or
shared feature changes. Explicit database and read-only flags are available.
Missing writable state has false defaults without creating files; changed writes
create private state. Missing read-only administrative state fails startup/reload.
The default appearance flag retains its existing meaning. No shared config schema
or CLI checker namespace changed. This is an explicit source ownership decision,
not an in-memory substitute or a claim of CAS against external editors.

Implemented src/lockdown.rs plus private dbus/store modules and tests. One bounded
worker admits16 queued operations, with a single event/ack handoff. A SQLite
immediate transaction reads current values, changes one switch and commits before
publication. Reader reloads use one deferred transaction for consistent metadata
and values. No-op writes publish nothing. Publication survives initiating-future
cancellation and precedes its successful reply. GetAll clones one snapshot. Normal
shutdown closes admission/drains while relay remains available. Read-only, busy,
invalid and replaced sources return content-free errors without relaxing cached
policy. Read-only sources require existing valid state. Source paths are bounded
regular non-symlink files, with schema/application/version/boolean/table checks.
Observed source replacement requires explicit reload; arbitrary file replacement
during an active transaction remains unsupported, not described as CAS.

Main registers Lockdown, starts its relay, independently reloads appearance and
policy on SIGHUP and drains policy at shutdown. Existing Settings subprocess and
activation fixtures now explicitly select private XDG_STATE_HOME, avoiding any
ambient runtime policy during tests. Descriptor advertisement is pending final
source checks; guide/ADR/context/preparation notes and packaged-doc inventory are
updated. No live user policy or GNOME settings touched.

Initial model-01 passes8 tests after1m14s compilation (0.27s runtime). Wire-01
passes8 models and comprehensive typed property/signals/actual daemon restart/
read-only test (2.73s). Portal-all-01 passes64 units and8 integration tests, one
separate native observer intentionally ignored. Clippy-01 passes45.44s; Clippy-02
passes26.45s after reader-transaction refinement. Wire-02 passes the added missing
read-only-source case:9 model tests (0.26s), comprehensive wire test (0.57s).
Final table-kind check rejects a view masquerading under a matching header;
portal-all-02 now validates that final source. All build flags remain ordinary
incremental default debug in root target/. No cache cleanup.

Native-dark-01 reaches real saving-policy enforcement and the restored Iced
chooser; its fixture expected an empty cancellation dictionary, while frontend
returns status1 with uris=[]. Corrected the assertion to require cancellation and
no accepted URIs. Native-dark-02 passes all9 groups with actual frontend, private
Niri/bus/data and a disposable recording handler. It proves7 typed changes/no-op,
SaveFile/SaveFiles denial before UI, restoring Iced without frontend restart,
handler denial/restoration with exact URI, durable activated backend restart,
last-good restrictive state after bad reload, coherent valid reload and explicit
unwritable-source failure. Result hash (before final read-only/table refinements):
f6bd3ffcc8906690e0bda84e93af3570d0be87642fc9ed7c1c1f49170a1be83d.
No real user apps/files were used. Final shipped native runs will be recorded.

Documentation stage-01 passes32 files/66 offline links; changed Python AST and3
layout checks pass. Broad Background gate remains red for the same ten Files
integration cases; no shared schema/vendor or cross-application source changed
in this slice. Later scoped/final outcomes will be appended here.

### Lockdown final scoped acceptance

Portal-all-02 is terminal0 on final production source:66 units pass2.07s and8
integration tests pass0.97s after1m52s compilation. One separate native observer
case remains intentionally ignored. The later Settings fixture only removes the
ambient Niri socket; its focused compatibility test passes0.63s after12.75s link.
Both Settings fixtures now use private XDG_STATE_HOME and no host Niri observer.
The actual shipped activation/reload script passes all public Settings checks.

After the nine-group development native pass, the descriptor advertises Lockdown.
Both final shipped native runs pass all9 groups:
/tmp/canopa-lockdown-shipped-dark-01 (8.81s) and ...-light-01 (8.61s,125%). Both
record shipped_descriptor=true and matching current default-debug binary SHA256
cbdd6ef3f8faaab1563d4e37296a20a08f29dbde0dde790b0b3d2953ce11bd89.
The light restored-Iced capture was visually inspected; it shows only private
fixture data. Final read-only absence and table-kind safeguards are in that binary.
No interface is being advertised on the strength of constant false values.

Clippy-03 passes all portal targets with -Dwarnings in23.82s. Headless all-target
check-01 passes16.16s, including the new core SQLite dependency. Core dependency
guard excludes GUI/audio, and normal portal graph has no GTK/GIO/GLib/GDK crates.
The attribution check initially reports stale usage entries for the new dependency
edge; regenerated inventory still contains602 existing release dependencies and
its check now passes. No dependency versions or shared/vendor source changed.
Companion-unit check, three test-layout cases, changed Python AST, package fmt and
whitespace pass. Documentation stage-03 passes32 files/67 offline links. README,
ADR0036, guide, context map, preparation and roadmap reflect implemented status.
All tool jobs are terminal; no approval or clarification pending.

The full18-interface goal remains active, with nine implemented/advertised
interfaces. The broad gate's same ten Files integration failures remain recorded;
it was not repeated for this portal-only source/usage-inventory slice. Wallpaper
inspection now records actual backend XML and frontend permission/URI/FD flow in
WALLPAPER.md. Repository search finds no supported wallpaper renderer; the bundled
Shell Lock button is a generic session-owned swaylock launch without an image.
Actual display and locker ownership needs an integration decision before that
portal can be implemented honestly. Shell AGENTS was read; no Shell source changed.
No new permission question, real user policy change, installation, commit, PR or
cache cleanup occurred. Disk has roughly16GiB free.

## Wallpaper owner: Shell runtime and scoped native acceptance

ADR0037 adds the actual display/locker owner before any public Wallpaper portal.
Shell src/wallpaper/{store,runtime,locker}.rs owns descriptor preparation,
durable selections, one bounded serialized worker and the Swaylock handoff.
app/wallpaper.rs owns a distinct per-output Iced surface registry. Private bus
org.canopa.Shell.Wallpaper exposes Wallpaper1 Apply(h,u) and Destinations at
/org/canopa/Shell/Wallpaper. Masks1/2/3 mean background/lockscreen/both. Unsupported
locker capability and invalid masks fail before admission. At most one preparation
and two queued requests; work admitted before caller loss remains owner-owned.
Local App ack follows durable publication; it is not a physical repaint ack.

PNG admission uses regular owned descriptors,128MiB encoded,16,384 maximum
dimension,64Mi source pixels and512MiB best-effort decoder allocation. Full
RGBA and canonical PNG are independent of future portal preview pixels. SHA256
assets and version1 selection.json live under XDG_STATE_HOME/canopa/shell/wallpaper
with absolute HOME fallback. File lock, immutable-image replacement, fsync and
atomic selection replacement preserve both requested destinations and retained
other selection. Cleanup removes obsolete managed assets and interrupted private
temporary files. Modes0700directory/0600metadata/0400images; state reads do not
create a missing source. Symlink final leaves, nonregular/oversized/damaged sources
and invalid selection versions/keys fail. These are logical resource bounds,
not total process/GPU caps or filesystem deadlines.

`canopa-shell lock` pins the chosen regular descriptor while holding the state
lock, makes only that descriptor inheritable immediately before exec, and runs
Swaylock with --image /proc/self/fd/N. Missing/invalid/busy state falls back to
ordinary locking; it does not wait behind an image update. The bundled literal
session-owned Launch action uses this command. User launch argv, Niri and locker
configuration stay user-owned. New Wallpaper Surfaces cover the complete named
output, reserve no space and cannot take input. Existing bar-layer Background
surfaces are recreated on that output after the new wallpaper's first configure,
through the existing retirement/foreground cleanup path. Later images and resizes
retain windows. This fixes a real same-layer stacking failure found natively.

Validation:
- check01 compiled successfully with an Event visibility warning, repaired before
  subsequent Clippy. Tests01 passed8 but the private bus bind was sandbox-denied;
  escalated private-socket validation passes. Owner05 passes12 cases including
  two-output registry/reordering, coherent target preservation, missing state,
  busy-lock fallback, open-image lifetime across GC, corrupt input, interrupted
  write cleanup, concurrent writers, private wire and App acknowledgement.
- Shell tests02 passed1273 units. Tests03 added interrupted-write coverage and
  passed1273 but failed the unchanged panel short-action row assertion
  (Reply.y215.8 vs Dismiss.y258). That exact case passed alone27.59s; final built
  unit executable passed all1275 serially91.68s. No unrelated panel code changed.
- CLI01 passed1 descriptor/exec/fallback integration. Final Shell integration02
  passes all30 cases1.36s and rebuilds the actual executable with the stacking fix.
- scripts/test-wallpaper-owner.py validates actual nested Niri pixels, complete
  output corners, original full-resolution storage, both/individual targets,
  source deletion, invalid inputs,125%->150% scaling, restart, two retained assets,
  a recording locker and installed Swaylock loading the inherited descriptor.
  Final top04 and background03 pass7groups each, using identical Shell SHA256
  dcbd94ac5fb05f8f3899df49f51dc8423191aff5a421de1d5a3b0866ea0f69be.
  Their captures were viewed. The top final uses a fixed copy under
  /tmp/canopa-wallpaper-native-bin so the concurrent gate cannot replace it.
  All native source/state/bus/compositor/locker targets are disposable.
- Top01 exposed a fixture first-frame race; it now waits for the real bar color.
  Background01 exposed actual stacking, fixed as above. Top03/background02 passed
  all desktop checks but installed Swaylock did not report image load within15s
  when the scenarios ran simultaneously. Sequential final runs pass with a30s
  bound; a timeout now captures only the private locker's process diagnostics.
  No production fallback or ownership behavior was changed to mask these timeouts.
- Workspace gate01 is still running. Tooling, all-workspace Clippy-Dwarnings
  (7m06s) and602-dependency inventory pass; it is compiling workspace test targets.
  Core dependency guard, Python AST, whitespace and three test-layout cases pass.
  Staged docs01 resolve32files/67offline links. No vendor/version change; Shell
  reuses existing optional sha2/tempfile edges, and inventory was regenerated.

Wallpaper is unadvertised. Next: freeze source URI/FD data before review, provide
Iced preview with parent/request cancellation, call the owner only after user
acceptance, and verify the real public frontend and owner-loss/error paths before
descriptor publication. Do not infer a set-on default from backend XML: the
inspected GNOME backend ignores set-on, so it does not establish a default.
Full18-interface goal remains active; nine advertised interfaces are unchanged.

### Wallpaper owner workspace gate terminal outcome

Workspace gate01/session17484 is terminal101. All pre-tooling, full formatting,
workspace all-target Clippy-Dwarnings (7m06s),602-dependency inventory and complete
workspace test compilation (16m55s) pass. Apps11, Assets integration11, CLI3 and
Config integration12 pass. Files library finishes219.05s:371pass/1fail/34ignore.
The failure is
playback::tests::video_uses_the_opened_file_for_paused_frames_seeks_rewind_and_owner_drop,
whose8s wait sees phasePaused,position0,seekingfalse,noticeNone. The exact same
workspace-built library executable reruns that case on a private bus and passes
0.96s (/tmp/canopa-wallpaper-files-playback-rerun-01.log). No playback source was
changed by this slice. This is evidence of the observed parallel failure and
isolated pass, not proof of its scheduling/root cause or a green parallel gate.

Cargo stopped before Files integration and later harness/script stages. Their
older ten-failure baseline remains historical evidence; it is not a fresh result
of this run. Shell's final1275 serial units,30 integration cases and both seven-group
native runs were completed separately as recorded above. All tool jobs are now
terminal, with no pending approval/question, install, commit, PR or cache cleanup.
The next goal continuation is the public Wallpaper backend and Rust/Iced preview,
which remain unimplemented/unadvertised. Preserve the full18-interface objective.

## Wallpaper portal: frozen source, Rust/Iced review and real owner integration

ADR0038 adds the actual backend flow on top of ADR0037. Portal wallpaper.rs
publishes exactly SetWallpaperURI(o,s,s,s,a{sv})->u with no properties/version.
Typed requests retain explicit background/lockscreen/both; omission defaults to
background as an explicit Canopa policy. show-preview type is validated, but all
requests show Iced review. The shared Requests registry owns handle admission,
sender Close, caller loss, retirement and concurrent Settings responsiveness.
Production resolves the Shell unique owner before preparation, verifies destination
capability, freezes the source, reviews it through the anonymous-pipe helper,
rechecks owner identity and applies that sealed descriptor only after Apply.
It does not start Shell, rewrite bindings, retry ambiguous requests or switch to
a replacement owner. Admitted Shell work can finish after a later caller loss.

wallpaper_image.rs admits two preparations/reviews.32MiB local sources require
regular descriptors; HTTP(S) streams have the same byte bound even without length.
Shared RasterBatch applies orientation and16,384dim/64Mipixel/512MiB best-effort
decoder limits. PNG/JPEG/GIF/WebP are explicit features; canonical full-resolution
PNG is capped128MiB in a sealed memfd. A separate384px aspect-preserving base64
RGBA preview fits the1MiB helper frame. Neither helper nor acceptance can reread
the caller URI or replace the full image. Blocking workers retain permits after
cancellation; custom system DNS also holds two separate permits until lookup
actually ends, with at most32 returned addresses.

HTTP uses Reqwest0.13.5 with Rustls-only features:20s total/5s connect/10s read,
five redirects, no embedded URL credentials or other schemes, cookies,
decompression, referrer forwarding or retry. Redirect and chunked-body limits,
download cancellation and resolver admission are tested. Source errors expose no
URL/path/body/credential. Cargo added35 packages including optional/platform
entries, with no previous package updates reported. The normal combined release
inventory grows602->632 and is regenerated/checks green. No vendor source changed.

wallpaper_ui.rs uses the shared Iced theme/font/controls, the existing foreign
parent/modal adapter, preview/dimensions/destination and explicit Cancel/Apply.
No GTK/GDK/GIO/GLib crate appears in the normal Portal dependency graph. Main adds
the private --wallpaper-dialog mode and registration. The shipped descriptor now
advertises Wallpaper only after scoped preparation acceptance. README, context
map, ADRs, user guide and package-doc list are updated.

Validation completed:
- all06:76 Portal units pass2.69s;11 integrations pass1.07s with the separate native
  observer intentionally ignored. Additional owner-replacement case plus all
  Wallpaper wire/production cases pass4/4 in0.30s. Corrected HTTP-header fixture
  runtime rerun07 passes all8 image cases in2.57s (terminal0). Wire tests assert
  exact method/response,
  absent properties, ownership/cancel/loss and frozen accepted bytes after source
  deletion. An old owner remains alive after name release in the replacement
  test; neither it nor its replacement receives the image after user acceptance.
- Clippy02 all Portal targets-Dwarnings passes12.09s; headless all-target check01
  passes21.12s. Core guard excludes GUI/audio.632 inventory, changed Python AST,
  whitespace and final docs04 (33files/69offline links) pass.
- Native scripts/test-wallpaper-portal.py uses actual shipped activation and
  metadata, a real frontend, nested Niri, exported C Wayland parent, private
  local/HTTP/HTTPS sources, state and recording locker. Final shipped dark01 and
  light01/125% pass10groups each. Both backend SHA256
  6f8c87e725400eee3d53397d68909e028a5a133045b911340559c82cd6fa61cd;
  Shell SHA256 dcbd94ac5fb05f8f3899df49f51dc8423191aff5a421de1d5a3b0866ea0f69be.
  Fixed copies avoid concurrent Cargo replacement. Cases prove real public FD and
  URI input, foreign parenting, aspect pixels, no publication on Cancel, deleted
  source after review, full output dimensions, actual desktop pixels and locker
  hash, individual/default destination preservation, all Close/caller paths,
  exactly-one HTTP fetch, oversize error, trusted private CA TLS success and
  untrusted-certificate refusal, no partial both when locker disappears, bad
  source, lost/absent owner, restart and scaling. All fixture resources are private.
  Earlier dark04/light01 development captures and final shipped-light parented
  review were inspected: preserved aspect, clear controls and no clipping.
- Full workspace gate01/session55886 exits101 at Files library:371pass/1fail/
  34ignore in248.61s. The failed case is
  operations::transfer::directory_boundary::reviewed_directory_choices_and_preappend_publication_keep_their_expansion:
  directory_boundary.rs:25 reports "private directory worker did not settle".
  Failure roots retained by the test: /tmp/canopa-directory-expansion-SnRj4h and
  crates/canopa-files/.canopa-directory-expansion-output-EoefDx. No cleanup done.
  Same executable target/debug/deps/canopa_files-ffb9094e395b619d passes that exact
  case alone7.69s on a private bus (retry01/session74516 terminal0). The previously
  failing video case passes in this gate. No Files production change or proved
  root cause; the parallel gate is not green. Tooling, full fmt, all-workspace
  Clippy-Dwarnings8m27s,632 inventory and all-target test compilation31m11s pass.
  Files integration, later workspace package tests and post-test scripts are not
  reached. Logs: /tmp/canopa-wallpaper-portal-workspace-01.log and
  /tmp/canopa-wallpaper-portal-files-directory-retry-01.log. All jobs terminal.

Observed failed attempts and corrections: check01 caught temporary syntax during
source refactor; check02 caught a fdo-closure error conversion. Tests02 passed6
source cases, but the macro exported SetWallpaperUri, causing two wire failures
and a pending receiver. Explicit URI naming, exact introspection and bounded
receivers fix that; tests03 pass8unit+3integration. Native01 assumed independent
fallbacks float; fixture now explicitly arranges valid tiled fallbacks. Visual
inspection found imageops::thumbnail resized to a square; integer fitted sizes
and unit/native aspect checks fix actual preview behavior without changing full
pixels. Native02 applied both after deletion but clicked a stale resize height;
the fixture now waits for width and height. Development dark03 passes8groups,
dark04/light01 pass9 with TLS, dark05 passes10 with unavailable-locker capability.
Clippy01 flags ignored test read amounts; a bounded complete-header reader fixes
it, and strict Clippy02 passes. None of these failed/intermediate runs are
substituted for final acceptance.

Wallpaper is the tenth advertised interface; the full18-interface goal remains
active. Real Flatpak acceptance and other remaining interfaces are not claimed.
No user setting/service install, commit, PR, worktree, external message or agent
cache cleanup occurred. Preserve concurrent human work and ordinary debug cache.

Read-only Screenshot follow-up during this gate is captured in SCREENSHOT.md.
Pinned Niri IPC mutates the clipboard and dispatch acknowledgments precede PNG
encoding. Disposable native inventory exposes wlr-screencopy3/xdg-output3, no
ext-image-copy source/capture globals. Its initial aligned-whitespace parser
assertion failed; corrected offline inventory is retained, no new frame captured.
Frontend1.22.1 source accepts target subsets and exports result URIs asynchronously.
Screenshot production and descriptor remain untouched. The broader goal is active.


## Screenshot: request-owned capture adapter,2026-10-30

ADR0039 establishes the finite portal-request capture seam. capture.rs and its
private wayland.rs use a fresh Tokio-connected Wayland socket, bounded polling,
end-to-end deadline, drop cancellation and two permits retained with snapshots.
The pure-Rust adapter copies supported32-bit shared-memory formats into upright
RGBA, respecting native-endian channels, alpha, row padding, y inversion and
inverse output orientation. Aggregate limits precede buffer allocation/copy.
Fixed-size seals protect compositor memfds; final snapshots own only pixels.
Metadata changes fail instead of mixing layouts. No screenshot IPC/clipboard
write, temporary compositor filename, live desktop action or interface stub.

Optional desktop Wayland client/protocol dependencies reuse existing versions;
no new package versions. Cargo.lock adds the direct edges; inventory still632,
only lock SHA changes. Core excludes native capture/GUI; no normal GTK/GIO crates.
No vendor or other application implementation changed. The shared C fixture adds
an opt-in asymmetric pattern; its existing default color is preserved and compiles
with-Wall/-Wextra/-Werror. Screenshot remains unadvertised with no wire/UI/cache.

Final all04 passes82 units3.56s,12 integrations1.04s,2 native fixtures intentionally
ignored. Clippy02 all Portal targets-Dwarnings passes27.70s. Headless all-target
check01 passes2m25s including artifact wait. Core dependency guard passes. All
source/native outputs use ordinary incremental debug target/, with a fixed native
harness copy so concurrent Cargo cannot replace acceptance artifacts.

Native final sway04 passes4groups and niri03/125% passes3groups. Shared harness
/tmp/canopa-screenshot-capture-bin-02/integration SHA256
9f8155da2c710356bb519a06625df1366eeb73e86caedf5aac4da032cc8e7e57.
The script compares complete physical frames with independent grim output, plus
logical metadata, held-snapshot admission/release, unchanged private clipboard,
all8 transforms on asymmetric pixels, negative origins/mixed scales across2outputs,
hotplug, descriptor retirement and socket absence/recovery. Niri125%->150% also
proves retained pixels stay immutable. For fractional logical rounding, Niri's
physical1280x800 frame is retained while grim composites to1280x799; independent
physical-mode dimensions and thousands of uniform interior colors are asserted,
not an exact equality claim for that resampled reference. All resources private;
this establishes a capture adapter, not Iced review or public portal acceptance.

Intermediate failures: check01 had a PollFd borrow mismatch, corrected in check02
(21.75s pass). Unit01 passed5. Sway01 passed early/pixel cases but compared before
hotplug settled. Niri01 assumed its fixed nested transform could be changed;
Niri02 reached150% but expected the resampled reference height to equal raw mode.
The fixtures now wait for settled output frames and record rounding separately.
Asymmetric Sway02 then exposed an actual quarter-turn inverse bug: a180-degree
correction made its90-degree result exactly equal the reference. Fixed inverse
90/270, retained reflected transforms, and added correct pixel-grid expectations.
Clippy01 rejected bare third-party _90/_180/_270 variant names; qualified variants
fix that without an allow. All02 passes81 units/12int; all03 adds display policy
but fails a fixture assumption that a500ms deadline must begin after its first
protocol write (81pass/1fail). Deadline coverage now includes queued startup, and
all04 passes82/12. Sway03 failed clipboard seed startup before capture; bounded
readiness now tolerates an absent initial selection. Final sway04/niri03 pass.
None of the intermediate runs is substituted for final acceptance.

Workspace gate01/session88036 exits101 at Files integration:596pass/10fail/
41ignore in589.00s. Its Files library passes372+34ignore in226.57s, including both
previous library failure cases. Tooling, full fmt, all-workspace Clippy5m20s,
632 inventory and all-target test compilation10m36s pass. Later package tests and
post-test scripts were not reached in this gate. Docs01 passes33files/69offline
links; changed-file whitespace passes. Normal Portal tree contains no GTK/GDK/
GIO/GLib. The gate remains red; scoped adapter acceptance does not establish broad
workspace/desktop acceptance.

Current failures exactly equal the ten names in
/tmp/canopa-background-workspace-01.log (cases:: prefix normalized). A machine
comparison is retained in /tmp/canopa-screenshot-capture-workspace-failures-01.json.
The exact current integration executable integration-a96be2701c532838 repeats the
catalog failure alone10.19s (retry01/session81021 terminal101), "Recovery catalog
did not settle" at tests/recovery_catalog.rs:20. This is not a parallel-only pass.
The 500-item destination-restoration case exceeds its timing gate (45.06s), while
other saved-history/crash assertions fail as shown in the log. No Files production
change or root-cause claim. Retained test fixtures were not deleted. All jobs
terminal; no cache cleanup, install, user config, commit, PR or vendor change.


Current Files integration failure names:

- `recovery_catalog::pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`
- `undo_destinations::restarting_a_killed_keep_both_destination_restore_preserves_newer_files_and_cleans_stages`
- `undo_unavailable_moves::an_external_move_does_not_let_older_copy_undo_own_an_independent_restored_destination`
- `undo_unavailable_moves::an_unavailable_moves_previous_destination_can_be_restored_without_returning_the_missing_item`
- `undo_unavailable_moves::changing_the_shown_saved_destination_refuses_restore_and_keeps_it_retryable_after_restart`
- `undo_unavailable_moves::restarting_a_killed_saved_only_restoration_preserves_missing_items_and_older_copy_undo`
- `undo_unavailable_moves::restoring_a_previous_destination_does_not_adopt_it_for_an_older_copy_of_the_missing_incoming_item`
- `undo_unavailable_moves::restoring_only_the_previous_destination_does_not_relocate_archives_as_if_the_missing_link_returned`
- `undo_unavailable_moves::restoring_the_shown_previous_destination_approves_its_current_version_in_one_review`
- `undo_unavailable_saved::source_only_replace_preserves_newer_original_work_while_saved_restoration_remains_pending`


Read-only next-workflow investigation: current GNOME screenshot.c reports backend
v2 and routes interactive calls to compositor selection. screenshotdialog.c sets
skip_dialog from permission_store_checked; permitted non-interactive capture does
not require a second preview. Source links and pending Canopa policy are recorded
in SCREENSHOT.md. No Iced/public wire/file publication was added in this phase.


## Screenshot workflow and publication — 2026-10-30

ADR0040 adds exact Screenshot/PickColor v3 and AvailableTargets5 (Screen/Area),
shared Requests lifetimes, frozen sealed RGBA descriptor transport to Iced, original
pixel/color and crop validation, and private retained immutable PNG publication.
Two admitted snapshots survive through detached composition workers; cancellation
cannot prematurely release their permits. Frontend permission_store_checked allows
permitted noninteractive Screen capture without an extra backend prompt. Window
sources remain unavailable. Multi-output composition uses maximum physical density,
negative logical origins and transparent gaps; single output retains exact pixels.
Private cache:0700 directory/0400 unique PNGs,128MiB output,1GiB/128entries,24hour
minimum retention, anchored publication, bounded scans/locking and old-only eviction.

Shared process.rs now supports explicit borrowed descriptor inheritance, clearing
CLOEXEC only in the child. No dependencies or vendor changes in this workflow phase.
Whole snapshot ownership in publish_sync fixes precise-closure field capture that
could otherwise release admission before a detached encoder exits.

Scoped all05 passes93 unit tests and13 integration tests (two explicit native
entry points ignored). Clippy03 all targets-Dwarnings passes; clippy02 was
terminated143 with no completed result and is not counted as a pass. Headless01
passes24.49s; core dependency guard passes and normal graph excludes GTK/GDK/GIO/
GLib. Docs01 stages34files/71offline links. Restricted session-units01 cannot set
socket options in the sandbox; the escalated workspace gate will run that check.

Native production binary /tmp/canopa-screenshot-portal-bin-02/xdg-desktop-portal-canopa
SHA256 d481d545a71bfd23ec485ae6bd0f2e1e7eaa7584144feff6b6bbf0150ed947c0.
Prepared dark04 and light02/125% each pass11groups: parented review, initial Enter
and Cancel, immutable full image after live desktop change, exact keyboard/pointer
PickColor, boundary-crossing exact keyboard Area, actual pointer drag, Close,
caller loss, real frontend Access consent and permitted no-review repeat, backend
loss/restart with durable prior images, explicit publication failure and unchanged
clipboard. Both captures inspected: visible actions, aspect retained, no clipping.
Composition01 on private two-output Sway passes2groups with actual production PNGs:
negative origin, different scales, transparent gaps and subsequent output removal.
Earlier dark03 reached restart but raced frontend capability refresh; wait fixed.
Light01 forced680px window onto640logical viewport; fixture now fits available
viewport, then light02 passes. No production fix was required for those two races.

Descriptor now includes Screenshot. Shipped dark01/session13004 and shipped
light01/session77300 both pass all11groups with the same binary hash and actual
activation/descriptor. Workspace01/session55194 remains running. Workspace log /tmp/canopa-screenshot-portal-workspace-01.log.
Do not count running jobs as passes. No live routing/install/user state changed.


ScreenCast read-only follow-up: SCREENCAST.md records exact installed v6 backend
XML and owned-session proposal. Host i915 render node exists outside sandbox.
Private hardware GL Weston/Niri+PipeWire probe01 exits1: installed26.04 Winit
cannot provide GBM, so Niri closes before any media node. Version-matched upstream
backend confirms Tty-only GBM, while current main adds nested support. This is
failed native media evidence, not successful streaming or a Canopa interface.
No live capture, DRM takeover, VT change, compositor upgrade or routing edits.

Workspace progress (not terminal): tooling/session units/full formatting pass;
all-workspace Clippy57.20s,632inventory and all-target test compilation4m16s pass.
Files library running. The earlier restricted service-unit error is superseded
by the actual escalated check; no source repair was needed.


Final Screenshot workflow gate: workspace01/session55194 terminates101. Files
library372pass/34ignore233.90s; integration596pass/10fail/41ignore584.16s. The ten
parent failure names exactly match the capture gate/Background baseline after
normalizing cases:: module prefix. JSON comparison:
/tmp/canopa-screenshot-portal-workspace-failures-01.json. Nested crash-helper
failures are separate child diagnostics, not additional parent cases. No Files
production changed in this phase and no root-cause claim is made. Later package
tests and post-test scripts were not reached. Full workspace remains red; scoped
Portal93unit/13integration and native11groups per appearance plus composition2
are green. All tool jobs terminal. Final docs02 stages34files/71links; normal
Portal graph no GTK/GDK/GIO/GLib. No live routing/state/install changed.

Next full-goal slice is the owned ScreenCast session/compositor seam. Screenshot
is now eleven implemented/advertised interfaces; remaining seven retained.
Owner probe01 also verifies empty raw Niri sessions survive creator disconnect;
explicit cleanup for backend death needs design/verification. SCREENCAST.md
records that and the version-specific failed native-media probe. No Canopa
ScreenCast interface is implemented or advertised, no compositor upgrade made.


## Owned ScreenCast capture worker — 2026-10-30

Concrete progress under ADR0041: new screencast_capture.rs/transport.rs/worker.rs,
unit private-bus fake, integration real worker exec, main --screencast-worker,
and scripts/test-screencast-capture.py. Controller8workers/16total sources retain
admission until worker exit, independent cleanup worker survives backend SIGKILL,
unique compositor owner binding/caller watches, monitor+window/cursor calls,
ordered ready nodes, no fake window geometry, partial/late mutation cleanup.
No public ScreenCast interface or Iced source consent yet, no descriptor change.
No dependencies, shared crates, vendor or other applications changed this phase.

All05/session98152 passes99units13.07s/17integration1.47s+2explicit native ignores.
Clippy02/session45535-Dwarnings all Portal targets passes29.59s. Headless01/
session81373 passes28.01s, coreguard and normal no GTK/GDK/GIO/GLib graph pass.
Portal fmt, whitespace/PythonAST and docs01(34files/71links) pass. Native02/session
18334 passes4 real Niri error/cleanup groups with fixed bin02 SHA256
 ea47be8d2d698f20f19aea33580fd51066985321f6bbdcbbd3d88bec94f59a31.
This is explicitly not successful media evidence. No full workspace rerun for
this Portal-only source phase; last full gate's ten Files failures retained above.

Native01 failed because subprocess.run closed the worker lease before Create;
the production worker correctly cancelled. Fixture now holds stdin open via
Popen. Earlier compiler/test fixture errors and Clippy test-conversion fix are
recorded in SCREENCAST.md. All jobs terminal, no permission/question pending.
No live compositor/user config/state/routing/install changed. Next implement
public session registry and Iced monitor/window consent, then actual frontend
and successful media acceptance. Broader18-interface objective remains active.

## ScreenCast v6 sessions and Iced consent — 2026-10-30

ADR0042 adds public CreateSession/SelectSources/Start, owner/app/path-bound16-slot
session actors, exact v6 streams/64-bit serials and always-fresh persist_mode0
consent. Requests remain short-lived; active recordings outlive Start. Cancellation
and guarded reply delivery close discarded grants, including a presenter that
ignores its cancellation watch. Session.Close, caller/source loss and shutdown
retire actors and owned recordings. No shared lock spans review or startup.

Production pins Niri's D-Bus owner/PID and authenticates the captured IPC socket
peer. Bounded outputs/windows inventory is refreshed by an Iced helper and checked
again after Share. The helper starts empty, supports search/single/multiple,
keyboard navigation and stable identity updates, and imports existing parent/
modal/activation and appearance controls. A bounded Rust PipeWire resolver returns
actual object.serial values; monitor-only logical geometry, no placeholder window
size. Optional desktop PipeWire0.9.2/v0_3_64 adds16lock packages; no existing version
update reported. Dependency attribution regenerated/checked649 release edges.

Final all01/session49213 terminal0:104unit pass/1native ignore,21integration pass/
2native ignores. Clippy02/session67043 terminal0,44.39s. Headless01/session97725
terminal0,33.75s. Normal Portal graph excludes GTK/GDK/GIO/GLib; headless excludes
Iced/Wayland/PipeWire/libspa too. Coreguard, Python AST, whitespace, protocol
provenance byte comparison and5docs/92local links pass. No vendor fork modified.

Final binary /tmp/canopa-screencast-workflow-bin-02/xdg-desktop-portal-canopa SHA256
068761fdf72a99b4743f1152ff92964d3451a9adfaa65c3bfab6a65585858863. Final native
frontend/Iced dark01/session99875 and light01/session91626 terminal0,6groups each:
initial Enter/Cancel, Session.Close, Request.Close, app disconnect, actual display
and window consent/start failure. Inspected both appearances, no clipping. Native
identities01/session10006 terminal0,3real PipeWire metadata groups including actual
numeric ID reuse with a fresh serial. Harness SHA256
8bf881298d88419bdd156c407ae4e21d7401f3d4e833b12a1396b63025ab60a6.

Earlier full native dark/pair failures traced Escape delivered when ephemeral
wtype sent Down; one persistent full-US-keymap test keyboard fixes sequential
requests. All temporary production action traces removed before final artifacts.
No production cancellation weakened. Tests explicitly report no video-flow proof.

Required full workspace gate running session34639, log
/tmp/canopa-screencast-workflow-workspace-01.log; outcome must be recorded below.
Eleven interfaces still advertised; ScreenCast remains absent from descriptor.
Successful restricted-frontend consumer frames and ongoing visible sharing/stop
controls remain. Pinned upstream Niri nested-GBM candidate and cache inventory in
SCREENCAST.md; no external compositor build/install/live session takeover made.
Full18-interface goal remains active. No commits, PRs, routing or user-state edits.

### Shared gate outcome, before the keyboard-only correction

workspace01/session34639 terminal101. Tooling, global fmt, workspace Clippy3m40s
and attribution649 pass. All-target test compilation13m43s succeeds. Early
Apps/Assets/CLI/Config/File suites pass; Files library372pass/34ignore,219.12s.
Files integration596pass/10fail/41ignore,590.14s. The normalized ten terminal
failure names exactly match the prior Screenshot/capture gate: no added/resolved
names. Comparison /tmp/canopa-screencast-workflow-workspace-failures-01.json.
This establishes no cause or general flake claim. Later Cargo packages and
post-test scripts are not reached. The full workspace remains red.

After that gate's test build, the Portal-only keyboard correction adds Ctrl+Enter
and a visible hint, plus Up/Down leaving search focus for source selection. Plain
Enter still grants nothing. A native scenario now uses Tab, typed search, Down,
Space and Ctrl+Enter; window confirmation retains the pointer path. One added
behavioral unit case covers empty/unavailable/valid choices. No dependency or
shared crate/vendor change follows the gate. Final all02/session24225 currently
running; final scoped Clippy/build and both native appearances remain before the
correction is validated. Bin-02 remains the pre-correction fixed artifact.

### Final keyboard correction and next native build

Final all02/session24225 terminal0:105unit pass/1native ignore,21integration pass/
2native ignores; compile3m40s, execution3.53s/1.84s. Clippy03/session8996 terminal0,
39.14s. Build02/session33481 terminal0,9.87s. Final fixed bin-03 is
/tmp/canopa-screencast-workflow-bin-03/xdg-desktop-portal-canopa, SHA256
82f5194e5e66d8a3f426431241c4f3eca7e12cd75646c9860bb7a96ba0f85a54;
unit harness SHA256
96b05d3a5f6b2cdadb7615012621e97c55a4c7584ea3f00e2f1c1ec740c4239a.

Keyboard dark01/session90379 and light01/session26713 terminal0,6groups each with
bin-03. Actual Tab→typed search→Down (blur search)→Space→Ctrl+Enter reaches monitor
startup; initial plain/Control Enter without selection cannot grant. Window Share
still tests the pointer. Inspected both selected-display screenshots: hint/footer
fit at100% and125%, no clipping. Wire/cancellation/native NoGBM limits remain as
above, media_flow_verified=false, descriptor unchanged. Serial resolver code did
not change after its3-group real PipeWire metadata proof on fixed bin-02.

Check/release workflows now install libpipewire-0.3-dev and libclang-dev for the new
binding. README records build/runtime prerequisites. Official Ubuntu noble package
metadata confirms the packages (PipeWire dev depends on libspa dev). YAML parses
and dependency declarations pass; affected tooling02/session75358 terminal0,
15tests15.755s. Final scoped fmt, whitespace, Python AST,5docs/92links and absence
of the temporary trace gate in bin-03 pass. The shared gate above predates this
Portal-only UI correction and CI dependency declarations; its red outcome stands.
No shared Rust/vendor/dependency change followed that gate.

Next: isolated upstream Niri dev build01 now RUNNING session95554, log
/tmp/canopa-screencast-niri-native-build-01.log, jobs1, target root
 target/niri-screencast-native. Clean source and original lock from pinned revision
b22eb842ca9506d4e2ff997b116031cdb3f650f8, independent workspace in
/tmp/canopa-screencast-niri-native-source-01. NIRI_BUILD_COMMIT set to the full pin;
no release flags, cache cleanup, Canopa profile change or installation. Receipt
/tmp/canopa-screencast-niri-native-source-01-receipt.json marks build_status=running.
Resume its actual session before restarting. Then test real restricted frontend
consumer frames with the private hardware-GL fixture and implement ongoing Iced
sharing/stop controls. ScreenCast stays unadvertised; full18-interface goal active.

## Real ScreenCast media and static resume — 2026-10-30

Previous goal turn classified PROGRESS: v6/consent/keyboard source code and scoped
checks plus exact shared gate evidence. This continuation finishes the pinned
compositor build and adds actual frontend video acceptance. Full18-interface goal
remains active; eleven interfaces advertised, ScreenCast still unadvertised.

Unmodified upstream main b22eb842ca9506d4e2ff997b116031cdb3f650f8 build01/session95554
terminal0,18m34s, original lock/source clean. Compiler rustc1.97.0-nightly14196dbfa.
Binary SHA2d0bfbefbb0e8985661beccb33aef54e5213d68827513748d9e7073afc1f1a9c,
153140544bytes. Original immutable snapshot now
 target/screencast-media-dark-01/compositor/bin/niri; /tmp original receipt updated
with original_build_output and that snapshot path before rebuilding candidate.

scripts/test-screencast-portal.py gains optional --media, explicit --niri and exact
--niri-sha256. New screencast_media.py snapshots/checks the executable, starts
private hardware-GL Weston/Niri, PipeWire and WirePlumber --profile policy (no
hardware discovery), then consumes public OpenPipeWireRemote FD targeted by the
returned64-bit serial. Actual asymmetric C parent pixels, physical monitor vs
logical metadata, actual window bounds, foreign FD denial and producer retirement
are checked. Default installed26.04 NoGBM mode remains. No Portal Rust source,
root Cargo graph, user installation/routing or live compositor changed this turn.

Unmodified dark01/session57568 terminal0,6groups:1280x800monitor and400x300window.
Light01/session75812 terminal1:1280x800monitor correct500x375source bounds; window
negotiatesBGRA500x375SHM and Streaming but delivers no sample20s. Freshwindow
fractional probe19945 terminal0; full light trace02/session52500 terminal0.
Repeats01/session86775 terminal1 only bootstrap10s timeout, not capture failure;
fixture StartServiceByName now allows30s cold debug startup. Repeats02/session49835
terminal1: run2passes, run3reproduces exact lost-window-frame symptom. Resume-red01/
8597 and resume-red02/62944 terminal1 even at100%, on INITIAL lost frame before
controlled pause hook; do not claim controlled hook reached in red.

Diagnosis skill applied. Ranked hypotheses were initial damage, FD/core reuse,
fractional stride. Trace proves cause: Niri queues seq1 while consumer transiently
Paused, resumes, then skips forever for no damage. GStreamer explicitly corks at
READY→PAUSED before PLAYING. Distinct created/closed Gst cores in passing trace;
fresh500x375windows and failure400x300 rule out a simple fractional stride limit.

Canonical niri-screencast-resume overlay and ADR0043 clear Ready content/cursor
trackers and last cursor position on every Streaming transition before redraw.
Niri only; original GPL retained, original dependency lock unchanged, existing
source-cancellation overlay untouched. Official Niri archive3.4MB SHA
865b04ff9f45a9d1285f7370eb20874f3d79fad56b0c3698e443a6a1aac77e60.
Patch SHAb03b123e00d7496679122d436a8894d7a7cd0445e2678ef35885d9262aebef78;
source SHA923964bb601a70491d0c26adb06919e8de4e71f2bcf77386d41a779d3eb2d35c→
2c042b8e640d13a0849e8de92a77ea44e4f1de674da8bc24f02f22c1b914fdea.
Archive materialization and check-vendor01/80362+02/57010 pass. New
build-niri-screencast.py records source/recipe/lock/compiler/binary, uses ordinary
incremental dev/jobs1 and optional --toolchain. Prepare-only/refusal74534 pass.

Build patch01/session25477 intentionally SIGINTed, tool254/Cargo-2, because its
prepared root directory inherited Root stable1.95 unlike baseline nightly1.97.
Receipt marks cancelled/why. Build02/session23177 terminal0,4m54s, explicit
nightly-x86_64-unknown-linux-gnu matches baseline compiler and reuses its cache.
Receipt target/niri-screencast-resume-source-02/canopa-build.json. Candidate SHA
24ab3878f5706cbd4f0a69ed1d58404307fc49c25247956852b0c71b0baf0f39.

Patched nontracing green-light01/session83692 terminal0,6groups. Final combined
7276 terminal0: dark100% and light125%6groups each with frozen Portal bin-03 SHA
82f5194e5e66d8a3f426431241c4f3eca7e12cd75646c9860bb7a96ba0f85a54 and exact candidate.
Original loop now passes. Added controlled static-window pause/resume: observe
original producer idle, resume, require larger SPA/Gst sequence and same actual
pattern pixels within5s with no source damage. Both final runs advance1→2 and save
media-2-resumed.png. Window400x300dark/500x375light, monitor1280x800both. Inspected
original dark window and final light resumed image: expected asymmetric pattern.
Final artifacts target/screencast-media-resume-final-{dark,light}-01/result.json.
No successful media claim is made for installed26.04nested Winit.

All temporary CANOPA_CAST_MEDIA_TRACE/[DEBUG-cast-media] fixture instrumentation
removed; AST confirms. Throwaway fresh-window prototype moved to clearly marked
/tmp/canopa-screencast-debug/. Default NoGBM regression66764 terminal0,6groups;
tooling35233 terminal0,15tests11.103s;3AST/4docs59links/whitespace pass. New helper
--help and prepare-only execute. Source needed a fresh producer snapshot on
resume; no portal-side cached substitute/changed source grant was introduced.
This real consumer seam now verifies initial/resumed pixels instead of only nodes.

Required full workspace gate started after final vendor/tooling edits; running
/tmp/canopa-screencast-resume-workspace-01.log, handle from latest tool response.
Record actual terminal outcome, not the prior gate. Ongoing Iced sharing/stop
controls, Embedded/Metadata cursor native checks, active caller/backend death
media checks and distribution selection remain before capability advertisement.
No subagents/commits/PRs/live changes. Next continue those after this gate.

Shared gate26599 live at checkpoint: tooling/fmt/workspace Clippy1m06s and
attribution649 pass; full test compilation currently running. Terminal outcome
still required. All scoped native/build/tooling jobs this turn are terminal.

## Session-owned screen-sharing controls — 2026-10-30

ADR0044 implements independent Iced controls owned by active sessions, bounded
readiness and retirement, and capture/UI Stop concurrency. Six helper-process
tests pass. Portal test74194 terminal0:111unitpass/1nativeignore and21integrationpass/
2nativeignore. Headless check46319 passes38.41s. Build63499 terminal0,1m24s; frozen
target/native-test-snapshots/screencast-status-bin-02/xdg-desktop-portal-canopa has
SHA04241bea35f6a87c641dde09f83ccc0279baa6a3a89d54b97e50dcb9b5a0eded.

Native dark58819 and light125%89088 terminal0,11groups each: initial floating
480×280, Stop/Escape/window close removes producer and emits public Closed,
active caller disconnect, source-loss/public Close retire controls, backend
SIGKILL leaves independent worker able to Stop the real producer. Six actual
streams per appearance verify asymmetric pixels; the final backend-death monitor
case verifies actual frame/dimensions and cleanup after the pattern source was
destroyed. Artifacts target/screencast-status-media-{dark-02,light-01}/result.json;
both theme screenshots inspected. Earlier dark17078 passed9groups but tiled
controls628×784; fixed size corrects natural Niri placement, verified afterward.

Previous shared gate26599 terminal101: tooling/fmt/Clippy/649inventory and all-target
test build6m08s pass, Fileslib372pass/34ignore. Files integration hits QuotaExceeded122;
truncated110592byte log has no trustworthy final summary. Do not equate this run
with the old ten-case baseline. Five inactive /tmp native snapshots relocated by
copy+size/SHA verification into target/native-test-snapshots, leaving original
paths as symlinks. Receipt relocation.json; session37315 terminal0; no Cargo cache
deletion. User confirms cleanup finished/rebuilding authorized. /tmp free1.9GiB.

Fresh final-control workspace gate12148 running, log
/tmp/canopa-screencast-status-workspace-01.log. Tooling, Clippy1m47s and attribution649
pass; full tests compiling. Cursor fixture implementation/acceptance continues;
SCREENCAST.md records the corrected pointer-entry wait and actual run handles.
ScreenCast remains unadvertised; distribution selection and full goal remain open.

Complete cursor/native acceptance now passes on frozen bin-02 and the pinned
compositor: dark2980/light125%81469 terminal0,17groups each. Six direct restricted-FD
captures per appearance verify both source types and all three cursor modes,
including actual separate SPA bitmap/hotspot/position and exclusion of Embedded
pixels in Hidden/Metadata. Bitmap24×24dark/30×30fractional; sourcewindow400×300/
500×375; every original producer removed. Seven existing actual media/control/
crash streams per appearance also pass. Images inspected. Artifacts
target/screencast-status-cursors-{dark-02,light-01}/result.json.

The first cursor run46495 fails only a repeated pointer-entry fixture wait after
display Hidden passed, before Embedded consumption. Moving pointer out/back fixes
the fixture; no producer change. C consumer and plain/cursor parent variants compile
with -Wall -Wextra -Werror. PipeWire runtime/header1.6.10. Pinned Niri falls back to
Embedded for Metadata below1.4.8, so runtime cursor capability gating is still needed
before advertisement, alongside compositor distribution selection. Shared12148
test build6m32s passes; Files library tests running. No Rust edit follows gate start.

Shared12148 ends143/SIGTERM before final Files integration summary; cause unknown
and no surviving gate test process. Tooling/fmt/Clippy1m47s/649inventory/test build
6m32s and Fileslib372pass/34ignore226.36s pass. Observed non-helper integration
failure names exactly match the earlier ten-case baseline, with no QuotaExceeded,
but no final counts or completed workspace result may be inferred. Receipt:
/tmp/canopa-screencast-status-workspace-observation-01.json. Later stages not reached.

After that gate's initial Rust compilation, version-policy red8411 with frozen
bin-02 reproduces old-library cursor mask7 instead of3 using a private version
shim. This simulates policy input, not an actual old ABI/media environment.
capabilities.rs now gates Metadata at linked PipeWire1.4.8, rejects unavailable
Metadata selection, handles malformed versions conservatively, and preserves the
longstanding0.3.64 ABI getter/headless0. Portal17105 terminal0:112unitpass/1ignore,
21integrationpass/2ignore,4m32sbuild,5.03/2.00sexecution. Clippy22052/build11626 and
final-binary version/native checks remain in progress. Shared vendor source
unchanged; this new policy change is Portal-only. Full goal active/unadvertised.

Final runtime-gated build11626 terminal0/1m38s, Clippy22052 terminal0/1m11s,
headless62304 terminal0/22.65s, fmt43616 pass. Bin-03 SHA
dd6f875e93fdb1981ebbce64307f41c6dbf94d1f0e327420c7fec03255977f8d at
target/native-test-snapshots/screencast-status-bin-03/xdg-desktop-portal-canopa.
Root debug hash matches. Version-policy green53470 terminal0: five old/supported/
unknown cases verify masks and Metadata selection/rejection. This is a policy
shim, not actual old-library ABI/media acceptance. ELF imports only the older
version getter, preserving existing minimum ABI; actual runtime1.6.10.

Final dark80481/light125%45183 terminal0,17groups each on bin-03 and pinned capture
candidate. Seven media/control/crash streams plus six actual SPA cursor captures
per appearance pass, including static resume, scaled bitmap/hotspot, active caller
loss and SIGKILL cleanup. Installed26.04 nested NoGBM62272 terminal0,6groups/no
media grant. Artifacts target/screencast-status-final-{dark,light,nogbm}-01.

Remaining standard gate stages continued independently: public Settings passes;
CLI build0/13.70s; configuration0/3.20s. First vendor attempt fails from my TMPDIR
override under the root workspace; rerun55139 with normal /tmp passes fork4,
event-batch3 and native-pipe6tests. Logs /tmp/canopa-screencast-{settings,vendor}-final-01.log.
The wrapper's first Settings log collided with aggregate output, so the separate
Settings rerun retains the actual result. The old receipt's failed vendor entry
is retained, not erased. This continuation does not make the interrupted shared
gate pass. Final AST/docs/whitespace and production dependency exclusions pass;
no GTK/GDK/GIO/GLib, and headless excludes Iced/Wayland/PipeWire/libspa too.

All processes terminal. No subagents/commits/PRs/installs/live routing or compositor
changes/cache deletion. ScreenCast remains unadvertised pending compatible
compositor distribution selection. Next investigate newer Smithay/source-grab
compatibility before claiming the capture candidate also satisfies ADR0024; the
two independent pins must not be conflated. RemoteDesktop contract/input seams
are prepared only. Full18-interface goal remains active. Shared gate12148 remains
incomplete143/SIGTERM with observed baseline failures and no final test summary.

## Combined capture/Files compositor continuation

ADR0045 and [the distribution ledger](SCREENCAST-DISTRIBUTION.md) now record one
explicit candidate with the capture resume and six-file Smithay source-lifetime
corrections. Canonical recipe/source/patch/licence/binding receipts are retained;
both older independent overlay records remain unchanged. The ordinary incremental
dev build produces SHA73b993cbc3c5036701bf612ecb8e260152cce79f408be61b7d2d4cf1bc923026.
Normalized feature/dependency graph equality passes for393packages. Strict GTK
cancellation, retained offers and selected actual Files workflows pass, plus all
17ScreenCast groups per appearance on that same compositor. Initial startup
timeouts and the failed first aggregate are retained; scoped passes do not prove
full matrix/release/distribution acceptance. ScreenCast stays unadvertised.

The generic session contract now selects mandatory Embedded cursor support and
passes21integrationtests/2ignored normally and under the old-version policy shim.
Metadata policy retains separate version-boundary coverage. Shared tooling,
formatting/Clippy and649-package inventory pass in the new tracked workspace gate;
Files library372pass/34ignored. Integration is still running with no final outcome.
Status /tmp/canopa-niri-desktop-workspace-01.json and log of the same basename.

RemoteDesktop preparation now has actual Rust EIS/libei protocol interoperability
evidence over an anonymous socket. No Wayland input, production dependency,
RemoteDesktop advertisement, installation or live routing change. Its bounded
worker/session integration remains implementation work; the full18goal is active.

Tracked workspace gate960997 has now finished101/1659.48s. Files integration
596pass/10fail/41ignored in847.98s, exact ten names equal the prior baseline;
comparison /tmp/canopa-niri-desktop-workspace-comparison-01.json. No quota error.
The gate remains failed and stops before later packages/script stages. Separate
continuation and full native desktop matrix are being prepared, with current
ordinary-debug app build54616 now terminal0. No failed result is superseded by
individual successful reruns.

App build54616 finishes0/12m52s. Immutable app/Portal/probe snapshots are verified
at target/native-test-snapshots/niri-desktop-matrix-apps-01/receipt.json. Detached
worker1004364 is verified live and compiling the remaining workspace packages,
excluding the failed Files package, before separate standard script stages and
the full188-case private desktop matrix. Status
/tmp/canopa-niri-desktop-matrix-worker-01.json. Preserve current Cargo/application/
vendor inputs until compilation/standard stages are terminal, and preserve native
fixture sources while the matrix runs. Exact hashes/current caveats are in the
distribution ledger; no full-matrix result is claimed yet.

Worker1004364 is now terminal. Remaining packages fail during compilation/linking
while disk reaches0; vendor fork4/event-batch3 pass but native pipe fails ENOSPC.
Settings, CLI (with cache-full warning) and configuration pass independently.
Full matrix fails before output.mkdir completes; no native scenarios run. User
cleanup is followed by actual24.5GB free-space verification. New tracked worker
1017717 retries only failed remaining-workspace/vendor-pipe/matrix stages with the
same snapshots; status /tmp/canopa-niri-desktop-matrix-worker-02.json. The completed
main gate101/ten Files baseline failures is not superseded by these continuations.

RemoteDesktop now has a staged finite EIS reader: deterministic original8192-byte
red, four actual-library socket regressions and real libei interoperability green,
all new output under /tmp. No production adoption or input backend yet. Exact
source/patch/receipt details are in REMOTE-DESKTOP.md and the distribution ledger.
The full18-interface goal remains active; disk recovery removes the transient
resource impediment, and validation/implementation resume without another approval.

Recovery retry1017717 completes remaining workspace packages excluding Files
0/1434.18s and native-pipe0/4.27s, then actually starts the frozen188-case native
matrix. Existing fixture sources are preserved. The main full gate's ten Files
failures remain; this is a separate continuation on the pre-EIS tree.

Reis0.7.1 bounded-reader fork is now canonical, desktop-only in Portal, with exact
archive/commit/licence/per-file/patch provenance. Only reis is newly locked; only
Portal's existing dependency list gains it; all other locked packages are unchanged.
Official archive/normal checker pass. Actual workspace Rustix1.1.4 check68535 pass;
112library/25integration pass29289 (1/2native ignored), including four EIS readers.
Focused Clippy and headless checks pass20669;650-package attributions,3layout groups,
normal/headless dependency exclusions pass. Six staged pending-budget tests and
libei interop also pass; the guard and worker are not yet integrated. ADR0046 and
RemoteDesktop ledger retain scope, evidence and remaining native/session/consent work.

2026-10-31: separate compositor keyboard-lease candidate builds locked/offline in
the existing incremental dev target. Three wire regressions and nine actual
private receiver checks pass: French layout, modifier state, overlapping sources,
FD EOF/worker SIGKILL cleanup, malformed input and bounded admission. Exact
candidate patch/binary identities and initial fixture failures remain in
REMOTE-DESKTOP.md. This does not implement EIS-to-input, pointer/touch, physical
keyboard acceptance or portal consent. Original matrix fixtures and frozen73b
compositor remain unchanged; its incomplete aggregate has additional failures.

The keyboard state extension subsequently passes13private native checks on one
new frozen ecd14 executable: asynchronous other-source modifiers, Caps Lock,
French/US group switching and idle keymap-change retirement join all earlier
lease/loss/admission behaviors. A cached Smithay keymap identity keeps refresh
finite without serializing the map per loop. Both exact patches/build receipts
remain separate from the unchanged73b full-matrix companion. EIS worker/session/
Iced consent and legacy/pointer capability work remain before advertisement.

2026-10-31: desktop-only Rust keyboard EIS worker now forwards framed input through
the compositor lease. Parent/caller/client/worker loss, exact grant/role/peer
checks, sealed keymaps, bounded unframed input, repeated periods and replacement
devices pass12real-libei/native groups on frozen worker950e/Niriecd14. Original
repeat/request red controls and initial10-case success remain recorded. Adopted
Pending guard adds6tests plus1serial test. Final Portal119library/25integration,
all-target Clippy/headless and3test-layout groups pass. The denied-socket sandbox
run is retained separately from its private host-bus rerun. This is an unadvertised
keyboard worker; parent adapter, legacy/pointer capabilities, Iced/shared-session
consent and full shared/distribution acceptance remain. See REMOTE-DESKTOP.md.


2026-10-31 terminal matrix and parent-adapter checkpoint:
The preserved 188-case desktop matrix02 has finished: 107 pass, 81 fail, aggregate
exit1. Host worker1017717/start_ticks25850704 is gone. Its terminal receipt says
all frozen executables were unchanged and changed_fixture_sources is empty.
Before subsequent tooling edits, all149 recorded fixture files were copied and
hash-verified under target/niri-desktop-compat-full-matrix-02/fixture-sources/.
Do not describe the original aggregate as accepted or classify all81 failures
as baseline. An unchanged isolated files-iced-clipboard-dark control on earlier
capture-only Niri24ab and the same frozen Files48b fails at the same final
process.wait(timeout=10). This establishes that this particular failure is not
unique to combined Niri73b; it does not establish the cause of other failures.
A no-job Copy/close/supersede diagnostic on Files48b/Niri73b passes with the same
10-second wait (target/niri-clipboard-minimal-01). Earlier scenario operations
are being narrowed without changing production code or the accepted fixtures.

The real input parent adapter now validates the unique compositor owner, bus/IPC
PID, protocol version and exact ready FD. Connector clones share eight admission
slots; a slot remains held until the actual child is reaped. Startup cancellation,
active drop and parent EOF close the worker lifetime pipe. The parent retains no
duplicate EI client FD. Five focused unit tests pass on the host; two sandbox
attempts are retained with EPERM creating the anonymous socket from stdout.
Frozen parent harness7022e782e9e6abb52f6a12e8678995be7517dacc2c7b5ccf338534581832513c
is recorded in target/native-test-snapshots/remote-input-parent-harness-01/.
Six real-libei native groups pass in target/remote-input-parent-native-02:
parent EOF, client SIGKILL, worker SIGKILL, actual parent/backend SIGKILL, caller
bus loss and keymap loss. Worker950e/Niriecd14/harness7022 remain byte-identical.
The initial native01 failure was metadata parsing of libtest's inline prefix,
before the EI client started; the parser fix and original artifacts are retained.
This is process/FD/lifetime acceptance, not an integrated RemoteDesktop portal.
Fresh Iced consent, shared ScreenCast/RemoteDesktop sessions, controls, legacy
Notify/keysym, pointer/touch and physical-input acceptance remain required.

The exact-added-file compositor source recipe niri-remote-input is now explicitly
registered, never the default. Four patches/14 file entries preserve the same
upstream commits and lock graph. Null upstream hashes require absence, rather
than masking existing files; three new source-addition tests and four existing
source-binding tests pass. Registered prepare-only materialization verifies all
14 resulting hashes and only the two recorded path bindings change the lockfile.
No compositor install, live replacement or portal advertisement occurred.

A new full bash scripts/check.sh --workspace run covers the adopted Reis fork,
parent adapter and source registration. Driver1107282/start_ticks27031661, receipt
/tmp/canopa-remote-input-workspace-01.json and log of the same stem. It records1462
source-input hashes; those inputs are frozen until the command is terminal.
Earlier pre-Reis full-gate failures remain evidence, not acceptance of this tree.


2026-10-31 shared session core and fixture correction adopted after full gate:
Driver1107282/start27031661 is terminal101/3803.34s and verified gone. All1462
recorded inputs were unchanged. Clippy passed7m20, all-target test compilation
36m45, Files library372pass34ignored276.42s and integration596pass10fail41ignored
646.23s. Exact ten failure names match the prior baseline; no added/removed names.
Comparison /tmp/canopa-remote-input-workspace-comparison-01.json. This remains a
red full gate; later packages, including Portal, were not executed by that run.

Guarded adoption adds a shared ScreenCast/RemoteDesktop host, immutable session
origin, independent sources/devices, approved active FD connection, common16-slot
admission and one retirement actor. Five real private-bus contracts now cover the
new paths. Standalone headless library/test metadata first passed before adoption;
actual Cargo tests caught ConnectToEis versus the required ConnectToEIS. An explicit
wire name fixes it. Portal library124pass/2native ignored4.85s; initial integration
28pass2fail/2ignored is retained; corrected full integration30pass/2ignored2.03s.
Logs /tmp/canopa-sharing-session-tests-{01,02}.log. Clippy passes1m15s, layout3groups
and formatting pass. Headless all-target Cargo check is still running53336.
The real production presenter, combined Iced approval/ongoing controls, legacy
Notify/keysym and pointer/touch remain; no main registration or advertisement.

Clipboard fixture correction is also adopted. The original full workflow retains
its file operations and now expects canceled unfinished work to hold the process
after clipboard supersession. Two new clipboard-lifetime cases isolate the no-job
owner and retain the exact10s natural-exit assertion. The manifest now has190cases;
the original188-case aggregate and149 source copies remain historical evidence.
The3/4 native correction batch remains failed. An unchanged isolated dark follow-up
passes after the filesystem stress ends (native dark-control-02); the earlier
wtype timeout did not reproduce and its cause remains unestablished. Four exact
verified source files for the original batch are archived alongside its receipt.
Both lifetime themes and both full workflows have now passed at least once.
Direct actual runner/CLI registration also passes at
target/niri-clipboard-lifetime-selector-01/summary.json. Python syntax, runner8tests,
preflight and whitespace checks pass. No Files lifetime implementation was changed.

Final session-core verification: headless all-target Cargo check passes35.88s; formatting/whitespace pass. All owned validation processes are terminal. Real production/Iced/legacy integration remains next, not advertised.


2026-10-31 production RemoteDesktop presenter and direct native keyboard proof:
The real presenter now reuses the capture factory/recorder budget, pins compositor
identity across fresh Iced consent, observes caller/compositor loss before the
first EIS connection, and owns capture, input leases and one ongoing scope-aware
Iced control surface together. Keyboard control starts unchecked; input-only
requires an explicit grant and a combined request permits view-only reduction.
The normal main service/descriptor still do not expose RemoteDesktop.

Portal library 130 passed/2 ignored and integration 30 passed/2 ignored; the added
native harness raises library ignored count to 3. All-target check, Clippy with
warnings denied, headless check and native build pass. Two production lifetime
regressions pass again after fixing fixture marker ordering. Logs are under
/tmp/canopa-remote-production-{tests,check,clippy,headless,lifetime-tests}-*.log.
Six actual Iced/libei native groups pass in
 target/remote-desktop-production-native-01/summary.json:
initially unchecked fresh approval and Request.Close, UI Stop, Session.Close,
client loss, caller loss and backend SIGKILL. Five owned input workers are terminal;
held Shift is released and modifiers clear in every accepted active case. Frozen
portal 9014f526 and harness c94bb45c, compositor ecd14ac0, are unchanged. The four
exact fixture sources are archived beside the native summary. Screenshots were
inspected; the input-only disabled button wording was subsequently corrected from
Share to Allow control. Rebuild and theme/combined/frontend proof are in progress.
This does not establish legacy Notify/keysyms, pointer/touch, shared media/frontend,
physical input, compatible distribution or the still-red full workspace/matrix.


2026-10-31 production theme and combined public-frontend acceptance:
The wording/config rebuild is terminal0: library test compilation53.35s, binary
build2m30s (/tmp/canopa-remote-production-native-build-02.log). Snapshot03 freezes
portal d4f278dbd3afcfabf523c76763128867153a9d69d901a86de60e3f7fd2141dc2 and
harness4896723068ed558d4a1d0498e72a2295c8feecb1ac52f89dd039b059cb020ee9 after
terminal build. Snapshot02 is explicitly unused: its portal was copied before the
rebuild completed; it cannot substantiate the wording change. Snapshot01 and its
six accepted dark input-only groups remain unchanged historical evidence.

Input-only light/125% passes all6groups at
 target/remote-desktop-production-light-03/summary.json.
The preceding light02 attempt failed before UI because the sandbox denied the
private D-Bus socket; no product result was obtained. Exact accepted fixture
sources are archived and unchanged. Fresh consent and ongoing controls were
visually inspected; the disabled action now says Allow control.

New scripts/test-remote-desktop-media.py serves the real production adapters with
one capture factory through the standard XDG frontend and a private opt-in
ScreenCast/RemoteDesktop descriptor. Six groups each pass at dark100/light125:
 target/remote-desktop-media-dark-04/summary.json and
 target/remote-desktop-media-light-01/summary.json.
Each run verifies six actual asymmetric monitor frames via restricted
OpenPipeWireRemote FD + exact producer serial, both source/device selection orders,
view-only reduction with exact AccessDenied/No input device was granted, and real
French shifted A/modifier feedback in five granted input sessions. UI Stop,
Session.Close, client/caller loss and backend SIGKILL remove the selected producer,
retire controls and release held Shift/modifiers. All ten owned input workers are
terminal. The same portal/harness/compositor hashes remain unchanged; exact7fixture
sources are archived before final runs, with no changed sources at completion.
Screenshots in both appearances were inspected. Aggregate verification receipt:
 target/remote-desktop-production-acceptance-03.json.

Keep prior failures: combined dark01 frontend SIGABRT due missing fixture GSettings
schema path; dark02 fixture KeyError persist_mode after actual approval. The public
contract reports restore_token rather than the backend-only persist_mode result.
Corrected dark03 passes6; final dark04 also tightens exact denial and positions
controls clear of the reference source at fractional scales. No product change or
deadline relaxation was needed for these fixture corrections. All old sources are
archived separately. Current formatter/test-layout3groups and Python syntax pass;
final Portal regression run is pending. The red full workspace gate and historical
107/81 native matrix remain red, with their causes/scopes recorded above. This
acceptance does not cover legacy Notify, pointer/touch, physical input, shipped
metadata or compatible compositor distribution; RemoteDesktop stays unadvertised.

Final production regressions are terminal0: Portal library130pass3ignored6.39s, integration30pass2ignored1.97s (tests03 log, compilation41.39s); Clippy03 passes71s. Formatter, test-layout3groups, Python syntax and whitespace pass. All owned validation/native jobs are terminal. The broader goal remains active.

Legacy continuation prototype: see LEGACY-KEYBOARD.md and /tmp/canopa-legacy-keysym-stage-01/receipt.json. Ten pure XKB/model tests pass0.51s, including760 ASCII/layout/group/Caps combinations. The real-feedback Shift ownership regression is retained before its fix. No supported dependency, vendor source or portal runtime changed; Notify/mode enforcement/native proof remains next. All owned jobs are terminal.


2026-10-31 typed compositor keyboard owner and portal transport continuation:
LEGACY-KEYBOARD.md records the adopted SymbolVersion1/OpenKeyboardSymbols protocol,
borrowed authoritative-map planner, sequence/status acknowledgements, exact refusal
and owned-chord cleanup. The registered explicit recipe changes only the Niri patch
(e1bd2127); Smithay/pins/lock graph remain unchanged. Archive replay15entries passes,
as do vendor integrity and source policy3+binding4tests. Frozen NiriE5e792 passes
12new symbol groups, all13v1 groups and6combined frontend/media/EIS groups with the
existing PortalD4/harness489. All native candidate jobs are terminal0. No install
or default compositor selection; prior accepted NiriECD bytes remain unchanged.

Portal LegacyInput/worker bridge is implemented but Notify remains unwired. It
uses one pending command, fixed partial-read buffers, finite acknowledged status
and a shared authenticated/reaped8-worker pool. Initial library check passes33.41s;
bridge3 and input-suite18pass1ignored pass. The added coverage retains a red
19pass1fail2ignored test03 caused by an immediate Tokio try_read readiness assertion
on a closed peer. Tests04 replace that assertion with bounded async EOF and are
running. The guarded real parent harness/private driver are ready but not run.
Full source details, receipts and logs are linked from LEGACY-KEYBOARD.md. Required
full vendor/shared gate remains pending for this slice; prior full gate/matrix reds
are still red. Legacy Notify/input mode, pointer/touch, physical input and compatible
distribution remain before advertisement; the18-interface goal remains active.

Final legacy transport evidence: new frozen worker477fc400/harnessae400644 pass5
real legacy parent/worker groups and6existing EIS groups on compositorE5, with
all11owned workers terminal and binaries unchanged. Exact fixture sources are
archived. Scope excludes public Notify and legacy consent. Full Portal scoped
validation passes138library/4ignored and30integration/2ignored; Clippy104.28s,
headless36.55s and format/layout/syntax/whitespace pass. Receipt
/tmp/canopa-legacy-keyboard-validation-01.json. Required full workspace gate is
next. Notify/actor draft is unadopted and passes headless metadata only.

2026-10-31 typed transport full gate is terminal101/1547.65s. All1483recorded
inputs were unchanged. Files integration596pass10fail41ignored606.54s stops the
gate before Portal/later stages. Failure names exactly match the prior ten; no
added/removed names. Comparison /tmp/canopa-legacy-keyboard-workspace-comparison-01.json.
Dedicated Portal138/30, Clippy and headless passes remain scoped evidence.
The historical matrix and full gate remain red. Notify/production draft stays
unadopted while its mock-bus contracts are completed; see staging checkpoint.


## Current ordered Notify acceptance, 2026-10-31

The final dispatcher passes library138/4ignored5.42s and integration36/2ignored2.88s,
including11RemoteDesktop contracts and1,024 ordered transitions across four threads.
Clippy all-targets with warnings denied passes88.64s; headless all-target check
passes49.39s; normal incremental default binary build passes4.09s. No new vendor,
profile, dependency graph or routing change accompanies this Portal-only slice.

Frozen ordered snapshot03 passes the isolated input-only native case, then all9
public reply-barrier groups in dark100 and light125 (58.10s and56.66s). Each full
run consumes8real monitor streams and records9terminal input workers. Legacy-to-EIS
releases the old held chord/worker, transfers the EI FD and keeps the original
approved stream usable. View-only denial, common Stop, explicit session close,
worker/caller/backend loss and Notify-after-EIS denial pass. Native screenshots
were reviewed in both appearances, including checked input-only consent and
unchecked view-only consent with one selected source. Exact7fixture sources per
run and99Portal/build inputs are archived unchanged; frozen binary hashes remain
unchanged. No live routing or RemoteDesktop main/descriptor registration.

Evidence: target/remote-desktop-notify-acceptance-03/receipt.json;
 target/native-test-snapshots/legacy-notify-ordered-bin-03/receipt.json;
 target/remote-desktop-notify-ordered-{dark,light}-03/summary.json.
The postbuild copies remove debug sections to fit storage; original Cargo debug
outputs/profile remain, and matching ELF build IDs/executable segment hashes are
recorded. Exact immutable test compositor duplicates are stored by reference to
the unchanged frozen compositor after each terminal run. Original tmp evidence
paths remain aliases to the stored successful artifacts.

This scope uses public reply barriers. The installed frontend1.22.1-2 concurrent
forwarding race remains independently reproduced and open, with persistent trace
and comparison at target/remote-desktop-notify-acceptance-03/frontend-ordering.json.
The older Start2 failure remains unexplained and retained; the current isolated
and full cases pass, without attributing its cause. The broader workspace and
historical full matrix remain red; focused Portal success does not clear them.
Pointer/touch, physical input, compatible distribution and the remaining interfaces
on the eighteen-interface roadmap remain active work.


## Canonical frontend companion acceptance, 2026-10-31

ADR0047 and the registered xdg-desktop-portal-input-order recipe address the
upstream worker-scheduling race. An exact official 1.22.1 build fails the same
concurrent burst; instrumentation shows reordering at worker/handler entry, while
proxy sends preserve handler order. Only three frontend files change. Cached
caller ownership and approved input are checked before a bounded per-session
queue; original handlers retain grant/option/EIS checks. Setup stays asynchronous.
The 64-pending/4-KiB/no-FD limits retire the owned session on overflow.

Canonical archive replay verifies 499 inputs and all three patched hashes.
Same-cache debug build/reuse succeeds; tampered/extra source, a symlinked root
and a nonempty generated lock are rejected. Frozen frontend b77492eb passes all
14 native groups in dark100 (86.95s, concurrent32events) and light125 (87.70s,
NoReplyExpected32events). Each has12real monitor consumers,14terminal workers,
unchanged7fixture sources and4binary pins. Explicit pre-consent denial and
rejected-FD closure pass. The separately frozen candidate also passes14groups
under ASan/UBSan with no memory-error report; global-cache leak scanning is
excluded. Original ordering/body/FD failing controls and interrupted-build logs
are retained, without assigning unrelated earlier failures a cause.

Persistent receipt: target/xdg-portal-input-order-acceptance-01/receipt.json.
The required post-adoption workspace gate finishes101in1667.42s at
 /tmp/canopa-frontend-order-workspace-01.json. Tooling/fmt/Clippy/all-target
compilation pass;2024inputs remain unchanged. Files integration finishes
596pass/10fail/41ignored648.07s, with exactly the same ten failure names and no
added/removed names. Remaining gate stages are tracked separately. Focused native
success does not clear this red gate or the historical107pass/81fail matrix. The installed frontend and live
routing remain unchanged. Pointer/touch, physical-input, compatible distribution
and the remaining eighteen-interface roadmap stay active; no RemoteDesktop
main registration or advertisement is added.


Canonical frontend Settings activation/read/reload also passes3.00s. FileChooser
passes12native groups per appearance (dark39.37s/light12541.66s) on the
portable root fixture after a diagnosed test-device lifetime correction. Both
original and candidate frontends fail the old one-shot Shift fixture identically;
a separate native probe observes mask0after return,1while the device lives and
0after retirement. The fixture now keeps that device alive across selection and
waits for its Wayland roundtrip. No application behavior changes. Root native
fixture sources and runtime binary pins remain unchanged during both corrected
runs. The full workspace gate above preceded this tooling-only correction.
Original failures and diagnostic/final fixture versions remain archived.


### Remaining shared checks and pointer continuation

The unreached stages finish with all remaining Rust tests passing1723.24s,
Settings activation1.10s, CLI build11.27s and vendor checks9.44s. Configuration
initially fails because the normal canopa-files executable is missing; the failed
control is retained. Its ordinary incremental debug build passes216.54s and the
configuration suite passes32.80s. The2024recorded source inputs remain unchanged.
This does not clear the required full gate's same ten Files integration failures.
Persistent continuation and recovery receipts are under
 target/xdg-portal-input-order-acceptance-01/.

Pointer continuation remains unadopted. Frozen E5 reproduces two virtual pointers
releasing each other's button state in8.13s, while its single-source control passes.
Five ownership tests and seven cycle/origin/routing model tests pass, each including
576four-source orders. A two-file getter/trace probe builds in345.28s in the same
nightly1.97 incremental dev cache. Its21.34s native diagnosis confirms teardown
releases already have a dead Wayland resource before Niri binding dispatch, while
ordinary releases have a live resource. The ownership contract remains red.
All owned probe processes are terminal; accepted E5 and source/runtime pins stay
unchanged. The temporary trace is removed afterward; only a read-only getter
remains in the unadopted prepared tree. Evidence and concrete binding/grab/mapping
continuation notes are at target/remote-input-pointer-investigation-01/.
This does not establish pointer/touch or physical-input support.

Completed inactive native snapshots were preserved on disk after process checks
and per-file hashes:11directories/1770596661bytes, with original/tmp paths retained
as aliases. The relocation receipt is
 target/native-test-snapshots/tmp-preserved-01/relocation.json. No active executable,
Cargo cache or root debug output was deleted. The full eighteen-interface goal
and distribution/full-matrix acceptance remain active; no live portal routing
or RemoteDesktop registration changed.

### Capture-bound private pointer workers

The production capture facade now carries complete private source bindings into
a separate Rust pointer worker. It authenticates live owner/PID/source/mapping
identities, translates approved PipeWire nodes to compositor streams, validates
paired input and waits for held-state retirement. Resize can interrupt an
acknowledgement or leave a pending Unix-socket reset. The worker requires actual
EOF before renewal, verifies changed size on the same binding, and never replays
an interrupted command. A real-socket regression reproduces reset with an unread
command before the fix and passes afterward.

Final normal incremental portal checks pass 151 library tests, 37 integration
tests, formatting and all-target Clippy. The frozen build passes 62 native groups
in 361.44 seconds: two complete pointer runs with 192 resize transitions each,
17 identity-capable video/control groups, 17 legacy view-only groups and six
combined keyboard EIS/media groups. All 1,308 source inputs and helper/binary pins
stay unchanged; all owned native processes terminate. The 16-file delta and its
baseline/replay, failed attempts, diagnostic controls and final native proof are
preserved in [the pointer worker ledger](pointer-workers/README.md).

One earlier integration startup timeout remains unexplained; its identical-binary
direct case and unchanged full rerun pass. Initial native resize failures and
the deterministic reset regression remain evidence. These focused results do
not clear earlier full workspace or native matrix failures. Public EI/Notify
pointer adapters, explicit Iced permission, remaining window/physical-input work,
canonical compositor adoption and distribution remain. The full eighteen-interface
goal stays active; normal RemoteDesktop registration and routing are unchanged.


### Public stream pairing and pointer Notify foundations

The next 12-file slice publishes validated mapping_id/logical-size metadata without
compositor paths and admits all five pointer Notify methods into the same typed,
receive-ordered session queue as keyboard input. Sender/grant/stream checks, the
EIS one-way gate, cancellation and overflow retirement remain session-owned.
Production pointer grants remain disabled until the real EI/Notify transport and
explicit Iced permission are integrated.

All 151 library and 43 integration tests pass, including four new private-bus pointer
groups, two mapping-publication groups and 1,280 mixed transitions across four
worker threads. Clippy, formatting and test-layout checks pass. The frozen build
passes 51 native groups in 274.96 seconds: 11 production-facade/private-pointer
handoff groups with 64 resize transitions, 17 mapped public video groups, 17 legacy
video groups and six keyboard EIS/media groups. Public mapped video exposes 13 mapped
streams; legacy exposes 13 streams without IDs. Public window logical size 400×300
matches decoded 500×375 pixels at 125%. All 1,313 recorded source inputs and helper/
binary pins stay unchanged, and all owned native processes terminate. The first
cargo check's accessor visibility error is preserved and corrected.

[Evidence, 12-file patch and replay](pointer-adapters/README.md) and ADR0050 retain
the exact scope. Protocol tests use a controlled input transport; they do not
establish actual public pointer injection. Next EI work includes region layout,
physical scale and immutable device replacement. A window-only capture may keep
logical size across a scale change, so size-only lease renewal does not prove
that EI case. No compositor adoption, distribution, broader failure resolution
or full eighteen-interface completion is claimed.


### Atomic physical scale for input regions

The frozen descriptor-v1 provider reproduces a window-only scale gap: source and
mapping identity and logical size 400×300 stay fixed while video changes from
500×375 to 600×450 pixels, and the held pointer lease remains open. The prepared
MappedPointerVersion 2 companion snapshots physical scale with logical size and
retires the old source when scale changes. The Rust capture/worker path carries
optional scale, negotiates v1/v2 descriptors and renews verified size/known-scale
changes after confirmed cleanup without replay or inherited held state.

The four-file compositor and six-file portal deltas pass isolated replay; a
20-file layered replay also passes from canonical E5 through prior drafts and
this delta. Same-cache compositor dev build passes in 170.87 seconds with 5,961
unchanged inputs. Portal checks pass 153 library and 43 integration tests, Clippy
and formatting, with 1,313 unchanged inputs. The initial integration run has
42 successes and one existing helper-startup timeout. Its cause remains
unexplained; the identical executable's direct case passes in 0.33 seconds and
the unchanged full rerun passes all 43.

All 198 scoped native groups pass: three compositor scale groups, 126 existing
mapped/pointer/drag/keyboard groups, and 69 portal groups including three
window-only scale changes, v1/v2 pointer compatibility, public mapped/legacy video
and combined keyboard EIS/media. Scoped artifacts retain helper/binary pins.
[Scale evidence and patches](ei-scale/README.md) and ADR0051 record this
prerequisite. No actual EI pointer device, Iced pointer grant, canonical adoption,
installation, broad failure resolution or full eighteen-interface completion is
claimed. EI region/device lifecycle and production pointer transport are next.
