# Shared ordinary Module presentation

Date: 2026-10-01
Status: accepted

Ordinary Module renderers previously repeated collapsed, horizontal, Panel and
rotated Bar paths. Their fallback order and container policy drifted; Media and
Idle Inhibitor ignored explicit rotation on horizontal Bars. Plain formatted-text
fallbacks were unreachable after the corresponding styled-template branch.

`ModuleRender::render_bar` is the Bar entry point. It derives orientation from
Edge. An ordinary Module supplies an `IconLabel` presentation: state values,
styled segments, rotation setting, and optional fallback or decoration. Shared
view code selects layout, applies resolved style and template inheritance,
constructs geometry, and rotates configured content. Unformatted fallback
views retain their original orientation. Expanded Panel content has no Bar
rotation. Notifications does not expose a rotation configuration.

Clock's date row, Script's truncated fallback, Button's expanded alignment,
notification count and battery critical badge remain Module-owned visuals.
Focused Window images, Workspace frames and Tray anchors retain their custom
render paths behind the same Bar boundary. The generic App no longer knows the
per-Module fallback matrix or carries a second orientation flag.

A real Iced layout regression reproduced the horizontal rotation omission. The
format matrix exercises all 11 format-capable Module types on all four Edges
with absent, inherited and explicit templates and all rotation settings. It
checks state-derived segments, style inheritance, resulting geometry and
unformatted fallback stability. Workspace items deliberately have no expanded
Panel form. Existing native rotation acceptance covers glyph and image quality
at integer and fractional scales.
