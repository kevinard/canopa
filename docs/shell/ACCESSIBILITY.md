# Accessibility assessment

Assessed 2026-09-23. This is a record of specific checks and remaining limits,
not a whole-application WCAG conformance or screen-reader usability claim.

## Default colors

The color calculation uses WCAG's sRGB relative luminance and
`(lighter + 0.05) / (darker + 0.05)`. Thresholds are 4.5:1 for the default small
text and status glyphs, and 3:1 for focus indicators against adjacent surfaces.
Ratios are compared before rounding. See the W3C explanations of
[text contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)
and [non-text contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html).

The palette is now defined in shared `../crates/canopa-theme/src/lib.rs` (integration
update, 2026-09-26). Run `python3 scripts/check-contrast.py` from Canopa Shell to check both
presets, or add `--markdown` to print their tables. The table below records the original
dark palette. Shared controls additionally test the actual hover text/background pairs.
CI runs this check; it checks numeric pairs, not whether every future rendering
path uses them correctly.

| Foreground | Background | Ratio | Required | Result |
| --- | --- | ---: | ---: | --- |
| TEXT | BASE | 11.34:1 | 4.5:1 | Pass |
| TEXT | SURFACE0 | 8.69:1 | 4.5:1 | Pass |
| SUBTEXT | BASE | 7.37:1 | 4.5:1 | Pass |
| SUBTEXT | SURFACE0 | 5.65:1 | 4.5:1 | Pass |
| ACCENT | BASE | 7.79:1 | 4.5:1 | Pass |
| ACCENT | SURFACE0 | 5.97:1 | 4.5:1 | Pass |
| CRITICAL | BASE | 7.08:1 | 4.5:1 | Pass |
| CRITICAL | SURFACE0 | 5.43:1 | 4.5:1 | Pass |
| WARNING | BASE | 12.91:1 | 4.5:1 | Pass |
| WARNING | SURFACE0 | 9.89:1 | 4.5:1 | Pass |
| SUCCESS | BASE | 11.03:1 | 4.5:1 | Pass |
| SUCCESS | SURFACE0 | 8.46:1 | 4.5:1 | Pass |
| TEXT | SURFACE1 | 6.31:1 | 4.5:1 | Pass |
| BASE | ACCENT | 7.79:1 | 4.5:1 | Pass |
| BASE | CRITICAL | 7.08:1 | 4.5:1 | Pass |
| ACCENT | BASE | 7.79:1 | 3:1 | Pass |
| ACCENT | SURFACE0 | 5.97:1 | 3:1 | Pass |
| ACCENT | SURFACE1 | 4.33:1 | 3:1 | Pass |
| SUBTEXT | BASE | 7.37:1 | 3:1 | Pass |
| SUBTEXT | SURFACE0 | 5.65:1 | 3:1 | Pass |
| SUBTEXT | SURFACE1 | 4.10:1 | 3:1 | Pass |

The default bar and hover surfaces use BASE and SURFACE0. Module panel frames
now use SURFACE0 (BASE when focused): the old SURFACE1 background gave SUBTEXT
4.10:1, ACCENT 4.33:1, and CRITICAL 3.94:1, below the small-text target. SURFACE1
remains suitable for TEXT in workspace pills, menus and tooltips.

Muted volume is an active status, not a disabled control. Its former OVERLAY0
foreground gave only 3.36:1 on BASE and 2.57:1 on SURFACE0. It now uses SUBTEXT.
OVERLAY0 remains available for disabled tray-menu entries (1.87:1 on SURFACE1),
which are exempt from the text contrast criterion. It should not be used for
active text in custom formats.

Keyboard/pointer highlights in module frames, module panels, selectable panel
menus and tray menus now include an ACCENT outline. Previously the fill change
between SURFACE0 and SURFACE1 was only 1.38:1. Decorative BORDER separators are
1.37–2.46:1 against the dark surfaces; they must not be the sole focus indicator
or the sole information needed to identify a control. The accent outline passes
against both adjacent dark surfaces. Focus-indicator area and complete keyboard
coverage still need operator review.

Active but unfocused workspaces also receive a persistent SUBTEXT outline.
Their SURFACE1 fill alone was only 1.80:1 against the bar background; the outline
passes against both the fill and the surrounding bar/hover surface. Module
frames reserve a two-pixel inset, so opaque workspace pills cannot paint over
the interaction outline. This also leaves a dark gap between a focused
workspace's accent fill and its outer interaction outline.

Empty/unavailable modules and workspaces belonging to other outputs are omitted
from bar frames and keyboard navigation. Empty panel presentations do not retain
blank framed rows. Source loss clears stale focus and tooltips; recovery restores
the module to the visible navigation list. These transitions have App-level
regression tests in addition to the isolated compositor check.

Custom colors, alpha blending, user format overrides, external application/tray
icons and emoji font rendering are outside
this token calculation. For custom text on SURFACE1, prefer TEXT; for colored
status text, prefer the darker SURFACE0 background. No contrast guarantee is
made for arbitrary user-defined token combinations.

Canopa Shell and Notify now select the same Canopa Iced palette explicitly. Shared
notification card/action styles, typography and focus treatment are owned by
`canopa-ui`; see [ADR 0003](../adr/0003-shared-appearance.md). The private desktop
fixture covers Center keyboard focus and native actions in both consumers. This
does not extend native accessibility coverage to the remaining bar/tray controls.

## Native accessibility

The isolated AT-SPI regression
`app::canopa_accessibility::tests::native_center_accessibility_exposes_geometry_focus_and_revision_owned_actions`
passes on this host. It exercises the real accessibility bus and registry:

- Canopa content names, styled/rich text and image alternatives.
- Button/link roles, hyperlink URI and bounds matching rendered layout.
- Programmatic focus, focused state, and action delivery to the owning provider.
- Replacement of content, rejection of stale actions, and removal on retirement.

The fixture does not run Orca, synthesize physical keyboard input, or establish
native accessibility coverage for ordinary bar modules, tray menus and generic
provider controls. Those remain unverified. See the
[desktop acceptance procedure](manual-validation.md) and the
[dated validation record](audits/2026-09-23-desktop-validation.md).
