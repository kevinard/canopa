# Deferred Work

## Deferred from: code review of 1-1-initialize-cargo-project-with-pinned-dependencies (2026-03-29)

- **No `#[tokio::main]` on main()** — tokio runtime is declared as a dependency but main() is synchronous. Story 1.2 will need to add `#[tokio::main]` or equivalent when iced_layershell app entry point is wired up.
- **No `[profile.release]` in Cargo.toml** — release profile not tuned (lto, opt-level). Add once binary has measurable size and startup time to optimize against.

## Deferred from: code review of 1-2-render-a-static-bar-surface-on-wayland (2026-03-31)

- **No diagnostic output on compositor failure** — `src/main.rs`: `.run()` propagates `iced_layershell::Result` silently; no log or message before process exit when compositor is unavailable. Anyhow context integration deferred to Story 1.3.
- **Bar-width constant (48px) encoded in three unlinked places** — `src/main.rs` `exclusive_zone: 48`, `size: Some((48, 0))`, and `src/app.rs` content width `47`; no single source of truth. Spec prohibits a constants file as premature abstraction; becomes a config-driven value in Story 1.3.
- **`SurfaceState::output_name` accepts empty string with no validation** — `src/surface/mod.rs`: no guard on construction; relevant when `SurfaceState` is first used in Story 2.1.

## Deferred from: code review of 1-3-load-kdl-configuration-file (2026-03-31)

- **Edge::from_str should implement std::str::FromStr** — custom `from_str` method used instead of the standard trait; would enable `"left".parse::<Edge>()` and interop with serde. Revisit in Story 1.4 or during config expansion.
- **saturating_sub(1) in view() masks collapsed_width=0 misconfiguration** — `src/app.rs`: a zero-width config value is silently clamped rather than rejected. Story 1.4 validation should reject `collapsed_width < 2`.
- **Test coverage of view() layout behavior** — `src/app.rs`: cannot meaningfully assert on `Element` output without a live Iced renderer; revisit when integration test infrastructure exists.
- **Edge::from_str case-sensitivity undocumented** — `"Left"` and `"LEFT"` are silently rejected; Story 1.4 error messages will document expected lowercase values.

## Deferred from: code review of 1-4-report-actionable-config-errors (2026-03-31)

- **`Edge::from_str` hardcodes `"bar.edge:"` field path** — error message embeds field path directly; if `from_str` is reused for another field the error will mislead. Single call site today; revisit when `std::str::FromStr` is implemented or a second `Edge` field is introduced.
- **`expanded_width` has no lower-bound or relational validation** — `validate()` only guards `collapsed_width`; no check that `expanded_width >= collapsed_width` or `expanded_width >= 2`. Deferred to Story 3.1 when expand/collapse semantics are defined.
- **`collapsed_width > i32::MAX` silently falls back to 48** — `src/main.rs`: `i32::try_from(collapsed_width).unwrap_or(48)` silently substitutes 48 for unrealistically large widths. Pre-existing; revisit when output dimension clamping is designed.
- **KDL property-style `edge="left"` silently ignored** — `get_arg("edge")` only reads positional arguments; property syntax produces no error and no effect. Pre-existing `get_arg` behavior; only surfaced when expanding KDL parsing.

## Deferred from: code review of 2-2-render-modules-from-configuration (2026-04-01)

- **Duplicate ModuleId not validated** — `src/config/mod.rs` (validate), `src/app.rs` (view): two config entries with the same module name (e.g., two `clock` entries) produce identical `ModuleId` instances; hover state highlights both simultaneously and `shared.modules` HashMap clobbers the second instance's state. Relevant only when real module instances exist (Story 2.3+).
- **Hover stuck if surface loses focus without MouseExit** — `src/app.rs:79-83`: `hovered_module` stays `Some(id)` with no clearing mechanism if the compositor hides the surface without delivering a MouseExit event. Resolves in Story 7.1 when `hovered_module` moves to `SurfaceState` with proper lifecycle handling.

## Deferred from: code review of 2-1-define-the-module-trait-and-zone-layout (2026-04-01)

- **ModuleConstructor uses bare fn pointer, can't capture config params** — `src/modules/mod.rs:84`: `fn() -> Box<dyn Module>` cannot represent closures or stateful constructors; will be insufficient in Story 2.2 when modules are config-parameterised. Consider `Box<dyn Fn() -> Box<dyn Module>>` or a constructor trait.
- **ModuleId::new accepts empty string** — `src/modules/mod.rs:11`: no guard against empty id producing a silent sentinel key in `SharedState::modules`; validation belongs at config parse time in Story 2.2 when module instances are constructed from config.

## Deferred from: code review of 2-3-clock-module-with-configurable-format (2026-04-01)

- **Subscription uses hardcoded `ModuleId::new("clock")` instead of self.id** — `src/modules/clock.rs::subscription`: not a current bug (both sides always produce `"clock"`) but a latent correctness risk if ID generation changes. iced 0.14's non-capturing closure constraint on `Subscription::map` forces this; revisit if subscription API expands.
- **Multiple `module "clock"` entries share a single ModuleId** — `src/modules/clock.rs`: two clock config entries both emit `ModuleId("clock")` ticks; second write unconditionally overwrites first in `shared.modules`. Pre-existing architecture concern tracked from Story 2.2 review; needs unique-ID or per-instance keying scheme.
- **Extra values on `time-format`/`date-format` nodes silently ignored** — `src/config/parse.rs`: `entries().first()` only reads the first positional arg; `time-format "%H:%M" "%I:%M"` silently uses only the first. Consistent with existing parser pattern; address if KDL multi-value config is ever validated.
- **`clock_ctor` silently falls back to defaults for mismatched ModuleParams** — `src/modules/mod.rs::clock_ctor`: the `_ =>` arm is unreachable given correct parsing but would silently mask misconfiguration. Add a defensive `unreachable!()` or debug assertion when a logging layer is introduced.
