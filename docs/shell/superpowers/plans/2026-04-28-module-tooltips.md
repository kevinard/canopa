# Module Tooltips Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add hover tooltips for bar modules. Tooltip content uses the same template grammar as the existing `format` field; tray icons get tooltips driven by the SNI `ToolTip` property; tooltips render as separate layer-shell popup surfaces with a per-bar dwell delay.

**Architecture:** A new `TooltipSetting` config enum (`Default | Disabled | Custom(FormatTemplate)`) on every module config, resolved to `Option<FormatTemplate>` at config-load time. A new per-bar `tooltip_delay_ms` controls dwell. Per-surface `tooltip_pending` state ticks alongside the existing hover-expand timer. When dwell crosses the threshold the app opens a layer-shell `TooltipPopup` surface — sibling to the existing `MenuPopup` — anchored on the module's bar-edge-opposite side. SNI `ToolTip` property is fetched at item registration and on `NewToolTip` signal; `{title}` and `{description}` placeholders bind from it for tray modules.

**Tech Stack:** Rust, iced + iced_layershell (Wayland layer-shell), zbus (D-Bus), KDL config.

**Spec:** `docs/niribar/superpowers/specs/2026-04-28-module-tooltips-design.md`

---

## Phase 1 — Config foundation: `TooltipSetting` and shared parser

### Task 1: Define `TooltipSetting` enum and resolution helper

**Files:**
- Modify: `crates/niribar/src/config.rs` (near `FormatConfig`, around line 207)

- [ ] **Step 1: Add `TooltipSetting` type**

In `crates/niribar/src/config.rs`, immediately after the existing `FormatConfig` block (around line 226) add:

```rust
/// User-facing tooltip configuration for a module. Three states:
/// - `Default`: field absent in config; resolves to the module's built-in default.
/// - `Disabled`: user wrote `tooltip false`; tooltip is suppressed.
/// - `Custom(template)`: user supplied a string or styled-segment block.
#[derive(Debug, Clone, PartialEq)]
pub enum TooltipSetting {
    Default,
    Disabled,
    Custom(FormatTemplate),
}

impl Default for TooltipSetting {
    fn default() -> Self {
        TooltipSetting::Default
    }
}

impl TooltipSetting {
    /// Resolve this setting to a runtime template using `default_template`
    /// when the user did not configure the field. `None` means the tooltip
    /// is disabled (either explicitly via `false` or because the module has
    /// no built-in default).
    pub fn resolve(&self, default_template: Option<FormatTemplate>) -> Option<FormatTemplate> {
        match self {
            TooltipSetting::Default => default_template,
            TooltipSetting::Disabled => None,
            TooltipSetting::Custom(t) => Some(t.clone()),
        }
    }
}
```

- [ ] **Step 2: Verify it builds**

Run: `cargo build`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/niribar/src/config.rs
git commit -m "feat(config): add TooltipSetting enum"
```

---

### Task 2: Shared parser helper for `tooltip` field

**Files:**
- Modify: `crates/niribar/src/config/parse/modules/formats.rs`
- Test: `crates/niribar/src/config/parse/modules/formats.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Append to `crates/niribar/src/config/parse/modules/formats.rs` (file currently has no tests; create the `tests` mod):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ::kdl::KdlDocument;

    fn parse(src: &str) -> KdlDocument {
        src.parse().expect("kdl parse")
    }

    fn allowed() -> &'static [&'static str] {
        &["icon", "value", "state"]
    }

    #[test]
    fn tooltip_absent_returns_default_setting() {
        let doc = parse("");
        let s = parse_tooltip_field(&doc, "m", allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Default));
    }

    #[test]
    fn tooltip_string_returns_custom() {
        let doc = parse(r#"tooltip "{value}%""#);
        let s = parse_tooltip_field(&doc, "m", allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_returns_disabled() {
        let doc = parse("tooltip #false");
        let s = parse_tooltip_field(&doc, "m", allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let doc = parse("tooltip #true");
        let err = parse_tooltip_field(&doc, "m", allowed(), &[])
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("tooltip true is not allowed"),
            "got: {err}"
        );
    }

    #[test]
    fn tooltip_block_returns_custom() {
        let doc = parse(
            r#"tooltip {
                segment "{icon}"
                segment "{value}"
            }"#,
        );
        let s = parse_tooltip_field(&doc, "m", allowed(), &[]).unwrap();
        match s {
            crate::config::TooltipSetting::Custom(t) => {
                assert_eq!(t.segments().len(), 2);
            }
            _ => panic!("expected Custom"),
        }
    }
}
```

Run: `cargo test --lib config::parse::modules::formats::tests`
Expected: FAIL — `parse_tooltip_field` not defined.

- [ ] **Step 2: Implement `parse_tooltip_field`**

Append to `crates/niribar/src/config/parse/modules/formats.rs` above the test mod:

```rust
use crate::config::TooltipSetting;

/// Parse the optional `tooltip` field on a module's child block.
///
/// Accepts:
/// - absence → `TooltipSetting::Default`
/// - `tooltip "..."` (positional string, optionally multiple segments) → `Custom`
/// - `tooltip { segment ... }` (styled-segment block) → `Custom`
/// - `tooltip #false` → `Disabled`
/// - `tooltip #true` → error
pub(super) fn parse_tooltip_field(
    children: &KdlDocument,
    module_path: &str,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
) -> anyhow::Result<TooltipSetting> {
    let Some(node) = children.get("tooltip") else {
        return Ok(TooltipSetting::Default);
    };
    let path = format!("{module_path}.tooltip");

    // Single bool entry (no children, one positional bool) → enabled/disabled flag.
    let positional: Vec<_> = node.entries().iter().filter(|e| e.name().is_none()).collect();
    if node.children().is_none() && positional.len() == 1 {
        if let Some(b) = positional[0].value().as_bool() {
            if b {
                anyhow::bail!(
                    "{path}: `tooltip true` is not allowed; omit the field to use the default."
                );
            }
            return Ok(TooltipSetting::Disabled);
        }
    }

    // Otherwise: delegate to the shared styled-segment parser.
    let segments =
        super::segment_styles::parse_segments(node, allowed_placeholders, allowed_states, &path)?;
    Ok(TooltipSetting::Custom(crate::config::FormatTemplate::from_styled(segments)))
}
```

Run: `cargo test --lib config::parse::modules::formats::tests`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/niribar/src/config/parse/modules/formats.rs
git commit -m "feat(config): add shared parse_tooltip_field helper"
```

---

### Task 3: `tooltip-delay` on `BarConfig`

**Files:**
- Modify: `crates/niribar/src/config.rs:578-621` (`BarConfig` struct + `Default`)
- Modify: `crates/niribar/src/config/parse/bars.rs:113-117` (parse + builder)
- Modify: `crates/niribar/src/config/parse/bars.rs` (tests around line 305)

- [ ] **Step 1: Write the failing test**

In `crates/niribar/src/config/parse/bars.rs`, near the existing `parse_hover_delay` tests (around line 305), add:

```rust
#[test]
fn parse_tooltip_delay() {
    let kdl = r#"bar "main" { tooltip-delay 250 }"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.bars[0].tooltip_delay_ms, 250);
}

#[test]
fn parse_tooltip_delay_defaults_to_500() {
    let config = parse(r#"bar "main" {}"#).unwrap();
    assert_eq!(config.bars[0].tooltip_delay_ms, 500);
}

#[test]
fn parse_tooltip_delay_negative_errors() {
    let kdl = r#"bar "main" { tooltip-delay -1 }"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(err.contains("tooltip-delay"), "got: {err}");
}
```

Run: `cargo test --lib config::parse::bars::tests::parse_tooltip_delay`
Expected: FAIL (field does not exist).

- [ ] **Step 2: Add field to `BarConfig`**

In `crates/niribar/src/config.rs`, inside `pub struct BarConfig` (line 578), add after `pub hover_delay: u64,`:

```rust
    /// Dwell delay (ms) before a module's tooltip appears on hover.
    /// Default 500. Hide is always immediate on hover-exit.
    pub tooltip_delay_ms: u32,
```

In the `Default for BarConfig` impl (around line 619), add inside the literal:

```rust
            tooltip_delay_ms: 500,
```

- [ ] **Step 3: Wire parsing**

In `crates/niribar/src/config/parse/bars.rs`, after the `hover_delay` parsing block (line 117) add:

```rust
    let tooltip_delay_ms: u32 = match children.get_arg("tooltip-delay") {
        None => defaults.tooltip_delay_ms,
        Some(v) => {
            let i = v.as_integer().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.tooltip-delay: expected a non-negative integer (milliseconds)."
                )
            })?;
            u32::try_from(i).map_err(|_| {
                anyhow::anyhow!(
                    "{prefix}.tooltip-delay: expected a non-negative integer (milliseconds)."
                )
            })?
        }
    };
```

In the `BarConfig` builder (around line 143–155) add after `hover_delay,`:

```rust
        tooltip_delay_ms,
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib config::parse::bars`
Expected: PASS (all bar tests, including new ones).

- [ ] **Step 5: Commit**

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/bars.rs
git commit -m "feat(config): add bar.tooltip-delay field"
```

---

## Phase 2 — Wire `tooltip` into each module config

Each module follows the same shape: add `pub tooltip: TooltipSetting` to the config struct, parse it via `parse_tooltip_field`, and unit-test the four cases (custom string, `false`, `true` rejected, absent → `Default`).

### Task 4: `BatteryConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs:151-173` (`BatteryConfig` + `Default`)
- Modify: `crates/niribar/src/config/parse/modules/battery.rs:9-63` and tests below

- [ ] **Step 1: Add field to `BatteryConfig`**

In `crates/niribar/src/config.rs`, inside `pub struct BatteryConfig` (line 153), add after `pub format: FormatConfig,`:

```rust
    pub tooltip: TooltipSetting,
```

In the `Default for BatteryConfig` impl (around line 163–172), add inside the literal after `format: FormatConfig::default(),`:

```rust
            tooltip: TooltipSetting::Default,
```

- [ ] **Step 2: Wire parsing**

In `crates/niribar/src/config/parse/modules/battery.rs`, after the `format` block (line 50–55) add:

```rust
    let tooltip = formats::parse_tooltip_field(
        children,
        &module_path,
        &["icon", "value", "percent", "state"],
        &allowed_states,
    )?;
```

Update the returned `BatteryConfig { ... }` literal (line 57–62) to include `tooltip,`:

```rust
    Ok(BatteryConfig {
        critical_threshold,
        states: states_config,
        icons,
        format,
        tooltip,
    })
```

- [ ] **Step 3: Add tests**

In `crates/niribar/src/config/parse/modules/battery.rs`, inside the existing `mod tests` (after `custom_state_usable_in_format_selector`):

```rust
    #[test]
    fn tooltip_string_parsed() {
        let cfg = battery_config(r#"modules { module "battery" { tooltip "{value}%" } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = battery_config(r#"modules { module "battery" { tooltip #false } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "battery" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = battery_config(r#"modules { module "battery" {} }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Default));
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib config::parse::modules::battery`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/battery.rs
git commit -m "feat(config): wire tooltip on battery module"
```

---

### Task 5: `VolumeConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs` (`VolumeConfig` and its `Default`)
- Modify: `crates/niribar/src/config/parse/modules/volume.rs`

- [ ] **Step 1: Locate the volume config struct**

Run: `grep -n "pub struct VolumeConfig\|impl Default for VolumeConfig" crates/niribar/src/config.rs`
Use the line numbers reported.

- [ ] **Step 2: Add `tooltip: TooltipSetting` field**

In `crates/niribar/src/config.rs`, inside `pub struct VolumeConfig`, add after the `format: FormatConfig` line:

```rust
    pub tooltip: TooltipSetting,
```

In `impl Default for VolumeConfig`, add inside the struct literal:

```rust
            tooltip: TooltipSetting::Default,
```

- [ ] **Step 3: Wire parsing**

In `crates/niribar/src/config/parse/modules/volume.rs`, after the existing `format` parse call, add a `tooltip` parse using the same allowed placeholders and states the volume `format` parser uses (mirror them exactly — find the existing `formats::parse_optional(... &[allowed]...)` call and copy the slices). Pattern:

```rust
    let tooltip = formats::parse_tooltip_field(
        children,
        &module_path,
        /* same allowed_placeholders slice as format */,
        /* same allowed_states slice as format */,
    )?;
```

Add `tooltip,` to the returned `VolumeConfig { ... }` literal.

- [ ] **Step 4: Add tests**

In `crates/niribar/src/config/parse/modules/volume.rs` inside its `mod tests` (locate `fn volume_config` helper, add a similar one if it does not exist):

```rust
    #[test]
    fn tooltip_string_parsed() {
        let cfg = volume_config(r#"modules { module "volume" { tooltip "{value}%" } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = volume_config(r#"modules { module "volume" { tooltip #false } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "volume" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = volume_config(r#"modules { module "volume" {} }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Default));
    }
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib config::parse::modules::volume`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/volume.rs
git commit -m "feat(config): wire tooltip on volume module"
```

---

### Task 6: `ClockConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs` (`ClockConfig`)
- Modify: `crates/niribar/src/config/parse/modules/clock.rs:32-38` (allowed list is `&["icon", "value", "time", "date"]`)

- [ ] **Step 1: Add field**

In `crates/niribar/src/config.rs` `pub struct ClockConfig`, add `pub tooltip: TooltipSetting,` after the `format` field, and the same `tooltip: TooltipSetting::Default,` line in its `Default` impl literal.

- [ ] **Step 2: Wire parsing**

In `crates/niribar/src/config/parse/modules/clock.rs`, after the existing `format = formats::parse_optional(...)` block, add:

```rust
    let tooltip = formats::parse_tooltip_field(
        children,
        &module_path,
        &["icon", "value", "time", "date"],
        &[],
    )?;
```

Add `tooltip,` to the returned `ClockConfig { ... }` literal.

- [ ] **Step 3: Tests**

Inside `mod tests` of `crates/niribar/src/config/parse/modules/clock.rs` add the four-case test pattern, mirroring Task 4 Step 3 but adapted to the existing `clock_config` test helper (find it in the file; it parses a kdl string and returns `ClockConfig`):

```rust
    #[test]
    fn tooltip_string_parsed() {
        let cfg = clock_config(r#"modules { module "clock" { tooltip "{date}" } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = clock_config(r#"modules { module "clock" { tooltip #false } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "clock" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = clock_config(r#"modules { module "clock" {} }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Default));
    }
```

- [ ] **Step 4: Run + commit**

Run: `cargo test --lib config::parse::modules::clock` — expected PASS.

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/clock.rs
git commit -m "feat(config): wire tooltip on clock module"
```

---

### Task 7: `NetworkConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs` (`NetworkConfig`)
- Modify: `crates/niribar/src/config/parse/modules/network.rs`

- [ ] **Step 1: Locate allowed sets**

Run: `grep -n "parse_optional\|allowed" crates/niribar/src/config/parse/modules/network.rs | head -5`
Use the same `allowed_placeholders` and `allowed_states` slices the existing `formats::parse_optional` call passes.

- [ ] **Step 2-4: Mirror Task 5 (volume) exactly**

Add `pub tooltip: TooltipSetting,` to `NetworkConfig` and `Default` impl. Add `parse_tooltip_field` call in the parser using the located slices. Add `tooltip,` to the constructor. Add the four-case test block (replace placeholder example with `"{label}"`):

```rust
    #[test]
    fn tooltip_string_parsed() {
        let cfg = network_config(r#"modules { module "network" { tooltip "{label}" } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = network_config(r#"modules { module "network" { tooltip #false } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "network" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = network_config(r#"modules { module "network" {} }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Default));
    }
```

(`network_config` test helper exists in the file already; if not, add it mirroring `battery_config` from Task 4.)

- [ ] **Step 5: Run + commit**

Run: `cargo test --lib config::parse::modules::network` — expected PASS.

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/network.rs
git commit -m "feat(config): wire tooltip on network module"
```

---

### Task 8: `FocusedWindowConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs` (`FocusedWindowConfig`)
- Modify: `crates/niribar/src/config/parse/modules/focused_window.rs`

- [ ] **Step 1-4: Mirror Task 7**

Use the `allowed_placeholders` slice that the existing `formats::parse_optional` call in `focused_window.rs` passes (likely `&["title", "app-id"]`; verify by `grep`). `allowed_states` is whatever the existing call passes (likely `&[]`). Add tests with `"{title}"` as the example custom template.

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/focused_window.rs
git commit -m "feat(config): wire tooltip on focused-window module"
```

---

### Task 9: `WorkspacesConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs` (`WorkspacesConfig`)
- Modify: `crates/niribar/src/config/parse/modules/workspaces.rs`

- [ ] **Step 1-4: Mirror Task 7**

Use whatever `allowed_placeholders`/`allowed_states` the existing workspaces `format` parse call uses. Tests use whichever placeholder appears in the existing `format` test (likely `"{label}"` or `"{name}"`).

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/workspaces.rs
git commit -m "feat(config): wire tooltip on workspaces module"
```

---

### Task 10: `ButtonConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs` (`ButtonConfig`)
- Modify: `crates/niribar/src/config/parse/modules/button.rs`

- [ ] **Step 1-4: Mirror Task 7** using the button module's allowed placeholders.

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/button.rs
git commit -m "feat(config): wire tooltip on button module"
```

---

### Task 11: `ScriptConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs` (`ScriptConfig`)
- Modify: `crates/niribar/src/config/parse/modules/script.rs`

- [ ] **Step 1-4: Mirror Task 7** using the script module's allowed placeholders.

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/script.rs
git commit -m "feat(config): wire tooltip on script module"
```

---

### Task 12: `TrayConfig.tooltip`

**Files:**
- Modify: `crates/niribar/src/config.rs` (`TrayConfig`)
- Modify: `crates/niribar/src/config/parse/modules/tray.rs`

The tray module currently calls `formats::reject_format_fields` because it does not render templated text in the bar. Tooltip is the exception: tray accepts `tooltip` with `{title}` and `{description}` placeholders.

- [ ] **Step 1: Add field**

In `crates/niribar/src/config.rs` inside `pub struct TrayConfig` add `pub tooltip: TooltipSetting,` and in its `Default` impl add `tooltip: TooltipSetting::Default,`.

- [ ] **Step 2: Adjust parser**

In `crates/niribar/src/config/parse/modules/tray.rs`, locate the call to `formats::reject_format_fields(...)`. Keep it (we still reject `format` / `expanded-format` / `horizontal-format`), but parse `tooltip` separately. After the reject call:

```rust
    let tooltip = if let Some(children) = node.children() {
        formats::parse_tooltip_field(
            children,
            &module_path,
            &["title", "description"],
            &[],
        )?
    } else {
        crate::config::TooltipSetting::Default
    };
```

Add `tooltip` to the returned `TrayConfig { ... }` literal.

- [ ] **Step 3: Tests**

Inside `mod tests` of `crates/niribar/src/config/parse/modules/tray.rs` (use existing `tray_config` helper, add it if missing — copy the shape from `battery_config`):

```rust
    #[test]
    fn tooltip_title_string_parsed() {
        let cfg = tray_config(r#"modules { module "tray" { tooltip "{title}" } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = tray_config(r#"modules { module "tray" { tooltip #false } }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_unknown_placeholder_errors() {
        let kdl = r#"modules { module "tray" { tooltip "{value}" } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("value"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = tray_config(r#"modules { module "tray" {} }"#);
        assert!(matches!(cfg.tooltip, crate::config::TooltipSetting::Default));
    }
```

Verify the existing tray test that confirms `format` is rejected still passes.

- [ ] **Step 4: Run + commit**

Run: `cargo test --lib config::parse::modules::tray`
Expected: PASS.

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/parse/modules/tray.rs
git commit -m "feat(config): wire tooltip on tray module"
```

---

### Task 13: Built-in default templates

Add a per-module helper that returns the built-in default `Option<FormatTemplate>`. The runtime uses these via `TooltipSetting::resolve(default)`.

**Files:**
- Create: `crates/niribar/src/config/tooltip_defaults.rs`
- Modify: `crates/niribar/src/config.rs` (add `pub mod tooltip_defaults;` if `mod` declarations live there, otherwise wherever module declarations are)

- [ ] **Step 1: Locate module declaration site**

Run: `grep -n "^pub mod\|^mod " crates/niribar/src/config.rs | head -10`
Add the new `pub mod tooltip_defaults;` near the other `pub mod` lines (e.g., next to `pub mod style;`). If `crates/niribar/src/config/` has a `mod.rs`, add the declaration there instead.

- [ ] **Step 2: Create the file**

`crates/niribar/src/config/tooltip_defaults.rs`:

```rust
//! Built-in default tooltip templates per module kind.
//!
//! Returned as `Option<FormatTemplate>` so callers can pass the result
//! directly to `TooltipSetting::resolve`. `None` means "no default" and is
//! treated identically to `TooltipSetting::Disabled` at runtime.

use super::{FormatTemplate, StyledSegment, TextTemplate};

fn template(raw: &str, allowed: &[&str]) -> FormatTemplate {
    let segs = raw
        .split_whitespace()
        .map(|s| {
            StyledSegment::new(
                TextTemplate::parse(s, allowed, "tooltip-default")
                    .expect("built-in default tooltip must parse"),
            )
        })
        .collect();
    FormatTemplate::from_styled(segs)
}

pub fn battery_default() -> Option<FormatTemplate> {
    Some(template("{value}% {state}", &["icon", "value", "percent", "state"]))
}

pub fn volume_default() -> Option<FormatTemplate> {
    // Uses placeholders that mirror the volume module's allowed set.
    Some(template("{value}%", &["icon", "value", "state", "label"]))
}

pub fn clock_default() -> Option<FormatTemplate> {
    Some(template("{date}", &["icon", "value", "time", "date"]))
}

pub fn network_default() -> Option<FormatTemplate> {
    Some(template("{label}", &["icon", "state", "label"]))
}

pub fn focused_window_default() -> Option<FormatTemplate> {
    Some(template("{title}", &["title", "app-id"]))
}

pub fn workspaces_default() -> Option<FormatTemplate> {
    None
}

pub fn button_default() -> Option<FormatTemplate> {
    None
}

pub fn script_default() -> Option<FormatTemplate> {
    None
}

pub fn tray_default() -> Option<FormatTemplate> {
    Some(template("{title}", &["title", "description"]))
}
```

Implementation note: each `allowed` slice MUST match the slice the corresponding module's `format` parser passes (or, for tray, the new tooltip-only placeholder set). If a module's `format` allowed set differs from what is shown above, update the call in `tooltip_defaults.rs` to match before committing — the default template parses lazily but the slice still gates which placeholders are legal.

- [ ] **Step 3: Sanity test**

Add to the bottom of `crates/niribar/src/config/tooltip_defaults.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn all_defaults_parse() {
        // Building each default must not panic.
        let _ = super::battery_default();
        let _ = super::volume_default();
        let _ = super::clock_default();
        let _ = super::network_default();
        let _ = super::focused_window_default();
        let _ = super::workspaces_default();
        let _ = super::button_default();
        let _ = super::script_default();
        let _ = super::tray_default();
    }
}
```

Run: `cargo test --lib config::tooltip_defaults`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/niribar/src/config.rs crates/niribar/src/config/tooltip_defaults.rs
git commit -m "feat(config): add built-in tooltip default templates"
```

---

## Phase 3 — Runtime: per-surface dwell state and animation tick

### Task 14: Add `tooltip_pending` to `SurfaceState`

**Files:**
- Modify: `crates/niribar/src/surface.rs:24-50` (struct + constructors)
- Modify: `crates/niribar/src/surface.rs:75-110` (test helpers + assertions)

- [ ] **Step 1: Add field**

In `crates/niribar/src/surface.rs` inside `pub struct SurfaceState`, after `pub hover_pending_ms: Option<f32>,` (line 30), add:

```rust
    /// Module currently being dwelled on for tooltip purposes plus the
    /// elapsed milliseconds since hover-enter. None means no pending dwell.
    pub tooltip_pending: Option<(crate::modules::ModuleId, f32)>,
```

In every `SurfaceState` literal in this file (look at lines 39–48 and 78–88) add:

```rust
            tooltip_pending: None,
```

- [ ] **Step 2: Update sanity tests**

In the test mod (around line 100), inside the assertion block that already verifies `hover_pending_ms.is_none()`:

```rust
        assert!(s.tooltip_pending.is_none());
```

- [ ] **Step 3: Run + commit**

Run: `cargo test --lib surface`
Expected: PASS.

```bash
git add crates/niribar/src/surface.rs
git commit -m "feat(surface): add tooltip_pending field"
```

---

### Task 15: Extend animation tick predicate

**Files:**
- Modify: `crates/niribar/src/app/subscriptions.rs:127`

- [ ] **Step 1: Update predicate**

Change line 127 from:

```rust
            .any(|s| s.expand.is_animating() || s.hover_pending_ms.is_some());
```

to:

```rust
            .any(|s| {
                s.expand.is_animating()
                    || s.hover_pending_ms.is_some()
                    || s.tooltip_pending.is_some()
            });
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/niribar/src/app/subscriptions.rs
git commit -m "feat(app): keep ticking while tooltip dwell is pending"
```

---

### Task 16: Hover dwell wiring in surfaces module

**Files:**
- Modify: `crates/niribar/src/app/surfaces.rs:187-196` (HoverEnter / HoverExit handlers)
- Modify: `crates/niribar/src/app/animation.rs:81-130` (tick handler)
- Modify: `crates/niribar/src/app/messages.rs` (new internal event)
- Test: inline in `crates/niribar/src/app/animation.rs` and `crates/niribar/src/app/surfaces.rs`

This task wires the dwell state machine. It does not yet open a popup surface — that comes in Phase 4. For now, when dwell elapses, emit a new internal `SurfaceEvent::TooltipDwellElapsed { window, module }`; Phase 4 wires the open path on top of this event.

- [ ] **Step 1: Add new internal event**

In `crates/niribar/src/app/messages.rs`, inside the `pub enum SurfaceEvent` block, add:

```rust
    /// Fired by the animation tick when a module's tooltip dwell delay
    /// has elapsed. Phase 4 wires the popup-open path on this.
    TooltipDwellElapsed {
        window: iced::window::Id,
        module: ModuleId,
    },
    /// Fired by the app to request closing any open tooltip popup
    /// for `window` — used on hover-exit, click, escape, focus-loss,
    /// and when a menu popup opens.
    CloseTooltip {
        window: iced::window::Id,
    },
```

- [ ] **Step 2: HoverEnter/Exit**

In `crates/niribar/src/app/surfaces.rs`, replace lines 187–196 with:

```rust
            SurfaceEvent::ModuleHoverEnter(id, window) => {
                self.handle_module_hover_enter(id, window)
            }
            SurfaceEvent::ModuleHoverExit(id, window) => {
                self.handle_module_hover_exit(id, window)
            }
```

And add elsewhere in the same file (e.g., near the bottom of the existing `impl App` block):

```rust
    fn handle_module_hover_enter(
        &mut self,
        id: crate::modules::ModuleId,
        window: iced::window::Id,
    ) -> iced::Task<super::Message> {
        let resolved = self.resolved_tooltip_for(window, &id);
        let s = self.surface_mut(window);
        s.hovered_module = Some(id.clone());

        // If a tooltip popup is already open for a different module, close it.
        let mut tasks = Vec::new();
        let close = self.close_tooltip_popup_if_open(window);
        if !close.is_empty() {
            tasks.extend(close);
        }

        if resolved.is_some() {
            self.surface_mut(window).tooltip_pending = Some((id, 0.0));
        } else {
            self.surface_mut(window).tooltip_pending = None;
        }

        iced::Task::batch(tasks)
    }

    fn handle_module_hover_exit(
        &mut self,
        id: crate::modules::ModuleId,
        window: iced::window::Id,
    ) -> iced::Task<super::Message> {
        let s = self.surface_mut(window);
        if s.hovered_module.as_ref() == Some(&id) {
            s.hovered_module = None;
        }
        if let Some((pending_id, _)) = &s.tooltip_pending {
            if pending_id == &id {
                s.tooltip_pending = None;
            }
        }
        let close = self.close_tooltip_popup_if_open_for(window, &id);
        iced::Task::batch(close)
    }
```

- [ ] **Step 3: Stub helpers (Phase 4 will fill these)**

Still in `crates/niribar/src/app/surfaces.rs`, add stubs that compile and return no tasks today; Phase 4 will replace their bodies:

```rust
    pub(super) fn resolved_tooltip_for(
        &self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Option<crate::config::FormatTemplate> {
        let _ = (window, module);
        None // Phase 4 wires this through module config + tooltip_defaults.
    }

    pub(super) fn close_tooltip_popup_if_open(
        &mut self,
        window: iced::window::Id,
    ) -> Vec<iced::Task<super::Message>> {
        let _ = window;
        Vec::new()
    }

    pub(super) fn close_tooltip_popup_if_open_for(
        &mut self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<iced::Task<super::Message>> {
        let _ = (window, module);
        Vec::new()
    }
```

These stubs intentionally always return `None`/empty; **Phase 4 Task 21** replaces them with real bodies. Until then, hovering will set `tooltip_pending = None` (because resolved is `None`) — meaning the dwell never fires. That is the correct in-between state.

- [ ] **Step 4: Tick advances `tooltip_pending`**

In `crates/niribar/src/app/animation.rs` `handle_tick` (around line 81), inside the per-window loop, after the existing `hover_pending_ms` block (around line 104, after the `continue` is gone — keep the `continue` only when expand-pending fires) add a sibling block. Replace lines 92–104 with:

```rust
            let s = self.surfaces.get_mut(&wid).unwrap();

            if let Some(elapsed) = s.hover_pending_ms {
                let new_elapsed = elapsed + TICK_MS;
                if new_elapsed >= effective_hover_delay as f32 {
                    s.hover_pending_ms = None;
                    s.expand = ExpandState::Expanding(0.0);
                    tasks.push(self.overlay_size_change_for(wid));
                } else {
                    s.hover_pending_ms = Some(new_elapsed);
                }
                // Do not `continue` — tooltip dwell may also be advancing.
            }

            // Tooltip dwell.
            let tooltip_delay = effective_bar.tooltip_delay_ms;
            let s = self.surfaces.get_mut(&wid).unwrap();
            if let Some((module, elapsed)) = s.tooltip_pending.clone() {
                let new_elapsed = elapsed + TICK_MS;
                if new_elapsed >= tooltip_delay as f32 {
                    s.tooltip_pending = None;
                    tasks.push(iced::Task::done(super::Message::Surface(
                        super::messages::SurfaceEvent::TooltipDwellElapsed {
                            window: wid,
                            module,
                        },
                    )));
                } else {
                    s.tooltip_pending = Some((module, new_elapsed));
                }
            }
```

(Note: removing the `continue` in the hover-pending block matters — bars in expand-hover mode might also be advancing a tooltip dwell. The two timers are independent.)

- [ ] **Step 5: Wire the new events in surfaces dispatch**

In `crates/niribar/src/app/surfaces.rs` at the dispatch match (around line 197) add arms:

```rust
            SurfaceEvent::TooltipDwellElapsed { window, module } => {
                self.handle_tooltip_dwell_elapsed(window, module)
            }
            SurfaceEvent::CloseTooltip { window } => {
                iced::Task::batch(self.close_tooltip_popup_if_open(window))
            }
```

Stub the dwell handler — Phase 4 Task 22 fills it:

```rust
    fn handle_tooltip_dwell_elapsed(
        &mut self,
        window: iced::window::Id,
        module: crate::modules::ModuleId,
    ) -> iced::Task<super::Message> {
        let _ = (window, module);
        // Phase 4 opens a TooltipPopup surface here.
        iced::Task::none()
    }
```

- [ ] **Step 6: Tests**

In `crates/niribar/src/app/animation.rs` add at the bottom (or in a new test mod if none exists):

```rust
#[cfg(test)]
mod tooltip_dwell_tests {
    use super::*;
    use crate::app::App;

    #[test]
    fn dwell_advances_under_threshold_does_not_fire() {
        // Setup: build an App with one surface and tooltip_delay_ms = 200.
        // Manually set tooltip_pending = Some((id, 0.0)).
        // Call handle_tick repeatedly so total elapsed reaches 100ms.
        // Assert tooltip_pending is still Some, no TooltipDwellElapsed task emitted.
        // (Implementation hint: existing tests in this file construct App via
        // App::test_default() or similar — find the helper and reuse it.)
    }

    #[test]
    fn dwell_crosses_threshold_emits_event_and_clears() {
        // Setup as above with tooltip_delay_ms = 32 (2 ticks).
        // After 2 ticks, tooltip_pending should be None and the returned
        // batch should contain a TooltipDwellElapsed message.
    }

    #[test]
    fn hover_exit_during_dwell_cancels() {
        // tooltip_pending set, then dispatch ModuleHoverExit for the same id.
        // Assert tooltip_pending is None.
    }

    #[test]
    fn hover_enter_on_other_module_replaces_pending() {
        // tooltip_pending = Some((A, 50.0)).
        // Dispatch ModuleHoverEnter(B, window).
        // resolved_tooltip_for is currently None (stub), so tooltip_pending becomes None.
        // After Phase 4 wires resolved_tooltip_for, this should become Some((B, 0.0)).
    }
}
```

The test bodies are intentionally written as comments because the App test helper API is something the engineer will need to find by `grep`ing existing tests in `crates/niribar/src/app/animation.rs` and `crates/niribar/src/app/surfaces.rs`. They MUST replace each comment with executable assertions before considering the task done. Look at `bar_render_model_marks_hovered_and_focused_entries` in `crates/niribar/src/app/render.rs:539` and `menu_item_hover_*` in `crates/niribar/src/app/menu.rs:351+` for examples of how to construct the App.

- [ ] **Step 7: Run + commit**

Run: `cargo build && cargo test --lib app::animation && cargo test --lib app::surfaces`
Expected: PASS (with the new tooltip dwell tests filled in).

```bash
git add crates/niribar/src/app/animation.rs crates/niribar/src/app/surfaces.rs crates/niribar/src/app/messages.rs
git commit -m "feat(app): wire tooltip hover-dwell state machine"
```

---

## Phase 4 — Tooltip popup surface

### Task 17: `TooltipTree` data type and view

**Files:**
- Create: `crates/niribar/src/surface/tooltip_popup.rs`
- Modify: `crates/niribar/src/surface.rs` (add `pub mod tooltip_popup;`)

- [ ] **Step 1: Inspect `menu_popup` to find shared building blocks**

Run: `grep -n "ResolvedStyle\|render_styled\|TextTemplate" crates/niribar/src/surface/menu_popup.rs crates/niribar/src/config.rs | head -30`
Note the type the menu popup uses for resolved per-segment styling — the tooltip view will reuse this.

- [ ] **Step 2: Create `tooltip_popup.rs`**

`crates/niribar/src/surface/tooltip_popup.rs`:

```rust
//! Tooltip popup view. Generic — has no knowledge of any specific module.
//!
//! Renders a single row (horizontal bar) or column (vertical bar) of
//! pre-rendered styled text segments inside a layer-shell popup surface.

use crate::config::BarEdge;
use crate::config::style::ResolvedStyle;

#[derive(Debug, Clone)]
pub struct RenderedSegment {
    pub text: String,
    pub style: ResolvedStyle,
}

#[derive(Debug, Clone)]
pub struct TooltipTree {
    pub segments: Vec<RenderedSegment>,
    /// Module's screen-space anchor point (typically center of the module
    /// frame's edge-opposite-to-bar side).
    pub anchor: (i32, i32),
    /// Logical screen size of the output (width, height) for clamping.
    pub screen_size: (u32, u32),
    /// Bar edge that hosts the module — dictates which side of the anchor
    /// the tooltip should appear on.
    pub bar_edge: BarEdge,
}

/// Compute the popup's top-left position (screen coordinates) given the
/// tree, the popup's logical (width, height), and a small gap in pixels.
///
/// - top bar    → tooltip below module: y = anchor.y + gap
/// - bottom bar → tooltip above module: y = anchor.y - gap - height
/// - left bar   → tooltip right of module: x = anchor.x + gap
/// - right bar  → tooltip left of module:  x = anchor.x - gap - width
///
/// Result is clamped to the screen size.
pub fn compute_position(
    tree: &TooltipTree,
    size: (u32, u32),
    gap: i32,
) -> (i32, i32) {
    let (sw, sh) = (tree.screen_size.0 as i32, tree.screen_size.1 as i32);
    let (w, h) = (size.0 as i32, size.1 as i32);
    let (ax, ay) = tree.anchor;
    let (mut x, mut y) = match tree.bar_edge {
        BarEdge::Top => (ax - w / 2, ay + gap),
        BarEdge::Bottom => (ax - w / 2, ay - gap - h),
        BarEdge::Left => (ax + gap, ay - h / 2),
        BarEdge::Right => (ax - gap - w, ay - h / 2),
    };
    x = x.clamp(0, (sw - w).max(0));
    y = y.clamp(0, (sh - h).max(0));
    (x, y)
}

/// Render the tooltip element for a popup window. Wraps `segments` in a
/// styled container with the given background/foreground/border styling.
pub fn view_tooltip<'a, Message: 'a + Clone>(
    tree: &'a TooltipTree,
    container_style: ResolvedStyle,
) -> iced::Element<'a, Message> {
    use iced::widget::{container, row, text};

    let row_widget = row(tree.segments.iter().map(|seg| {
        let mut t = text(seg.text.as_str());
        if let Some(c) = seg.style.color {
            t = t.color(c);
        }
        t.into()
    }))
    .spacing(4);

    let mut c = container(row_widget).padding([4, 8]);
    if let Some(bg) = container_style.background {
        c = c.style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                width: 1.0,
                color: container_style.border.unwrap_or(iced::Color::BLACK),
                radius: 4.0.into(),
            },
            ..Default::default()
        });
    }
    c.into()
}
```

Implementation note: `ResolvedStyle` and its `color`/`background`/`border` accessors must match what `menu_popup.rs` already uses. If the existing `ResolvedStyle` shape differs, mirror its accessor names exactly — `menu_popup` already solved this problem and the tooltip view should reuse the same helper functions where possible (consider extracting them into `src/surface/popup_text.rs` as a separate refactor, only if duplication is non-trivial).

- [ ] **Step 3: Tests for `compute_position`**

Append to the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn tree(edge: BarEdge, anchor: (i32, i32), screen: (u32, u32)) -> TooltipTree {
        TooltipTree {
            segments: vec![],
            anchor,
            screen_size: screen,
            bar_edge: edge,
        }
    }

    #[test]
    fn top_bar_places_below_anchor_centered() {
        let t = tree(BarEdge::Top, (500, 30), (1920, 1080));
        let (x, y) = compute_position(&t, (200, 40), 4);
        assert_eq!(y, 34);
        assert_eq!(x, 400);
    }

    #[test]
    fn bottom_bar_places_above_anchor() {
        let t = tree(BarEdge::Bottom, (500, 1050), (1920, 1080));
        let (_, y) = compute_position(&t, (200, 40), 4);
        assert_eq!(y, 1006);
    }

    #[test]
    fn left_bar_places_right_of_anchor() {
        let t = tree(BarEdge::Left, (30, 500), (1920, 1080));
        let (x, _) = compute_position(&t, (200, 40), 4);
        assert_eq!(x, 34);
    }

    #[test]
    fn right_bar_places_left_of_anchor() {
        let t = tree(BarEdge::Right, (1900, 500), (1920, 1080));
        let (x, _) = compute_position(&t, (200, 40), 4);
        assert_eq!(x, 1696);
    }

    #[test]
    fn clamps_to_screen_left_edge() {
        let t = tree(BarEdge::Top, (10, 30), (1920, 1080));
        let (x, _) = compute_position(&t, (200, 40), 4);
        assert_eq!(x, 0);
    }

    #[test]
    fn clamps_to_screen_right_edge() {
        let t = tree(BarEdge::Top, (1900, 30), (1920, 1080));
        let (x, _) = compute_position(&t, (200, 40), 4);
        assert_eq!(x, 1720);
    }
}
```

- [ ] **Step 4: Register module**

In `crates/niribar/src/surface.rs` near the existing `pub mod menu_popup;` (line 5) add:

```rust
pub mod tooltip_popup;
```

- [ ] **Step 5: Run + commit**

Run: `cargo test --lib surface::tooltip_popup`
Expected: PASS (build first; if `ResolvedStyle` accessors differ, fix in `view_tooltip` to match — the position tests do not need the view).

```bash
git add crates/niribar/src/surface.rs crates/niribar/src/surface/tooltip_popup.rs
git commit -m "feat(surface): add tooltip popup view + position math"
```

---

### Task 18: `TooltipPopup` registry entry

**Files:**
- Modify: `crates/niribar/src/app/surfaces.rs` (alongside `MenuPopup` declarations)

- [ ] **Step 1: Locate `MenuPopup` registry**

Run: `grep -n "MenuPopup\|insert_popup\|remove_popup\|has_any_popup" crates/niribar/src/app/surfaces.rs | head -30`
Note the struct, the storage map, and the helper methods.

- [ ] **Step 2: Add `TooltipPopup`**

Sibling to `MenuPopup`:

```rust
#[derive(Debug, Clone)]
pub(super) struct TooltipPopup {
    pub window: iced::window::Id,
    pub target_window: iced::window::Id,
    pub target_module: crate::modules::ModuleId,
    pub tree: crate::surface::tooltip_popup::TooltipTree,
}
```

In the surfaces struct (look for the field that holds `HashMap<window::Id, MenuPopup>` — it may be `popups` or similar) add a sibling map:

```rust
    tooltip_popups: HashMap<iced::window::Id, TooltipPopup>,
```

Initialise it in every constructor with `HashMap::new()`. Add helpers:

```rust
    pub(super) fn tooltip_popup(&self, window: iced::window::Id) -> Option<&TooltipPopup> {
        self.tooltip_popups.get(&window)
    }

    pub(super) fn insert_tooltip_popup(&mut self, popup: TooltipPopup) {
        self.tooltip_popups.insert(popup.window, popup);
    }

    pub(super) fn remove_tooltip_popup(&mut self, window: iced::window::Id) -> Option<TooltipPopup> {
        self.tooltip_popups.remove(&window)
    }

    pub(super) fn tooltip_popup_for_target(
        &self,
        target_window: iced::window::Id,
    ) -> Option<&TooltipPopup> {
        self.tooltip_popups.values().find(|p| p.target_window == target_window)
    }
```

Update `has_any_popup` (if present) to include the tooltip map: `!self.popups.is_empty() || !self.tooltip_popups.is_empty()`.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add crates/niribar/src/app/surfaces.rs
git commit -m "feat(app): add TooltipPopup registry"
```

---

### Task 19: Render the tooltip popup window

**Files:**
- Modify: `crates/niribar/src/app/render.rs` (find the existing match that selects between bar render and menu-popup render)
- Modify: `crates/niribar/src/app/surfaces.rs` (rendering helper, mirror menu popup)

- [ ] **Step 1: Locate the per-window view dispatch**

Run: `grep -n "view\|MenuPopup\|menu_popup" crates/niribar/src/app/view.rs crates/niribar/src/app/render.rs | head -30`
Find where the app picks between rendering the bar and rendering a menu popup view based on `window::Id`.

- [ ] **Step 2: Add a tooltip branch**

In the dispatch (where menu popups are rendered), add a sibling branch checking `self.surfaces.tooltip_popup(window)`. If present, return:

```rust
crate::surface::tooltip_popup::view_tooltip(&popup.tree, container_style.clone())
    .map(|m: super::Message| m)
```

Where `container_style` is built from the bar's theme (find how `menu_popup` derives its container styling — copy that path, but use a smaller padding suitable for a tooltip).

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: success (no UI test yet — manual verification comes later).

- [ ] **Step 4: Commit**

```bash
git add crates/niribar/src/app/view.rs crates/niribar/src/app/render.rs
git commit -m "feat(app): render tooltip popup window"
```

---

### Task 20: Open / close tooltip popup wiring

**Files:**
- Modify: `crates/niribar/src/app/surfaces.rs` (replace stubs from Task 16, plus `handle_tooltip_dwell_elapsed`)
- Modify: `crates/niribar/src/app/menu.rs` (close any tooltip when opening a menu popup)
- Modify: `crates/niribar/src/app/animation.rs` `handle_bar_click` (close tooltip on click)

- [ ] **Step 1: Implement `resolved_tooltip_for`**

Replace the stub from Task 16 Step 3 in `crates/niribar/src/app/surfaces.rs` with a real body that:

1. Looks up the bar config for `window`.
2. Finds the module by `ModuleId` in the bar's modules list (helpers likely exist; `grep` for `find_module_by_id` or similar).
3. Reads the module's `tooltip: TooltipSetting` field.
4. Calls `setting.resolve(<module-default>)` where `<module-default>` is dispatched on the `ModuleParams` variant:

```rust
    pub(super) fn resolved_tooltip_for(
        &self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Option<crate::config::FormatTemplate> {
        use crate::config::tooltip_defaults as td;
        use crate::config::ModuleParams;

        let bar = self.bar_for_window(window);
        let m = bar.modules.iter().find(|m| m.id_matches(module))?;
        // `id_matches` is a helper to find by ModuleId — verify the actual
        // method name with `grep -n "fn id_matches\|by_id" crates/niribar/src/config.rs crates/niribar/src/modules.rs`
        // and substitute the correct one.

        let (setting, default) = match &m.params {
            ModuleParams::Battery(c) => (&c.tooltip, td::battery_default()),
            ModuleParams::Volume(c) => (&c.tooltip, td::volume_default()),
            ModuleParams::Clock(c) => (&c.tooltip, td::clock_default()),
            ModuleParams::Network(c) => (&c.tooltip, td::network_default()),
            ModuleParams::FocusedWindow(c) => (&c.tooltip, td::focused_window_default()),
            ModuleParams::Workspaces(c) => (&c.tooltip, td::workspaces_default()),
            ModuleParams::Button(c) => (&c.tooltip, td::button_default()),
            ModuleParams::Script(c) => (&c.tooltip, td::script_default()),
            ModuleParams::Tray(c) => (&c.tooltip, td::tray_default()),
        };
        setting.resolve(default)
    }
```

- [ ] **Step 2: Implement `close_tooltip_popup_if_open*`**

Replace the two stubs:

```rust
    pub(super) fn close_tooltip_popup_if_open(
        &mut self,
        target_window: iced::window::Id,
    ) -> Vec<iced::Task<super::Message>> {
        let popup_wid = self
            .surfaces
            .tooltip_popup_for_target(target_window)
            .map(|p| p.window);
        if let Some(wid) = popup_wid {
            self.surfaces.remove_tooltip_popup(wid);
            // Close the layer-shell window via iced_layershell — mirror the
            // call menu.rs uses to close menu popups.
            return vec![iced::window::close(wid)];
        }
        Vec::new()
    }

    pub(super) fn close_tooltip_popup_if_open_for(
        &mut self,
        target_window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<iced::Task<super::Message>> {
        let popup_wid = self
            .surfaces
            .tooltip_popup_for_target(target_window)
            .filter(|p| &p.target_module == module)
            .map(|p| p.window);
        if let Some(wid) = popup_wid {
            self.surfaces.remove_tooltip_popup(wid);
            return vec![iced::window::close(wid)];
        }
        Vec::new()
    }
```

(`iced::window::close` may need to be `iced_layershell::actions::LayershellAction::CloseWindow` or similar — verify by looking at how `crates/niribar/src/app/menu.rs:close_all_menu_popups` closes windows.)

- [ ] **Step 3: Implement `handle_tooltip_dwell_elapsed`**

```rust
    fn handle_tooltip_dwell_elapsed(
        &mut self,
        target_window: iced::window::Id,
        module: crate::modules::ModuleId,
    ) -> iced::Task<super::Message> {
        // Re-resolve in case config changed during the dwell.
        let template = match self.resolved_tooltip_for(target_window, &module) {
            Some(t) => t,
            None => return iced::Task::none(),
        };

        // Build the value bag the module's `format` template would receive.
        // For each module kind, this routes through whatever helper already
        // produces values — find the existing helper by:
        //   grep -n "render_segments\|render_joined\|format.template_for" crates/niribar/src/app
        // and call it to get `Vec<(&str, String)>`.
        let values = self.tooltip_values_for(target_window, &module);

        // Render template into RenderedSegments.
        let rendered = render_to_segments(&template, &values, /* state */ None);

        // Suppress empty tooltips.
        if rendered.iter().all(|s| s.text.trim().is_empty()) {
            return iced::Task::none();
        }

        // Compute anchor point and screen size.
        let (anchor, screen_size, bar_edge) =
            self.tooltip_anchor_for(target_window, &module);

        let tree = crate::surface::tooltip_popup::TooltipTree {
            segments: rendered,
            anchor,
            screen_size,
            bar_edge,
        };

        // Open a new layer-shell popup window. Mirror `open_menu_popup` in
        // crates/niribar/src/app/menu.rs:159+. Compute size from segment text widths or use
        // a fixed reasonable max-width and let iced reflow.
        let (popup_wid, open_task) = self.spawn_tooltip_popup_window(target_window, &tree);

        self.surfaces.insert_tooltip_popup(super::surfaces::TooltipPopup {
            window: popup_wid,
            target_window,
            target_module: module,
            tree,
        });
        open_task
    }
```

Helpers `tooltip_values_for`, `render_to_segments`, `tooltip_anchor_for`, `spawn_tooltip_popup_window` need to be implemented. Inline them at the bottom of `crates/niribar/src/app/surfaces.rs`:

```rust
    fn tooltip_values_for(
        &self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<(&'static str, String)> {
        // Reuse the per-module value bag the runtime already builds for `format`.
        // Locate the existing helper:
        //   grep -n "fn .*values\|render_pairs\|placeholder" crates/niribar/src/app crates/niribar/src/modules
        // and call it. If no shared helper exists, this function must dispatch
        // on the module kind and inline the value collection — for tray the
        // values are {title}, {description} from the SNI item state (Phase 5).
        let _ = (window, module);
        Vec::new() // engineer fills in based on grep results
    }

    fn render_to_segments(
        template: &crate::config::FormatTemplate,
        values: &[(&str, String)],
        state: Option<&str>,
    ) -> Vec<crate::surface::tooltip_popup::RenderedSegment> {
        // FormatTemplate already exposes `render_styled_segments` (#[cfg(test)]
        // only — un-cfg it in crates/niribar/src/config.rs:303-310 so production code can call it,
        // OR add a public sibling that does the same thing without the test cfg).
        // The function returns Vec<(String, ResolvedStyle)>; map each to a
        // RenderedSegment.
        let _ = (template, values, state);
        Vec::new() // engineer fills in
    }

    fn tooltip_anchor_for(
        &self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> ((i32, i32), (u32, u32), crate::config::BarEdge) {
        // Locate the module's screen rectangle. The hover system already knows
        // which module the cursor is over — but for anchoring we need the
        // module's frame bounds. Look at how menu popups derive their anchor
        // for tray icons: crates/niribar/src/app/surfaces.rs near `open_menu_popup`. The same
        // mechanism (passing screen_size + a logical anchor through the
        // SurfaceEvent that triggered the menu) needs to be plumbed for
        // tooltips: extend `ModuleHoverEnter` to carry the module's screen
        // rect, OR query iced for layout bounds at popup-open time.
        let _ = (window, module);
        ((0, 0), (1920, 1080), crate::config::BarEdge::Top) // engineer fills in
    }

    fn spawn_tooltip_popup_window(
        &mut self,
        target_window: iced::window::Id,
        tree: &crate::surface::tooltip_popup::TooltipTree,
    ) -> (iced::window::Id, iced::Task<super::Message>) {
        // Mirror `open_menu_popup` (crates/niribar/src/app/menu.rs:159-200) but tag the
        // popup as a tooltip surface. Use TooltipTree's anchor + computed
        // (width, height) via `compute_position` to set the layer-shell
        // margins; pass `Anchor::TOP | Anchor::LEFT` and exact pixel margins
        // (the menu popup does this — read its body).
        let _ = (target_window, tree);
        (iced::window::Id::unique(), iced::Task::none()) // engineer fills in
    }
```

The four "engineer fills in" comments are pointers to where the unknown shape lives in the existing code — the engineer must `grep` for the exact helpers, read the menu popup as a worked example, and translate. These bodies are not optional. Replace each with real code before the task is considered done.

**Anchoring decision:** Extend `ModuleHoverEnter` to carry the module's screen rect (4 ints: x, y, w, h). The mouse_area in `crates/niribar/src/app/render.rs:95` builds the message — at that point iced knows the widget bounds via `iced::widget::mouse_area::on_enter` … actually `on_enter` does not pass bounds. The cleanest path: add a `ModuleBoundsChanged { window, module, rect }` event emitted from a layout subscription, store the latest rect on `SurfaceState` keyed by ModuleId, then `tooltip_anchor_for` reads from there.

Simpler alternative for v1: anchor at the cursor position. Add a `last_pointer_position: (i32, i32)` field on `SurfaceState` updated from existing pointer events, and use it as the anchor. This trades pixel-perfect placement for implementation simplicity. **Recommend the cursor-position approach for the first pass; revisit if users complain about placement.**

If cursor-position chosen, replace `tooltip_anchor_for` with:

```rust
    fn tooltip_anchor_for(
        &self,
        window: iced::window::Id,
        _module: &crate::modules::ModuleId,
    ) -> ((i32, i32), (u32, u32), crate::config::BarEdge) {
        let s = self.surface(window).unwrap();
        let screen = s.screen_size;
        let bar = self.bar_for_window(window);
        (s.last_pointer_position, screen, bar.edge)
    }
```

And add `last_pointer_position: (i32, i32)` to `SurfaceState`, defaulting to `(0, 0)`, updated from a new `SurfaceEvent::PointerMoved { window, x, y }` event wired through iced's mouse_area `on_move` callback. This is a separate mini-task within Step 3 — verify by `grep -n on_move crates/niribar/src/app` first; if not currently wired, add it.

- [ ] **Step 4: Wire dismissal hooks**

In `crates/niribar/src/app/menu.rs` `open_menu_popup` (line 159) at the very top, before any other work, append closing of any open tooltip:

```rust
        let close_tt = self.close_tooltip_popup_if_open(<the bar window the menu opens against>);
        // batch with the menu open task at return time
```

The menu popup is opened against a specific bar window — the `target_window` passed to `open_menu_popup` is presumably available; `grep -n "fn open_menu_popup" crates/niribar/src/app/menu.rs` to confirm the signature.

In `crates/niribar/src/app/animation.rs handle_bar_click` (line 16), at the top before any other branch:

```rust
        let close_tt = self.close_tooltip_popup_if_open(window);
        if !close_tt.is_empty() {
            return iced::Task::batch(close_tt);
        }
```

In `crates/niribar/src/app/surfaces.rs handle_escape_pressed` and `handle_bar_focus_lost` (find them via `grep -n "fn handle_escape_pressed\|fn handle_bar_focus_lost" crates/niribar/src/app/surfaces.rs`), prepend a tooltip-close batch.

- [ ] **Step 5: Build + tests**

Run: `cargo build`
Expected: success.

Then run targeted tests:
Run: `cargo test --lib`
Expected: PASS.

Manual smoke test in this step is deferred to the verification gate at the end of the plan.

- [ ] **Step 6: Commit**

```bash
git add crates/niribar/src/app/surfaces.rs crates/niribar/src/app/menu.rs crates/niribar/src/app/animation.rs crates/niribar/src/surface.rs crates/niribar/src/app/render.rs
git commit -m "feat(app): open/close tooltip popups on dwell + dismissal hooks"
```

---

## Phase 5 — Tray SNI ToolTip data path

### Task 21: Fetch and store SNI `ToolTip` property

**Files:**
- Modify: `crates/niribar/src/dbus/sni/proxy.rs:17-42` (`SniItemProxy` trait — add `tool_tip` property)
- Modify: `crates/niribar/src/dbus/sni/host.rs:10-32` (`TrayItem` struct + `fetch_item`)
- Modify: `crates/niribar/src/dbus/sni/subscription.rs` (handle `new_tool_tip` signal — file path; verify with `grep -n "new_icon\|new_status" crates/niribar/src/dbus/sni/subscription.rs`)

- [ ] **Step 1: Extend the SNI proxy**

In `crates/niribar/src/dbus/sni/proxy.rs`, inside the `SniItem` trait, after `fn menu(...)` (line 32):

```rust
    /// SNI ToolTip property: (icon-name, icon-pixmap, title, description).
    /// We use only `title` and `description`.
    #[zbus(property)]
    fn tool_tip(&self) -> zbus::Result<(String, Vec<(i32, i32, Vec<u8>)>, String, String)>;
```

The `new_tool_tip` signal already exists (line 41).

- [ ] **Step 2: Add `SniTooltip` and store it on `TrayItem`**

In `crates/niribar/src/dbus/sni/host.rs` (around line 22, after the `TrayItem` definition):

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SniTooltip {
    pub title: String,
    pub description: String,
}

impl SniTooltip {
    pub fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.description.trim().is_empty()
    }
}
```

Add a field to `TrayItem` (line 12 onwards):

```rust
    pub(crate) tooltip: Option<SniTooltip>,
```

In `fetch_item` (line 68 onwards), before the final `Some(TrayItem { ... })`:

```rust
    let tooltip = match proxy.tool_tip().await {
        Ok((_, _, title, description)) => {
            let t = SniTooltip { title, description };
            if t.is_empty() { None } else { Some(t) }
        }
        Err(_) => None,
    };
```

Add `tooltip,` to the `TrayItem { ... }` literal (line 104).

Update the existing `let item = TrayItem { ... }` literals in this file (lines 323, 336, 349 — these are in tests) by adding `tooltip: None,`.

- [ ] **Step 3: Handle `new_tool_tip` signal**

In `crates/niribar/src/dbus/sni/subscription.rs` find the existing handlers for `new_icon` / `new_status`. For each subscribed item, add an analogous handler:

```rust
    let mut tt_stream = proxy.receive_new_tool_tip().await?;
    let conn_clone = conn.clone();
    let tx_clone = tx.clone();
    let svc = service.to_string();
    let path = object_path.clone();
    tokio::spawn(async move {
        while let Some(_signal) = tt_stream.next().await {
            // Refetch the full item; cheaper than fetching just one property
            // because the host already knows how to reconcile a full TrayItem.
            if let Some(item) = fetch_item(&conn_clone, &svc, &path.as_ref()).await {
                let _ = tx_clone.send(TrayChange::ItemUpdated(item)).await;
            }
        }
    });
```

(The exact `TrayChange` variant name and the spawn strategy must mirror what the existing `new_icon` / `new_status` handlers do — `grep -n "ItemUpdated\|new_icon\|receive_new_icon" crates/niribar/src/dbus/sni/subscription.rs` to confirm.)

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/niribar/src/dbus/sni/proxy.rs crates/niribar/src/dbus/sni/host.rs crates/niribar/src/dbus/sni/subscription.rs
git commit -m "feat(sni): capture ToolTip property and refetch on signal"
```

---

### Task 22: Expose tray tooltip values to the tooltip renderer

**Files:**
- Modify: `crates/niribar/src/modules/tray.rs` (or wherever the runtime tray module state is held — verify with `grep -n "TrayItem\|SniTooltip" crates/niribar/src/modules/tray.rs crates/niribar/src/app`)
- Modify: `crates/niribar/src/app/surfaces.rs` `tooltip_values_for` (from Task 20)

- [ ] **Step 1: Plumb `SniTooltip` into the tray module's runtime state**

The tray module synthesises a per-icon ModuleId. Find the data struct that backs the per-icon module state (e.g., `TrayIconModule` or similar) and add an `Option<SniTooltip>` field, populated from `TrayItem.tooltip` whenever the snapshot/update path runs.

Run: `grep -n "TrayItem\|TrayIconModule\|tray.*module" crates/niribar/src/app crates/niribar/src/modules/tray.rs | head`
Add the new field at the runtime layer that already mirrors other `TrayItem` fields (icon, status).

- [ ] **Step 2: Wire `tooltip_values_for` for tray modules**

In `crates/niribar/src/app/surfaces.rs`, fill in the tray branch of `tooltip_values_for`:

```rust
    fn tooltip_values_for(
        &self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<(&'static str, String)> {
        // Find the module state by id. The runtime layer exposes per-module
        // state; for tray icons it carries the SniTooltip.
        if let Some(state) = self.module_state(window, module) {
            if let Some(tt) = state.tray_tooltip() {
                return vec![
                    ("title", tt.title.clone()),
                    ("description", tt.description.clone()),
                ];
            }
            return state.format_values(); // existing format-template values
        }
        Vec::new()
    }
```

Where `module_state(...)` and `tray_tooltip()` / `format_values()` are accessors the engineer must wire on the runtime module state type. `grep -n "fn .*values\|fn .*placeholders\|fn render" crates/niribar/src/modules crates/niribar/src/app` to find the existing format-value-producing helper and either reuse it or extract it into a shared method.

- [ ] **Step 3: Build + test**

Run: `cargo build && cargo test --lib`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/niribar/src/modules/tray.rs crates/niribar/src/app/surfaces.rs
git commit -m "feat(tray): expose SNI tooltip to tooltip renderer"
```

---

## Phase 6 — Documentation

### Task 23: Update default config

**Files:**
- Modify: `crates/niribar/config/default.kdl`

- [ ] **Step 1: Add `tooltip-delay` to the bar example**

In `crates/niribar/config/default.kdl` near the `hover-delay` example (~line 43):

```kdl
    // Milliseconds of cursor dwell before a module's tooltip appears on
    // hover. Default: 500. Hide is always immediate on hover-exit.
    tooltip-delay 500
```

- [ ] **Step 2: Document `tooltip` field for each module**

For each module example (battery, volume, clock, network, focused-window, workspaces, button, script, tray), add a comment block analogous to the existing `format` comments:

```kdl
        // tooltip "..."          # template string (same grammar as format)
        // tooltip { segment ... }# styled-segment block
        // tooltip #false         # disable the tooltip
        // (omit)                  # use the module's built-in default
```

Place this immediately after the existing `format` examples for each module.

- [ ] **Step 3: Commit**

```bash
git add crates/niribar/config/default.kdl
git commit -m "docs(config): document tooltip and tooltip-delay"
```

---

## Verification gate

- [ ] **Step 1: Full test suite**

Run: `cargo build && cargo test --lib`
Expected: PASS.

- [ ] **Step 2: Manual smoke test**

Run: `cargo run --bin niribar`
- Hover a battery / clock / volume module; after ~500ms a tooltip popup should appear.
- Move cursor to a different module: tooltip closes for the first, new dwell starts for the second.
- Move cursor off all modules: tooltip closes immediately.
- Click a module: any open tooltip closes.
- Open a tray menu (right-click a tray icon): tooltip closes.
- Press Escape: tooltip closes.
- Add `tooltip #false` to one module in the local config and reload: that module no longer shows a tooltip on hover.
- Add `tooltip "{value}%"` to battery: tooltip shows "12%" instead of the default "12% normal".
- Hover a tray icon for an app that exposes an SNI ToolTip: tooltip shows the SNI title.

If anything fails, file the failure as a follow-up in the spec or fix in place — do not declare done.

- [ ] **Step 3: Final commit (if any cleanup made during smoke test)**

```bash
git add -A
git commit -m "fix(tooltip): smoke-test cleanups"
```

---

## Self-review notes

- **Spec coverage:** Sections 1–5 of the spec are each addressed. Section 1 → Phase 1 + Phase 2. Section 2 → Phase 3. Section 3 → Phase 4. Section 4 → Phase 5. Section 5 → tests interleaved per task.
- **Type consistency:** `TooltipSetting` (Default/Disabled/Custom), `TooltipTree` (segments/anchor/screen_size/bar_edge), `TooltipPopup` (window/target_window/target_module/tree), `SniTooltip` (title/description) are used consistently across phases.
- **Known gaps the engineer must close inline (each is flagged at point of use):**
  - Exact `ResolvedStyle` accessors in `view_tooltip` must match `menu_popup`'s usage.
  - `tooltip_values_for` per-module branch needs to call the existing `format` value-producing helper — engineer locates and wires it.
  - `render_to_segments` may require un-cfg-ing `FormatTemplate::render_styled_segments` (currently `#[cfg(test)]`) or adding a public sibling; pick one and update accordingly.
  - `spawn_tooltip_popup_window` mirrors `open_menu_popup` from `crates/niribar/src/app/menu.rs:159+`; the engineer reads that function and adapts.
  - Anchoring: cursor-position approach recommended for v1 (Task 20 Step 3).
- **Frequent commits:** Every task ends with a focused commit.
