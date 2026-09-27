# Repository layout validation

Executed locally on 2026-09-27 for [ADR 0009](adr/0009-consolidated-repository-layout.md),
against baseline `b4c47f9a374c3ed54e139c724d9ba6d15508511d`. Commands run from the
Canopa root with locked, offline dependencies, `CARGO_INCREMENTAL=0`, and two
Cargo build jobs. Tests use private D-Bus sessions.

## Workspace checks

- `cargo metadata --locked --offline --no-deps --format-version 1` locates all
  eleven supported packages under `crates/` and build outputs in root `target/`.
- Metadata for `experiments/focused-window-rotation/Cargo.toml` still resolves an
  independent workspace, excluded from the supported build.
- `dbus-run-session -- bash scripts/check.sh --core`: passed, 375 Rust tests,
  no ignored tests; GUI/audio dependencies remain excluded from the core graph.
- `dbus-run-session -- bash scripts/check.sh`: passed, 1,692 Rust tests including
  fork regressions, no ignored tests. Formatting, Clippy, updater fixtures,
  session-unit validation, dependency attributions, Settings frontend activation,
  configuration fixtures and vendor integrity also passed.
- Python and shell syntax checks and desktop-runner prerequisites passed.

The existing local target cache was moved to root `target/`. One cached portal
integration test retained its old absolute `CARGO_BIN_EXE` path and failed to spawn
the backend. Rebuilding only that package with `cargo clean --locked --offline
-p canopa-portal` removed the stale fixture. The subsequent full check passed;
no source workaround was required.

## Desktop and packaging checks

`cargo build --locked --offline --workspace --bins` passed before these checks.
All graphical fixtures used isolated compositor and D-Bus sessions.

- `python3 scripts/test-desktop.py --notify target/debug/canopa-notify --niribar
  target/debug/niribar --artifacts /tmp/canopa-layout-desktop --scale 1.25`: passed.
  Coverage includes native actions, Center interaction, shared image rendering,
  DND, appearance reload and independent shell/daemon restart recovery.
- `python3 scripts/test-fork-desktop.py --niribar target/debug/niribar --artifacts
  /tmp/canopa-layout-forks`: passed. The moved helper/fixture paths work for tray
  popup grabs, dismissal, fractional scaling and output removal/restoration.
- `python3 scripts/validate-desktop.py --binary target/debug/niribar --output
  /tmp/canopa-layout-shell --label 'repository layout, debug smoke check' --runs 1
  --settle-seconds 1 --sample-seconds 2`: passed with the relocated default config
  and both panel cycles. This short run verifies the harness, not performance.
- `bash scripts/package-release.sh target/debug target/layout-validation/dist`
  followed by `bash scripts/smoke-release.sh
  target/layout-validation/dist/canopa-x86_64-unknown-linux-gnu.tar.gz`: passed.
  All five binaries execute, configuration validation and v1/v5 provider shutdown
  pass, and original dependency notices are collected for 552 packages. This
  validates package assembly using debug binaries; it is not a release build.

## Source preservation

All retained tracked files have destinations in the consolidated tree. A baseline
comparison verified 466 application source, experimental source/vendor, shared
vendor, license and lockfile entries byte for byte. Application runtime source,
package/executable names, feature declarations, state paths and service-unit
contents are unchanged. Relative-link checks found no newly broken documentation
links; unresolved links in historical research records predate this move.

## Standards review

No documented-standard violations or actionable code-smell findings. Root tooling,
agent navigation, licenses and dependency snapshots follow the repository rules.

## Spec review

No missing, incorrect or out-of-scope requirements found against
`.scratch/repository-consolidation/PRD.md`. Independent application lifetimes and
the excluded prototype remain intact.
