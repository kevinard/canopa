# Iced devices — implementation in progress

## Current checkpoint: native device drops, 2026-10-20

The unchanged appaf0924853110 passes eight device-drop checks per appearance and
nine existing light device UI checks (25 total). Default Move, Ctrl Copy and
shared Undo target only the mounted Open row, leaving the destination browser's
local location and original drive notes unchanged. The separate removal control
and unmounted row refuse drops without device commands or operation admission.
Disconnect/replug changes the private provider UUID while keeping the same root
path and inode: the held gesture remains refused, while a fresh gesture works.
Explicit Open mounts through GIO; the same mounted root also supports Move/Undo
in a 340px tile with no sidebar. The private provider itself is unchanged.

The first attempt's post-Undo source-selection assumption was corrected through
ordinary Home input. No production change or broader device completion claim.
Native04/Native05 have terminal-zero aggregates; primary review only. See
[device-drop verification](dnd-devices-verification.json) and ADR0095. Physical
devices, expanded questions/helper loss, accessibility/performance and full
product/release acceptance remain open.

## Earlier checkpoint: private Power off/Eject acceptance

The full PRD remains active. Iced controls and backend attention are implemented
as recorded in ADR0091/0093 and the later checkpoints below. The private provider
now also implements actual GDrive Stop/Eject callbacks; application Rust/Cargo
inputs and the preserved `88c9b7d2eb52` executable are unchanged.

Twelve new headless drive checks and sixteen UI checks pass, including busy
errors, exact cancellation/EOF teardown, unplug and stale refusal, no-force
completion, all-window closure, same-owner reopening and orderly Quit. Existing
device UI (18) and helper/catalog (15) regressions also pass: 61 checks over nine
selected scenarios. Captures of both capability labels in dark/light and result
rows were inspected. The inventory now contains 160 scenarios.

Source01/native02 verifies the provider. Native03's four new UI cases each
finished successfully, then its outer runner terminated with status 143 during the existing
dark regression, without an aggregate receipt. Cause unknown. Source04 changes
only a helper summary's scope wording; native05 completes 27 helper and nine dark UI checks and
native06 completes nine light UI checks with terminal-zero aggregate receipts. All source
and application hashes are checked before preservation. No whole-workspace,
full-matrix, physical-device or production-performance result is claimed.
This extension received primary review only. See device-drives-verification.json.

## Historical implementation checkpoints

The full Files goal remains active. ADR0091 records the complete backend-owned
device catalog/command direction; prepared catalog observations are verified.
Shared backend admission now coordinates filesystem work and
arbitrary custom actions, with scoped verification. DeviceRequests now exposes
renderer-independent parent commands, questions and actual teardown ownership.
Iced controls and dedicated device attention remain pending. Exact origin
navigation and full native/physical-device
acceptance remain required. Do not redefine the device requirement as catalog only.

Baseline: media-source-53.tar.gz / media-runtime-source-53.json, all623 inputs
verified before editing. Prior binary03ed1b63 remains available and compressed.
New wire version2 publishes a deduplicated local catalog with opaque observation
tokens, raw root paths and mount/safe-removal capabilities. Storage preparation
precedes catalog readiness; failure/stop clears entries. Volume/drive signals
publish even without a mount. Existing observer/readiness ownership is preserved.

Source05 validation passes: 68 library parents and the actual isolated lazy-init
child, four operation-startup tests, Files all-target Clippy with warnings denied,
workspace formatting and the headless local-runtime library check. The normal
headless dependency graph has no Iced or GStreamer. Python syntax, all 151 scenario
references and the 585-package dependency inventory check pass. The default test
build produced app dcdab9df33b75402cb381242b8667361f6567f67953c9d72ffa2c5f4f27bb2b4.

The actual preserved old helper emitted version1 mounted roots without a catalog
(red03). The new private GIO provider/helper case passes five checks in 2.64s
(native06). Existing Iced operation coverage passes 12 checks in 106.64s, including
queued work and helper loss; initial headless residency passes four in 6.50s
(runtime08). All 628 source hashes and the app hash match after each completed run.
These fixtures use private actual files/providers and never change user mounts.
They establish neither physical-device nor Iced device-control acceptance.

Both source05 scoped review axes report zero findings and verify 628 source / 10 scope
hashes before/after. See device-catalog-review-results-05.json, validation07 and
runtime-observation08 receipts. Catalog checkpoint source/app identities are recorded in
device-catalog-preservation-10.json. Device execution, shared removal coordination, credential
attention, cancellation, Iced controls and complete acceptance remain pending.

The shared LocalActivity reserves work before submission and retains it through
questions, queued execution, cancellation and result saving. Application and
Iced use the actual hold and retirement notices for residency. The new public
tests use temporary files, real command processes, failed SQLite saves and
bounded history reads. They do not simulate GIO removal or establish device UI
acceptance.

Source04 passed 96 scoped default tests but review found control starvation at
1,024 deferred admissions. The actual saturation test fails on source06 (red07);
trace08 confirms its cancelled Recovery inspection cannot settle. Source09 keeps
receiving controls and explicitly refuses excess admissions. Its first default
run10 stopped at an existing media test's setup race: a frame was available
while playback still reported Preparing. The corrected setup waits for a frame
and Paused, retaining its original assertion and deadline. The decoder can
publish preroll after sampling an earlier pipeline state. This is a test setup
correction, with no media behavior or timeout change.

## Shared admission verification

Source11 green12 passes 97 parent tests: library69, coordination8, actions4,
operations12 and startup4. Source17 validation18 passes another53 across
clipboard_capacity1, local_actions1, operation_recovery12, operation_transfers27
and operation_trash12. Actual isolated child fixtures run through their parents;
nested child summaries are not added to150. The notification-bus ignored test
is not covered by that count. Files all-target Clippy with -Dwarnings, workspace
formatting and local-runtime/no-desktop library check all pass. Source20's normal
headless dependency graph has no Iced or GStreamer. All630 pinned inputs match
before/after each final run; no whole-workspace or full-matrix result is claimed.

Expanded validation13 first stopped at an existing Trash startup-lock EAGAIN.
Isolated14 and parallel retry15 passed unchanged. A raw OperationEngine probe
without the new queues reproduced0 lock conflicts before/after vs1100 conflicts
(including retries) during two concurrent child-spawn threads,600 successful
opens per phase. Source17 isolates the existing leased-restore test in an exact
child with unchanged assertions and15-second timeout/kill/reap. Production lock
and retries are unchanged. Diagnosis16 and validation18 retain exact evidence.

Native19 app8849 passes headless4 but fails action traversal and resident Review.
Captured focus and windows show outdated Tab counts after Recovery was added:
actions opened Recovery; resident input cancelled Undo and released its queue.
Source20 updates only those traversal counts and adds focus captures. Native21
passes actions15 and resident6 on the same125842056-byte application SHA256
8849dd155e0ab5da632e355342c34e8c1c93ca7fe4f6eb4dda242f3cf33867dd.
Together these are25 native checks, not full device acceptance. Copy-all and
Review focus captures were inspected; clipboard/output assertions, cancellation,
raw filenames, pending decision, preserved later bytes and actual queued work
remain exercised. No new timeout/retry or application change was needed.

Both read-only review axes report0 on source17's18-path implementation scope,
all630/18 hashes before/after. Follow-up source20 review agents failed due to the
account usage limit; primary review checks the two fixture changes and final
documentation against actual traversal/results. Do not report independent
review of the final fixture/doc delta. Full command/credential/cancellation/UI/
physical-device, complete native/workspace/release and whole PRD gates remain open.

## Helper command path in progress

The next stage adds bounded helper command frames, actual async GIO dispatch,
current-token/capability revalidation, cancellation, owner-EOF draining and
sequence-bound password/choice interaction. Parent command admission and shared
RemovalGuard retention are not connected yet, and no Iced control invokes it.
New helper traffic is internal version1 alongside unchanged observation version2.
Secrets have no Debug implementation and parser errors never quote input.

Source01 compile02 stopped at three GIO methods gated behind v2_58. Enabling that
existing dependency feature (no new package) resolves it. Source03 focused04
passes8 local-runtime tests in67.67s: four actual GIO signal/credential/choice/
abort tests and four prepared-observation tests. Follow-up teardown now also
drains bounded input overflow through cancellation rather than quitting ahead
of an active callback, and rejects questions after cancellation/retirement.
The private provider and maintained helper-command scenario now cover delayed
requests, errors, credentials, superseded questions and disconnection.

Source05 validation06 passes8 focused local-runtime tests, the default app build,
Files all-target Clippy with warnings denied, formatting, the152-scenario native
inventory and585-package dependency inventory. All633 input hashes match
before/after. App126542776B SHA256
33401537b748e2962bb716b21880b4a8500153d02d4777d01cd782e1fd7595c8
passes native07: helper commands10 in4.60s, catalog5 in1.51s and headless4 in6.64s.
The helper case verifies actual provider calls and unchanged private bytes,
credentials accepted with saving disabled, no secret in output/logs, explicit
stale-answer refusal, cancellation and helper callback retirement on ownerEOF.
All633 source hashes and the app hash match after native acceptance. No actual
user mount/device is touched. Drive stop/eject are compiled paths only; provider
execution, parent admission/guard lifetime and Iced controls remain pending.

Primary review checked the helper lifetime, bounded input, exact target checks,
GIO signal types and actual native oracles. Independent follow-up reviewers remain
unavailable at the account usage limit; do not claim independent review of this
helper stage. The parent stage below adds an explicit terminal refusal flag;
hold retirement is never inferred from message text. Full device and entire PRD
gates remain open.

## Parent workflow in progress

The parent now reserves shared work/removal holds before bounded command
enqueueing, revalidates current observations and publishes typed request/question
snapshots. A private writer owns pipe I/O; the observation reader also processes
typed helper events. Terminal refusal is explicit, cancelled work retains its
hold, and lost completion becomes Uncertain only after helper/writer teardown.
Shutdown closes device admission around filesystem teardown and refuses active
device work. Application's existing shared activity hold remains authoritative.
Iced controls and dedicated device attention are still pending.

Parent source01 compile02 passes the eight existing local-volume tests. The new
public-runtime harness initially stops at two test-only Pid accessor compile
errors (runtime04); it had not executed at that point. Corrected test accessors and owned
helper discovery scan the observer thread's child list. The harness uses the
real private provider and temporary files, with no GUI or actual user mounts.
The subsequent checks below verify the parent workflow.

Corrected source05 public-runtime06 passes in7.50s. The actual private helper is
stopped during cancellation: dropping the frontend and requesting shutdown do
not release removal; real filesystem admission and a custom action remain
blocked until the native callback retires. Further checks cover stale token
refusal without a provider call, actual credentials, current choices and killed
helper uncertainty after reaping. The old mounted directory remains present;
queued mutation after helper loss fails and preserves its files. This is public
backend acceptance without a GUI. At that point default-feature regressions,
build/lint and native host checks remained pending. Source635 hashes are unchanged
by the run.

Source07 validation08 passes101 default-feature parents (library73, actions4,
coordination8, operations12, startup4) plus the actual custom runtime program's
five check groups. Its build passes; Clippy then stops at two !is_some_and style
expressions, so formatting/inventory do not run in08. Source09 normalizes those
expressions, updates the harness's matching style and strengthens native refusal
oracles: a stale Start is terminal, stale active answers are not. No intended
behavior changes. Validation10 passes headless8, the public program including
unchanged filesystem Undo, default build, all-target Clippy with warnings denied,
formatting and the585-package inventory. All635 hashes match before/after.

App127265752B SHA256
546265eeb1b69278e2b84e0327c447d24665e479bdab53ccdb195ebbb6cd07e3
passes native11: helper10 in3.45s, catalog5 in1.42s, headless4 in7.01s and resident6
in37.58s. The resident review capture shows the same waiting decision with Cancel
focused; later private bytes and queued work remain asserted. All635 source and
app hashes match after native acceptance. No actual user mounts/files changed.
This parent stage received primary review only; independent follow-up remains
unavailable. Device notification attention, backend current-root lookup, Iced
controls/origin behavior, drive stop/eject provider execution, physical devices
and the entire PRD/workspace/release/performance gates remain open.

The parent checkpoint identified the next availability check before device UI:
action_jobs::ActionRequest.run then canonicalized its folder and prepares paths only after the shared
removal wait; neither ActionJobs nor LocalActions consumes Storage. Add a public
regression for a queued action targeting a successfully unmounted device whose
old directory/files deliberately remain present, then bind the prepared original
media/availability in the backend. The current positive queued-action case uses
an unrelated local folder after cancellation, and does not prove this guard.
At that parent checkpoint this was a coverage/interface gap, not yet a reproduced failure.


## Managed custom-action availability

Test-only source01/red02 reproduced actual queued execution after successful
private-device unmount: result_ok=true and output_exists=true. The old directory
and original file deliberately remain present. This was a behavioral failure,
not a setup, compilation or timeout result. Application production composition
is used without Iced, with private HOME/XDG/GIO and real temporary files.

Source03/green04 exits0 in77.36s: action_jobs5, operation_startup4,
storage_requirements5 and the actual public runtime program pass. PreparedStorage
publishes the existing Operations store to managed actions. Capture is memory
only and refuses missing preparation or known observation loss. Workers validate
cwd and each selected entry/referent after the shared removal wait. No command
replay, persistent execution authority or frontend filesystem policy is added.

Expanded verification now covers both symlink directions, selected device files,
unrelated local work after a successful unmount, explicit reinvocation after
remount and new managed submission after helper loss. Final checks are pending.
This stage has primary review only; independent reviewers remain unavailable.

Expanded06 exits101 after action_jobs5, actions5 and coordination8 pass. The
public-runtime assertion required the word unavailable; an epoch change correctly
refused execution with device-changed and no output. This is a test-oracle
correction, not a new product failure. Accept either explicit storage refusal
while retaining the no-output assertions. Remaining targets had not run.


Source07/final08 passes40 renderer-free integration parents plus the public
runtime program's eight check groups in20.33s; ignored private helper definitions
are excluded. Default Iced library73 passes in101.94s (including compilation),
build124.21s, all-target Clippy-Dwarnings82.27s, formatting6.57s and maintained
native inventory0.60s pass. The final dependencies invocation mistakenly omitted
required --output and exits2 (argument parsing only). Corrected dependencies09
passes the585-package inventory in3.34s. No product change was needed.

App127310128B SHA256
d67e851655404661726dfd8e6b7978fec4551dba6b3032f8a977d18b83dfbd19
passes native10: helper10, catalog5, actionsdark15, actionslight125%15, headless4,
resident6 =55 checks. The actual runtime checks last-browser command survival,
process-group cancellation, same-process reactivation and a windowless decision
plus queued mutation. All636 source hashes and app hash match after the run.
Representative resident-review and light narrow paged-output captures inspected.
All compiler/native handles terminal. No user mount/file mutation and no cleanup.

Primary review checked publication identity, startup refusal, shared hold ordering,
worker storage/cancellation checks, descriptive results and actual test oracles.
No independent review claim. The completed unmount may refuse through either
unavailability or changed epoch; tests require an explicit device refusal and
no command output. Selected alias targets are resolved on the worker rather than
pinned at invocation; no content identity, atomic validation/start, script sandbox
or durable replay authority is claimed. Full source/journal/restart gates remain.
Device current-root lookup, backend attention, Iced controls and complete PRD
acceptance are next; this guard checkpoint does not redefine the full goal.


## Current roots and backend device attention

Source01/root02 exits101 after8 headless library tests pass. The public runtime
then assumed Ready while a new observation was explicitly non-navigable during
preparation. The harness now waits for a prepared snapshot; no old-root authority
is inferred from that transient state. Source03/attention04 passes8 headless
library tests and all9 public runtime check groups in68.72s.

Source05/core06 adds a real Storage/protocol regression for unrelated revisions,
remount and failed preparation, plus immediate navigation invalidation before
observer teardown. Nine headless library tests and runtime9groups pass59.93s.
The actual private provider checks unplug/replug reusing the same native objects
and retaining the old directory; completed-root proof stays invalid while a new
explicit current selection succeeds. All original bytes and Undo are preserved.

Native07 passes4 headless device-attention checks in4.03s. The new devices mode
in verify-files-action-host compiles the local-runtime custom harness, preserving
the production helper pin. A real private Notify reply is held; Application
lifetime remains held and shutdown refuses. Explicit credentials answer the exact
question; all3 expected notifications settle, original bytes survive and secret
text is absent from notifications/logs. No actual user bus/mount/files are used.

Default08 passes103 ordinary parents (lib74,actions5,coordination8,operations12,
startup4), plus runtime9 check groups in282.78s. Default build4.57s, all-target
Clippy-Dwarnings93.85s, fmt6.29s,153-case inventory0.26s and585-package inventory
3.01s pass. All637 source hashes match before/after. Ignored private helpers
are excluded from parent totals.

App127851672B SHA256
f19d9bf4de647c168911dac320ca21af6587c641bf1d3c253da4f2f511cd2d0d
passes native09: helper10/catalog5/headless4/resident6 =25 checks. Same-process
reactivation keeps the original decision and queued work; actual private bytes
remain asserted. Resident review capture inspected, including default Cancel.
All637 inputs and app hash are unchanged after acceptance.

Primary review checked readiness invalidation, private mount/epoch proof, query
lock order, held notification receipts, bounded admission/delivery/errors, obsolete
question coalescing and error-before-ack retirement. No independent review claim.
The query is an observed point-in-time location, not filesystem execution or
restart authority. This stage does not expose Iced device controls or claim native
credential-window/physical-device acceptance. Those controls and precise origin
navigation are next; the full PRD and release gates remain active.


## Iced device controls and floating questions

Source01/check02 exits101 at three incorrect Wrapping.push calls; no runtime
checks had run. Rows now wrap after all children are added. Source03/Clippy04
passes all targets with warnings denied in72.80s. Source05/validation06 adds the
actual native fixture, private delayed-question provider and backend anonymous
normalization. It passes77 library parents+runtime9groups, build, Clippy, fmt,
155-case inventory and585-package inventory. Intermediate4304 was not native
accepted or preserved as a final candidate.

Source07/final08 moves Open's root-versus-mount decision into DeviceRequests.
Public runtime uses that interface for both forms and stale refusal; Iced keeps
only browser intent. Clippy78.55s,77lib+runtime9groups210.83s,build6.22s,fmt6.40s
pass. App129171472B SHA256
ccc108f60b5acf0b6905bcaea40d01c5d79dbe7a082d80fcd232de8dbb88b4d3
passes native09 darkUI9. Native10 light fails its first pointer click: hardcoded
1280x800 normalization hit Downloads on the125% output, whose logical dimensions
were1024x640. Images show the unchanged Downloads browser and no provider request.
This was fixture calibration, not a product failure. Other native10 cases pass
44 checks (helper10/catalog5/actions15/headlessattention4/headless4/resident6).

Visual review found duplicate Cancel buttons when the provider also offered
Cancel, plus inert controls on finished rows. Source11 projects shared Cancel
once, preserves all other provider indices, hides unavailable finished controls,
uses No commands yet beneath Commands and reads actual Niri logical dimensions
for pointer input. Fixture choice traversal changes fromTab2 toTab1 accordingly;
provider success still requires the original choice index1.

Source11/final12 passes all-target Clippy-Dwarnings81.38s,77library parents plus
public runtime9groups203.81s,defaultbuild5.60s,fmt7.02s. Inventory inputs match06.
Final app129182144B SHA256
88c9b7d2eb52aa5ed17e146894aeea22b375c82fb2395a9af1d50a65d35bb210.
Native13 passes darkUI9/light125%UI9/helper10/catalog5 and writes passing action
assertions15. The OUTER runner then exits143, with no aggregate; cause unknown.
Its action log prints final JSON after Session.close, and compositor logs show
retirement. No remaining-case artifact was started. Exact handle46973 terminal143.
Remaining native14 handle86203 terminal0: headlessattention4(3.32s), headless4
(5.88s),resident6(34.94s). Action native15 handle96059 terminal0 repeats only the
uncertain final action process receipt, passes15 in36.56s. The eight selected
scenarios therefore have62 passing checks with complete final process evidence.
No whole native-matrix pass or runner termination fix is claimed.

Both final device captures show masked password fields above340px tiles, shared
Cancel focus and the current question after supersession. Reopened Operations
review and compact result rows inspected. All640 compiled input hashes and app
hash match after acceptance. Four headless-attention checks use a separate
local-runtime test executable from those sources, without Iced; the remaining
58 checks use the same app. No actual user bus/mount/file/speaker was modified.

Primary review checked current-token Open/removal, stable non-authoritative row
IDs, exact question-bound controls/draft retirement, anonymous normalization,
parent-versus-explicit-dialog closure, same-origin navigation and no GUI FS/GIO
work. No independent review claim. New ADR0093 records those decisions, including
conservative invalidation after refresh revisions and the shared Cancel alias.
Drive stop/eject provider/physical acceptance, expanded credential/busy/loss UI,
full browser-session extraction and the full PRD/release gates remain open.
