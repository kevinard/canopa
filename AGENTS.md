# Canopa

This is one Cargo workspace. Run commands from its root unless a script documents
otherwise. `bash scripts/check.sh` runs the focused Files Copy/fingerprint
development checks. Select tests for the area being changed with
`bash scripts/check.sh --files --test CASE [FILTER]` or
`bash scripts/check.sh --files --lib [FILTER]`; repeat `--test` to cover related
consumers. Ordinary cases share the `integration` executable; the script translates
case names into module filters. Direct Cargo commands use `--test integration cases::CASE::`.
Keep `--lib` or explicit integration selection in the development loop.
See `docs/testing.md`.

Development checks use incremental debug builds and reuse root `target/`. Keep
the same feature/profile selection between edits. Do not set
`CARGO_INCREMENTAL=0`, use release/measurement settings, a fresh target directory
or cache cleanup in ordinary checks. The focused script explicitly enables
incremental compilation even if a measurement wrapper exported zero.
Normal dev/test builds use line-table debug info for file/line backtraces; opt in
to full debugger data with `--profile debugging`. Use `bash scripts/cargo.sh`
instead of invoking Cargo directly for compiling commands: it checks free space
before and during the build. Existing check/update scripts use that guard.
The default reserves are 10 GiB on build/cache filesystems and 1 GiB on a separate
temporary filesystem. See `docs/building.md` for overrides and additional paths.

Keep validation storage bounded. Native harnesses write into a new output
directory you choose (prefer one under `target/` or `/tmp`); never commit their
output. Native runs share compositor executables through the checksum-addressed
store under `target/native-test-snapshots/`; do not copy binaries into every
output directory. When resuming broad checks, retain the original
workspace/feature selection: excluding applications can compile another
dependency graph. Workspace tests use `--no-fail-fast` to run the remaining test
executables after a failure. Direct compiled-harness continuations must use each
crate's working directory, as Cargo does.

Run `bash scripts/check.sh --workspace` for CI/release validation, shared
crate/fork or cross-application changes, or when focused failures suggest a
wider regression. Run it once after the relevant edits, not after every Files
change or automatically at the end of a Files-only task. For tooling/docs-only
changes, validate the affected scripts and documentation. The existing
`bash scripts/check.sh --core` remains an explicit broad check without desktop
dependencies. Record which checks ran and any failures; focused success does not
establish full workspace or native acceptance.

Application packages live in `crates/canopa-shell/`, `crates/canopa-notify/`,
`crates/canopa-files/` and the other workspace crates under `crates/`.
Repository tooling and packaging live in root `scripts/` and `packaging/`; Cargo
outputs go to root `target/`.

Read `CONTEXT-MAP.md`, `docs/architecture/overview.md` and the relevant crate's
`AGENTS.md` before feature work. Issues and pending work live in GitHub Issues;
see `docs/agents/issue-tracker.md`. Do not add plans, tickets, run logs or
evidence files to the repository. Record significant decisions in `docs/adr/`
(see its README) and keep `docs/` describing the current code: update user guides,
the configuration reference and architecture pages in the same change as the
behaviour. See `docs/agents/domain.md` for ownership and terminology, and
`docs/contributing.md` for the general workflow.

Patched upstream dependencies live in GitHub forks under `kevinard/`. Rust crates
are pinned by git `rev` in the root `Cargo.toml`; the separately built compositor,
portal frontend and CUPS components are pinned by fork commit and archive
checksum in `packaging/forks.json`. Read `docs/vendor-maintenance.md` before
changing them: one tracking PR per change in the fork, preserve upstream
attribution, and never vendor a copy or patch file into the workspace.
