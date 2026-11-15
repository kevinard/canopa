# Niri scrolling-layout integration

Date: 2026-11-14
Status: accepted

The Shell exposes Niri's scrolling layout through the minimap, taskbar layout
menu, window previews, session restore, workspace templates, overview typing and
touchscreen gestures. Several of these need compositor support that upstream Niri
does not offer, which the pinned fork provides (see `docs/vendor-maintenance.md`,
topics `viewport-ipc`, `window-parent-ipc`, `spawn-placement`, `overview-typing`,
`touch-gestures`, `window-preview`, `pinned-columns`).

## Decisions

- **No "scroll without focus" action.** Niri always keeps the focused column in
  view; scrolling elsewhere would need a detached-view state in the compositor.
  The minimap and keys move focus instead.
- **Placement is bound to the activation token, not to a PID.** `SpawnPlaced`
  (used by session restore and workspace templates) attaches the target
  workspace, column and width to the activation token of the launch. PIDs are
  unreliable for double-forking, Flatpak and D-Bus-activated applications. A
  placement is single use, expires after 60 seconds, and at most 32 are pending.
- **Restore uses fixed widths.** Niri applies proportional widths to the working
  area minus gaps, so restoring proportions shrinks columns a little every
  session. The Shell saves and restores fixed pixel widths.
- **Overview typing is opt-in in the compositor and on in Canopa's defaults.**
  The fork's `overview { emit-typed-text; }` sends typed text to every
  event-stream client, so it is off by default in the fork. Canopa's shipped
  `packaging/niri/defaults.kdl` turns it on to open the launcher from the
  overview, which ties those defaults to the pinned compositor.
- **Window previews trust same-user session-bus clients.**
  `org.canopa.Niri.WindowPreview1` follows the same trust model as Mutter's
  ScreenCast interface: any client of the user's session bus may request a
  thumbnail.
- **Touchscreen gestures yield to stronger owners.** Strip and overview gestures
  do not start while a remote-touch lease, the lock screen or another grab owns
  touch input.
- **"Window layout" is a Personalization section in Settings, not a page.** A
  new page would renumber the Ctrl+N shortcuts and the public page ids.
- **Wheel bindings follow Niri's own defaults**, and blur is not offered because
  it needs a translucent surface theme.

## Consequences

The Shell and Settings speak the fork's IPC (`niri-ipc` is pinned to the
compositor commit). Running Canopa on stock Niri loses these features; the
packaged session always uses the pinned compositor at `/usr/lib/canopa/niri`.
