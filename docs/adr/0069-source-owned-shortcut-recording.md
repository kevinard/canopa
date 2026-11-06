# Source-owned shortcut recording

Status: scoped recording, lifetime and shared desktop acceptance verified;
broader acceptance pending, 2026-11-01. Strict whole-workspace Clippy, ten
actual-source policy tests and three standalone runtime tests pass. The normal
application passes87 canonical native groups across US dark/100% and light/125%,
French and Caps-as-Control, with71 helpers and28 owned processes terminal and
unchanged guards/binaries. Retained-handle/window/event-loop lifetime checks and
all six shared desktop suites pass. The completed workspace gate passes215 Portal
library and55 integration tests (8 and2 ignored); the same ten Files integration
failures remain, while every other Rust target and all four post stages pass.
Minimum-window/long-label, broader physical/sandbox/policy and distribution
acceptance remain open. See
`.scratch/xdg-portals/global-shortcuts/recording-evidence.json` and
`target/portal-global-shortcuts-28/` for the exact scope.

The GlobalShortcuts editor needs a trigger that the compositor can resolve in
the current keyboard layout. An ordinary Iced key event lacks effective native
modifier bits and Num Lock. It also arrives after native input handling. Recording
there would let text fields, repeats or dialog Escape react to captured input,
and a coalesced permission stream could hide a loss followed by renewed focus.

Extend the native inhibitor from ADR0068 with one optional owned recording
attempt. Winit captures on the existing window connection before ordinary input,
repeats and drag-Escape dispatch. A source-owned terminal state survives retained
handle clones. Explicit cancellation revokes the attempt even when its change
stream owns another clone; dropping only the editor field cannot provide that
guarantee. Unsupported adapters continue to return None. No GTK, extra Wayland
connection, dependency or Cargo feature is introduced.

Each attempt has a checked generation inside its native window generation.
Focus, permission, seat topology, keymap and layout discontinuities cancel at
source before notifications coalesce. Window and event-loop retirement close
the attempt. Every arm/candidate callback checks both generations, so a late
callback cannot revive an old attempt or affect its replacement. State changes
are stored under the hub lock and bounded wake notifications run after unlock.

An ordered sync fence excludes already queued input. Per-seat held keys delay
arming until all keys are released; Enter imports held keys without synthesizing
presses. Down-key and consumed-cycle storage are bounded. Malformed Enter arrays
or excess key growth invalidate that seat until a fresh authoritative Enter.
Consumed press/release/repeat cycles stay suppressed after completion or cancel
until release or authoritative source reconciliation. Arming and consumed input
reset compose state, preventing a previous dead-key sequence from surviving
capture into the restored text field.

Freeze the current per-key layout's single base symbol at press. Use the actual
post-press XKB modifier state rather than predicting it with a second synthetic
XKB state. The matching native modifier serial may update that candidate;
unrelated modifier serials seal the older snapshot before changing live state.
The next key edge or ordered sync completes the association. Three real private
Wayland cases on the current companion confirm twelve such key/modifier pairs,
including Ctrl, Shift, Num Lock, AltGr and Caps remapped to Control. This is a
verified premise for that companion, not arbitrary-compositor acceptance.

The first ordinary trigger wins. Modifier-only chords complete after release,
and actual modifier changes identify remapped modifier keys. Preserve the press
snapshot when a modifier is released first. Reject unknown effective bits,
ambiguous base symbols and an implied modifier absent from the observed mask.
Use the resolver's existing XKB-name and CTRL/ALT/SHIFT/NUM/LOGO representation.
Plain Escape cancels only recording; ambient Num Lock does not prevent cancel.

The Iced runtime maps native states through a bounded coalesced stream. The
editor tags results by row and attempt generation, replaces text-input widgets
while capturing, and disables editing, selection changes and Apply. Cancellation
or failure keeps the previous draft and checkbox. A recorded trigger changes
only that draft: the separate Apply action still performs permission and
conflict checks. Dialog closure and backend feedback changes explicitly cancel
any live recording.

Native acceptance must prove mouse arming with held/repeating keys, plain Escape,
modifier-first release, modifier-only chords, focus loss, conflict retry and
unchanged unchecked consent. Verify committed triggers through actual compositor
activation in both appearances. Keep the prior normal application as one
losslessly archived control, reuse the original incremental workspace graph,
and run the required shared gate after the recording sources are ready. These
changes do not advertise GlobalShortcuts or complete the broader portal goal.
