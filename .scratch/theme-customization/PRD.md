# Theme and UI customization

Status: in-progress

Design proposal, 2026-10-02; implementation authorized 2026-10-03.
The original proposal is retained below. The implemented configuration contract is
[documented here](../../docs/theme-customization.md), with architecture in
[ADR 0013](../../docs/adr/0013-configurable-theme-and-surface-geometry.md).

Users should be able to create and share a complete Canopa appearance, then adjust
individual elements and bar placement in their own configuration. One shared theme
must cover Canopa Shell and Notification Banners, including panels, menus, the
Notification Center, tooltips and common controls.

## Confirmed scope

- A shared theme with local overrides for individual bars and UI elements.
- Custom colors, borders, corner radii, margins, padding and gaps.
- Bar screen-edge offsets, shorter lengths and start, center or end alignment.
- Configurable font families and text sizes.
- Reusable theme files selected from the main Canopa configuration.
- Design and scope first, before implementation.

## Existing foundations

[Shared appearance](../../docs/adr/0003-shared-appearance.md) already connects
`canopa-config`, `canopa-theme` and `canopa-ui` to both applications. The shared
configuration currently accepts only a light or dark preset and an accent color.
Most dimensions and visual treatments remain constants or literals.

Shell `style` currently means text color, size and weight. Bar defaults and module
segment overrides already have precedence rules; extending UI styling must retain
those rules. Colors such as `base` and `surface0` are existing compatibility names.

Bar physical plans currently span an edge with zero margins. Their `collapsed-width`
is the thickness for either orientation. Panel placement, drawer motion, tooltips
and menu virtualization contain assumptions tied to that geometry or fixed metrics.

The [configuration guide](../../docs/configuration.md) and
[shell reload decision](../../docs/shell/adr/0021-live-configuration-reload.md)
define validation and reload behavior. Notify and the Settings portal have separate
installation and reload lifetimes.

## Configuration model

Keep appearance and desktop arrangement separate. A theme file contains colors,
typography and UI defaults. The main configuration selects that file and can
override its appearance, while application namespaces own placement and behavior.

This example is proposed syntax and is not accepted by current binaries:

```kdl
schema-version 1

appearance {
    theme "themes/nord.kdl"

    // Optional overrides of the selected theme.
    palette {
        accent "#88c0d0"
    }
    typography {
        family "Inter" "Noto Sans" "sans-serif"
        body-size 14
        label-size 12
        title-size 18
    }
    ui {
        surface {
            border width=1 color="border"
            radius 10
        }
        control {
            radius 6
        }
        panel {
            padding 16
            gap 8
        }
    }
}

shell {
    bar "main" {
        edge "top"
        thickness 40
        length 900
        align "center"
        margin 10 16 0 16
        reserve-space #true

        ui {
            padding 6 10
            gap 6
            radius 12
            border width=1 color="accent"
        }
        panel {
            ui {
                radius 12
                padding 18
            }
        }
        modules {
            start "workspaces"
            center "clock"
            end "volume" "notifications"
        }
    }
    modules {
        module "clock" {
            ui {
                padding 4 8
                radius 6
                background "surface"
            }
        }
    }
}
```

The theme file contains an appearance document without `schema-version`, `shell`,
`notifications`, or another theme reference:

```kdl
color-scheme "dark"
palette {
    background "#2e3440"
    surface "#3b4252"
    raised "#434c5e"
    text "#eceff4"
    muted "#d8dee9"
    disabled "#6c7789"
    border "#4c566a"
    accent "#88c0d0"
    danger "#bf616a"
    warning "#ebcb8b"
    success "#a3be8c"
}
typography {
    family "Inter" "sans-serif"
    body-size 14
    label-size 12
    title-size 16
}
ui {
    surface {
        border width=1 color="border"
        radius 10
    }
    control {
        radius 6
    }
    bar {
        padding 6
        gap 6
    }
}
```

The example illustrates role coverage, not a contrast-certified preset.

## Theme resolution

Resolve every property independently. Omitting a property inherits it; `0` is an
explicit value, such as no border or square corners.

1. Start from the selected built-in preset and existing per-element defaults.
2. Apply the theme file: shared surface/control defaults, then element-kind defaults.
3. Apply inline `appearance`: shared defaults, then element-kind defaults.
4. Apply the named element's local `ui` overrides.
5. Apply applicable visual state overrides to the resolved element.

This order makes an inline shared default override a theme file's more specific
setting; an inline element-kind setting can then refine it. Resolve color references
against the final palette, so an accent override updates every reference to accent.

Surface kinds are bar, panel, menu, tooltip, notification and card. Control kinds
are module, button, input and menu-row. Each kind inherits only its corresponding
surface/control defaults. These are documented visual roles, not arbitrary widget
selectors. Standalone text and internal layout wrappers do not acquire borders.

Local overrides belong to existing configured identities: `bar.ui`,
`bar.panel.ui`, `modules.module.ui`, named menu `ui`, and a Notify banner `ui`
block. The exact Notify block location should follow its existing schema during
implementation. A bar's `ui` changes its frame; it does not implicitly restyle all
descendants. Module overrides apply to every placement of that configured Module.
Per-placement module overrides are deferred.

The Notification Center is styled by the shell's panel/card/control roles; Notify
owns banner presentation. Provider documents continue to supply semantic content
and state, not host placement or arbitrary theme selectors.

Palette overrides cover all existing semantic roles. Add explicit foreground-on-
accent, foreground-on-danger and focus roles where automatic choices currently
prevent a faithful custom theme. Derive those values when omitted. Shared palette
entries remain opaque RGB; per-element colors accept RGB or RGBA and semantic role
names. This allows transparent surfaces without multiplying text/icon opacity.

Keep `appearance.accent` as a compatibility alias for `palette.accent`; reject both
in the same source layer. Keep existing shell token aliases while documenting the
semantic names. Explicit text/segment styles retain their current precedence over
shared typography and palette defaults.

## Borders and spacing

All dimensions use logical pixels. Validate finite, bounded, nonnegative dimensions;
font sizes and explicit bar dimensions must be positive. Define concrete limits in
the parser contract before marking implementation tickets ready.

| Setting | Proposed meaning |
| --- | --- |
| `border width=… color=…` | Solid border, drawn inside the element's outer bounds; width zero disables it. |
| `radius N` | All corners; zero gives square corners. |
| `radius TL TR BR BL` | Separate corner radii, clamped to the element bounds at layout time. |
| `padding N`, `padding V H`, `padding T R B L` | Space between border and content. |
| `margin N`, `margin V H`, `margin T R B L` | Space outside the element; margins never collapse. |
| `gap N` | Space between immediate siblings owned by that layout. |
| `background`, `foreground` | Semantic role name or literal color. |

Shared surface/control defaults expose only properties that every corresponding
kind can honor. Kind-specific properties such as gap, row height or outer placement
live on the appropriate kind. Unknown or inapplicable properties are errors.

For ordinary boxes, adjacent margins and parent gap add together. For popovers and
tooltips, outer spacing participates in placement and output clamping. Banner outer
spacing participates in stack layout; stack screen offsets remain Notify-owned.
The bar has the specific placement contract below.

Interactive kinds support explicit hover, pressed, selected, disabled and focus
colors. State overrides cannot change layout dimensions. Compose base, selected,
hover, pressed, then disabled; apply the focus indicator last for enabled controls.
Only kinds exposing a state accept its configuration. User styling must not change
focus ownership, target identities or action dispatch.

Unconfigured elements retain their existing appearance, including the bar's current
inner-edge separator and square attached drawers. An explicitly configured bar
border requests a full frame. An explicit radius applies to attached drawers too.
Document supported differences instead of silently discarding settings.

## Bar placement

Expose orientation-independent `thickness`. Retain `collapsed-width` as its legacy
alias and reject configurations specifying both. Thickness is the complete visible
frame size across the edge, including border and padding.

`length` accepts `"fill"` (default), a positive logical-pixel value, or a percentage
string such as `"80%"`. The percentage is of the available edge span after end
insets. `align` accepts `"start"`, `"center"` or `"end"`; start is left on a
horizontal bar and top on a vertical bar. Alignment positions a shorter bar within
the remaining span. Content-sized bars are deferred.

For a top bar, left/right margins inset that span, top margin separates the frame
from its placement edge, and bottom margin is space after the frame toward content.
Rotate those roles for the other edges. These values are outer spacing, not content
padding. The proposed example puts a 900 by 40 frame 10 logical pixels below its
placement edge, centered between 16-pixel end insets.

`reserve-space` defaults to true. Its requested reservation includes the near-edge
margin, frame thickness and inner margin. A shorter bar requests an edge-wide
reserved strip; it does not promise an irregular application work area. With false,
application windows may extend underneath it; its inner margin still contributes
to the separation of associated panels.

Preserve existing compositor avoidance and ordering when several bars occupy an
output: horizontal bars are planned first, then vertical bars. Margins offset the
placement edge available to that bar, which may already be inset by another
reserved surface. A margin is not a promise to overlap another bar to reach a
physical output edge. Multiple matching bars keep their named identities and
Output Targets.

The visual bar bounds, input region, module coordinates, panel/tooltips anchors and
reservation must agree. Empty space around a short or rounded bar must not intercept
pointer input. The center module zone stays centered on the visible bar frame.
Moving a bar for push-bar drawer presentation must preserve its configured offset,
length and alignment, then restore the original placement on close.

Oversized lengths clamp to the available span. Excessive insets must leave a bounded,
nonzero surface and cannot cause negative dimensions, overflow or protocol errors.
Resolve a deterministic fitting policy in the geometry slice, with diagnostics for
clamping. Reject locally impossible fixed thickness/padding/border combinations.

The layer-shell contract restricts positive exclusive zones for corner anchors, and
margins participate in the exclusive geometry. Therefore, simply changing a short
start-aligned bar to top-left anchoring is insufficient. Validate a native placement
strategy before committing to the adapter design; do not count margins twice.
This requirement follows the upstream
[layer-shell protocol](https://raw.githubusercontent.com/swaywm/wlr-protocols/master/unstable/wlr-layer-shell-unstable-v1.xml).

## Typography and reusable files

Support an ordered list of installed text font families with generic fallback,
plus body, label and title sizes. Text shaping, wrapping, row measurement, focus
rectangles and accessibility bounds use the same resolved typography. Preserve
dedicated symbol/icon fonts and normal Unicode fallback. No font downloading is
involved. Missing families fall back predictably and can be reported by an explicit
local configuration diagnostic.

A theme reference is a local path, resolved relative to the main configuration
file; absolute paths are also allowed. One theme file is sufficient for this version.
It cannot include another file, change application behavior, or run commands.

Load the main file and theme as one bounded candidate. Retain existing regular-file,
UTF-8 and symlink handling; apply a documented combined byte budget. Missing or
invalid selected themes are errors. Validators must consume the same captured bytes
for both applications, including when temporary snapshots are used, so relative
paths cannot accidentally resolve against a temporary directory.

Extend source provenance to record the theme dependency and a combined fingerprint,
while preserving the existing main-source fingerprint contract. Errors identify the
correct source and span in explicit local diagnostics; runtime logging retains safe
codes. Include the theme path, missing-file state and symlink target in shell reload
monitoring. Editing only the theme must trigger shell reload.

Preserve independent consumer reload behavior: Shell watches; Notify and the Settings
portal use their existing reload commands. A single theme file does not imply a
cross-process atomic update. Invalid candidates preserve the complete prior theme,
fonts, UI metrics and geometry. Notify appearance updates retain notification
identities and running timeout deadlines. The portal continues exposing effective
color-scheme/accent, not Canopa-specific spacing or arbitrary application styling.

## Implementation shape

`canopa-theme` owns toolkit-independent appearance data and deterministic resolution:
palette, typography, visual roles and resolved metrics. `canopa-config` owns syntax,
source dependencies, diagnostics and complete candidate preparation. `canopa-ui`
adapts resolved values to Iced rendering and measurement. Application owners supply
local overrides, interaction states and placement inputs.

Resolve immutable styles once per effective configuration. Rendering, preferred-size
calculation, virtualized row ranges, hit testing and accessibility consume the same
resolved metrics. Do not create a second set of sizing constants beside configurable
rendering values. Keep toolkit/process state out of shared theme data.

Bar geometry should expose a small calculation interface whose result describes
visible bounds, host geometry if different, input region and reserved edge depth.
Placement and rendering consume that result. The compositor adapter strategy is an
implementation investigation, not another user-facing configuration choice.

This extends shared ADRs 0002 and 0003. Theme dependencies also extend the intake
described in ADR 0008; shell live reload extends ADR 0021's watched-source contract.
The shell's midpoint terminology must describe the visible bar if the chosen adapter
uses a larger transparent host. Record those changes in an ADR when the design is
accepted; existing decisions remain authoritative until then.

## Delivery sequence

1. **Prove bar geometry.** Exercise fill and short bars on every edge, all alignments,
   margins, space reservation and click-through areas under an isolated compositor.
   Include panels, tooltips, push-bar drawers and multiple bars. Select the adapter
   approach and deterministic fitting policy from this evidence.
2. **Complete shared themes.** Add full semantic palette, typography, theme-file
   intake/provenance and reload support. Carry a complete custom theme through bars,
   a panel and a live notification banner, plus the portal's effective accent.
3. **Expose UI styles and local overrides.** Add role defaults and per-element
   overrides end to end, updating measurements and virtualized menu row geometry
   together with rendering. Include interaction-state styles.
4. **Ship bar placement.** Expose the validated geometry through thickness, length,
   alignment, margins and reserve-space; integrate reload/reconciliation.
5. **Document and validate the combined feature.** Ship complete example themes,
   config reference, migration notes and maintained desktop acceptance scenarios.

These are delivery slices of one agreed feature scope, not permission to stop after
palette customization. No implementation tickets are published until the proposed
schema and remaining geometry/parser contracts are settled.

## Acceptance evidence

- A theme with every palette role overridden renders consistently across the shell,
  Notification Center, menus, tooltips and Notification Banners.
- Square, rounded, borderless and translucent examples demonstrate real visual
  differences. Local overrides affect only their documented targets.
- Theme-file versus inline versus local precedence is exercised with conflicting
  values; omitted values and explicit zero behave differently.
- A font change updates wrapping and measured geometry without clipped menu rows,
  incorrect scroll ranges or stale accessibility targets.
- A large searchable menu remains usable with custom font sizes, row padding and
  gaps; scrolling reaches every entry and selection follows the visible row.
- Geometry coverage includes all four edges, three alignments, fill/fixed/percentage
  lengths, asymmetric margins, multiple bars, small outputs and fractional scaling.
- Panel/module anchors, rounded-corner and outer-gap click-through, push-bar motion,
  outside dismissal, keyboard focus and output hotplug remain correct.
- Editing only a theme file reloads the shell. Missing files, invalid colors,
  invalid dimensions and duplicate/unknown settings preserve the last good state.
- Checkers and applications resolve the same theme bytes, including explicit paths,
  relative dependencies, symlinks and atomic editor replacement.
- Existing configurations retain their default rendering and placement. Preset
  contrast checks remain; custom themes are not silently recolored or rejected
  solely for artistic contrast choices.
- Run focused parser/resolver/geometry regressions and maintained compositor
  scenarios, then the repository's normal `bash scripts/check.sh` validation.

## Deferred scope

Arbitrary CSS/selectors, programmable theme expressions, recursive theme inheritance,
remote theme/font downloads, a graphical theme editor, content-sized bars, gradients,
blur, shadow customization, animations, icon-theme selection and application/Niri
window-decoration theming are separate work. The Settings portal's existing color
scheme/accent export continues to work; it does not apply a Canopa theme to other
toolkits. Further widget-specific styling can be added only with matching rendering
and measurement support.
