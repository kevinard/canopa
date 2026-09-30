# Canopa

This is one Cargo workspace. Run commands from its root unless a script documents
otherwise. `bash scripts/check.sh` is the normal validation entry point;
`bash scripts/check.sh --core` checks Notify without desktop dependencies.

Application packages live in `crates/canopa-shell/` and `crates/canopa-notify/`, alongside
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
