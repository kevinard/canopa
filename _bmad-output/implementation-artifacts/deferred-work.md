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
