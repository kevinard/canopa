# Consolidated repository layout

Status: Accepted, 2026-09-27.

The initial workspace import deliberately retained the two applications' old
repository layouts. Shared dependencies, configuration, appearance and provider
contracts are now established, so those nested project roots no longer express
how Canopa is maintained. This supersedes only the source-layout choice in
[ADR 0001](0001-shared-workspace-and-upstream-forks.md).

All supported Cargo packages live under root `crates/`, including `niribar` and
`canopa-notify`. Their source, tests, embedded defaults, examples, migrations,
README files and original licenses remain with their packages. Package names,
executable names, features and application ownership are unchanged. Niribar stays
MIT; Notify stays MIT OR Apache-2.0.

Root `scripts/` owns validation, desktop fixtures, installation and release
tooling. Root `packaging/` owns the companion service units and distribution
metadata. The old forwarding scripts are removed; all development commands run
from the workspace root, using root `Cargo.lock` and `target/`.

Root `docs/niribar/` and `docs/notify/` retain the separate domain glossaries and
ADR histories without renumbering them. Shared decisions remain in `docs/adr/`.
Historical tickets move to `.scratch/niribar/` and `.scratch/notify/`, preserving
their feature directories and identifiers. New cross-application work uses root
`.scratch/<feature>/`. Agent instructions point to this single repository layout.

The excluded rotation prototype moves to `experiments/focused-window-rotation/`.
Its independent manifest, lockfile and experimental renderer snapshots remain
intact. The supported applications continue to share only root `vendor/` forks.

This is a structural consolidation. Notify and Niribar remain separate processes
with independent restart lifetimes, existing runtime paths and unchanged domain
ownership. Validation covers both the isolated Notify core profile and combined
workspace, moved desktop tooling, and release assembly from the new paths.

Update, 2026-11-10: root `vendor/` no longer exists. Shared crate forks are GitHub forks pinned by git `rev` in the root manifest, and the separately built compositor, portal frontend and CUPS components are pinned fork archives in `packaging/forks.json`. See ../vendor-maintenance.md.
