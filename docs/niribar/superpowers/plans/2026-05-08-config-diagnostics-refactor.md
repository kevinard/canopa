# Config Diagnostics Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the config parser easier to maintain while giving users actionable, source-aware config errors.

**Architecture:** Introduce first-class config diagnostics and a small span-aware parser helper layer. Keep module parsers hand-written, but centralize path construction, expected-type errors, source spans, help text, and closed-vocabulary suggestions. Prove the design with a vertical slice through `script` configuration before converting the rest of the config surface.

**Tech Stack:** Rust 2024, `kdl` 6.5, direct `miette` dependency, direct `thiserror` dependency, existing `anyhow` boundary for non-config app errors.

---

## Agreed Scope

- Add first-class `ConfigError` / `ConfigDiagnostic` types instead of flattening config failures into `anyhow` strings.
- Add `miette` and `thiserror` as direct dependencies.
- Split diagnostics into `crates/niribar/src/config/error.rs`.
- Split validation into `crates/niribar/src/config/validate.rs`.
- Add an immutable/lightweight `ParseCtx` that creates child contexts for paths and builds diagnostics.
- Render config diagnostics with plain/default `miette` formatting at the CLI boundary.
- Provide rich spans for KDL syntax errors and local semantic parse errors.
- Keep validation errors in the same error family, but do not require full source-origin metadata for cross-reference validation in the first pass.
- Keep module parsers hand-written; add typed helpers for repeated field patterns.
- Add strict unknown-field checks only in parser areas converted to the new helper layer.
- In converted parser areas, validate allowed child nodes, allowed named properties, and allowed positional argument counts/types.
- In converted parser areas, reject duplicate scalar fields. Put the primary label on the second occurrence; add a secondary label for the first occurrence if cheap.
- Add basic typo suggestions for closed vocabularies only.
- Use a vertical slice through `script` config first.
- Include command-policy rejection in the first slice when it can point at the offending local command node.
- Parse `command-policy` before modules so command parsing can use the policy while local source spans are still available.
- Convert `command-policy` local errors in the first slice.
- Convert shared command/action parsing in the first slice because `script` supports `actions` and command-policy can reject shared action commands.
- Preserve the actual config source name in diagnostics, including the embedded default fallback.

## Explicit Non-Goals For First Slice

- No warnings.
- No multi-error accumulation.
- No `niribar --check-config`.
- No `--config` path override.
- No full config model origin tracking.
- No broad mechanical conversion of every parser area.
- No conversion of `FormatTemplate` / `TextTemplate` diagnostics until the next slice.
- No global strict unknown-field enforcement until each parser area is reviewed.
- No `button`-specific parser conversion in the first slice, even though `button` uses shared command parsing.

## Diagnostic Shape

Tests should assert diagnostic structure rather than snapshotting full terminal output:

- Error kind: IO, syntax parse, semantic parse, or validation.
- Stable user-facing config path, for example:
  - `modules[0].script.command`
  - `modules[0].script.command.argv`
  - `bar["main"].collapsed-width`
  - `command-policy.allow-shell`
- Key message phrase.
- Help text for common mistakes.
- Span existence for syntax errors and converted local semantic parse errors.

Module paths remain index-based for stability. Diagnostic messages may include the user-provided module name for recognition.

## Task 1: Add First-Class Config Error Boundary

**Files:**
- Create: `crates/niribar/src/config/error.rs`
- Modify: `crates/niribar/src/config.rs`
- Modify: `crates/niribar/src/main.rs`
- Modify: `Cargo.toml`
- Test: config error unit tests or focused parser tests

- [ ] Add direct dependencies on `miette` and `thiserror`.
- [ ] Introduce `ConfigError`, `ConfigDiagnostic`, and a config result alias.
- [ ] Model the loaded source as user file vs embedded default.
- [ ] Change config loading to preserve source name/content for diagnostics.
- [ ] Change `main` so config errors render through default/plain `miette`, while non-config app errors keep the current generic error path.
- [ ] Add tests for config source labelling and error type preservation.

## Task 2: Extract Validation

**Files:**
- Create: `crates/niribar/src/config/validate.rs`
- Modify: `crates/niribar/src/config.rs`
- Test: existing config validation tests

- [ ] Move `validate()` out of `crates/niribar/src/config.rs`.
- [ ] Convert validation failures to the new config error family.
- [ ] Keep validation diagnostics path/help-rich even when no source span is available.
- [ ] Preserve existing validation behavior for valid documented configs and the shipped default config.

## Task 3: Add Parse Context And Script Vertical Slice

**Files:**
- Create or modify: `crates/niribar/src/config/parse/error.rs`
- Modify: `crates/niribar/src/config/parse/mod.rs`
- Modify: `src/config/parse/kdl.rs`
- Modify: `crates/niribar/src/config/parse/actions.rs`
- Modify: `crates/niribar/src/config/parse/modules/script.rs`
- Test: `crates/niribar/src/config/parse/tests.rs`, `crates/niribar/src/config/parse/modules/script.rs`

- [ ] Add immutable `ParseCtx` child-path helpers.
- [ ] Add helper methods for required strings, optional bounded integers, command nodes, and unknown child checks.
- [ ] Add helper methods that distinguish missing values from wrong-type values.
- [ ] Add helper methods for duplicate scalar fields and unknown named properties.
- [ ] Preserve KDL syntax diagnostics with source labels and spans.
- [ ] Parse `command-policy` before modules.
- [ ] Convert `command-policy` local diagnostics, including unknown fields and duplicate scalar fields.
- [ ] Pass a narrow command-policy context into module/shared command parsing.
- [ ] Convert `script` parser errors to first-class diagnostics.
- [ ] Add strict unknown-field checks inside `module "script" { ... }`.
- [ ] Include all currently valid script children in strict checks: `type`, `command`, `interval`, `actions`, `format`, `expanded-format`, `horizontal-format`, and `tooltip`.
- [ ] Keep matching explicit `type "script"` valid for built-in script modules.
- [ ] Add closed-vocabulary suggestions for converted script fields.
- [ ] Make command-policy rejection span-aware for script command nodes.
- [ ] Convert shared `actions` / command-node parsing so script actions use the same diagnostic path.
- [ ] Keep old string-based tests outside the converted parser area working until those areas are migrated.

## Task 4: Next Parser Slice

**Files:**
- Modify: `crates/niribar/src/config.rs`
- Modify: `crates/niribar/src/config/parse/modules/formats.rs`
- Modify: `crates/niribar/src/config/parse/modules/segment_styles.rs`
- Modify module parsers that use format/tooltip helpers
- Test: format, tooltip, placeholder, and styled segment tests

- [ ] Convert `FormatTemplate` / `TextTemplate` errors to diagnostics.
- [ ] Make placeholder errors point at the relevant format or segment node.
- [ ] Add suggestions for closed placeholder vocabularies.
- [ ] Convert tooltip-specific diagnostics after format diagnostics are stable.

## Verification Gate

Run after each task that compiles:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```
