# 0159. Keep window chrome and the overview with the compositor

Status: superseded by [0160](0160-window-bar-and-scrolling-presentation.md)

## Context

The scrolling workspace spans more columns than a monitor can show. Window
outlines need a reliable focus signal in both color schemes, and the overview
needs to distinguish workspaces and keep previews clear of the rail.

## Decision

Niri owns the window outline, shadows, titlebar, movement, and overview
geometry. The framed overview uses the reserved area reported by layer-shell
surfaces for rendering and hit testing. Captions stay at the output's normal
display scale as workspaces zoom out.

Floating previews sit beside the scrolling strip when they fit at the
configured overview zoom. Selection and dragging use the preview positions.
Pointer motion and arrow keys choose the same preview target. The action below
the workspace previews brings the selected window forward on click or Enter.
Opening Overview from the rail gives the picker keyboard focus. In a workspace
with both tiled and floating windows, Tab switches between those layers.

When enabled, the compositor clips the scrolling layout to the work area and
reserves a window strip above ordinary tiles on sufficiently large outputs. A
tile can land beside a neighbor when both fit. The compositor clips small
fragments of neighboring tiles as scrolling settles. The strip keeps its tabs
in layout order as the viewport moves and scales their widths to reflect the
columns they represent. A labeled outline tracks the visible tiles during a
scroll. The ends of the strip reserve space for stepping to offscreen tiles
even when the window names fill the row.
The directional buttons share a tint with the windows they focus. Hovering a
destination previews the tiles that will be in view. The compositor uses
thumbnails where they fit in empty workspace space. When windows fill the area,
the current group keeps its labeled outline. A **Next view** row names the
projected tiles and identifies the focus destination and the tiles that stay.
The preview stays dismissed after an arrow click until the pointer leaves the
button. The strip
and thumbnail preview share window labels derived from all windows in the
workspace. The focus marker identifies the active window during the slide.
Selecting a tiled window from the strip temporarily hides the floating layer on
that workspace. The strip and overview retain the floating windows as targets,
and selecting one restores the layer and its window placement. Where
space permits, the strip also provides a dedicated control to show or hide the
floating layer. It uses the same activation path as the window buttons. Window
previews respect the compositor's capture rules.

Settings maps the shared appearance to compositor colors and title typography
through the generated Niri include. Session owns the wallpaper behind the
compositor's workspaces. Shell offers an optional minimap backed by compositor
IPC. The default rail groups workspace navigation and running applications.

## Consequences

A Shell restart leaves workspace navigation and window controls available.
User overrides retain priority through `niri-custom.kdl`, and configured
wallpaper images take priority over the embedded artwork.
