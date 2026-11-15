# Canopa Files

Follow [root instructions](../../AGENTS.md). Before feature work, read
[the Files context](../../docs/files/CONTEXT.md) and the
[Files architecture](../../docs/architecture/files.md). Track work in
[GitHub issues](../../docs/agents/issue-tracker.md) labelled `area: files`.
Record significant decisions in [`docs/adr/`](../../docs/adr/README.md) and update
the architecture page in the same change.

The interface is built with the workspace's Iced stack and `canopa-ui`
(`src/iced_desktop/`, compiled as the `desktop` module). Keep filesystem and
application models independent of the renderer.

Run commands from the workspace root. `bash scripts/check.sh` defaults to the
fingerprint unit tests and `uutils_copy` integration tests. For other work use
`bash scripts/check.sh --files --lib FILTER` or
`bash scripts/check.sh --files --test CASE [FILTER]`; multiple `--test` selectors
run related modules in the shared integration executable. Direct Cargo commands
use `--test integration cases::CASE::`; native fixtures and `uutils_copy` retain their
own targets. Choose tests covering
the changed behavior; the default Copy checks alone do not validate other Files
features. Library/content-copy lifecycle changes also need the affected transfer,
Undo, Recovery and source-retention targets.

Keep incremental debug compilation enabled and reuse the same features and
`target/` cache throughout development. A bare `cargo test -p canopa-files FILTER`
still builds all enabled test targets, so keep explicit selection. Reserve
`bash scripts/check.sh --workspace` for the broader gates described in root
instructions; a Files-only task does not require a workspace rebuild by default.
Optimized performance measurements use an explicit release build separately.

Filesystem tests use private temporary directories and the public browsing,
transfer, trash, preferences, and custom-action interfaces. Never exercise
mutating operations on real user folders during automated verification.

Filesystem paths retain their original Unix bytes. Display labels are lossy
only at the UI edge. Directory reads and file operations belong off the UI
thread. Preserve source files until a cross-filesystem move has fully committed.
