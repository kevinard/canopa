# Accessibility assessment

Assessed 2026-09-23. This is a record of specific checks and remaining limits,
not a whole-application WCAG conformance or screen-reader usability claim.

## Current Canopa colors

The 2026-10-03 [Canopa visual direction](../adr/0014-canopa-visual-direction.md)
replaces the initial palette. Run `python3 scripts/check-contrast.py` from the
workspace root for current dark/light ratios. Shared theme tests additionally
check primary/secondary text on spine, panel, raised and quiet selection fills,
focus contrast against those fills, and text on the active accent. These numeric
checks do not establish whole-application accessibility or custom-theme contrast.

## Historical color assessment

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

Modules without a useful bar presentation and workspaces belonging to other
outputs are omitted from bar frames and keyboard navigation. Empty panel
presentations do not retain blank framed rows. Quick settings retains Wi-Fi and
Sound controls when status is unavailable so their providers remain reachable;
Tab/Shift+Tab select cards and Enter activates them. Unavailable DND is disabled,
and unavailable Keep awake offers retry. Source loss clears stale bar focus and tooltips; recovery restores
the module to the visible navigation list. These transitions have App-level
regression tests in addition to the isolated compositor check.

Quick settings keeps its session buttons in a two-column footer with 44-pixel
activation targets and the shared focus outline. Log out, Reboot and Power off
open confirmation menus with Cancel focused first; Enter on Cancel returns to
Quick settings and Escape dismisses without running the action. Native dark/light
fixtures exercise these paths with harmless command stubs.

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

## Shared AT-SPI bridge (2026-11-11)

The launcher, bar modules, Quick settings cards, power confirmations, menus,
calendar and tray menus now publish through the shared `canopa-ui`
accessibility layer ([ADR 0144](../adr/0144-shared-accesskit-layer.md)), with
native AT-SPI tests for launcher search and selection and a bar item. Provider
panels keep the bridge described above; a surface never has both. Secondary
actions (right click, scroll) and tooltips are not exposed, and no Orca pass
has been recorded yet.
