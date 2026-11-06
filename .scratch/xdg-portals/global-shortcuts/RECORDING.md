# Source-owned shortcut recording

Current acceptance, 2026-11-01: phase28 is sealed in
`target/portal-global-shortcuts-28/acceptance-01.json`. The canonical v2 fixture
passes87 groups (US dark21, US light21, French22, Caps-as-Control23), with71 Iced
helpers and28 owned processes terminal, unchanged source/binary guards, and
representative light/French captures visually inspected. Native retained-handle,
window and event-loop lifetime and all six shared desktop suites pass. The
original workspace gate completes: Portal215 library/55 integration pass
(8/2 ignored); the same ten Files integration failures remain; every other Rust
target and all four post stages pass. The exact negative control uses preserved
canonical v1, separately from expanded v2. No interface promotion or installed
routing change occurred. Broader geometry, physical/sandbox/policy and distribution
acceptance remain open. The earlier pending/pilot statements below are historical
and superseded by this acceptance.

## Historical implementation checkpoints

Implementation checkpoint, 2026-11-01. Recording is implemented in source across
Winit native dispatch, Iced runtime/adapter and the Portal editor. Ten policy and
three standalone runtime tests pass; application tests and end-to-end recording
acceptance remain pending beyond the scoped US results below. Strict whole-workspace
Clippy passes; the required shared gate is running. Consult CONTINUE.md for its
live receipt and source lock. Phase27 inhibition acceptance
at `target/portal-global-shortcuts-27/acceptance-01.json` remains separate.
No new dependency graph or compositor API is used.

The actual new normal Portal executable9617… passes18 public native groups per
appearance in dark/100% and light/125%, including seven recording groups per run.
All28 Iced helpers and14 owned processes are terminal and guards/binary unchanged.
Recorded Ctrl+A and bare Control activate through the compositor; held keys,
modifier-first release, disabled Apply, Escape, unchecked consent, focus loss and
conflict/retry pass. Both held-recording captures were visually inspected. See
recording-evidence.json and phase28's recording-native-{dark-02,light-01}.json.
These results do not clear additional-layout, native retained-handle, shared
workspace/desktop, physical/sandbox or distribution acceptance. The archived previous app fails the SAME final fixture in its first recording
case; the replay verifies terminal processes and removes its temporary raw
restore. This completes the exact fixture negative/positive comparison.

The actual Wayland probe at
`target/portal-global-shortcuts-27/recording-dispatch-01/summary.json` passes three
private cases on the revised canonical Niri: US, FR/AltGr and remapped Caps Lock.
All owned processes are terminal and guarded inputs are unchanged. It observes
Control_L with pre-mask0 followed by effective Control4; caps:ctrl_modifier keeps
the Caps_Lock base name but produces Control4 too. Shift+a renders A with base a.
AltGr+e renders EuroSign with base e and an unrecognized Mod5 bit. This is source
dispatch evidence; it does not establish recorder or final SDK acceptance.

Phase28's serial-logging probe at
`target/portal-global-shortcuts-28/recording-dispatch-serial-02/summary.json`
passes the same three cases and twelve actual key/modifier pairs. Each observed
modifier transition has the preceding key's serial, including modifier release,
Num Lock and Caps-as-Control. All twelve owned processes are terminal and source
guards are unchanged. The first attempt's denied sandbox socket bind is retained;
the native rerun reuses its single compiled receiver. This confirms the current
companion's serial-pairing premise, not arbitrary-compositor or Portal acceptance.

A separate temporary fixture draft now passes US21, FR22 and Caps-as-Control23
pilot groups on the same SDK3 binary. All54 helpers/21 owned processes terminate
and source/binary guards match. This adds actual Shift/base-symbol and Num Lock
activation, Num+Escape, unsupported AltGr preservation, and remapped chord/bare-key
activation. Its first US attempt missed an async-enabled approval button; the
fix waits for actual accepted approval without weakening held-key assertions.
The first FR attempt times out in pointer setup before keyboard tests; the
unchanged retry after compilation passes. These pilots remain distinct from
canonical acceptance until the repository source lock ends, the draft is
published and canonical cases rerun. See recording-evidence.json.

## Verified constraints

The XDG language uses XKB names and CTRL/ALT/SHIFT/NUM/LOGO modifiers, with base
layer symbols. The canonical Niri resolver reads exactly one symbol at level0
for the current per-key layout; unknown effective modifier bits prevent matching.
See [the specification](https://specifications.freedesktop.org/shortcuts/latest/)
and `target/niri-pointer-stage-01/niri-b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/dbus/global_shortcuts/resolver.rs`.

Iced's ordinary keyboard event retains the unmodified logical key, but its
modifier bitset drops native bits and Num Lock. It cannot alone prove that the
recorded trigger matches the source. Winit already has the borrowed client XKB
state/keymap in `platform_impl/linux/common/xkb/` and actual Wayland dispatch in
`platform_impl/linux/wayland/seat/keyboard/mod.rs`. Reuse those objects; do not
reconstruct a default US map or infer a physical key from rendered Unicode text.

The local `/usr/share/wayland/wayland.xml` requires a modifier change caused by
a key to be sent after that key event. Enter supplies the already-held key set;
clients must not turn those into presses. A recorder must therefore distinguish
the base symbol observed at a press from its resulting modifier state. Merely
reading the pre-press mask mishandles modifier triggers and remapped modifiers.

xkbcommon separates client state updated from serialized masks from server state
updated from key edges. Its documentation warns that serialization is lossy and
the two update modes should not be mixed. Do not create an alleged server clone
from the client mask and call update_key to predict the result. Read actual
subsequent client state instead. See
[the primary state documentation](https://xkbcommon.org/doc/current/group__state.html).

## Ownership and input boundary

Extend the optional ordinary-window inhibition handle with an owned, one-shot
native recording attempt. The request is usable only with actual Active
permission. The native owner decides capture, consumes its press/release cycles,
and produces one terminal result; the Portal does not implement capture from
application events after they have already reached text fields or Escape logic.

The inhibition hub must preserve a protection revision. Any relevant focus,
permission, keyboard-seat, keymap or layout change cancels an in-flight attempt
at source, even when a coalesced notification later observes Active again. Store
status and revision coherently; two independent reads do not prove continuity.
Guard/window/event-loop retirement also cancels. A stale native callback or UI
result must match both inhibitor and recording generations before it can act.

Use a Wayland sync fence when arming. Input queued before that fence is not a
candidate. Import already-held keys from Enter, and wait for them to be released
before displaying Ready. A held keyboard activation of the Record button cannot
become the newly recorded trigger; repeats cannot become candidates either.

Snapshot the candidate's one base XKB symbol and layout at a fresh press. Resolve
its post-press effective modifiers from subsequent native modifier dispatch,
before processing a later key, or at a following ordered sync fence. Preserve
this press-associated snapshot across a later release; modifiers-first release
must not erase CTRL from a captured CTRL+a. Reject unresolved or unrepresentable
state, including effective bits outside the resolver's recognized names. Custom
modifier mappings and lock-key behavior require actual-source tests before this
association is accepted; this is an implementation constraint, not yet a proven
algorithm for arbitrary compositors.

Capture a normal key with the current chord. Modifier-only choices wait for the
chord to finish and retain the intended trigger's press snapshot. Plain Escape
cancels the recording attempt; it must not cancel the whole consent dialog.
Manual trigger entry continues to support Escape itself. Final acceptance of a
captured trigger remains an explicit Apply action subject to conflict checking.

Consumed cycles outlive a completed or dropped attempt until their native key-up
or source retirement. Neither the text field nor the dialog-level subscription
may see their orphan release, repeated key or Escape. Native focus loss cancels
the attempt without releasing existing compositor shortcut reservations. Bound
held-key/candidate storage and reject malformed native state without affecting
ordinary windows that never request recording.

The Iced adapter maps the owned native result to a bounded coalesced stream;
terminal results remain readable even if a wake is merged. No unbounded key-event
queue or second Wayland connection is necessary. The UI associates results with
the selected row and attempt generation, leaves the previous trigger intact on
cancellation/failure, and gives a separate retry action when protection is lost.

## Required evidence

- Pure lifetime tests: queued permission loss/regain, seat/focus replacement,
  stale callbacks, attempt replacement, final guard/window/event-loop closure,
  bounded storage, terminal stream behavior and consumed-cycle retirement.
- Actual XKB tests: CTRL+a with both release orders, SHIFT plus a base symbol,
  per-key layouts, Caps/Num Lock, bare modifier chords, remapped modifiers,
  unsupported AltGr/unknown bits, multiple/no base symbols and non-ASCII names.
- Real Iced tests on the final application: mouse/keyboard arming, held key and
  repeats, Escape, text-field focus, focus loss and regain, conflict retry,
  simultaneous editors, cancellation preserving reservations, dark/100% and
  light/125%. Verify the resulting trigger through actual compositor activation.
- Shared SDK changes require the original workspace graph/cache and relevant
  existing ordinary-window/layer-surface regressions. Retain current inhibition
  controls and exact binary/source identities; do not label pilot SDK1 results as
  final shared-build or recording acceptance.

## Next implementation seam

Extend the native inhibitor with one owned recording attempt and a separate
recording state handle. Keep actual capture inside Winit's existing native
keyboard dispatch, before ordinary key input, repeat and drag-Escape delivery.
Expose that attempt through an optional runtime Driver operation and the ordinary
Iced adapter; an unsupported adapter continues to return None. No new compositor
API, dependency or Cargo feature graph is necessary.

Give every attempt a generation and revoke it immediately at source when focus,
permission, seat topology, keymap or layout changes. A terminal attempt cannot
revive after a coalesced loss/regain. Generation-check every arm/candidate sync
callback. Keep bounded per-seat logical down keys and consumed-cycle tombstones.
Enter imports already-held keys without synthesizing presses and reconciles old
tombstones; focus loss cancels capture but cannot expose a consumed release.
Record-button keyboard activation and older queued input wait behind an arm
fence and held-key release. Ignore repeats as candidates.

Freeze each candidate's base symbol at press. Associate its actual resulting
modifier mask before a subsequent key edge, or at its ordered sync callback.
Never overwrite that snapshot when modifiers are released first. Use the first
non-modifier key; support a modifier-only chord after release, including Caps Lock
remapped as Control. Detect modifier behavior from actual mask changes as well as
the symbol. Verify the resolver's implied Control/Shift/Alt/Super bit exists in
the observed post-press state; a remapped modifier name must not yield an
unmatchable success. Preserve Num Lock behavior from the actual press snapshot.
Reject unknown modifier bits and unresolved/multiple base symbols.

Complete a chord after its keys are released. Cancellation and focus loss retain
consumed-cycle cleanup until actual release or authoritative source retirement.
Plain Escape cancels the recording attempt and stays inside the consent editor.
Keep the old row trigger on cancellation/error, preserve its checkbox state, and
require the existing separate Apply action for any application authority.

The bounded change stream owns a clone of its native attempt. Add an explicit
cancel operation which revokes authority even while other clones or the stream
remain; merely dropping the UI field is insufficient. Cancel on dialog closure or
feedback/generation changes and tag UI results by row plus attempt generation.
Disable Apply and trigger editing while recording. Older UI Escape events must
cancel recording, rather than bypass it and close the consent request.

Develop pure policy tests against the actual new source first, using observed
Ctrl+a, Shift/base-a, Caps-as-Control, AltGr and Num Lock traces. Keep root
workspace feature selection stable for SDK compilation; excluding applications
would build another graph. Run the required workspace gate once after the SDK/UI
implementation is ready, while retaining the existing caches and all prior controls.
