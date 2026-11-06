# Maintained receiver source-only replay

Exact helper/orchestrator snapshots, with no binaries or private queues. Canonical
builder, test, config and overlay live in root scripts/vendor. Execute from root
using stable paths and shared checksum stores. The current target pipeline requires
an explicit --receiver-build receipt; old prototype/intermediate executables are
losslessly archived and may be restored by checksum only when needed.

Public replays accept CANOPA_PRINT_SOURCE_GUARD. The historical native guard02
and final guard distinguish later documentation-only changes; use a current verified
guard for new runs and preserve historical receipts. Do not rerun the completed
workspace gate solely because notes/docs changed. Existing Files failures remain.
