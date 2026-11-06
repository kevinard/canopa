# Combined compositor preparation and acceptance

Status: active native verification, 2026-10-30. Full18-interface portal goal active.
Previous goal turn was PROGRESS: Iced controls, complete cursor/crash acceptance
and runtime cursor policy implemented/verified. This continuation investigates
compatible compositor selection without changing the installed session.

## Reproduced compatibility gap

Capture-only candidate24ab3878f5706cbd4f0a69ed1d58404307fc49c25247956852b0c71b0baf0f39
fails the unchanged strict independent GTK cancellation control twice. Runs4618
and25760 terminal1; artifacts target/screencast-distribution-cancel-baseline-{01,02}.
Sway control12242 terminal0; target/screencast-distribution-cancel-sway-01.
No Iced/Files process is needed for this minimized reproduction.

Ranked hypotheses were dead active grab, wrong source identity, and receiver not
entered. Wire evidence falsifies the latter two: source52 is the actual start_drag
source; it negotiates Copy, is destroyed and receives delete_id, while the entered
receiver receives no Leave. /tmp/canopa-screencast-cancel-wire-baseline-01.json.
The newer Smithay source still marks the source dead without retiring its grab.
Diagnosis skill applied; existing maintained control is the regression seam.

## Canonical combined recipe

ADR0045 and vendor/source-overlays/niri-desktop-compat describe the new explicit
recipe. Niri pin b22eb842ca9506d4e2ff997b116031cdb3f650f8, Smithay pin
8a270232f1bfc416c781273c98a3f52a812a0bba. Original capture and26.04 lifetime
recipes remain byte-for-byte equivalent in their provenance records; receipt
target/niri-desktop-compat-patch-source-01/retained-recipes.json. The debug patch
copy is marked and not used to build. No Cargo cache source was edited.

Official cached Smithay archive SHA
3c49f63df8665c8639bae0dabb36705be921c9619f1ebe08bad20fdda34fb558;
license331891ba7afe9ea12e7f9834bdb37db7abd9131183ed85215f6b4109b0303877.
Six original files and licence match the exact Cargo Git checkout. New Smithay
patch db59669ed645cefa0d5499aa218f76e9d7f370df1feaaa60bbc6d87c90a4beaa.
It preserves the old lifetime policy with InputTime::now for the newer interface.
Existing Niri resume patch and GPL licence are reused unchanged; MIT retained.
Fresh materialization verifies both archives, exact patch scope and hashes.
Prepare-only49292 terminal0; target/niri-desktop-compat-prepare-01/canopa-build.json.

Source binding shared in scripts/niri_source_bindings.py by both builders.
Only smithay/smithay-drm-extras become local paths; exact official URL/revision
validated, all unrelated manifest/lock values retained. Invalid manifests/locks
are refused before either file is written. Four unittest groups cover both pins,
spaces, unrelated Smithay-client package, wrong URLs/revisions and duplicate/
missing nodes. Tests pass. Existing check-script15groups/10.899s pass91413, and
the new binding checks are registered in the explicit workspace gate. No legacy
build flags were carried into the new companion build.

## Build and first native evidence

Combined build66944 terminal0,8m12s. Compiler rustc1.97.0-nightly14196dbfa; one
incremental dev job, locked/offline, existing target/niri-screencast-native cache.
Receipt target/niri-desktop-compat-source-01/canopa-build.json. Original lock SHA
a3774f2ee8da8ff96a1a7e9c4f795578fb60918724e1a4feb4618375f1809bc6;
prepared lock8e4397ae9e95917464b13427278486a0da66c0d109e9eaa0d7840c6cce337d6a;
all other locked dependencies unchanged. New binary SHA
73b993cbc3c5036701bf612ecb8e260152cce79f408be61b7d2d4cf1bc923026.
Version suffix +canopa-desktop distinguishes this candidate. It is not installed.

Original strict GTK control50799 terminal0 on this exact candidate. Source52
destroy16:16:51.092022/delete_id16:16:51.149106 → data-device Leave16:16:51.149117,
before virtual-pointer button release16:16:51.316199; no Drop. Artifact
target/niri-desktop-compat-cancel-green-01. This is virtual pointer evidence,
not physical input/touch acceptance.

Root drag probe61316 terminal0,7m47s, standard default debug features/cache. Current
Files executable build71122 in progress. Combined media dark run's handle is in
the latest tool response. Freeze the app/probe together before remaining native
transport/Files checks. Portal stays fixed at bin-03 SHA
dd6f875e93fdb1981ebbce64307f41c6dbf94d1f0e327420c7fec03255977f8d.

Next: finish Iced/GTK retained/cancel controls, actual Files exchange/clipboard/
gesture cases and ScreenCast17groups per appearance on the same combined hash.
Then run required shared validation once after final canonical/tooling edits.
Do not infer distribution or full desktop-matrix acceptance from these scoped
checks. Application archive still does not install Niri. No subagents, commits,
PRs, user routing changes, VT/DRM takeover or cache deletion.

## Native checkpoint and startup investigation

Files build71122 terminal0/14.30s; frozen app/probe receipt at
target/native-test-snapshots/niri-desktop-files-bin-01/receipt.json.
Files SHA68b6555613bbf9423484594d9c9b899c68d435a1b34178d156f433e1de084bce;
probe SHAc168ec3ad1c4252ab65e2705dc36275b3712223e8741269bc24dc1d3c1eaec22.

First combined media82335 terminal1 before the first consent window; no capture
or media was reached. The cause is not established. Added bounded consent failure
snapshots and fail-fast reporting for an early Start response to the maintained
fixture, without changing production code or deadlines. Minimized debug-only
initial-consent loop under /tmp/canopa-screencast-debug/initial-consent.py passes
on combined11570 and old capture38492, both terminal0. Do not claim a cause/fix
from those passes alone.

Serialized driver12964 terminal1; receipt /tmp/canopa-niri-desktop-native-01.json.
Its first seven-case matrix has six passes and one failure. Transport5checks,
Iced cancellation, independent GTK cancellation and Sway control all pass. Actual
Files light exchange and dark gestures/Undo pass. Dark exchange fails before Niri
IPC, before Files starts. The aggregate remains failed; individual successes
must not be described as a green batch.

The same driver then passes full ScreenCast17groups in dark and light125%, each
seven real media/control/crash streams plus six SPA cursor captures, on combined
SHA73b993cbc3c5036701bf612ecb8e260152cce79f408be61b7d2d4cf1bc923026 and frozen
Portal bin-03. Artifacts target/niri-desktop-compat-media-{dark-02,light-01}.
Dark exchange retry24607 also terminal1, but now Niri is ready and the initial
Files Downloads window misses its15s deadline. Wayland trace shows client startup
continuing near that deadline; no exchange operation was reached. Artifact
target/niri-desktop-compat-exchange-dark-02. Do not conflate the two startup failures
or attribute either to source-lifetime behavior without evidence. Next compare the
same frozen Files/unchanged case on the old capture compositor.

An additional version-policy red uses the existing integration harness under the
private version shim1.4.7: the generic session-lifetime test incorrectly insists
on mask7 and optional Metadata. It fails101 with actual3/expected7. Log
/tmp/canopa-screencast-optional-cursor-red-01.log. Updated that generic test to use
Embedded and accept exactly the supported desktop masks3/7; headless remains0.
Metadata boundary/rejection retains its separate five-case wire-policy coverage.
The changed test still needs compilation and normal/old-policy execution.

Snapshot copies consumed disk space. Content-preserving dedup92745 currently
running; only verified inactive goal-created compositor snapshots become
read-only hardlinks, with original paths/hashes retained and no Cargo cache
changes. Wait for that actual session before starting another native process.
Receipt target/native-test-snapshots/compositor-dedup-20261030.json when complete.

## Completed reruns and shared gate in progress

Dedup92745 terminal0: 31 inactive compositor snapshots retain their paths and
verified hashes as read-only hardlinks. Duplicate payload reduced by4135506536B;
no Cargo build output/cache deletion. Receipt above records each original mode,
hash and canonical path. Root free space recovered to5.8GB.

The unchanged dark Files exchange passes on old capture-only24ab (13373 terminal0,
8checks, target/niri-desktop-exchange-old-control-01) and combined73b (86410 terminal0,
8checks, target/niri-desktop-compat-exchange-dark-03). These individual passes do
not erase the original failed aggregate or establish the startup timeout causes.

Updated Portal integration compilation/run13468 terminal0:21pass/2native ignored,
41.98s compile and2.07s execution. The same updated harness under the private
PipeWire1.4.7 policy shim35082 terminal0:21pass/2ignored,1.91s. Logs
/tmp/canopa-niri-desktop-portal-integration-01.log and
/tmp/canopa-screencast-optional-cursor-green-01.log. The shim tests capability
policy, not real old-library ABI/media acceptance. Root formatting34846 passes.

Normalized feature/dependency comparison passes for all393 resolved Niri packages:
/tmp/canopa-niri-desktop-feature-graph-01.json. Only intentional source identity
and local path differences are normalized; features and dependency edges match.
The original terminal handle75008 no longer exists after its result was lost to
output truncation, so this evidence is the durable computed receipt, not a
recovered terminal exit code. No comparison was restarted solely for observation.

Full workspace gate launched2026-10-30T17:10:58UTC as detached tracked worker960997,
with exact host command/start-ticks verified live. Durable status
/tmp/canopa-niri-desktop-workspace-01.json; main log
/tmp/canopa-niri-desktop-workspace-01.log. Tooling, formatting, all-target Clippy
and649-package attribution checks pass; Cargo workspace tests are running.
The previous gate's SIGTERM cause remains unproven. Wait for this worker's actual
completion receipt before reporting a final workspace outcome.

## Completed full workspace gate

Tracked worker960997 finished2026-10-30T17:38:38UTC, exit101,1659.48s. Full shared
gate is failed. Tooling, formatting, workspace all-target Clippy,649-package
attribution and test compilation4m25s pass. Files library372pass/34ignored in
251.59s. Files integration finishes847.98s:596pass/10fail/41ignored. The exact ten
main failure names match the recorded preceding baseline, with no additional or
removed names. Comparison /tmp/canopa-niri-desktop-workspace-comparison-01.json.
No quota error observed. This is a completed test result, distinct from the prior
143/SIGTERM run; its termination cause remains unknown. The current gate stops
before later packages/standard script stages, which need a separate continuation.

Current Shell/Notify/Files ordinary incremental debug binary build54616 is still
running for a full188-case native desktop matrix. Temporary tracked orchestration
/tmp/canopa-niri-desktop-matrix-worker-01.py is prepared, not launched: it will
retain immutable app/probe snapshots, continue remaining package/script checks
after the failed workspace gate, then run the unchanged full matrix on combined
Niri73b. Wait for the build's actual terminal result before snapshot/launch.

## Frozen application build and tracked continuation

App build54616 terminal0,12m52s; log
/tmp/canopa-niri-desktop-matrix-apps-build-01.log. Snapshot copy92771 terminal0,
source/copy checksums verified before/after each copy; read-only snapshots and
receipt target/native-test-snapshots/niri-desktop-matrix-apps-01/receipt.json:

- Shell e837101276261d1f00e3f359464670ba7f95a51eca8c1fc80ed6dd7835918be7.
- Notify1d706f4b7c1b6b6fb287e1fee49d2618fe5ecd8d2973c2c5c3812160130ee36b.
- Files48ba6c39d29c6b851c0ed65d553aec0b77168d725ab2b0e67a7b59596544e840.
- Workspace-built Portal f69c86efe170d409c5a3597d5675c04002d429fdb1e33b330bcd459acd1a7cb9.
- Existing probe c168ec3ad1c4252ab65e2705dc36275b3712223e8741269bc24dc1d3c1eaec22,
  under examples/ so the unchanged native fixture discovers its sibling path.

These app hashes differ from earlier scoped native runs; preserve that distinction.
The earlier bin-03/Files68b acceptance remains tied to its original receipts.
Root free space after snapshots3.37GB; no cache cleanup. No new Cargo profile or
incremental override from measurements was used.

Detached continuation worker1004364 launched with actual host command/start-ticks
verified. Durable status /tmp/canopa-niri-desktop-matrix-worker-01.json; worker log
/tmp/canopa-niri-desktop-matrix-worker-01.log. It runs the remaining workspace
packages excluding the failed Files package, then Settings, CLI/configuration and
vendor stages separately. Those results do not make the failed full gate green.
It then invokes the unchanged188-case desktop runner with the frozen apps and
exact combined73b compositor into target/niri-desktop-compat-full-matrix-01.
Existing fixture source hashes and binary hashes are retained and checked for
changes. Wait for actual per-stage/matrix outcomes, not just a live PID.

Do not edit Cargo/application/vendor sources until the remaining compilation and
standard stages are terminal. During the native matrix, frozen app/compositor
bytes are independent of root rebuilds; preserve its existing fixture sources.
No subagents, commits, PRs, installs, live routing/VT/compositor changes or Cargo
cache deletion. Full18-interface goal stays active; RemoteDesktop has only a
private Rust EIS/libei socket proof and preparation notes, not a backend yet.

## Disk exhaustion, completed continuation and recovery

Worker1004364 is terminal, not a live wait. Its remaining-workspace stage fails101
after1475.43s while free workspace disk reaches zero; Shell test linking reports
collect2/ld signal7 (Bus error). Do not call that a test assertion failure or infer
its exact cause solely from the signal. Settings passes2.09s; CLI build passes
3.06s with a cache-database-full warning; configuration passes3.40s. Vendor fork4
and event-batch3 pass, then the native-pipe leaf fails explicitly with ENOSPC while
writing its fingerprint. Full-matrix stage fails1/1.09s at output.mkdir with ENOSPC.
No matrix scenario starts and target/niri-desktop-compat-full-matrix-01 never exists.
The original full workspace gate101/ten Files failures remains a separate result.

Two further completed compositor snapshots become verified read-only hardlinks,
retaining paths/bytes/modes and reclaiming306397760 duplicate bytes. Receipt
target/native-test-snapshots/compositor-dedup-20261030-02.json. No Cargo output or
cache was deleted by the agent. Space was exhausted again by ongoing compilation;
ordinary sandbox startup also reports ENOSPC. The user is asked to free8GB and
retain active/debug artifacts. All validation/probe processes then become terminal.
Temporary checkpoint /tmp/canopa-remotedesktop-staging-checkpoint-01.json preserves
the results while repository writes are unsafe.

User subsequently confirms they will free at least8GB. Actual disk inspection,
rather than that promise alone, verifies24515985408 free bytes. Retry worker1017717
is launched and its actual host command/start-ticks verified live. Status
/tmp/canopa-niri-desktop-matrix-worker-02.json. It retries the same remaining-workspace
command and only the failed vendor-pipe leaf, retaining passed Settings/CLI/config
and vendor fork/event-batch outcomes. It then runs the unchanged188-case matrix
with the same frozen apps/compositor into target/niri-desktop-compat-full-matrix-02.
No production source/Cargo/vendor edits until these compile/standard stages end.
No full-matrix acceptance is claimed from startup, receipts or a live process.

During the disk outage, a finite Rust EIS read primitive is staged and verified
entirely under /tmp: original read consumes8192bytes and fails the2048-byte chunk
invariant; the actual modified library passes four socket regressions and real
libei interoperability. Patch d6d96cf4f22e8fc5312513366fe095e9f10e1e7c51e6ce87798e02dc0935d223;
/tmp/canopa-remotedesktop-bounded-reis-stage-01/verification-02.json. This is not a
production dependency, worker or native desktop-input proof. See RemoteDesktop
preparation for adoption and remaining frame-budget/session work.

Retry1017717's remaining packages pass0/1434.18s and native-pipe0/4.27s. The full
matrix02 actually begins with frozen matrix-apps-01 and Niri73b; it is still running.
This is a partial-progress observation, not a completed green matrix. Existing
fixture sources remain unchanged. The new canonical EIS reader fork is adopted
only after those compilation/standard stages end; it is independent of the frozen
matrix executables. Its112library/25integration, focused Clippy/headless and
provenance checks pass; whole-workspace validation of that new dependency remains
part of the active RemoteDesktop phase. See ADR0046/REMOTE-DESKTOP.md.

Partial native matrix02 observation: files-iced-drops-light fails before any drop
workflow begins, waiting for the initial Downloads — Canopa Files window. Its
summary has windows:[], files.log is empty, and Niri is ready before timeout.
Artifacts target/niri-desktop-compat-full-matrix-02/files-iced-drops-light and the
sibling .log. This is a startup failure with no established cause, not an observed
drag behavior failure. Preserve it and complete the remaining cases. Diagnose
with the same frozen app/control compositor and an unchanged isolated case after
the batch; do not erase the original aggregate or extend deadlines to obtain green.

2026-10-31 partial matrix02 observation:88cases finished,53passed/35failed. The
worker1017717 remains running; this is not its terminal aggregate. In addition to
the initial drops-window startup failure, retained failures include clipboard
process exit, job notification text, device operations and later recovery/Undo
checks. For example job-lifetime-dark records an error body about a non-regular
child, while its predicate waits for a Completed with errors prefix. That is an
observed mismatch, not yet an established cause or baseline classification.
Finish the unchanged frozen batch, then diagnose exact isolated controls. The new
RemoteInput candidate a4edd is independently frozen and does not replace Niri73b
or any application executable used by this matrix. No existing fixture source
was edited for the new candidate; only new keyboard harness/receiver files exist.

Latest 2026-10-31 matrix02 observation:168cases finished,90pass/78fail; worker
1017717 is confirmed live by host PID/start_ticks25850704/command. Every recorded
pre-existing fixture hash is unchanged. It is not a terminal aggregate and remains
red. Private EIS worker acceptance uses separate frozen Niriecd14 and worker950e;
its12groups do not replace this matrix's frozen73b compositor/application results.
One final read-only process-observer approval review timed out before execution;
the permitted single retry succeeded. No restart or source mutation followed it.


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


Clipboard diagnosis narrowed further on unchanged Files48b/Niri73b. The Copy/Undo-only
prefix passes (target/niri-clipboard-prefix-02). Adding the first conflict Cancel
reproduces the same10s process-exit timeout (prefix03), while an independent read
observes the foreign clipboard. Captured private SQLite state records request4,
revision3, phase3 TransferPending with canceled=true and exact remaining
original.txt. The public operation-transfer test explicitly requires unfinished
work to hold the process until resume/Abandon. No Files lifetime code was changed.
The temporary explicit-Abandon diagnostic is not accepted: initial syntax, startup,
immediate persistence assumption and fixture C-compile failures are retained.
An alternative correction is staged under /tmp/canopa-clipboard-fixture-stage-01:
a dedicated no-operation clipboard lifetime case keeps the original10s assertion,
while the original multi-operation workflow checks the independent unfinished-work
owner. Python syntax only; native proof and adoption remain pending. Original
188-case red aggregate and149 exact fixture-source copies remain untouched.

Shared-session draft /tmp/canopa-remote-sharing-stage-01 is unadopted/uncompiled.
It extends the existing ScreenCast registry/actor with immutable session origin,
independent device/source selection and cancelable EI FD connection; five private
bus tests are prepared for shared ownership, startup/connection cancellation,
invalid grants, common16-session admission/shutdown and caller/source/backend loss.
Only rustfmt has run. No RemoteDesktop production presenter, legacy Notify, Iced
remote consent, main registration or advertisement is claimed. Source adoption
waits for the full gate's1462-input receipt to become terminal.


2026-10-31 fixture correction native batch01 is terminal1: lifetime-dark passes
33.87s, lifetime-light passes22.98s, full workflow-dark fails103.21s, full
workflow-light passes127.65s. Artifacts and source/binary receipt are under
target/niri-clipboard-fixture-correction-01/. Frozen Files48b/Niri73b are unchanged;
changed_sources is empty. Both dedicated lifetime cases retain the exact10s exit
assertion. Full light workflow reaches the corrected final check and confirms
unfinished transfers retain Files after clipboard supersession. Dark fails at
fixtureline238 while wtype enters the first background-conflict destination; its
command times out10s before reaching the changed final check. Cause remains
unestablished. Preserve the red3/4 batch; do not report four passes or extend any
deadline. The correction is staged, not yet adopted. The broad gate also passes
the explicit unfinished-work lifetime/Abandon policy integration tests.


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
