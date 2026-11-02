# Canopa

This is one Cargo workspace. Run commands from its root unless a script documents
otherwise. `bash scripts/check.sh` runs the focused Files Copy/fingerprint
development checks. Select tests for the area being changed with
`bash scripts/check.sh --files --test TARGET [FILTER]` or
`bash scripts/check.sh --files --lib [FILTER]`; repeat `--test` to cover related
consumers. A bare Cargo test filter still builds every integration-test executable,
so always select `--lib` or named `--test` targets in the development loop.

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

Shared dependency forks live only in `vendor/`. Read `docs/vendor-maintenance.md`
before changing them, preserve upstream attribution, and update provenance and
reviewable patches together. Application crates must not vendor another copy.
The archived rotation prototype in `experiments/focused-window-rotation/` is
excluded from the supported workspace and retains its experimental renderer.
