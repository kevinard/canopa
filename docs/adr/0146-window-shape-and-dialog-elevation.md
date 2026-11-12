# 0146 Window shape, dialog elevation and inactive browsers

Status: accepted. Builds on [ADR 0003](0003-shared-appearance.md),
[ADR 0019](0019-iced-transient-toplevels.md) and
[ADR 0129](0129-supported-session-and-settings.md).

Files drew a 24 px rounded, outlined panel inside every window. Its windows are
opaque, so the window background showed as dark square corners behind each
rounded panel, most visibly on floating dialogs. Floating dialogs had no
shadow or outline, so they barely separated from the browser below them.

## Options considered

- **Transparent client windows** with an application-drawn radius and shadow.
  Iced styles all windows of a daemon at once, a shadow needs a transparent
  margin that the compositor counts as part of the window, and alpha support
  depends on the renderer and surface.
- **Square, edge-to-edge client surfaces, with shape and elevation from the
  compositor.** The supported session ships the compositor defaults. Chosen.

## Decision

- Files windows fill their surface edge to edge (`chrome::window_surface`). A
  customized Panel radius or border still applies. In high contrast, floating
  dialogs keep a full-strength outline of their own.
- `packaging/niri/defaults.kdl` gives every window a 12 px corner radius,
  clipped to that shape, and a soft shadow. Floating windows replace the tiling
  focus ring with a hairline border and a deeper shadow. The tiling focus ring
  uses a neutral tone instead of a second accent.
- Floating dialogs use a surface one step lighter than the browser in the dark
  scheme.
- A browser dims while a delete confirmation or name prompt that it owns is
  open, and more lightly while another Files window, such as Quick Look,
  Properties or Preferences, has focus. The dimming only paints: the browser
  keeps receiving input, and its widget tree keeps the same shape.

## Consequences

Other applications in the supported session get the same window shape and
shadow. Unsupported compositors show square Files windows without the
compositor shadow, which is still correct. The capture harness for design
reviews uses the shipped compositor defaults, without key bindings.
