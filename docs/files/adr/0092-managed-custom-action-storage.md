# 0092: Managed custom-action storage

Status: scoped backend and native verification, 2026-10-19. Extends
ADR0078/0082/0091. The full product and execution-authority gates remain open.

Custom actions in the owning Application use the same prepared Storage as file
operations. Shared removal scheduling prevents commands from starting during
removal, but an old directory can remain accessible after its device disappears.
Canonicalizing that directory alone does not establish current availability.

OperationJobs publishes its existing prepared store through a cloneable,
read-only PreparedStorage handle. Application supplies that handle to
LocalActions. Submission reserves its shared activity hold, then captures an
in-memory storage snapshot and the original working-folder binding. Missing
preparation or known observer failure refuses submission without creating work.
There is no fallback to unprotected execution in the managed application.
Storage publication remains independent of history startup: valid observation
does not grant filesystem history or Undo authority.

After any removal wait, a command worker checks the captured folder binding,
prepares the action, and validates the working folder, each selected entry and
each selected referent against current storage observations and kernel mount
identity. Checking both forms covers a local symlink into an unavailable device
and an unavailable device's symlink into a local folder. Validation and command
execution stay off the frontend thread. Cancellation is checked during storage
validation and retains the existing normal cancelled result.

The private execution owns its snapshot until actual worker retirement. Public
job rows retain only descriptive results. A remount never replays commands; a
new explicit invocation captures current storage. Standalone ActionJobs and
LocalActions constructors remain usable without a platform observer; Application
always selects the managed constructor.

This validates the paths Files passes to a custom action. Selected symlinks are
resolved during worker validation; this does not pin their targets at invocation
or establish file-content identity. It does not sandbox arbitrary script effects,
make validation and process start an atomic filesystem transaction, or authorize
durable command replay. Existing global scheduling still refuses device removal
during accepted commands, including commands that access paths beyond the selection.

The public runtime fixture uses Application without Iced, an isolated GIO
provider and real private files. Its original regression ran a queued command
after successful unmount and created output in the surviving old directory.
The prepared-storage guard passes that regression. Expanded checks cover selected
entries, both directions of symlinks, unrelated local work, a fresh invocation
after remount, and refusal after helper loss. Forty renderer-free integration
parents and 73 default Iced library tests pass, along with the public runtime
program's eight check groups. Ignored private helper definitions are not counted.
The default build, all-target Clippy with warnings denied, workspace formatting,
native inventory and locked dependency inventory pass on the same inputs.

The same executable passes 55 native checks: helper commands10, catalog5, custom
actions15 in each appearance, headless residency4 and resident activation6.
Representative narrow output and the reopened waiting decision were inspected.
All 636 input hashes and the application hash match before and after acceptance.
This stage received primary review only. Device attention and Iced controls,
physical-device acceptance, original source and restart authority, production
performance and the full product gates remain open. Exact attempts and evidence
are recorded in the implementation ledger and action-placement-verification.json.
