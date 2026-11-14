# Live window previews

Status: implemented. Fork PR kevinard/niri#19, pinned (`b4fc5ea7`); taskbar
tooltips in the Shell.

## Niri fork

Branch `canopa/window-preview` (`fbd22037` on `canopa/base`): "dbus: add a
window preview service". `org.canopa.Niri.WindowPreview1` at
`/org/canopa/Niri/WindowPreview1`: `Capture(window_id: t, max_size: u) ->
(width: u, height: u, png: ay)`, `Version: u = 1`.

- Renders the window's own surface tree (no borders, focus ring, shadows or
  popups; `block-out-from` applies) directly, so windows on hidden workspaces
  work; scaled to fit `max_size` (16..=512), never up.
- Rendering on the main loop through GLES at full size (capped at 4096 px),
  halved on the GPU, then scaled; only the thumbnail is downloaded. PNG
  encoding on another thread.
- One capture in flight per caller, four overall (`LimitsExceeded`); unknown
  window `InvalidArgs`; failures `Failed`; `window-previews { off; }` turns it
  off (`AccessDenied`, or the name is not claimed when off at startup).
- Same trust model as Niri's Mutter ScreenCast and GNOME Shell screenshot
  services: same-user session-bus clients. Authorizing the Shell alone, as first
  proposed, would need a policy Niri does not have.

## Shell

Taskbar item tooltips show up to four of the application's windows, in screen
order, above the tooltip text, at most 192 px, keeping their aspect. Opening the
tooltip shows the cached thumbnails and asks for fresh ones; when they arrive,
the tooltip is reopened with them. Windows that close are dropped from the
cache. A slow capture (over two seconds) is skipped. The taskbar now declares
that it needs the session bus, which the Shell otherwise opened only for other
modules.

## Validation

- Fork: at its branch niri 264, niri-config 36 + 1, niri-ipc 2 pass; on
  `canopa/main` niri 458 pass; unit, headless render and limit tests; a nested
  session captured visible and hidden-workspace windows, clamped sizes,
  refused an unknown window and limited concurrent calls.
- Shell: `cargo test -p canopa-shell`: thumbnail cache order, limit, closed
  windows and aspect.
- Private Weston/Niri session with the pinned build and the Shell: hovering a
  taskbar application with three windows showed their three thumbnails above its
  name (`.scratch/artifacts/niri-scrolling/preview-pin/hover.png`). One capture
  took 2.9 s in that software-rendered debug build; release builds on a GPU are
  expected to be far faster, but this was not measured.

Not done: thumbnails in the minimap (the bar is too small) or a window switcher.

## Comments
