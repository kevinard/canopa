# Phase85: private compositor keyboard integration

Goal active; Print deferred. Phase84 was progress (canonical pure ownership core).
This turn wires that core into Niri before ordinary seat XKB, adds independent
capture XKB, source withdrawal, emergency pause, private v2 OpenCapture/KeyboardSnapshot,
keymap revisions and separate pointer/keyboard grants. Public EI/consent remains
unfinished; normal Portal application code/Cargo graph and live routing are unchanged.

Current source recipe: niri-input-capture-keyboard. Prepared source paths stay
under target/niri-pointer-stage-01/, existing native Cargo target is reused:
target/niri-screencast-native, one incremental dev job, recorded nightly1.97.
Builder now accepts this experimental recipe; packaging still rejects it until
public integration/acceptance. Old niri-global-shortcuts recipe/patch is intact.
Complete archive materialization equals the stable candidate with only the two
original Smithay bindings. Delta is7Niri paths, Smithay unchanged:
input_capture.rs, input_capture/{keyboard,keys}.rs, dbus/mod.rs,
dbus/global_shortcuts.rs, handlers/virtual_keyboard.rs, input/mod.rs.
Current recipe2b1dbc8205711afcd4edeaf41b1ca5f3b471dab840d8521fbcf1f49cbbf6054d.
Canonical/source proof: target/portal-input-capture-keyboard-85/canonical-01.json
(latest proof); publish-01/02/03.log retain prior exact recipes and deltas.

Current checks/evidence under target/portal-input-capture-keyboard-85/:
- Build01 terminal0,3m10s, normal same-cache compiler. Binary94931c42961d11c29b0e1df84c6cdde1b0e9bf40beca1973f5c7940b4d026b93.
  Actual private native-01 passes preheld-local Control/new-key routing and
  captured Shift/retired-tail non-delivery. It FAILS explicit virtual modifier
  routing: local client receives mods4 then0. Receipt/log preserve failure;
  all8 directly owned processes terminate, changed_inputs empty.
- Baseline949 snapshot compressed losslessly to34,366,679bytes AFTER exact
  checksum and no executing inode checks, then only that snapshot raw removed.
  Canonical store archive.json + archive-baseline-01.log preserve restoration.
  Root Cargo outputs/debug cache and current92294 compositor are untouched.
- Added explicit virtual modifier interception. Per-bit local ownership retains
  only the original local mask bits and their clears; new bits/group changes
  follow capture. Filtered ordinary mask updates are tracked as expected changes
  so external synchronization cannot overwrite capture-owned modifier bits.
- Build02 terminal0 (binary715a6fee...), then refined per-bit ownership BEFORE
  native acceptance. No native claim for02. candidate-01/02 build receipts and
  canonical source patch preimages are compressed in this directory.
- Core03 terminal0,13PASS with denied warnings. Full old/new source check passes
  7Niri delta,5628Niri/345Smithay files. Build-reuse3PASS, vendor checkerPASS,
  Python syntaxPASS. Early core01 extractor failed because it dropped the final
  newline when the canonical added file was no longer last; regex boundary now
  preserves it and02/03 actual-source checks pass. C probe strict compile first
  caught misleading indentation; corrected strict compile passes.

All handles are now TERMINAL. Build03 exec88660 terminal0 in2m18s, with
binary2839fe8b0ebd503490a3525be824867e45c4e55c1b5fcfabe5fbad4a1e3a3dab.
Resource guard minimum3,462,340,608bytes remained above3GiB. No compiler warnings.
The exact5973prepared inputs match its build receipt after native verification.
Do not restart any of the completed builds or private native runs.

Actual current native-02 (exec17819 terminal0) passes7groups in11.22s, with all8
owned processes reaped and no changed fixture inputs. It includes new captured
Shift bits while preheld local Control clears; no new bit reaches the ordinary
client. The failed native01 remains failed and its exact binary is archived.

Targeted regressions exec43775 terminal0, same2839binary, run sequentially:
- pointer-legacy-01:8PASS,12.91s, including original OpenPointer ABI, held buttons,
  stale release, topology, backpressure, admission, escape and source loss.
  Its verifier now accepts exact (Version,Capabilities) pairs(1,2) or(2,3).
- keyboard-source-close-01:10PASS,9.09s, actual ordinary/consumed/a11y key delivery,
  overlapping sources and explicit versus teardown release actions.
All owned processes reaped; no changed fixture inputs. No65-group replay.
Core04 exec87425 terminal0:13PASS and complete7-file source materialization proof.
Build reuse3PASS, vendor finalPASS, Python/strict C fixture buildsPASS.
Combined receipt: target/portal-input-capture-keyboard-85/result-01.json.
Root available3,459,530,752bytes (~3.22GiB). Only current candidate2839 and the
unchanged delivered92294compositor remain raw in their checksum store; baseline
949 is lossless gzip. No cache cleanup/new Cargo graph/SDK/runtime downloads.

Current private API is v2/capability3. Existing OpenPointer ABI still works; new
OpenCapture selects1/2/3 and returns zones plus bounded keymap/revision.
KeyboardSnapshot returns current revision/text; keyboard Enable requires that
revision. Keymap changes deactivate/disable before KeymapChanged. Keyboard-only
activation does not divert pointer events. The normal Portal adapter still knows
private v1 only, so do NOT package/select this candidate as a public replacement.

NEXT implement the actual public path as one source cohort before its Cargo
build: input_capture_transport endpoint v1/v2 negotiation and selected grants,
private OpenCapture/current keymap revision bootstrap, receiver-role EI keyboard
with immutable keymap FD, ordered modifiers, paired held-key release before stop,
keymap-revision device replacement, then separate initially unchecked Iced keyboard
consent. Existing pointer-only providers remain supported, touch is declined.
Production Control and ongoing indicator must report only selected capabilities.
Public capability1 stays unadvertised until the actual public/Iced/EI case passes.

One specific coexistence case remains for that EI acceptance: a RemoteInput
modifier already held before activation and released during capture. Current
keys::State::new copies ordinary depressed state, while raw injected key events
are excluded; verify initial/changed EI modifier semantics and absence of stuck
modifiers rather than assuming raw-key exclusion proves that boundary. Also
exercise source withdrawal/revocation through actual EI, not just this private
JSON stream. These are part of the unfinished keyboard integration, not a new
physical-device/matrix gate. Preserve the preceding private proof while editing.

The completed shared integration still requires its original whole-workspace
gate; phase83's known Files500 timing failure remains explicit. No Files/Print
detour, live service/routing change, goal completion/pause/block or new user input.
