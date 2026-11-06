# InputCapture implementation

Status: experimental Rust/Iced InputCapture v2 pointer sessions and active-only
Clipboard verified in scoped acceptance, 2026-11-01. Portal 176 library/53
integration tests pass. The latest frontend companion passes 146 native
GUI/input/media and 45 private-bus owner checks. Keyboard/
touch, broader acceptance and distribution remain open. Normal descriptor/routing
and installed programs are unchanged.

The later [frontend phase checkpoint](frontend-phase-evidence.json) fixes the
reproduced repeated-Enable/stale-Release Clipboard denial in the explicit frontend
companion. Thirty-four public capture/Clipboard groups, 112 other frontend consumer
groups and 45 private-bus owner checks pass. Its required workspace validation
repeats the same ten Files integration failures; other Rust targets and all four
post-Rust continuations pass, with unchanged source guards. See ADR0063. The completed
phase-20 evidence below retains its original binary/source pins.

[ADR0059](../../../docs/adr/0059-compositor-owned-input-capture.md) records the
compositor boundary. The experimental canonical recipe is `niri-input-capture`;
its private protocol is documented in vendor/source-overlays/niri-input-capture/.
It extends the verified desktop-pointer recipe with a real pointer capture
lease, not a portal stub or an injection worker with a renamed EI role.

## Verified checkpoint

- Seven actual geometry-module tests pass, including adjoining/partial neighbors,
  negative origins, inclusive fractional pixel spans, stale IDs, wrapping and
  finite limits without unintended mutation.
- Eight real-compositor groups pass: truthful private v1 API, preclip crossing and
  actual diverted motion/buttons/scroll, FD-close held cleanup/local restoration,
  exclusive enablement and stale release, live topology invalidation, compositor
  escape, event-reader backpressure, finite admission/malformed input and active
  pointer-source loss followed by a clean device's click cycle.
- The native source and receiver are independent Wayland clients. A receiver-side
  ordered keyboard marker verifies delivery ordering; the escape-producing source
  stays alive until capture is disabled, separating escape from source loss.
- The complete prepared tree matches 5963 canonical archive/patch inputs. Its two
  local Smithay package bindings are validated explicitly. The stable build paths
  and existing incremental compositor cache are reused.
- Existing Clipboard, pointer/media and keyboard regressions pass 54 groups in
  both appearances on the same compositor hash. Source and binary pins remain
  unchanged across those runs; private processes are terminal. Combined with the
  capture fixture, 62 scoped native groups pass.
- The required original-graph workspace gate completes all Rust executables with
  no-fail-fast. Files integration is the sole failing target: 596 passed, 10 failed
  and 41 ignored. The failure-name set exactly matches the previous capture gate.
  Portal 172 library/48 integration tests, formatting and workspace Clippy pass.
  Settings, CLI, configuration and vendor continuations pass. All guarded source
  inputs remain unchanged; observed gate storage grows by about 0.45 GB. The
  workspace remains red; native success does not clear those failures.

The candidate compositor is SHA-256
`9b9ba92c383332ff6f0c7cd2eecff0220d763a96b4fbb595dd2ce214a12b114e`.
It is retained once in the checksum-addressed store and referenced by native
runs. The old `925c49…` compositor fails with the named missing InputCapture
service, while the candidate passes. No per-case Niri executable copies are made.
The compositor build retains the recorded nightly compiler (`1.97.0-nightly`,
`14196dbfa`, 2026-04-12); root application checks retain the normal workspace
toolchain. Do not switch the compositor cache to the root compiler implicitly.

Exact receipts live in target/portal-input-capture-17/: build-02.json,
canonical-sources-01.json, reuse-01.log, geometry-01.log and native-04/summary.json.
Existing native regressions are recorded in regressions-01.json, with individual
summary.json files in clipboard/pointer/keyboard dark/light directories.
The baseline-source-receipt.json retains the previous recipe proof. Initial
build-01 fails on an immutable/mutable borrow overlap in the locked-pointer path;
cloning the focus before revocation fixes it. Native-01 reaches the new API but
fails compiling the C fixture without libm. Adding its missing link flag changes
only the fixture; native-02 passes seven groups. Native-03 strengthens the local
delivery observation with an ordered receiver marker; native-04 adds held-source
loss and passes eight groups. Failed checks are retained separately from success.

## Rust receiver transport

[ADR0060](../../../docs/adr/0060-input-capture-receiver-worker.md) adds an isolated
Rust worker. It retains the privileged compositor lease and hands its application
one separate EI Receiver socket. Original compositor/backend/caller supervision,
checked FD transfer, finite queues/framing/deadlines and an independent revoker
bound its authority. It announces pointer/button/scroll only, preserves one EI
connection through Disable/Enable, matches activation IDs to EI start sequences,
and neutralizes held input before graceful stop. New receiving devices require
explicit re-enablement. No public InputCapture backend or consent is implied.

Nine native groups pass in target/portal-input-capture-18/native-07/summary.json:
authenticated ownership and one worker-owned FD; actual diverted motion, buttons,
finger and wheel scrolling and
repeated activation; topology invalidation; receiving-device replacement; eight
sender-role denials followed by a healthy lease; original caller loss; exact
worker SIGKILL; isolated EI output backpressure; and backend process SIGKILL.
Independent local delivery and pidfd/process terminal checks accompany these
cases. Graceful loss requires explicit EI releases and stop. Abrupt death,
blocked output and client-requested device removal establish retirement, without
claiming that synthetic releases reached a receiver that cannot read them.

The first native run exposed libei 1.6's receiver ready request. A diagnostic
candidate identified the exact request; device-v2 negotiation fixes the
sender-only-v3-ready interoperability problem. Subsequent fixture corrections
keep the ordered keyboard marker alive through capture, and retire the local
libei device state when the client closes it. Failed receipts remain separate.
Final Portal checks pass 173 library tests (7 ignored), 48 integration tests
(2 ignored), formatting, Clippy and the main binary build in
final-validation-03.json. All guarded inputs remain unchanged during that check.
A later C-only wheel-fixture signature correction is compiled with strict
warnings and exercised by native-07; tooling-followup-01.json confirms no Rust or
manifest changed afterward. Existing Clipboard (8), pointer/media (17) and
keyboard (2) groups pass in each appearance: 54 more groups in
regressions-01.json. The same final Portal/harness/Niri/frontend pins remain
unchanged throughout; all 14 new capture workers are terminal. These 63 native
groups are scoped transport/regression acceptance, not a public portal or a green
workspace. [transport-evidence.json](transport-evidence.json) ties the receipts
together. Native evidence executables are compressed once per checksum and
verified losslessly before Cargo overwrites root outputs.

## Remaining implementation

[PLAN.md](PLAN.md) keeps the complete InputCapture work visible. The backend needs
real keyboard/keymap capture, wider Clipboard/phase and
loss/physical/device/race acceptance and
companion distribution. The current pointer capability does not advertise
keyboard or touchscreen capture. Scoped pointer acceptance does not establish a
production replacement or a green workspace.

## Public pointer sessions and Iced consent (phase 19)

[ADR0061](../../../docs/adr/0061-input-capture-sessions-and-consent.md) adds the
exact backend v2 interface and its own finite session actor. CreateSession2 opens
no capture resource. Start asks afresh, with pointer permission unchecked and no
trust in restore data. One EI handoff is permitted before enablement. Directed
phase/zone signals preserve compositor activation IDs and invalidated zone sets.
The control window truthfully says capture is allowed while awaiting an edge;
Stop, Escape or losing the window retires the session. Clipboard is not offered
and Start returns clipboard_enabled=false.

Final complete Portal checks pass 173 library tests (7 ignored), 52 integration
tests (2 ignored), formatting, Clippy and the main build in
target/portal-input-capture-19/final-validation-02.json. Four D-Bus integration
tests cover ownership, one receiver, receive-ordered controls/directed signals,
cancellation retirement, one Start and invalid deprecated-create cleanup.

Eight public groups pass in each appearance in native-dark-04/summary.json and
native-light-01/summary.json. These run the actual main executable through a
private descriptor and the pinned standard frontend, with real Iced windows and
EI input. They cover exact wire/capabilities; unchecked consent and denial;
legacy public CreateSession translating to backend CreateSession2/Start;
zone/barrier/FD/activation/release/reuse; ordered live topology invalidation with
held cleanup; Iced Stop; original public caller loss; and original backend death.
Consent and control screenshots are retained; worker pidfds confirm termination.
The later native-only topology test is recorded in tooling-followup-01.json; no
Rust source or manifest changed after the complete Rust check.

The first candidate exposes the macro's ConnectToEis acronym spelling; an explicit
ConnectToEIS annotation fixes it. The second reaches actual consent but sends
backend Closed before a canceled Start reply. The frontend then tries to export
the closed session and changes cancellation to generic failure. Failed startup
now retires through its response after cleanup; live-session Closed remains after
resource retirement. The third candidate passes seven groups before the trace
delta/overshoot assertions and topology check strengthen final native acceptance.
The same frozen Niri/frontend are reused throughout; installed programs are not
changed.

The final candidate also passes the nine private receiver groups, all 54
Clipboard/RemoteDesktop pointer/keyboard groups in both appearances, and 34
standalone ScreenCast groups with actual pixels/cursor modes and controls in
both appearances. Together with the sixteen public InputCapture groups, 113
scoped native groups pass on fixed binary pins. The receipts are
receiver-01/summary.json, regressions-01.json, screencast-dark-01/result.json,
screencast-light-01/result.json and native-regressions-01.json in the same
phase-19 directory. [session-evidence.json](session-evidence.json) ties the
checks, source follow-up, screenshots and verified executable archives together.
All twenty recorded capture workers are terminal. The entire phase grows the
filesystem by about 0.35 GB, including incremental build changes and compressed
evidence; about 15.6 GB remains free at finalization. No additional Niri copy is
made per case.

The last original-graph workspace gate remains the phase-17 result: ten Files
integration failures, with their cause unestablished. This Portal-only phase
changes no shared crate/vendor/cross-application production code, so complete
Portal checks and the affected native consumers are the validation scope.
These checks do not clear the workspace gate or the remaining full-goal work.

## Active-only Clipboard (phase 20)

[ADR0062](../../../docs/adr/0062-input-capture-clipboard-activation.md) extends the
same InputCapture session with separately unchecked Clipboard consent. A grant
does not open a connection while inactive. Actual compositor activation opens a
fresh transport; an independent gate revokes offers and pipes when it ends.
Queued commands and events retain their activation epoch. Known retired
completions do no I/O against a replacement transport; backend-wide nonwrapping
serials prevent numeric reuse across sessions and workers.

Portal checks pass 176 library tests (7 ignored), 53 integration tests (2 ignored),
formatting, all-target Clippy and the main build in
target/portal-input-capture-20/final-validation-01.json. Guarded inputs remain
unchanged during that complete check. A later native-script-only extension adds
explicit Disable and topology cases without changing Rust or manifests.

Fifteen public groups pass in each appearance in native-dark-02/summary.json and
native-light-02/summary.json. These include the prior pointer session cases plus
unchecked Clipboard denial during actual capture; no inactive connection, byte
access or owner metadata; readable/writable pipe revocation on Release; fresh
activation and late-completion isolation; explicit Disable; live topology
invalidation; shared Iced Stop; and original caller/backend death with held
Clipboard pipes. Already admitted bytes may drain from retained readable pipes;
closing backend endpoints cannot recall application descriptors. The Iced consent
and control screenshots have been inspected in both appearances.

The same candidate also passes sixteen pointer-only baseline groups, nine private
receiver groups, 54 existing Clipboard/RemoteDesktop pointer/keyboard groups and
34 standalone ScreenCast groups. Combined with the thirty public Clipboard/capture
groups, 143 scoped native groups pass. Mode and appearance repetitions are counted
explicitly. All thirty capture-worker handles in this final suite are terminal;
binary pins remain unchanged. The driver is native-regressions-01.json, with
individual receipts in its named directories. tooling-followup-01.json verifies
the native-only script extension after the complete Rust check.

[clipboard-evidence.json](clipboard-evidence.json) ties the results and remaining
work together. The main executable and harness are archived once per checksum
and verified losslessly, about 78 MB compressed. The whole phase grows the
filesystem by about 0.36 GB, leaving about 15.2 GB free. Native runs reference the
existing checksum-addressed Niri executable. No workspace gate is rerun for these
Portal-only production changes; the recorded ten Files failures remain unresolved.
At that phase-20 checkpoint, frontend stale-Release/repeated-Enable compatibility
remained open. The later correction follows below; keyboard/touch, broader
phase/physical/failure acceptance and distribution remain open.

## Confirmed frontend phase (phase 21)

[ADR0063](../../../docs/adr/0063-input-capture-frontend-phase.md) adds the canonical
one-file frontend correction. Four unchanged-frontend runs reproduce Clipboard
denial despite continued EI motion, a live Clipboard worker and no backend read.
Repeated Enable now preserves Active; Release waits for actual backend phase
signals. Both regressions and the original release/Disable/topology/Stop/loss
cases pass seventeen groups per appearance on the final application executable.

Existing Clipboard/RemoteDesktop passes 54 groups, standalone ScreenCast 34, and
FileChooser 24, all in both appearances. Ten owner-lifetime scenarios pass 45
private-bus checks across five session families. The final 146 native and 45 bus
checks share exact binary pins. All ten final capture-worker handles are terminal.
The full workspace's configuration continuation rebuilt the normal application;
native-dark-02 and native-light-02 reconcile public capture acceptance to that
same artifact. Earlier native-dark/light-01 use the pre-workspace executable and
remain separate evidence. No additional backend Rust edit was needed.

The exact 499-file frontend tree matches the official archive and canonical
recipe before/after the one-file transition. Its original stable source and
one-job debug Meson/Ninja cache are reused. final-source-proof-01.json verifies
all guarded workspace inputs and prepared frontend sources still match at
finalization. The workspace red is the same sole Files integration target:
596 passed, ten failed, 41 ignored. Portal 176 library/53 integration tests,
formatting, Clippy, other Rust targets and Settings/CLI/configuration/vendor
continuations pass. workspace-analysis-01.json extracts the final main failure
set separately from nested helper output. Failure causes remain unestablished.

[frontend-phase-evidence.json](frontend-phase-evidence.json) links all receipts.
The final portal/harness archives total about 82 MB compressed, with the existing
identical harness archive reused. Observed filesystem growth since the preceding
checkpoint is about 0.37 GB, leaving about 14.9 GB free. The frontend candidate
and Niri are stored once per checksum. Normal descriptor/routing remain unchanged;
keyboard/touch, broader physical/failure acceptance, distribution and remaining
interfaces are still open.
