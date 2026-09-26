# Story 1.1: Initialize Cargo Project with Pinned Dependencies

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a **developer**,
I want the project initialized with a `Cargo.toml` containing pinned dependencies,
So that the build is reproducible and the iced_layershell/Iced compatibility is verified from day one.

## Acceptance Criteria

1. **Given** an empty repository with no `Cargo.toml`
   **When** `cargo build` is run
   **Then** the binary compiles successfully with `iced_layershell` 0.13.7, `iced` 0.13.x (iced_layershell-compatible), `zbus` 5.12.0, `kdl` 6.5.0, `tokio` 1.x (feature = "full")

2. **Given** the project is initialized
   **When** `cargo clippy -- -D warnings` is run
   **Then** it produces zero warnings

3. **Given** the project is initialized
   **When** `cargo fmt --check` is run
   **Then** it passes (code is formatted)

4. **Given** a minimal `main.rs` that imports and uses iced_layershell
   **When** `cargo build` is run
   **Then** the binary compiles and exits cleanly (no runtime errors, no Wayland connection required)

5. **Given** the project is initialized
   **When** the project structure is inspected
   **Then** the directory layout matches the architecture spec: `src/`, `config/`, `contrib/systemd/`, `assets/` directories exist with placeholder files as needed

## Tasks / Subtasks

- [x] Task 1: Create `Cargo.toml` with pinned dependency set (AC: #1, #4)
  - [x] 1.1: Run `cargo init --name niribar` in repo root (or create Cargo.toml manually since git already initialized)
  - [x] 1.2: Populate `[package]` section: name = "niribar", version = "0.1.0", edition = "2021"
  - [x] 1.3: Add `[dependencies]`: `iced_layershell = "0.17.1"`, `iced = "0.14"`, `zbus = "5.12.0"`, `kdl = "6.5.0"`, `tokio = { version = "1", features = ["full"] }`
  - [x] 1.4: Add `anyhow = "1"` (used for config/startup error handling per architecture)
  - [x] 1.5: Verify `Cargo.lock` is generated after `cargo build`

- [x] Task 2: Write minimal `main.rs` that compiles and exits cleanly (AC: #1, #4)
  - [x] 2.1: Create `src/main.rs` with a `fn main()` that uses `iced_layershell` in `use` statements to force linkage verification
  - [x] 2.2: `main.rs` must compile — the binary can immediately return/exit; no Wayland surface needed at this stage
  - [x] 2.3: Do NOT implement any real application logic — that belongs to Story 1.2

- [x] Task 3: Create project directory skeleton (AC: #5)
  - [x] 3.1: Create `src/config/` directory with empty `mod.rs`
  - [x] 3.2: Create `src/modules/` directory with empty `mod.rs`
  - [x] 3.3: Create `src/niri/` directory with empty `mod.rs`
  - [x] 3.4: Create `src/dbus/` directory with empty `mod.rs`
  - [x] 3.5: Create `src/surface/` directory with empty `mod.rs`
  - [x] 3.6: Create `src/app.rs` as empty file (stub for Story 1.2's Iced Application impl)
  - [x] 3.7: Create `config/` directory with empty `default.kdl` placeholder (populated in Story 1.3/8.3)
  - [x] 3.8: Create `contrib/systemd/` directory with empty `niribar.service` placeholder (populated in Story 8.1)
  - [x] 3.9: Create `assets/` directory (placeholder for icons/fonts)
  - [x] 3.10: Declare all new submodules in `main.rs` with `mod` statements so they compile

- [x] Task 4: Verify all quality gates pass (AC: #2, #3)
  - [x] 4.1: Run `cargo build` — must succeed with zero errors
  - [x] 4.2: Run `cargo clippy -- -D warnings` — must produce zero warnings
  - [x] 4.3: Run `cargo fmt` to format all files, then verify `cargo fmt --check` passes
  - [x] 4.4: Run `cargo test` — no tests yet, but must not error

- [x] Task 5: Write unit tests for the dependency linkage verification (AC: #1)
  - [x] 5.1: Add a `#[cfg(test)] mod tests` block in `src/main.rs` with a trivial smoke test confirming the binary entry point compiles
  - [x] 5.2: Verify `cargo test` passes

## Dev Notes

### Pinned Versions

Use the following versions (latest as of 2026-03-29, upgraded from architecture spec per project decision):

| Crate | Version |
|---|---|
| iced_layershell | **0.17.1** |
| iced | **0.14** |
| zbus | 5.12.0 |
| kdl | 6.5.0 |
| tokio | 1 (feature: full) |

**iced_layershell 0.17.1 requires `iced = "^0.14"` — these are the correct compatible pair.** Do not mix versions (e.g., iced_layershell 0.13 + iced 0.14 will not compile).

### What This Story Creates

This is purely a scaffolding story. At the end of 1.1:
- The binary compiles but does nothing (no Wayland surface, no UI)
- Project directory skeleton is in place for all subsequent stories
- All subsequent stories assume this structure exists

### Critical Architecture Rules (apply from day one)

- **Single Tokio runtime** — never create a second one (AR4). Iced, zbus, and niri-ipc must all share the same executor.
- **`SharedState` / `SurfaceState` boundary** — module data goes in `SharedState`; expand/collapse and focus go in `SurfaceState`. This boundary must never be crossed. Stubs created in this story should already reflect this structure.
- **`anyhow`** is the error type for `main()` and config loading. Module subscriptions never return `Result` — they emit `{Name}Event::Unavailable` instead.

### Cargo.toml Template

```toml
[package]
name = "niribar"
version = "0.1.0"
edition = "2021"

[dependencies]
iced_layershell = "0.17.1"
iced = "0.14"
zbus = "5.12.0"
kdl = "6.5.0"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

### Minimal `main.rs` Pattern

```rust
mod app;
mod config;
mod dbus;
mod modules;
mod niri;
mod surface;

fn main() -> anyhow::Result<()> {
    // Story 1.2 will replace this body with iced_layershell app setup
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn binary_entry_point_compiles() {
        // Smoke test: if this test runs, the binary entry point compiled
        assert!(true);
    }
}
```

### Project Structure Notes

The complete target structure (per architecture `src/main.rs` comment and Project Structure section):

```
niribar/
├── Cargo.toml
├── Cargo.lock
├── .gitignore
├── config/
│   └── default.kdl           # placeholder — populated in Story 1.3/8.3
├── contrib/
│   └── systemd/
│       └── niribar.service   # placeholder — populated in Story 8.1
├── assets/                   # placeholder for icons/fonts
└── src/
    ├── main.rs               # entry point (this story); app.rs wired up in Story 1.2
    ├── app.rs                # stub (Story 1.2 implements Iced Application)
    ├── config/
    │   └── mod.rs            # stub (Story 1.3 implements)
    ├── modules/
    │   └── mod.rs            # stub (Story 2.1 implements)
    ├── niri/
    │   └── mod.rs            # stub (Story 4.1 implements)
    ├── dbus/
    │   └── mod.rs            # stub (Story 5.1 implements)
    └── surface/
        └── mod.rs            # stub (Story 1.2 implements)
```

All `mod.rs` stubs can be empty (`// placeholder`) — they just need to exist so `mod` declarations in `main.rs` compile.

### Naming Conventions (enforce from day one)

- Functions, variables, fields, module files: `snake_case`
- Types, structs, enums, traits: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`
- No `Niribar` prefix on internal types — the crate name provides namespace

### Testing Standards

- Tests co-located in source files: `#[cfg(test)] mod tests { ... }` at bottom of each file
- No separate `tests/` directory for unit tests
- `cargo test` must pass with zero errors before marking any task complete

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Starter Template Evaluation] — iced_layershell 0.13.7 selection rationale
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure] — directory layout
- [Source: _bmad-output/planning-artifacts/architecture.md#Naming Patterns] — naming conventions
- [Source: _bmad-output/planning-artifacts/architecture.md#State Management Patterns] — SharedState/SurfaceState boundary
- [Source: _bmad-output/planning-artifacts/architecture.md#Process Patterns] — anyhow for startup errors
- [Source: _bmad-output/planning-artifacts/epics.md#Story 1.1] — acceptance criteria
- AR1, AR2, AR3 from epics Requirements Inventory

## Review Findings

- [x] [Review][Patch] AC4 violation: main.rs declares modules but never imports or uses iced_layershell [src/main.rs] — fixed: added `use iced_layershell as _;`

- [x] [Review][Defer] No `#[tokio::main]` on main() — tokio runtime not initialized [src/main.rs] — deferred, pre-existing; Story 1.2 adds iced_layershell app entry point which will require async runtime setup
- [x] [Review][Defer] No `[profile.release]` section in Cargo.toml — binary size/startup not optimized [Cargo.toml] — deferred, pre-existing; can be added once binary has measurable size to optimize

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

- All 5 tasks / 19 subtasks completed.
- `cargo build` succeeded: iced_layershell 0.17.1 + iced 0.14 + zbus 5.12.0 + kdl 6.5.0 + tokio 1.50.0 compile cleanly together.
- `cargo clippy -- -D warnings`: zero warnings.
- `cargo fmt --check`: passes.
- `cargo test`: 1 test passed (`tests::binary_entry_point_compiles`).
- Project directory skeleton created with stub `mod.rs` files for all submodules (config, modules, niri, dbus, surface) and placeholder artifacts (config/default.kdl, contrib/systemd/niribar.service, assets/).
- Note: AC #1 references old versions (0.13.7) — implementation uses 0.17.1/0.14 per Kevin's explicit instruction.

### File List

- Cargo.toml
- Cargo.lock
- src/main.rs
- src/app.rs
- src/config/mod.rs
- src/modules/mod.rs
- src/niri/mod.rs
- src/dbus/mod.rs
- src/surface/mod.rs
- config/default.kdl
- contrib/systemd/niribar.service
- assets/.gitkeep

### Change Log

- 2026-03-29: Story 1.1 implemented — Cargo project initialized with pinned deps, directory skeleton created, all quality gates pass.
