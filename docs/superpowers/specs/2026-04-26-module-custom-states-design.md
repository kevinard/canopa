# Module Custom States — Design

## Goal

Let users declare custom states per module in config, and override built-in state thresholds, using a single unified mechanism.

## Scope

Numeric-threshold states only. Applies to modules whose state derives from a numeric value:

- **battery** (percentage)
- **volume** (volume level)

Out of scope: categorical-state modules (network, workspaces), boolean-gated predicates, removing the `critical-threshold` knob (kept for backward compat).

## Semantics

A state has a name and a single integer threshold `T` ∈ `0..=100`. Given the module's current numeric value `v`, the active state is the one whose threshold is the **smallest `T` such that `v ≤ T`**. If `v` exceeds every declared threshold, the state with the **highest** threshold is used as fallback.

Built-in non-numeric overrides — battery `charging`, volume `muted` — short-circuit ahead of the threshold lookup, exactly as today.

### Built-in defaults

Battery (matches today's exposed state vocabulary `critical`/`low`/`normal`):

| Name     | Threshold |
|----------|-----------|
| critical | 15        |
| low      | 30        |
| normal   | 100       |

Volume (single numeric tier; `muted` overrides it when the sink is muted):

| Name    | Threshold |
|---------|-----------|
| unmuted | 100       |

## KDL syntax

A unified `states` child block per module. Each `state` node carries a positional name and a `threshold` property.

```kdl
module "battery" {
    states {
        state "critical" threshold=10        // overrides built-in critical (15)
        state "needs_charge" threshold=60    // new custom state
    }
}
```

User entries with the same name as a built-in **replace** the built-in. New names are added.

### Backward compatibility

`critical-threshold N` is still parsed. If `states` is absent, it overrides the built-in `critical` threshold as it does today. If both are present, `states` wins and a warning is logged.

## Resolution

After parsing:

1. Start from the module's built-in defaults table.
2. For each user-declared `state`, replace by name (or insert).
3. Sort the merged list by threshold ascending.
4. At runtime: pick the first rule whose `threshold ≥ v`; if none, pick the rule with the highest threshold.

## Validation (parse time)

- threshold out of `0..=100` → error
- duplicate name within the user's `states` block → error
- empty name → error
- non-integer threshold → error
- missing `threshold` property → error

## Where the resolved state flows

Unchanged. The resolved state name continues to feed:

- `{state}` template placeholder
- icon selection (`icon "…" state="…"`)
- segment styling (`style state="…" color="…"`)

The format parser's `allowed_states` vocabulary, currently `crate::config::states::BATTERY` / `VOLUME`, is replaced per-module by the merged set (built-in + user) so custom names validate as legal selectors.

## Touch points

- `src/config.rs` — add `StatesConfig` / `StateRule`; add `states: StatesConfig` field to `BatteryConfig` and `VolumeConfig`; update `states` module helpers.
- `src/config/parse/modules/battery.rs`, `…/volume.rs` — parse `states` block; merge with built-ins; pass merged vocabulary to `formats::parse_optional`; handle `critical-threshold` deprecation.
- `src/config/parse/kdl.rs` — small helper for parsing the `states` block (or inline in each module parser).
- `src/modules/battery.rs::battery_state_key` — replace hardcoded match with `StatesConfig::resolve`.
- `src/modules/volume.rs` — same; `muted` short-circuit stays.
- `src/modules.rs` — pass `StatesConfig` through `battery_ctor` / `volume_ctor`.
- `config/default.kdl` — show new `states` block syntax in the battery and volume comment examples.

## Out of scope

- Boolean-gated predicates (`is_charging`, `muted`).
- Custom states on categorical modules (network, workspaces).
- Removing the `critical-threshold` field.
