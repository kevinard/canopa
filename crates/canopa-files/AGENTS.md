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

Run commands from the workspace root. Use `cargo test -p canopa-files` for
focused checks and `bash scripts/check.sh` for final workspace validation.
Filesystem tests use private temporary directories and the public browsing,
transfer, trash, preferences, and custom-action interfaces. Never exercise
mutating operations on real user folders during automated verification.

Filesystem paths retain their original Unix bytes. Display labels are lossy
only at the UI edge. Directory reads and file operations belong off the UI
thread. Preserve source files until a cross-filesystem move has fully committed.
