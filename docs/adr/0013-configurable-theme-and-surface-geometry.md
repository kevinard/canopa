# 0013: Configurable themes and surface geometry

Status: accepted, 2026-10-03.

Extends [shared appearance](0003-shared-appearance.md) and
[shared configuration](0002-shared-configuration.md). The existing reload ownership
and notification presentation acknowledgement contracts remain in place.

Users need reusable themes across Shell and Notify, with local frame and placement
overrides. `canopa-theme` owns immutable palette, typography, partial UI overrides,
resolved styles and bounded logical dimensions. It remains independent of Iced,
filesystem access and process state. `canopa-config` owns theme intake, grammar,
validation and dependency provenance. `canopa-ui` owns Iced font and frame/control
adapters; consumers own state, layout, action identity and accessibility.

Property precedence is preset, theme file, inline appearance, then local element.
Groups precede kinds inside each layer; state colors apply last. Unsupported
properties/states are errors. Explicit zero differs from an omitted property.
Font selection resolves installed names without interning arbitrary configuration
strings. The system's text engine retains glyph fallback.

Theme files contain appearance nodes, with no recursive includes. They resolve
relative to the selected configuration. Main and theme input share the existing
1 MiB budget. A dependency fingerprint and combined fingerprint extend provenance
without changing the meaning of the main fingerprint. Combined validation captures
immutable copies of both files before invoking consumers. Shell watches both inputs;
Notify and the portal retain independent reload lifetimes and last-good behavior.

Bar placement separates the visible frame from its native host. The host spans an
edge, preserving layer-shell exclusive-edge semantics for start, center and end
alignment. A single geometry calculation drives visible bounds and rounded input
rectangles. Transparent margins and unused length do not intercept desktop input.
Reservation counts near margin, thickness and inner margin exactly once. This avoids
corner-anchored short surfaces losing their reservation, and requires no fork change.
Panels and tooltips account for unreserved bars separately from compositor reservation.

Rendering, menu virtualization, keyboard reveal and icon preparation share row metrics.
Tooltip text uses the same font/shaping for measurement and rendering. Local named
menu gaps participate in the same metrics. Existing module `style` and segment styles
continue to describe text; generic frame styling uses a separate `ui` block.

See [the configuration contract](../theme-customization.md) for supported properties,
bounds, compatibility defaults and fitting behavior. The maintained
`theme-customization` desktop scenario exercises edge placement, file reload and
transparent input alongside the existing desktop acceptance matrix.
