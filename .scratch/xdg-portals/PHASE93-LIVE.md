# Phase93: mapped RemoteDesktop touch compositor implementation

Goal ACTIVE; Print deferred. The preceding user-facing status reply was no
implementation progress; this turn changes canonical source and verifies actual
touch delivery. No missing user answer or external blocker. Do not restart Print
or completed matrices. Public touch is still unfinished and unadvertised.

## Current acceptance

The niri-remote-touch recipe extends the complete accepted keyboard/input/capture
companion and identical Smithay. Six Niri files add/modify private touch ownership,
transport and common touch handling. Private RemoteInput1.TouchVersion1 and
OpenMappedTouch are implemented; public Portal device bit4/Notify/EI/Iced approval
are NOT implemented. Existing application sources, Cargo files and prior vendor
recipes remain unchanged (1398 non-Markdown input guard). The package still ships
the accepted phase91 companion, not this experimental private candidate.

Final canonical recipe SHA:
7bfc711fa6eb64f9653f32c22c7ac38c5cca77584a808dcbaa121d85d4d21e8b.
Final Niri SHA:
d49fd215bc18b78a1c0deadb10f9fd42e06151cf237d537ecf41da56e3234176.
One immutable executable is at the checksum-addressed compositor store; native
artifact directories contain symlinks. Stable source paths remain beneath
target/niri-pointer-stage-01, with its canopa-build.json now naming the touch
recipe. Do not try --reuse with the old keyboard recipe at that source path.
The previous accepted keyboard executable remains available and unchanged.

Evidence in target/portal-remote-touch-93/:
- core-final-02.json:16 ownership/wire tests pass on canonical edition2021 source.
  Full cached official archive materialization proves the exact six-file Niri
  delta, unchanged Smithay and original notices. Temp comparison sources removed.
- compositor-build-03.json/log:terminal0, final candidate above, original nightly
  compiler, dependency graph, features, source paths and incremental dev cache.
- native-final-04/result.json:terminal0,6actual Wayland groups. Unstarted capture
  rejection; independent client contact IDs and existing touch capability;
  owner-only socket cleanup/frame; malformed-source isolation; final-Up without
  Frame cleanup; capture-stop release/EOF while preserving physical capability.
  This is private compositor acceptance, not public/EI/Iced/physical-device proof.
- extraction-green-01.log:8pass; keyboard-parser-regression-01.json:20pass;
  check-scripts-01.log:15pass; Python syntax pass. Core check joins workspace gate.
- source-final-01.json: compiled source inputs match the canonical candidate;
  staging /tmp/canopa-remote-touch-adapter and provisional core source removed.
- native-host-audit-01.json:all24recorded native PIDs absent in actual host
  namespace. User Niri PID1264 is outside this work and untouched.

## Retained failures, fixed without expanding the cohort

Initial build01 ed4bb00b9 delivered touch, but native-lease-01's initial-capability
assertion accepted loss of Winit's virtual touchscreen. Its5pass result is NOT
accepted; native-01-scope-correction.json records the issue. Corrected assertion
native-capability-red-02 fails before any remote lease. Generic touch-device
addition/removal tracking fixes it; the libinput-only Niri.touch set is not the
complete seat inventory.
Build02 c225ed568 preserves that device. native-frame-03 then fails the exact
unframed final-Up wait; its receiver has Up without Frame until another source
closes during cleanup. Retirement now flushes even with no contacts left. Final
build03/native04 pass both boundaries. Both obsolete unique native executables
are gzip archived losslessly with verified checksums and no raw test copy left.
Current candidate and accepted baseline stay raw only while needed. No caches
were deleted and no fresh graph/SDK/runtime was downloaded.

## Live required gate and next implementation

The original full workspace gate is TERMINAL101, session73127 collected. All
Rust targets ran under --no-fail-fast. Only Files integration failed:605pass/1fail/
41ignored. The existing500-item Keep Both Undo case took33.715085323s against20s.
Files375/34 library, Portal457/20 library and56/2integration pass; every other
Rust target passes. No clean full-workspace acceptance is claimed.
files-isolated-01.log:the exact same compiled Files case passes once in isolation
from crates/canopa-files, no build/new graph. The original failure stays failed.
Its11MiB expanded fixture remains in /tmp for the unresolved timing evidence.
Source guard confirms Files/all applications/previous vendor runtime unchanged.
Do not fix unrelated Files performance as a portal continuation detour.

The independent post-test stages skipped by set-e after Cargo's failure all PASS,
session27316 terminal0 and collected: settings, CLI, configuration and vendor.
This does not turn the original failed full-workspace run into a pass. No current
validation/build/native handle remains live. Source cleanup20024 and final source
identity check27082 are terminal. Combined receipt: result-01.json.
Final compiled/canonical equality and1398previous non-Markdown inputs are verified;
all four native runtime directories and24recorded PIDs are gone. Root available
3208867840bytes. The two obsolete compositor candidates are
losslessly archived; current candidate and accepted delivery baseline are retained
once in the checksum store. Existing incremental caches were reused; no SDK download or live service change.
No bundle refresh occurred; exact phase91 archive identity remains unchanged.

After collecting the remaining post stages, implement the Portal touch worker/transport, public
NotifyTouch methods, EI touchscreen device and independent unchecked Iced touch
approval before advertising device4. Reuse final frozen compositor above. This
component is concrete progress toward the original eighteen-interface goal,
which remains ACTIVE; it is not completion of RemoteDesktop touch or the goal.
ADR0117 records the ownership/protocol contract. Live routing remains unchanged.

## Public-layer seams inspected during the gate

Official RemoteDesktop API/backend v2 docs were checked on2026-11-05 (ADR0117
links both). NotifyTouchDown/Motion take session/options/stream-u32/slot-u32/x/y;
Up has no stream/position. Coordinates use stream logical space. There is no
public NotifyTouchFrame: the legacy adapter must frame each ordered Notify call.
EIS must replace all Notify input after successful handoff.

Next source seams: screencast/remote/input.rs currently has only Keyboard/Pointer;
screencast/remote.rs lacks three NotifyTouch methods; session.rs uses mask3 for
input admission; remote/production.rs resolves only requested bits1/2; its pointer
module is the ordered-worker ownership pattern. remote_input/parent.rs currently
negotiates PointerVersion/MappedPointerVersion only. pending.rs permits two EI
devices and explicitly rejects Touchscreen. screencast_ui.rs has independently
unchecked keyboard/pointer fields; add a separately unchecked touch choice.
Reis already translates touch requests; inspect its interface version negotiation
and TouchCancel v2 before claiming support for that extension.

Compositor follow-up before public advertisement: touch dispatch currently calls
logical helpers directly, bypassing process_input_event's animation/activity/
monitor-wake/overlay prelude and tail. Route synthetic touch through the shared
input pipeline (with physical guards distinguishing owned remote events) as part
of public integration. This is a known unfinished behavior, not an external
blocker. Reuse the current dependency graph; do not replay completed keyboard,
Print or toolkit matrices. A focused full-path touch/native cohort should prove
this next layer, followed by the required gate after its final vendor edits.

The existing frontend companion already queues ALL Notify methods in receive
order; its frontend.patch maps non-keyboard/non-pointer Notify to TOUCHSCREEN
and dispatches the original generated handlers. The prepared frontend already
has NotifyTouchDown/Motion/Up. Reuse accepted frontend277568bfecc9e75c1a155a370a69f7a992c335bf6745344601bac54cea132233;
no new frontend fork/build is indicated for touch forwarding.
