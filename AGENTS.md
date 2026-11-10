# Canopa

This is one Cargo workspace. Run commands from its root unless a script documents
otherwise. `bash scripts/check.sh` runs the focused Files Copy/fingerprint
development checks. Select tests for the area being changed with
`bash scripts/check.sh --files --test CASE [FILTER]` or
`bash scripts/check.sh --files --lib [FILTER]`; repeat `--test` to cover related
consumers. Ordinary cases share the `integration` executable; the script translates
case names into module filters. Direct Cargo commands use `--test integration cases::CASE::`.
Keep `--lib` or explicit integration selection in the development loop.

Development checks use incremental debug builds and reuse root `target/`. Keep
the same feature/profile selection between edits. Do not carry
`CARGO_INCREMENTAL=0`, release/measurement settings, a fresh target directory or
cache cleanup into ordinary checks. The focused script explicitly enables
incremental compilation even if a measurement wrapper exported zero.
Normal dev/test builds use line-table debug info for file/line backtraces; opt in
to full debugger data with `--profile debugging`. Use `bash scripts/cargo.sh`
instead of invoking Cargo directly for compiling commands: it checks free space
before and during the build. Existing check/update scripts use that guard.
The default reserves are 10 GiB on build/cache filesystems and 1 GiB on a separate
temporary filesystem. See `docs/building.md` for overrides and additional paths.
This policy supersedes build flags in historical `.scratch/` execution notes;
retain those notes as evidence rather than copying their settings into new runs.

Keep validation storage bounded. Reuse stable source paths for fork checks and
the checksum-addressed compositor store for native runs; do not copy binaries
into every artifact directory. Preserve concise receipts and regression evidence.
Archive obsolete unique native evidence executables after verifying their lossless
checksum; keep raw baseline/candidate copies only while they are needed.
When resuming broad checks, retain the original workspace/feature selection:
excluding applications can compile another dependency graph. Workspace tests use
`--no-fail-fast` to run the remaining test executables after a failure.
Direct compiled-harness continuations must use each crate's working directory,
as Cargo does.

Run `bash scripts/check.sh --workspace` for CI/release validation, shared
crate/vendor or cross-application changes, or when focused failures suggest a
wider regression. Run it once after the relevant edits, not after every Files
change or automatically at the end of a Files-only task. For tooling/docs-only
changes, validate the affected scripts and documentation. The existing
`bash scripts/check.sh --core` remains an explicit broad check without desktop
dependencies. Record which checks ran and any failures; focused success does not
establish full workspace or native acceptance.

Application packages live in `crates/canopa-shell/`, `crates/canopa-notify/`, and
`crates/canopa-files/`, alongside
the other workspace crates. Repository tooling and packaging live in root
`scripts/` and `packaging/`; Cargo outputs go to root `target/`.

Read `CONTEXT-MAP.md` and the relevant crate's `AGENTS.md` before feature work.
Issues and PRDs are local Markdown files under `.scratch/`; see
`docs/agents/issue-tracker.md`. Integration decisions live in `docs/adr/`.
Shell and notification domain docs and ADR histories live in `docs/shell/` and
`docs/notify/`. See `docs/agents/domain.md` for ownership and terminology.

Patched upstream dependencies live in GitHub forks under `kevinard/` and are
pinned by git `rev` in the root `Cargo.toml`; separately built programs still use
the overlay recipes in `vendor/source-overlays/`. Read `docs/vendor-maintenance.md`
before changing them: one tracking PR per change in the fork, preserve upstream
attribution, and never vendor a copy into the workspace.
The archived rotation prototype in `experiments/focused-window-rotation/` is
excluded from the supported workspace and retains its experimental renderer.
