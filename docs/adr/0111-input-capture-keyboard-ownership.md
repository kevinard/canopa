# InputCapture keyboard presses keep their original recipient

Status: compositor and public Iced/EI keyboard path scoped native verified; experimental opt-in packaging; full workspace gate passed.

InputCapture negotiates keyboard/pointer capabilities from its selected companion.
Keyboard support uses compositor routing, receiver-role EI keymaps/modifiers and
separately unchecked Iced consent; older pointer-only providers remain supported.
The new `niri-input-capture-keyboard` source recipe extends the complete current
GlobalShortcuts/InputCapture companion. Eight Niri paths now wire ownership,
independent XKB, source cleanup, virtual modifier routing and private v2 keyboard
leases into the compositor. Builders and explicit opt-in packaging accept the keyboard recipe. The existing recipe and
delivered companion remain unchanged; stable prepared source/cache paths are reused.

Captured keyboard input must be routed before ordinary seat XKB processing.
Intercepting a key after that state changes can publish its modifiers to local
clients. Keep a separate capture XKB state, using the compositor's current keymap.
Track eligible raw sources before activation so that already-local held keys keep
their local release. Convert Smithay's XKB codes to Linux codes at the adapter;
the pure module accepts1..0x2ff. Exclude authorized RemoteDesktop injection sources
to prevent feedback. Per-press exclusion cannot be promoted while that press is
still held.

The module pairs first-down/final-up across overlapping eligible sources. Captured
presses belong to a specific lease/activation. Deactivation returns explicit EI
key-ups before emulation stops and retains physical tails as suppressed. New
activations cannot adopt those held keys or seed their modifier state. Source
withdrawal pauses an affected keyboard lease and lifts its held keys; ordinary
local teardown continues through the seat's existing source-release path. The emergency pause
key is reserved after deactivation, without an EI press or local key-up.

Storage is bounded to256held sources and4096source/key pairs, plus one fixed
emergency Escape tombstone and256virtual modifier sources. Exhaustion rejects
further capture admission until restart, preserving already-owned release tails.
The caller must deactivate and process the new untracked edge locally. Fresh
capture admission cannot reinterpret an untracked held input after exhaustion.

The EI adapter must publish its keymap before device completion, retain it for
that device's lifetime, and publish modifier changes immediately after the key's
frame. A keymap revision therefore requires a replacement device after lifting
held keys and suspending capture. These constraints follow the
[EI keyboard protocol](https://libinput.pages.freedesktop.org/libei/interfaces/ei_keyboard/index.html).
The compositor adapter and independent modifier routing now have actual local
Wayland/capture-stream proof. The normal Portal adapter negotiates private v1/v2 and selected grants, and
receiver device replacement and EI delivery have actual public-path proof.

Twenty std-only checks pass with denied compiler warnings, including emergency
pause, resource exhaustion, source removal, generation replacement and20,000
interleaved ownership transitions. The maintained runner tests the module
extracted from its canonical patch, with a stable source/harness path under root
target and no additional Cargo graph. Cached official archives reproduce both
complete source trees and prove the exact eight-file Niri delta, no Smithay change,
and preserved original notices. Temporary trees retire after verification.

Earlier private-integration evidence: `target/portal-input-capture-keyboard-85/`. The first native run
passes two groups and then reproduces explicit virtual-modifier leakage to the
ordinary client. Its failed receipt and lossless baseline remain. The corrected phase85
candidate passed seven groups: preheld raw/local masks, captured modifiers and
release tails, explicit modifier messages, keymap pause/current revision recovery,
combined pointer/keyboard and emergency pause before source destruction.

Private v2 retains OpenPointer and adds capability-selected OpenCapture plus a
bounded KeyboardSnapshot. Keymap changes disable before revision notification;
keyboard Enable requires the current revision. Independent capture XKB prevents
ordinary modifier leakage, including new captured bits while old local bits clear.
The candidate compiler uses the same nightly/dev/incremental cache and remains
above the3GiB free-space floor. Obsolete failed-baseline evidence is compressed
losslessly after checksum and process-lifetime checks; no debug cache is cleared.

Those earlier private checks do not establish public or whole-workspace
acceptance. Phase86 public acceptance is recorded below; physical-device and
whole-workspace limits remain explicit. Print remains deferred.

See [pointer capture](0059-compositor-owned-input-capture.md),
[EI receiver](0060-input-capture-receiver-worker.md) and
[session consent](0061-input-capture-sessions-and-consent.md).

Phase86 adds modifier ownership after capture stops. Explicit late modifier
packets retain the retired press's recipient, including across a new activation;
a completed press does not suppress a fresh local modifier-only interaction.
The capture's initial depressed state also excludes held RemoteDesktop injection.
The previously accepted phase85 executable reproduces the late-packet leak; its
failed receipt remains. Candidate6e3d passes all nine private native groups and
all twenty pure checks, with exact eight-file source materialization. Every owned
native process is reaped. See `target/portal-input-capture-keyboard-86/`.

Public v1/v2 negotiation, selected grants, immutable EI keymap publication,
keyboard/modifier events and separate Iced choices pass six native groups in each
appearance on backenda30ae94e/Niri6e3d/frontend277568. The ongoing indicator's old
pointer-only validation first reproduced Start response2; a capture-specific
constructor now validates and names exactly the selected1/2/3 classes. Its real
module regression passes, and strict whole-workspace Clippy passes.

The first combined fixture also correctly paused when its old French source
changed a fresh US consent keymap; independent cases now reset their layout.
A source-withdrawal marker matches its F24 keysym because wtype chooses code1,
while the raw source uses194. Failure receipts remain; no delivery assertion was
weakened. Every recorded worker/direct process retires and fixture inputs remain
unchanged. Evidence: public-keyboard-dark-03 and public-keyboard-light-02 under
the phase86 directory. Touch remains declined. The existing development archive
and live desktop selection are unchanged; full shared workspace acceptance remains
separate from these scoped native results.
