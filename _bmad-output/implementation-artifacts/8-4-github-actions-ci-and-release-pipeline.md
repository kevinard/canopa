# Story 8.4: GitHub Actions CI and Release Pipeline

Status: review

## Story

As a contributor or maintainer,
I want CI to validate every PR and produce a binary release on tagging,
so that the project stays green and releases are reproducible.

## Acceptance Criteria

1. **Given** a pull request is opened against the main branch
   **When** CI runs
   **Then** `cargo fmt --check` passes
   **And** `cargo clippy -- -D warnings` passes
   **And** `cargo test` passes

2. **Given** a push lands on the main branch
   **When** CI runs
   **Then** `nix build` passes (gated on `flake.nix` existing — activates automatically when Story 8.2 lands)

3. **Given** a semver tag (e.g., `v1.0.0`) is pushed
   **When** the release workflow runs
   **Then** a stripped binary for `x86_64-unknown-linux-gnu` is built and attached to a GitHub release

## File Map

| File | Action | What changes |
|------|--------|--------------|
| `.github/workflows/ci.yml` | **CREATE** | PR/push CI: fmt + clippy + test + conditional nix job |
| `.github/workflows/release.yml` | **CREATE** | Tag-triggered release: build + strip + upload binary |

## Tasks / Subtasks

- [x] Task 1: Create `.github/workflows/ci.yml` (AC: #1, #2)

  Create `.github/workflows/ci.yml` with this exact content:

  ```yaml
  name: CI

  on:
    push:
      branches: [main]
    pull_request:
      branches: [main]

  env:
    CARGO_TERM_COLOR: always

  jobs:
    check:
      name: fmt / clippy / test
      runs-on: ubuntu-latest

      steps:
        - uses: actions/checkout@v4

        - name: Install Rust (stable)
          uses: dtolnay/rust-toolchain@stable
          with:
            components: rustfmt, clippy

        - name: Cache Rust build artifacts
          uses: Swatinem/rust-cache@v2

        - name: Install system dependencies
          run: |
            sudo apt-get update -qq
            sudo apt-get install -y --no-install-recommends \
              libpulse-dev \
              libwayland-dev \
              libxkbcommon-dev \
              libudev-dev \
              pkg-config

        - name: cargo fmt --check
          run: cargo fmt --all -- --check

        - name: cargo clippy
          run: cargo clippy -- -D warnings

        - name: cargo test
          run: cargo test

    nix:
      name: nix build
      runs-on: ubuntu-latest
      # Only runs on push to main AND only when flake.nix exists.
      # This job activates automatically when Story 8.2 adds flake.nix.
      if: github.event_name == 'push' && hashFiles('flake.nix') != ''

      steps:
        - uses: actions/checkout@v4

        - name: Install Nix
          uses: DeterminateSystems/nix-installer-action@v14

        - name: nix build
          run: nix build
  ```

- [x] Task 2: Create `.github/workflows/release.yml` (AC: #3)

  Create `.github/workflows/release.yml` with this exact content:

  ```yaml
  name: Release

  on:
    push:
      tags:
        - 'v[0-9]+.[0-9]+.[0-9]+'

  env:
    CARGO_TERM_COLOR: always

  jobs:
    release:
      name: Build and release
      runs-on: ubuntu-latest

      permissions:
        contents: write

      steps:
        - uses: actions/checkout@v4

        - name: Install Rust (stable)
          uses: dtolnay/rust-toolchain@stable

        - name: Cache Rust build artifacts
          uses: Swatinem/rust-cache@v2

        - name: Install system dependencies
          run: |
            sudo apt-get update -qq
            sudo apt-get install -y --no-install-recommends \
              libpulse-dev \
              libwayland-dev \
              libxkbcommon-dev \
              libudev-dev \
              pkg-config

        - name: Build release binary
          run: cargo build --release --target x86_64-unknown-linux-gnu

        - name: Strip binary
          run: strip target/x86_64-unknown-linux-gnu/release/niribar

        - name: Upload to GitHub release
          uses: softprops/action-gh-release@v2
          with:
            files: target/x86_64-unknown-linux-gnu/release/niribar
            generate_release_notes: true
  ```

## Dev Notes

### Pure file creation — no Rust code changes

This story creates two YAML workflow files in a new `.github/workflows/` directory. No `.rs` files are touched. No `cargo test` baseline change.

### Directory creation

The `.github/` directory does not exist yet. The Write tool creates intermediate directories automatically. Create `.github/workflows/ci.yml` first — the directory will be created.

### Action versions pinned to current stable

| Action | Version | Why |
|--------|---------|-----|
| `actions/checkout` | v4 | Current LTS |
| `dtolnay/rust-toolchain` | `@stable` | Always latest stable; avoids pinned-version drift |
| `Swatinem/rust-cache` | v2 | Caches `~/.cargo/registry`, `~/.cargo/git`, `target/` keyed by lock file |
| `DeterminateSystems/nix-installer-action` | v14 | Most reliable Nix + flakes installer for GitHub Actions |
| `softprops/action-gh-release` | v2 | Current stable; replaces deprecated v1 |

### System dependencies — why each package

| Package | Required by |
|---------|-------------|
| `libpulse-dev` | `libpulse-binding` (Cargo.toml dep) — PulseAudio C headers and `.so` |
| `libwayland-dev` | `iced_layershell` — Wayland protocol C headers and client library |
| `libxkbcommon-dev` | `iced_layershell` — keyboard handling (xkbcommon C library) |
| `libudev-dev` | `iced` / winit backend — udev device enumeration |
| `pkg-config` | All C library discovery — finds .pc files for the above |

### `x86_64-unknown-linux-gnu` — no cross-compilation needed

The Ubuntu runner (`ubuntu-latest`) is already `x86_64-unknown-linux-gnu`. Passing `--target x86_64-unknown-linux-gnu` explicitly ensures the release binary is built for the correct ABI and the output lands in `target/x86_64-unknown-linux-gnu/release/niribar` (not `target/release/niribar`). No `cross` tool or target installation needed.

### `permissions: contents: write` on release job

`softprops/action-gh-release` creates and updates GitHub releases. The `contents: write` permission is required for this operation. Without it, the job fails with a 403. This permission is scoped to the release job only.

### Nix job — auto-activates when Story 8.2 lands

The `nix` job uses `if: github.event_name == 'push' && hashFiles('flake.nix') != ''`. When `flake.nix` doesn't exist (i.e., Story 8.2 hasn't landed), `hashFiles('flake.nix')` returns `''` and the job is skipped. Once Story 8.2 adds `flake.nix`, the condition becomes true and the job runs automatically on every push to main. No changes to `ci.yml` needed when Story 8.2 lands.

### Tag pattern for release trigger

`'v[0-9]+.[0-9]+.[0-9]+'` matches semver tags like `v0.1.0`, `v1.0.0`, `v2.3.14`. It does not match pre-release suffixes like `v1.0.0-rc1` (intentional — pre-releases should be tagged differently or the pattern extended). The `on.push.tags` trigger fires only when a tag matching the pattern is pushed; it does not fire on branch pushes.

### `generate_release_notes: true` on release job

`softprops/action-gh-release` with `generate_release_notes: true` auto-generates the release body from commits since the previous tag. No manual release notes required for initial releases.

### `cargo fmt --all` vs `cargo fmt`

`cargo fmt --all` ensures all workspace members are checked. For single-crate projects (like niribar today), this is equivalent to `cargo fmt`, but using `--all` is forward-safe if a workspace is added later.

### No `rust-toolchain.toml` file

The project does not have a `rust-toolchain.toml`. `dtolnay/rust-toolchain@stable` installs the latest stable Rust, which is fine. If a specific MSRV needs enforcing, add a `rust-toolchain.toml` in a future story.

### Project Structure Notes

```
.github/
└── workflows/
    ├── ci.yml       # PR checks + conditional nix build (this story)
    └── release.yml  # Tag-triggered binary release (this story)
```
[Source: architecture.md, "CI/CD & Release Pipeline" and "Canonical Directory Tree"]

### References

- [Source: epics.md, Epic 8, Story 8.4 — acceptance criteria]
- [Source: architecture.md, "CI/CD & Release Pipeline" — AR8, AR9]
- [Source: architecture.md, Canonical Directory Tree — `.github/workflows/ci.yml` and `release.yml`]
- [Source: Cargo.toml — `libpulse-binding = "2.30.1"` (system dep requirement)]

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

- Created `.github/workflows/ci.yml`: `check` job runs `cargo fmt --all -- --check`, `cargo clippy -- -D warnings`, `cargo test` on PR/push to main; `nix` job gated on `hashFiles('flake.nix') != ''` so it's skipped today and auto-activates when Story 8.2 adds `flake.nix`
- Created `.github/workflows/release.yml`: triggers on semver tags `v*.*.*`, builds `--release --target x86_64-unknown-linux-gnu`, strips binary, uploads to GitHub release via `softprops/action-gh-release@v2` with `generate_release_notes: true`; `permissions: contents: write` scoped to the release job
- No Rust code changes; test baseline unaffected

### File List

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-04-12 | 1.0 | Implemented: ci.yml (fmt/clippy/test + conditional nix) and release.yml (tag-triggered binary release) | claude-sonnet-4-6 |
