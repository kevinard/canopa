# Module Format Templates Design

**Date:** 2026-04-26
**Author:** Kevin

## Summary

Add shared text format templates for every non-tray module. Users can configure where module values appear in rendered text with `{placeholder}` syntax, including a common `{icon}` placeholder that inserts the module's selected icon. Existing configs keep their current behavior unless `format`, `expanded-format`, or `horizontal-format` is configured.

This builds on the shared icon-rules work: icon selection remains separate, but format templates can include the resolved icon via `{icon}`.

## Goals

- Provide common `format`, `expanded-format`, and `horizontal-format` fields for every module except tray.
- Use `{value}` placeholders, with `{{` and `}}` escaping literal braces.
- Make `{icon}` available to every formatted module.
- Preserve current module output by default.
- Validate templates at config parse time so unknown placeholders produce actionable errors.
- Keep module-specific values explicit and testable.

## Non-Goals

- Do not add text labels to tray icons.
- Do not change colors, spacing, click behavior, subscriptions, or icon-rule matching.
- Do not replace clock's existing `time-format` and `date-format`; those still control chrono formatting before template substitution.
- Do not introduce conditionals, loops, width formatting, or expression evaluation in templates.

## Config Syntax

Each non-tray module may define:

```kdl
format "<template>" ["<template>" ...]
expanded-format "<template>" ["<template>" ...]
horizontal-format "<template>" ["<template>" ...]
```

Examples:

```kdl
module "battery" {
    format "{icon}" "{value}%"
    expanded-format "{icon}" "{value}%" "{state}"
    horizontal-format "{icon}" "{value}%"
}

module "volume" {
    format "{icon} {value}%"
    expanded-format "{icon} {state}: {value}%"
    horizontal-format "{icon} {value}%"
}

module "network" {
    format "{icon} {value}"
    expanded-format "{icon} {label} ({state})"
    horizontal-format "{icon} {state}"
}

module "clock" {
    time-format "%H:%M"
    date-format "%a %d %b"
    format "{icon} {time}"
    expanded-format "{icon} {time}\n{date}"
    horizontal-format "{icon} {time}"
}

module "focused-window" {
    format "{icon}"
    expanded-format "{title}"
    horizontal-format "{title}"
}

module "script" {
    format "{icon} {value}"
    horizontal-format "{icon} {output}"
}

module "button" {
    format "{icon}"
    expanded-format "{icon} {label}"
    horizontal-format "{icon} {label}"
}

module "workspaces" {
    format "{icon}{label}"
    horizontal-format "{icon}"
}
```

Fallback order:

- Collapsed/primary rendering uses `format` when present.
- Expanded rendering uses `expanded-format` when present, otherwise `format` when present.
- Horizontal rendering uses `horizontal-format` when present, otherwise `expanded-format` when present, otherwise `format` when present.
- If none of the relevant fields are present, the module uses its current hard-coded display.

Each string argument is one rendered segment. A single string argument behaves like the current combined string format. Multiple string arguments render as separate text/container segments in order, so users can keep icon/value/state pieces independently laid out.

## Shared Types

Add shared template types in `crates/niribar/src/config.rs`:

```rust
pub struct FormatConfig {
    pub collapsed: Option<FormatTemplate>,
    pub expanded: Option<FormatTemplate>,
    pub horizontal: Option<FormatTemplate>,
}

pub struct FormatTemplate {
    segments: Vec<TextTemplate>,
}

pub struct TextTemplate {
    raw: String,
    placeholders: Vec<String>,
}
```

Each segment `TextTemplate` parses once at config load. Rendering receives a placeholder map and returns one substituted string per segment.

Template parsing rules:

- `{name}` inserts placeholder `name`.
- `{{` inserts a literal `{`.
- `}}` inserts a literal `}`.
- A single unmatched `{` or `}` is a parse error.
- Empty `{}` is a parse error.
- Placeholder names use ASCII letters, digits, `_`, and `-`.
- Unknown placeholders are rejected during module config parsing.

## Common Placeholder

`{icon}` is available for all formatted non-tray modules. It resolves through the same icon path the module would otherwise render:

- Battery, volume, and network use configured icon rules when they match, then built-in fallback icons.
- Clock uses its clock icon.
- Script uses its script icon.
- Button uses configured button icon.
- Focused-window uses the window placeholder icon.
- Workspace item uses configured workspace icon when present, otherwise its workspace number.

## Module Placeholders

### Battery

- `{icon}`
- `{value}`: rounded percentage without `%`
- `{percent}`: rounded percentage without `%`
- `{state}`: `charging` or `discharging`

### Volume

- `{icon}`
- `{value}`: rounded volume without `%`
- `{percent}`: rounded volume without `%`
- `{state}`: `muted` or `unmuted`

### Network

- `{icon}`
- `{value}`: connection label if present, otherwise default state label
- `{label}`: connection label if present, otherwise default state label
- `{state}`: `full`, `limited`, or `none`

### Clock

- `{icon}`
- `{value}`: time
- `{time}`: formatted time
- `{date}`: formatted date

### Focused Window

- `{icon}`
- `{value}`: title
- `{title}`: title

When no window is focused, the module still renders nothing.

### Script

- `{icon}`
- `{value}`: command output
- `{output}`: command output

### Button

- `{icon}`
- `{value}`: configured label
- `{label}`: configured label

### Workspaces

For each runtime workspace item:

- `{icon}`
- `{value}`: configured label if present, otherwise workspace index
- `{index}`: workspace index
- `{name}`: Niri workspace name if present, otherwise empty string
- `{label}`: configured label if present, otherwise workspace index
- `{state}`: `focused`, `active`, or `inactive`

## Rendering Behavior

For modules that already use `IconLabel`, configured templates replace the label string for the matching render mode: `expanded-format` for expanded vertical bars, `horizontal-format` for horizontal bars, and `format` where the module displays collapsed text. Collapsed views that are icon-only remain icon-only unless the module already displays text in the collapsed layout or a configured `format` deliberately includes text.

For custom-layout modules:

- Clock collapsed uses `format` when present; expanded uses `expanded-format` when present, otherwise `format`; horizontal uses `horizontal-format` when present, otherwise the normal fallback order.
- Script collapsed applies `format` before the existing five-character truncation.
- Focused-window collapsed can use `format` when configured; otherwise it remains icon-only.
- Button collapsed can use `format` when configured; otherwise it remains icon-only.
- Workspace items use `format` for vertical item content and `horizontal-format` for horizontal item content. If `horizontal-format` is absent, they follow the normal fallback order.

The layout code may still split icon and text into separate widgets when that preserves existing styling, but the visible string comes from the template.

## Parser Design

Add a reusable parser helper for `format`, `expanded-format`, and `horizontal-format`, with allowed placeholder lists supplied by each module parser.

Every non-tray module config struct gains:

```rust
pub format: FormatConfig
```

Tray remains unchanged.

Validation:

- `format`, `expanded-format`, and `horizontal-format` must include at least one non-empty string if present.
- Unknown placeholders are errors with paths such as `modules[0].battery.format`.
- Invalid braces are errors with the same field path.
- Duplicate placeholders are allowed.

## Compatibility

All new fields default to `FormatConfig::default()`, so existing configs render exactly as before. Default config comments can document examples without changing behavior.

## Testing

Template tests:

- Substitutes `{value}`.
- Substitutes repeated placeholders.
- Renders multiple template segments in order.
- Escapes `{{` and `}}`.
- Rejects unmatched braces.
- Rejects empty placeholders.
- Rejects placeholder names outside the allowed list.

Parser tests:

- Parses `format`, `expanded-format`, and `horizontal-format` for battery, volume, network, clock, focused-window, script, button, and workspaces.
- Rejects `format` for tray.
- Rejects unknown placeholders for a module.

Module tests:

- Battery/volume/network formatted labels include resolved `{icon}` and values.
- Clock collapsed and expanded formats use `{time}` and `{date}`.
- Script collapsed truncates formatted output after substitution.
- Button expanded format uses `{icon}` and `{label}`.
- Focused-window expanded format uses `{title}` and still renders nothing when title is absent.
- Workspace item format uses configured label/icon and workspace state.
