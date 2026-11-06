# Experimental Niri keyboard capture

This recipe extends the complete `niri-global-shortcuts` companion using the same
pinned Niri/Smithay archives, dependency bindings, patches and original licences.
Eight Niri paths differ; Smithay is unchanged. The private compositor keyboard
path is implemented and passes nine scoped native groups. Public receiver EI
translation and Iced keyboard consent pass six scoped groups in each appearance. Builders accept this
experimental recipe; explicit optional packaging accepts it. Existing combined recipe,
delivered bundle and live selection remain intact.

## Private control interface

`org.canopa.Niri.InputCapture1` now has Version2 and SupportedCapabilities3:
keyboard1 and pointer2. Touch is unavailable. This is a privileged desktop control
capability, not portal consent and not an FD to hand directly to applications.

- Existing OpenPointer() retains `(fd, zone_set, zones)` and pointer-only authority.
- OpenCapture(capabilities:u32) accepts1,2,3 and returns
  `(fd, zone_set:u32, zones:a(uuii), keymap_revision:u32, keymap_text:string)`.
  Keyboard text is bounded to256KiB; pointer-only opens return empty text.
- KeyboardSnapshot() returns `(keymap_revision:u32, keymap_text:string)` without
  opening another capture lease. Discovery alone still grants no interception.

Control frames retain the bounded16KiB length-prefixed JSON protocol. Keyboard
leases require their current keymap revision on Enable:

```json
{"sequence":2,"command":{"op":"enable","keymap":1}}
```

Pointer-only clients retain the original Enable form. A stale/missing keyboard
revision is refused. Keymap changes deactivate and disable capture before
`{"event":"keymap_changed","keymap":2}`. The trusted receiver obtains the new
snapshot and replaces its EI keyboard device before enabling with that revision;
the old device's map cannot change in place. The normal Portal adapter implements this device replacement. Zones/barrier generation remains independently validated.

Activated precedes initial Modifiers. Captured Key events carry Linux code,
pressed bool and monotonic microseconds; the producer prevents timestamps from
moving backward across its input stream. Modifiers carries depressed, latched,
locked and group fields, ordered after the key that changed them. Key-up/held-key
cleanup on Deactivated or EOF must finish before receiver stop_emulating.
Keyboard-only capture activates through the approved pointer barrier while leaving
pointer events/cursor local. A combined grant diverts both capabilities.

## Keyboard ownership

Input is routed before ordinary seat XKB, with an independent capture XKB state.
Preheld local keys retain local releases. Captured first-down/final-up is paired
across eligible sources. Retirement retains physical release tails as suppressed;
a later activation cannot adopt old held keys or their modifiers. Source loss uses
the existing ordinary release path for local keys and the capture ledger for its
own keys. Authorized RemoteDesktop injection sources are excluded from capture.

Virtual keyboard modifier messages follow the same recipient boundary. Only bits
already held locally retain their local clear; newly set bits/group changes follow
capture, including new captured Shift while a preheld local Control remains down.
Filtered ordinary mask updates cannot overwrite independent captured mask bits.
The existing absolute modifier semantics of virtual keyboards remain unchanged
outside keyboard capture. Input-method passthrough retains its existing path.

The pure core bounds256held sources and4096source/key pairs (codes1..0x2ff), plus
one fixed emergency Escape tombstone. Virtual local mask ownership is bounded to
256entries. Exhaustion closes keyboard admission while retaining release tails.
Ctrl+Alt+Escape lifts captured input, disables capture and suppresses its own
ordinary release. It works before the input source is destroyed.

## Verification and remaining integration

```sh
python3 scripts/test-input-capture-keyboard-core.py --archives CACHED_ARCHIVES
python3 scripts/test-input-capture-keyboard.py --niri BUILT_NIRI \
  --niri-sha256 BUILT_SHA256 --output target/NEW_NATIVE_EVIDENCE
```

The std-only core check uses stable root target source/harness paths and no new
Cargo graph. Cached official archives prove the exact eight-file Niri delta and
original notices; temporary materializations are removed. Twenty pure checks
pass. The first actual native run found an explicit virtual-modifier leak after
two passing groups. The phase85 candidate passed seven groups, preserving the
failure and its lossless baseline archive. Native checks use synthetic sources
and an independent ordinary Wayland receiver on a private desktop.

See [ADR0111](../../../docs/adr/0111-input-capture-keyboard-ownership.md) and
`target/portal-input-capture-keyboard-85/`. Whole-workspace and completed public
keyboard acceptance are not established by these private checks. The normal
Portal adapter supports private v1/v2, receiver EI keymaps/modifiers and separately
unchecked keyboard/pointer approval. Six actual public groups pass in each
appearance. Whole-workspace and distribution acceptance remain separate.

Phase86 additionally retains explicit modifier ownership through retirement and
suppresses late modifier packets from old presses. Fresh local interactions stay
available. Initial captured modifiers exclude already-held RemoteDesktop
injection. Twenty pure checks and nine private native groups pass on candidate
6e3d; the earlier post-stop modifier leak remains in its failed baseline receipt.
The normal Portal's public EI/Iced path is scoped native verified and the recipe
is accepted by optional packaging. See `target/portal-input-capture-keyboard-86/`;
default selection is unchanged; workspace and distribution acceptance are recorded separately.
