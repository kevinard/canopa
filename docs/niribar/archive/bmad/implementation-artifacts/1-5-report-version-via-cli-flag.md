# Story 1.5: Report Version via CLI Flag

Status: done

## Story

As a **user or package maintainer**,
I want to run `niribar --version` and see the version number,
So that I can verify which release is installed.

## Acceptance Criteria

1. **Given** niribar is installed
   **When** `niribar --version` is run
   **Then** the version string from `Cargo.toml` is printed to stdout
   **And** the process exits with code 0
   **And** no Wayland surface is created

2. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

3. **Given** niribar is built
   **When** `cargo fmt --check` is run
   **Then** it passes

## Tasks / Subtasks

- [x] Task 1: Add `--version` flag handling in `crates/niribar/src/main.rs` (AC: #1)
  - [x] 1.1: In `main()`, before calling `try_main()`, check if any argument equals `"--version"` or `"-V"`
  - [x] 1.2: If the flag is present, print `env!("CARGO_PKG_VERSION")` to stdout and return (exits 0)
  - [x] 1.3: Verify no surface is created — the check must be before `try_main()` is called

- [x] Task 2: Add required tests (AC: #1)
  - [x] 2.1: In `crates/niribar/src/main.rs` tests: add `version_string_is_non_empty` — assert `!env!("CARGO_PKG_VERSION").is_empty()`
  - [x] 2.2: In `crates/niribar/src/main.rs` tests: add `version_string_matches_cargo_toml` — assert `env!("CARGO_PKG_VERSION") == "0.1.0"` (current version; keeps test honest against accidental empty-string substitution)

- [x] Task 3: Quality gates
  - [x] 3.1: `cargo build` — must succeed
  - [x] 3.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 3.3: `cargo fmt` then `cargo fmt --check` — must pass
  - [x] 3.4: `cargo test` — all 18 pre-existing tests must pass; at least 2 new tests must pass

## Dev Notes

### Implementation Pattern

Add to `crates/niribar/src/main.rs`, in `main()` **before** the `try_main()` call:

```rust
fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Err(e) = try_main() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

**Why `return` not `std::process::exit(0)`:** `return` from `main()` is idiomatic Rust for exit 0 and avoids potential destructor-skip surprises with `process::exit`. Both are correct here; prefer `return`.

**`env!("CARGO_PKG_VERSION")`:** This is a compile-time macro that reads `version` from `Cargo.toml`. No build script or runtime I/O needed. The current version is `"0.1.0"`.

**Output to stdout:** Per AC1, version goes to stdout (`println!`), not stderr (`eprintln!`). This is the Unix convention for `--version` and allows `niribar --version | head -1` pipelines.

**No new dependencies:** Do NOT add `clap` or any CLI parsing crate. `std::env::args()` is sufficient for this single flag. The architecture dependency set is: iced_layershell 0.17.1, iced 0.14, zbus 5.12.0, kdl 6.5.0, tokio 1, anyhow 1.

**Position in `main()`:** The version check must happen before `try_main()`. This guarantees:

- No config file is read
- No Wayland surface is created
- Exit code is 0 (not 1 from error handling)

### Project Structure Notes

- Only file to modify: `crates/niribar/src/main.rs`
- Do NOT modify `Cargo.toml` — version is already `"0.1.0"`, no changes needed
- Do NOT add a new module — the check lives in `main()` alongside the existing error-exit pattern

### Known Patterns from Previous Stories

- Story 1.4 established `main()` + `try_main()`: the version check slots into `main()` as an early return
- `rustfmt` may reformat chained iterator calls — run `cargo fmt` before `cargo fmt --check`
- Existing test `binary_entry_point_compiles` (line 51 in `crates/niribar/src/main.rs`) must continue to pass — it is a compile smoke test, unaffected by adding args handling

### Deferred Items Not in Scope

- Handling `--help` or any other CLI flags — not in AC, out of scope
- Case-insensitive matching for `--Version` — out of scope, lowercase convention is universal
- Structured version output (JSON, etc.) — out of scope

### References

- Epic 1, Story 1.5 (epics.md): user story + acceptance criteria
- PRD FR38: "The bar reports its version number via a CLI flag"
- Architecture: "Binary Entry Point Location — `--version` in `main.rs` for FR38"
- `crates/niribar/src/main.rs` — current entry point with `main()` + `try_main()` pattern (Story 1.4)
- Cargo.toml `version = "0.1.0"` — source of truth for version string

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

- No surprises. `env!("CARGO_PKG_VERSION")` is compile-time; both new tests pass immediately as expected.
- `cargo run -- --version` confirmed `0.1.0` on stdout, exit 0.

### Completion Notes List

- All 3 tasks / 7 subtasks completed.
- `cargo build`: succeeded.
- `cargo clippy -- -D warnings`: zero warnings.
- `cargo fmt --check`: passes.
- `cargo test`: 20 tests passed (18 pre-existing + 2 new).
- AC1 satisfied: `niribar --version` prints `0.1.0` to stdout and exits 0 via `return` (no surface created).
- AC2/AC3: clippy and fmt pass.
- Version check in `main()` runs before `try_main()` — Wayland surface is never initialized.
- `-V` short form also supported (Unix convention).

### File List

- crates/niribar/src/main.rs

### Change Log

- 2026-03-31: Story 1.5 implemented — `--version` / `-V` flag added to `main()` before `try_main()`; prints `env!("CARGO_PKG_VERSION")` to stdout and returns (exit 0); 2 new tests added; all 20 tests pass.
