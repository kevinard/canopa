# Iced editing state and optional action icons

Status: accepted under delegated implementation authority, 2026-10-18.
Scoped implementation and native verification pass; the full PRD stays open.

Responsive toolbars must preserve the actual Iced input state, including a
partial selection and an intermediate caret position. Keeping only the draft
String or asynchronously restoring focus does not satisfy this requirement.
The location field occupies the same child in one toolbar widget at every
width. Layout changes its geometry and reserves its configured position among
the commands with an elastic spacer. Narrow windows place commands below the
field. The menu and command strip also keep their children and native state.
The command strip must not receive pointer input through the location field.

Keep an always-present notice slot in the Custom Actions editor. Showing an
external-source change or validation error may change geometry but must not
move the editable form to another widget-tree position. Source-change notices
retain the current draft and its native focus/selection; the existing writer
continues to refuse stale saves. No asynchronous select-all/end-of-field repair
may substitute for the user's selection.

Optional action icons use `canopa-assets` for indexed XDG lookup and bounded
raster preparation, with an application-owned SVG adapter using the workspace's
existing resvg version. Match Shell's hicolor policy: nominal size 22, source
scale 1, PNG/SVG/XPM path selection, and the existing indexless hicolor fallback.
An unavailable or unsupported icon leaves the action's text label and behavior
usable. This adds no GUI toolkit and no shared-fork change.

Prepare at most 256 catalog names on one admitted worker, with one replaceable
pending request. Superseding the catalog/appearance cancels between stages and
rejects stale deliveries. A callback from an earlier worker cannot clear a newer
worker. Views only clone prepared Iced image handles; they perform no filesystem
lookup, decoding or SVG parsing. Reload rebuilds the catalog cache, including
same-name icon changes. An optional asset failure cannot prevent a command.

Files policy is 256 KiB per encoded icon, at most 8 MiB of raster reads and 8 MiB of
SVG reads per batch, 1024 px source dimensions and 1,048,576 source pixels, an 8 MiB
best-effort raster decoder allocation limit, and one 16 MiB source/output RGBA
admission budget. Render icons within 72×72 for 18 logical pixels. Shared lookup
retains its own root/index/check limits. These are logical budgets, not total
process-memory limits or filesystem deadlines. No recursive icon-directory scan
or UI-thread decode is introduced.

SVG input is plain UTF-8; compressed SVG, embedded/external images and system-font
discovery are not admitted. The SVG adapter converts premultiplied output to
straight RGBA. `currentColor` resolves to the shared button's base foreground;
names ending in `-symbolic` use that foreground and retain their alpha silhouette.
Regular icons retain their authored colors. Disabled toolbar icons are dimmed.
Labels, tooltips and keyboard invocation remain available beside icons.

The first native editing fixture reproduced selection/focus loss on wide-to-narrow
adaptation: subsequent arrow keys navigated folders instead of editing the path.
The fixed toolbar passes actual partial-selection replacement, widening and
intermediate-caret insertion, followed by navigation to the resulting real
directory. External editor notices also preserve native selection, and stale
save keeps both source bytes and the unsaved draft. Scoped evidence and remaining
acceptance gates belong in `.scratch/file-manager/IMPLEMENTATION.md`.

The final executable passes 33 adapter tests and 136 declared native checks across
ten sequential runs in dark at 100% and light at 125% scaling, including the existing browsing, search,
preview and custom-action workflows. All-target Clippy lints, the desktop build
and workspace formatting pass. Four existing unused operation-helper warning
groups remain. Fourteen selected final captures were inspected; this is not full
visual/accessibility acceptance. Exact sources, executable, logs and scope are in
[the verification record](../../../.scratch/file-manager/iced-interface-verification.json).
