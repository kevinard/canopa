# Canopa

This is one Cargo workspace. Run commands from its root unless a script documents
otherwise. `bash scripts/check.sh` is the normal validation entry point;
`bash scripts/check.sh --core` checks Notify without desktop dependencies.

Read `CONTEXT-MAP.md` and the relevant application's `AGENTS.md` before feature work.
Issues and PRDs are local Markdown files under `.scratch/`; see
`docs/agents/issue-tracker.md`. Integration decisions live in `docs/adr/`.

Shared dependency forks live only in `vendor/`. Read `docs/vendor-maintenance.md`
before changing them, preserve upstream attribution, and update provenance and
reviewable patches together. Application crates must not vendor another copy.
