# Module Format Templates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `format`, `expanded-format`, and `horizontal-format` templates for every non-tray module, with `{icon}` and module-specific placeholders.

**Architecture:** `src/config.rs` owns parsed `FormatConfig` and `TextTemplate` matching. A shared parser helper reads the three format fields with per-module allowed placeholder lists. Rendering code passes placeholder maps into templates per render mode and falls back to current hard-coded output when no template is configured.

**Tech Stack:** Rust, KDL config parser, existing module trait/render helpers, `cargo test`.

---

## File Structure

- Modify `src/config.rs`: add `FormatConfig`, `FormatMode`, `TextTemplate`, and fields on all non-tray module configs.
- Create `src/config/parse/modules/formats.rs`: parse `format`, `expanded-format`, `horizontal-format`.
- Modify module parsers under `src/config/parse/modules/`: call format parser for every non-tray module and reject formats for tray.
- Modify `src/modules/view.rs`: add a small helper for rendering formatted icon/text strings without changing existing layout when no format is present.
- Modify module files under `src/modules/`: thread `FormatConfig` into constructors and render using placeholders.
- Modify `src/modules.rs`: pass format config through constructors and synthesized defaults.
- Modify `config/default.kdl`: document commented examples.

## Task 1: Template Engine

- [ ] Write tests in `src/config.rs` for substitution, repeated placeholders, escaped braces, unmatched braces, empty placeholders, and disallowed placeholders.
- [ ] Run `rtk cargo test config::format_template_tests` and verify it fails because the types do not exist.
- [ ] Implement `FormatConfig`, `FormatMode`, `TextTemplate::parse`, `TextTemplate::render`, and `FormatConfig::template_for`.
- [ ] Run `rtk cargo test config::format_template_tests`; expect PASS.

## Task 2: Parser Wiring

- [ ] Add parser tests in `src/config/parse/tests.rs` for `format`, `expanded-format`, and `horizontal-format` on battery, volume, network, clock, focused-window, script, button, and workspaces.
- [ ] Add parser tests rejecting `format` on tray and rejecting an unknown placeholder for battery.
- [ ] Run `rtk cargo test config::parse::tests::parse_battery_formats`; expect FAIL before parser implementation.
- [ ] Create `src/config/parse/modules/formats.rs` and wire it into each non-tray parser.
- [ ] Run `rtk cargo test config::parse::tests`; expect PASS.

## Task 3: IconLabel Modules

- [ ] Add tests for battery, volume, and network formatted labels using `{icon}` and values.
- [ ] Run targeted module tests and verify they fail before integration.
- [ ] Thread `FormatConfig` into battery, volume, and network modules and render labels via `format`/`expanded-format`/`horizontal-format`.
- [ ] Run `rtk cargo test modules::battery::tests modules::volume::tests modules::network::tests` as separate commands; expect PASS.

## Task 4: Custom Layout Modules

- [ ] Add tests for clock, script, button, focused-window, and workspace placeholders.
- [ ] Run targeted module tests and verify they fail before integration.
- [ ] Thread `FormatConfig` into clock, script, button, focused-window, and workspaces, applying render-mode fallback.
- [ ] Run targeted tests for those modules; expect PASS.

## Task 5: Docs and Verification

- [ ] Add commented examples to `config/default.kdl`.
- [ ] Run `rtk cargo fmt --check`; if it fails, run `rtk cargo fmt` then re-run check.
- [ ] Run `rtk cargo test`; expect full suite PASS.
- [ ] Commit with `rtk git commit -m "feat: add module format templates"`.

## Self-Review

Spec coverage: tasks cover all three fields, `{icon}`, module-specific placeholders, tray exclusion, parsing validation, default compatibility, and verification.

Placeholder scan: no implementation placeholders remain in this plan.

Type consistency: plan consistently uses `FormatConfig`, `FormatMode`, and `TextTemplate`.
