# Shared Module Icons Design

**Date:** 2026-04-26
**Author:** Kevin

## Summary

Add one shared icon selection feature that modules can opt into. Users can configure icons by named state or by numeric thresholds, and modules that already vary icons by state or value can use the same config shape instead of hard-coded module-specific icon tables only.

The first implementation wires the shared icon config into `battery`, `volume`, and `network`. Existing configs keep working unchanged; configured icons override only the icon string, not labels, colors, badges, subscriptions, or interactions.

## Goals

- Provide a standard `icons` block that can be reused by any module.
- Support state-based icons for enum-like states such as `charging`, `muted`, `full`, `limited`, and `none`.
- Support numeric icons where a rule applies when the module value is at or above a configured threshold.
- Preserve current built-in icons as defaults and fallbacks.
- Keep parsing, matching, and module integration independently testable.

## Non-Goals

- Do not introduce global theming or color overrides for icons.
- Do not change module labels or expanded layouts.
- Do not require every module to expose icon configuration immediately.
- Do not replace workspace-specific labels/icons; workspace config is already a separate per-workspace feature.

## Config Syntax

Each supported module may include an `icons` child block:

```kdl
module "battery" {
    icons {
        icon "󰁹" at 90
        icon "󰁽" at 50
        icon "󰁺" at 10
        icon "󰂃" state "charging"
    }
}

module "network" {
    icons {
        icon "◆" state "full"
        icon "◈" state "limited"
        icon "✕" state "none"
    }
}
```

`icon` takes the glyph/string as its first argument. A rule must then provide either:

- `state "<name>"` for exact state matching.
- `at <number>` for numeric threshold matching.

## Matching Semantics

State rules take precedence over numeric rules. This lets states such as `charging` and `muted` override threshold-based icons.

For numeric rules, thresholds mean “at or above.” The selected icon is the rule with the highest threshold less than or equal to the current value. Example: with thresholds `10`, `50`, and `90`, a value of `65` uses the `50` icon.

If no configured rule matches, the module returns its existing built-in icon. This includes an empty `icons` block, an unknown state key for the current module, and numeric values below the lowest configured threshold.

## Shared Types

Add a reusable config model in `src/config.rs`:

```rust
pub struct IconRulesConfig {
    pub states: HashMap<String, String>,
    pub thresholds: Vec<IconThreshold>,
}

pub struct IconThreshold {
    pub at: f64,
    pub icon: String,
}
```

The parser sorts thresholds once, so runtime matching is deterministic and simple. Duplicate state keys and duplicate thresholds are rejected during parsing with an actionable config error.

The matcher exposes small focused methods:

```rust
impl IconRulesConfig {
    pub fn icon_for_state(&self, state: &str) -> Option<&str>;
    pub fn icon_for_value(&self, value: f64) -> Option<&str>;
    pub fn icon_for_state_or_value(&self, state: Option<&str>, value: Option<f64>) -> Option<&str>;
}
```

## Module Integration

### Battery

`BatteryConfig` gains `icons: IconRulesConfig`.

Battery state keys:

- `charging` when `is_charging` is true.
- Numeric value is `percentage`.

Selection order:

1. `state "charging"` when charging.
2. Numeric threshold matching against percentage.
3. Existing built-in battery icon tier.

The critical badge and icon color logic remain unchanged.

### Volume

`VolumeConfig` gains `icons: IconRulesConfig`.

Volume state keys:

- `muted` when muted or effectively zero volume.
- Numeric value is `volume`.

Selection order:

1. `state "muted"` when muted or volume is below the current zero threshold.
2. Numeric threshold matching against volume.
3. Existing built-in volume tier icon.

Scroll behavior and labels remain unchanged.

### Network

`NetworkConfig` becomes a normal struct with `icons: IconRulesConfig`.

Network state keys:

- `full`
- `limited`
- `none`

Selection order:

1. Exact state match.
2. Existing built-in connectivity icon.

Network has no numeric value.

## Parser Design

Add one parser helper for `icons` blocks under `src/config/parse/`. Module parsers call this helper and include the returned `IconRulesConfig` in their module config.

Validation:

- `icons` with no child nodes is allowed and behaves like no custom icons.
- Each child must be named `icon`.
- The first argument must be a non-empty string.
- A rule must specify exactly one matcher: `state` or `at`.
- `state` must be a non-empty string.
- `at` must be numeric. Thresholds are allowed to be integer or floating-point, because runtime values are floating-point percentages.
- Duplicate states and duplicate thresholds are errors.

Error paths follow current module parser style, for example `modules[0].battery.icons[1]`.

## Testing

Parser tests:

- Parses state icon rules.
- Parses numeric threshold icon rules.
- Rejects missing icon value.
- Rejects rules with both `state` and `at`.
- Rejects rules with neither `state` nor `at`.
- Rejects duplicate states and thresholds.

Matcher tests:

- Numeric matching uses the highest threshold at or below the value.
- Numeric matching returns `None` below the lowest threshold.
- State matching takes precedence over numeric matching.

Module tests:

- Battery configured `charging` icon overrides percentage icon while charging.
- Battery configured threshold icon overrides built-in tier icon while discharging.
- Volume configured `muted` icon overrides threshold icons.
- Volume threshold icon selects by current volume.
- Network configured state icon selects by connectivity.
- All three modules fall back to current built-in icons when no configured rule matches.

## Compatibility

All new config fields default to empty `IconRulesConfig`, preserving the current visible bar for existing users. The default bundled config can document the `icons` blocks as commented examples, but it does not change the default icon set.
