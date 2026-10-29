# Canopa Files

Follow [root instructions](../../AGENTS.md) and read
[the Files context](../../docs/files/CONTEXT.md) before feature work. Product
requirements and the implementation ledger live in `.scratch/file-manager/`.
Record product and implementation decisions in `docs/files/adr/`; shared
integration changes also need a root ADR.

The GUI direction is the workspace's Iced stack and `canopa-ui`, per
[ADR 0018](../../docs/adr/0018-files-iced-and-shared-controls.md). The current GTK
adapter is migration reference code, not the intended final architecture.
Preserve the complete product brief while replacing it; keep filesystem and
application models independent of the renderer.

Run commands from the workspace root. `bash scripts/check.sh` defaults to the
fingerprint unit tests and `uutils_copy` integration tests. For other work use
`bash scripts/check.sh --files --lib FILTER` or
`bash scripts/check.sh --files --test TARGET [FILTER]`; multiple `--test` selectors
run related integration targets without compiling the rest. Choose tests covering
the changed behavior; the default Copy checks alone do not validate other Files
features. Library/content-copy lifecycle changes also need the affected transfer,
Undo, Recovery and source-retention targets.

Keep incremental debug compilation enabled and reuse the same features and
`target/` cache throughout development. A bare `cargo test -p canopa-files FILTER`
still compiles every integration target. Reserve
`bash scripts/check.sh --workspace` for the broader gates described in root
instructions; a Files-only task does not require a workspace rebuild by default.
Optimized performance measurements use an explicit release build separately.

Filesystem tests use private temporary directories and the public browsing,
transfer, trash, preferences, and custom-action interfaces. Never exercise
mutating operations on real user folders during automated verification.

Filesystem paths retain their original Unix bytes. Display labels are lossy
only at the UI edge. Directory reads and file operations belong off the UI
thread. Preserve source files until a cross-filesystem move has fully committed.
