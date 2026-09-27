# Shared Module Icons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add shared, configurable icon rules for battery, volume, and network modules.

**Architecture:** `crates/niribar/src/config.rs` owns the shared icon-rule data model and matching behavior. `crates/niribar/src/config/parse/modules/icons.rs` parses a reusable `icons { icon ... }` block. Battery, volume, and network configs carry an `IconRulesConfig`, and each module consults it before falling back to its current built-in icon logic.

**Tech Stack:** Rust, KDL config parsing via `kdl`, existing module trait and `IconLabel` rendering helpers, `cargo test`.

---

## File Structure

- Modify `crates/niribar/src/config.rs`: add `IconRulesConfig` and `IconThreshold`, then add `icons` fields to `BatteryConfig`, `VolumeConfig`, and `NetworkConfig`.
- Create `crates/niribar/src/config/parse/modules/icons.rs`: parse and validate shared `icons` blocks.
- Modify `crates/niribar/src/config/parse/modules/mod.rs`: include the new helper module.
- Modify `crates/niribar/src/config/parse/modules/battery.rs`: parse `icons` with `critical-threshold`.
- Modify `crates/niribar/src/config/parse/modules/volume.rs`: parse `icons` with `step`.
- Modify `crates/niribar/src/config/parse/modules/network.rs`: parse `icons`.
- Modify `crates/niribar/src/modules.rs`: pass icon rules to constructors and synthesized defaults.
- Modify `crates/niribar/src/modules/battery.rs`: choose configured charging/threshold icons before built-in tiers.
- Modify `crates/niribar/src/modules/volume.rs`: choose configured muted/threshold icons before built-in tiers.
- Modify `crates/niribar/src/modules/network.rs`: choose configured connectivity state icons before built-in icons.
- Modify `crates/niribar/config/default.kdl`: add commented examples only; no default behavior change.

## Task 1: Shared Icon Rule Model

**Files:**
- Modify: `crates/niribar/src/config.rs`

- [ ] **Step 1: Write failing tests**

Add tests inside a new `#[cfg(test)] mod icon_rule_tests` in `crates/niribar/src/config.rs`:

```rust
#[test]
fn icon_rules_select_highest_threshold_at_or_below_value() {
    let rules = IconRulesConfig {
        states: std::collections::HashMap::new(),
        thresholds: vec![
            IconThreshold { at: 10.0, icon: "low".into() },
            IconThreshold { at: 50.0, icon: "mid".into() },
            IconThreshold { at: 90.0, icon: "high".into() },
        ],
    };

    assert_eq!(rules.icon_for_value(65.0), Some("mid"));
}

#[test]
fn icon_rules_return_none_below_lowest_threshold() {
    let rules = IconRulesConfig {
        states: std::collections::HashMap::new(),
        thresholds: vec![IconThreshold { at: 10.0, icon: "low".into() }],
    };

    assert_eq!(rules.icon_for_value(9.0), None);
}

#[test]
fn icon_rules_state_takes_precedence_over_value() {
    let mut states = std::collections::HashMap::new();
    states.insert("charging".into(), "plug".into());
    let rules = IconRulesConfig {
        states,
        thresholds: vec![IconThreshold { at: 10.0, icon: "low".into() }],
    };

    assert_eq!(
        rules.icon_for_state_or_value(Some("charging"), Some(15.0)),
        Some("plug")
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test config::icon_rule_tests --lib`

Expected: FAIL because `IconRulesConfig` and `IconThreshold` do not exist.

- [ ] **Step 3: Implement minimal model**

Add public structs and matcher methods in `crates/niribar/src/config.rs`.

- [ ] **Step 4: Run tests**

Run: `rtk cargo test config::icon_rule_tests --lib`

Expected: PASS.

## Task 2: Shared `icons` Parser

**Files:**
- Create: `crates/niribar/src/config/parse/modules/icons.rs`
- Modify: `crates/niribar/src/config/parse/modules/mod.rs`
- Modify: `crates/niribar/src/config/parse/tests.rs`

- [ ] **Step 1: Write failing parser tests**

Add tests in `crates/niribar/src/config/parse/tests.rs` that parse:

```kdl
modules { module "battery" { icons { icon "low" at 10 icon "plug" state "charging" } } }
```

Assert battery config contains one threshold and one state. Also add error tests for missing matcher, both matchers, duplicate state, and duplicate threshold.

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test config::parse::tests::parse_battery_icons --lib`

Expected: FAIL because the parser ignores or lacks icon rules.

- [ ] **Step 3: Implement parser helper**

Create `icons.rs` with `parse_optional(children, path) -> anyhow::Result<IconRulesConfig>`. Read `children.get("icons")`, iterate child nodes named `icon`, require a non-empty first string argument, require exactly one named entry `state` or `at`, reject duplicates, and sort thresholds ascending by `at`.

- [ ] **Step 4: Wire battery parser and run tests**

Update `battery.rs` to include parsed icon rules in `BatteryConfig`.

Run: `rtk cargo test config::parse::tests --lib`

Expected: PASS for existing config parser tests plus new icon parser tests.

## Task 3: Volume and Network Config Parsing

**Files:**
- Modify: `crates/niribar/src/config.rs`
- Modify: `crates/niribar/src/config/parse/modules/volume.rs`
- Modify: `crates/niribar/src/config/parse/modules/network.rs`
- Modify: `crates/niribar/src/config/parse/tests.rs`

- [ ] **Step 1: Write failing tests**

Add parser tests proving:

```kdl
modules { module "volume" { step 8 icons { icon "muted" state "muted" icon "loud" at 66 } } }
modules { module "network" { icons { icon "ok" state "full" icon "off" state "none" } } }
```

The tests should assert `step` remains parsed for volume and state icons are stored for both modules.

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test config::parse::tests::parse_volume_icons --lib`

Expected: FAIL because volume/network configs do not yet carry parsed icon rules.

- [ ] **Step 3: Implement config fields and parsers**

Make `NetworkConfig` a struct with `icons: IconRulesConfig`. Add `icons` to `VolumeConfig`. Update defaults and parser return values.

- [ ] **Step 4: Run tests**

Run: `rtk cargo test config::parse::tests --lib`

Expected: PASS.

## Task 4: Module Runtime Integration

**Files:**
- Modify: `crates/niribar/src/modules.rs`
- Modify: `crates/niribar/src/modules/battery.rs`
- Modify: `crates/niribar/src/modules/volume.rs`
- Modify: `crates/niribar/src/modules/network.rs`

- [ ] **Step 1: Write failing module tests**

Add focused unit tests:

```rust
// battery.rs
// custom charging icon wins over thresholds
// custom threshold icon wins while discharging
// fallback built-in icon remains when no rule matches

// volume.rs
// custom muted icon wins
// custom threshold icon wins
// fallback built-in icon remains when no rule matches

// network.rs
// custom full/none state icon wins
// fallback built-in icon remains when no rule matches
```

- [ ] **Step 2: Run tests to verify failure**

Run: `rtk cargo test modules::battery::tests::battery_custom --lib`

Expected: FAIL because constructors do not accept icon rules or icon methods ignore them.

- [ ] **Step 3: Implement minimal integration**

Store `IconRulesConfig` in each module struct. Constructors accept icon rules. `modules.rs` extracts icon rules from module params and passes them through. Icon methods consult rules first and otherwise call the existing built-in functions.

- [ ] **Step 4: Run module tests**

Run: `rtk cargo test modules::battery::tests modules::volume::tests modules::network::tests --lib`

Expected: PASS.

## Task 5: Defaults and Full Verification

**Files:**
- Modify: `crates/niribar/config/default.kdl`

- [ ] **Step 1: Add commented config examples**

Document `icons` blocks under battery, volume, and network as comments only.

- [ ] **Step 2: Run formatting**

Run: `rtk cargo fmt --check`

Expected: PASS. If it fails, run `rtk cargo fmt` and re-run `rtk cargo fmt --check`.

- [ ] **Step 3: Run full test suite**

Run: `rtk cargo test`

Expected: PASS.

- [ ] **Step 4: Commit implementation**

Run:

```bash
rtk git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules crates/niribar/src/config/parse/tests.rs crates/niribar/src/modules.rs crates/niribar/src/modules/battery.rs crates/niribar/src/modules/volume.rs crates/niribar/src/modules/network.rs crates/niribar/config/default.kdl docs/niribar/superpowers/plans/2026-04-26-shared-module-icons.md
rtk git commit -m "feat: add shared module icon rules"
```

Expected: implementation and plan are committed separately from the earlier design commit.

## Self-Review

Spec coverage: shared model, parser, matching semantics, battery/volume/network integration, compatibility, and default config docs are each covered by tasks.

Placeholder scan: no TBD/TODO placeholders remain.

Type consistency: the plan uses `IconRulesConfig`, `IconThreshold`, `icon_for_state`, `icon_for_value`, and `icon_for_state_or_value` consistently across parser and module tasks.
