# Deferred Work

## Deferred from: code review of 1-1-initialize-cargo-project-with-pinned-dependencies (2026-03-29)

- **No `#[tokio::main]` on main()** — tokio runtime is declared as a dependency but main() is synchronous. Story 1.2 will need to add `#[tokio::main]` or equivalent when iced_layershell app entry point is wired up.
- **No `[profile.release]` in Cargo.toml** — release profile not tuned (lto, opt-level). Add once binary has measurable size and startup time to optimize against.
