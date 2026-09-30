# Module Segment Styles Design

**Date:** 2026-04-26
**Author:** Kevin

## Summary

Allow each segment of a module's `format`, `expanded-format`, or `horizontal-format` to carry a style override (color, size, weight). Style overrides may be unconditional or keyed on module state, so users can reproduce or replace the built-in state coloring (battery critical red, network limited yellow, etc.) on top of their formatted output.

This builds directly on the format-templates spec (`docs/niribar/superpowers/specs/2026-04-26-module-format-templates-design.md`). The bare positional-string form continues to work; styling is opt-in via a child-node form.

## Goals

- Per-segment color, size, and weight overrides for every formatted non-tray module.
- State-keyed style maps so styling can react to module state (battery critical, network limited, volume muted, workspace focused, etc.).
- Theme-token color references (`accent`, `critical`, ...) as well as raw hex.
- Backward compatibility: existing `format "..." "..."` configs render exactly as before.
- Validation at config parse time with actionable field paths.

## Non-Goals

- No styling for tray icons.
- No styling for modules that have not configured a `format`. Modules left at their hard-coded default rendering keep their current hard-coded styles.
- No background colors, borders, padding, italics, or text decorations in v1.
- No inline markup inside template strings (Pango-style).
- No expression language for style values; each style entry is a flat set of fields.

## Config Syntax

A `format`, `expanded-format`, or `horizontal-format` field accepts either the existing positional-string form, or a child-node form. Both forms may be mixed: positional strings are unstyled segments; child `segment` nodes carry styles.

Bare form, unchanged:

```kdl
format "{icon}" "{value}%"
```

Child-node form:

```kdl
format {
    segment "{icon}" color="accent" size=18
    segment "{value}%" weight="bold"
}
```

Mixed form:

```kdl
format "{icon}" {
    segment "{value}%" color="text" weight="bold"
}
```

Each `segment` node takes one positional string (the template) and any of the style props described below. Segment order is the order the nodes appear, which is also their render order.

### Style props on a segment

Unconditional styling sets props directly on the `segment` node:

```kdl
segment "{value}%" color="accent" size=18 weight="bold"
```

State-conditional styling uses one or more `style` child nodes. The first `style` with no `state` is the default; the first `style` whose `state` matches the module's current state wins. Specific `state` matches override the default.

```kdl
segment "{icon}" {
    style color="accent"
    style state="critical" color="critical"
    style state="low"      color="warning"
    style state="charging" color="success"
}
```

A segment may carry either inline props or `style` children, not both. Mixing both on the same segment is a parse error.

### Style values

- `color` — string. Either a theme token (case-insensitive) or a hex literal:
  - Tokens: `accent`, `text`, `subtext`, `base`, `surface0`, `surface1`, `border`, `overlay0`, `critical`, `warning`, `success`.
  - Hex: `#rrggbb` or `#rrggbbaa`.
  - Anything else is a parse error.
- `size` — positive number, font size in pixels. Floats are accepted; non-positive values are a parse error.
- `weight` — string, one of `light`, `normal`, `bold`. Anything else is a parse error.

All three props are optional on every `segment` and `style` node. A node with no style props is allowed (treated as no override, useful for documenting intent).

### State vocabulary (revision of prior spec)

The format-templates spec defined `{state}` placeholder values per module. To let the styling matcher reuse the same vocabulary, this spec widens those vocabularies to match the visual states modules already track:

| Module          | State values                              |
|-----------------|-------------------------------------------|
| Battery         | `charging`, `critical`, `low`, `normal`   |
| Volume          | `muted`, `unmuted`                        |
| Network         | `full`, `limited`, `none`                 |
| Workspaces      | `focused`, `active`, `inactive`           |
| Clock / script / focused-window / button | (none — no state styling) |

`charging` takes precedence over the level buckets when the battery is on AC. Thresholds for `critical` and `low` follow the existing built-in thresholds; this spec does not change them.

The `{state}` text placeholder now resolves to one of the values above. This is a behavior change for battery (`charging`/`discharging` becomes `charging`/`critical`/`low`/`normal`), but the prior spec has not shipped any user-visible default that depends on the old vocabulary.

Modules without state in the table above ignore `state="..."` selectors; using one is a parse error against that module's allowed selector list.

## Shared Types

Extend the types introduced by the format-templates spec in `crates/niribar/src/config.rs`:

```rust
pub struct FormatTemplate {
    segments: Vec<StyledSegment>,
}

pub struct StyledSegment {
    template: TextTemplate,
    styles: SegmentStyles,
}

pub enum SegmentStyles {
    None,
    Static(Style),
    Stateful {
        default: Option<Style>,
        by_state: Vec<(StateKey, Style)>,
    },
}

pub struct Style {
    pub color: Option<ColorValue>,
    pub size: Option<f32>,
    pub weight: Option<Weight>,
}

pub enum ColorValue {
    Token(ThemeToken),
    Rgba(iced::Color),
}

pub enum ThemeToken {
    Accent, Text, Subtext, Base,
    Surface0, Surface1, Border, Overlay0,
    Critical, Warning, Success,
}

pub enum Weight {
    Light, Normal, Bold,
}
```

`StateKey` is a typed enum per module (battery state, network state, etc.); the parser converts the string from `state="..."` into the appropriate enum based on the owning module's allowed-selectors list.

`SegmentStyles::Stateful::by_state` is a `Vec` rather than a map to preserve declaration order for diagnostics; lookups are linear (segment counts are tiny).

## Parsing

Add to the existing format-template parser helper:

- A `format` argument may be a positional string or a child `segment` node, in any combination.
- A `segment` node takes exactly one positional template string.
- A `segment` may have inline style props OR `style` children, not both.
- A `style` child takes no positional args and may have a `state` prop plus any of `color`, `size`, `weight`.
- A `style` without `state` is the default. At most one default per segment; a second default is a parse error.
- A `style` with a `state` value not in the owning module's allowed selectors is a parse error.
- Two `style` entries with the same `state` value on the same segment is a parse error.
- All other format-template parse rules from the prior spec carry over (placeholder validation, unknown placeholders, brace escaping).

Field paths in errors follow the existing convention, e.g. `modules[0].battery.format.segment[1].style[2]`.

## Rendering

At render time the module passes its current `StateKey` (or `None` for stateless modules) into the segment renderer. For each segment:

1. Substitute placeholders in `template` against the module's value map (existing behavior).
2. Resolve the effective `Style`:
   - `None` → no override.
   - `Static(s)` → `s`.
   - `Stateful { default, by_state }` → first `by_state` entry whose key matches the module's current state, otherwise `default`, otherwise no override.
3. Apply the resolved style to the segment widget:
   - `color` resolves through `theme.rs` for tokens or directly for hex; sets the iced text style callback.
   - `size` overrides the widget's font size.
   - `weight` maps to iced `font::Weight`.

Where a property is `None` in the resolved style, the module's existing default for that property is used. This is the contract that lets users style only the bits they care about and leave everything else alone.

When the resolved style for an icon segment sets `color`, the configured color overrides the built-in state coloring for that icon. When `color` is left unset, the module's built-in state coloring continues to apply — so users who add only `size` or `weight` overrides keep the existing color behavior.

## Compatibility

- Bare positional-string `format` continues to parse and render unchanged.
- `format` configs that use only the child-node form with no style props render identically to the equivalent bare form.
- The `{state}` text placeholder for battery now expands to the four-value vocabulary; users who rely on the literal string `discharging` in `format` text will need to update. This is called out in the changelog.
- All other prior-spec semantics (fallback order across `format`/`expanded-format`/`horizontal-format`, escaping, unknown-placeholder errors) are unchanged.

## Testing

Parser tests:

- Bare positional-string form parses to unstyled segments.
- Child `segment` nodes with inline style props parse to `Static`.
- Child `segment` nodes with `style` children parse to `Stateful` with the right default and entries.
- Mixed positional-string and child-node form parses in declaration order.
- Inline props plus `style` children on the same segment is a parse error.
- Two default `style` entries on the same segment is a parse error.
- Two `style` entries with the same `state` value on the same segment is a parse error.
- Unknown theme token in `color` is a parse error.
- Hex color shorter or longer than 6/8 hex digits is a parse error.
- Non-positive `size` is a parse error.
- Unknown `weight` value is a parse error.
- `state` value outside the module's allowed selectors is a parse error.
- `state` selector used on a stateless module (clock, script, focused-window, button) is a parse error.

Renderer tests:

- A static-styled segment renders with the configured color, size, and weight.
- A stateful-styled segment picks the entry matching the module's current state.
- A stateful-styled segment with no matching state and a default falls back to the default.
- A stateful-styled segment with no matching state and no default applies no override.
- Leaving `color` unset preserves the module's built-in state coloring.
- Setting `color` overrides the module's built-in state coloring.
- Hex and theme-token colors resolve to the same iced color when they refer to the same RGB.

Module-level tests cover at least:

- Battery icon styled red when state is `critical` and theme color when `normal`.
- Network state-keyed colors for `full`/`limited`/`none`.
- Volume muted vs unmuted state styles.
- Workspace items pick the style matching each item's own `focused`/`active`/`inactive` state.
