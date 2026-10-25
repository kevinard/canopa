# Running transfers check storage before work and commits

Status: accepted under delegated implementation authority; implementation and
acceptance in progress, 2026-10-16.

ADR 0021 protects request preparation and startup reconciliation. A running
transfer must also stop when storage changes while it copies, waits at a conflict,
or verifies a cross-filesystem move. A source-specific failure must not prevent
independent files from completing.

The operation engine accepts a worker-side placement check through
`transfer_with_check` and `resume_transfer_with_check`. The caller receives a
source entry or destination folder path, with explicit final-link semantics.
The native adapter supplies its captured storage snapshot, preserving original
device identity, mount epochs and browsing bindings. Standalone `transfer` and
`resume_transfer` retain their established filesystem behavior. Storage policy
stays outside the core journal schema and GTK stays outside the transfer engine.

Transfers check placement before each item, after conflict replies, during file
copying in 64 KiB steps, before metadata finalization, before commits and before
source quarantine/removal. Source removal checks again after verifying the
quarantined bytes. Typed placement failures distinguish an unavailable source
from an unavailable destination without interpreting error-message strings.
The destination is checked first when both locations might be unavailable.

A refused destination pauses with an explanation and exact continuation targets.
Completed changes remain in Undo and sources remain intact. A refused source
fails that item and permits independent sources to continue when journal cleanup
can safely finish. Desktop transfer preparation therefore checks destinations
and pending journal paths together, while the engine checks sources per item.
Retry failed items continues to use exact original destination names.

Once a copy stage is durably registered, its journal owns cleanup. Its temporary
directory no longer deletes through an unchecked path during unwinding. A refused
placement leaves unfinished owned data and its intent intact. Reconciliation,
including an attempted Continue while storage remains unavailable, checks every
pending journal location before mutation. A failure after source quarantine also
preserves the payload and intent; it must not attempt an unchecked rollback on
the unavailable device. Such unresolved work can temporarily block ordinary
mutations until reconciliation is safe; Recovery remains inspectable.

Continue after reconnection is a deliberate request with fresh availability and
the original media/browsing requirements from ADR 0021. It reconciles the
unfinished stage, then resumes only the exact remaining targets in a new Undo
entry. The completed prefix and resumed portion remain separately undoable.
Operations now also recognizes the configured Undo shortcut and uses the shared
latest-Undo/busy-state checks already used by Recovery.

The core regression first reproduced a copy ignoring a mid-file refusal, then
passed with preserved completed work and exact continuation. A second failing
assertion reproduced Continue cleaning the unavailable stage; it passes after
checking pending locations before replay. Another cross-filesystem regression
reproduced a missing post-quarantine availability check and now preserves the
actual source payload for recovery. Native cases independently reproduced an
active copy committing into the provider's unavailable backing folder and a
single offline source blocking an independent local file. Exact final evidence
belongs in the implementation ledger.

The final core suite passes 99 tests with eight crash helpers ignored at top
level and invoked by their parents. Build and all-target Clippy pass.
`native-device-inflight-25` (dark) and `native-device-inflight-28` (light, 1.25×)
each pass four checks covering actual staged bytes before disconnect, pause,
preserved prefix/sources, exact continuation and separate Undo. Source cases
`native-device-sources-20` (dark) and `native-device-sources-27` (light, 1.25×)
pass all three isolation/backing-folder/refused-source Retry checks.
`native-device-sources-29` repeats the light workflow with a settled completed
retry frame; rendered inspection confirms its own Undo control and the original
batch's independent completed-with-errors outcome.
`native-acceptance-32` passes the existing 27-check native regression.
Final Files logs contain no GTK warnings or assertions. Rendered inspection
confirms the pause explanation and Continue/Undo controls, and the completed
continuation with its own Undo control.

The 96 MiB debug-build Undo measured 34.529 and 35.924 seconds. Its fixture now
allows 60 seconds for that large-file reversal rather than the ordinary
15-second UI-readiness deadline. Earlier attempts were kept as failed evidence;
one light run received SIGTERM during Undo and was repeated. Four simultaneous
fixtures also exceeded their helper-readiness deadline before Files launched;
an isolated notification helper became ready in 0.36 seconds. No production
timeout or behavior was changed to hide either observation. Screenshot capture
waits for the GTK frame after completion rather than retaining a stale Starting
frame from just before notification delivery.

These are transfer checkpoints, not an atomic filesystem authority mechanism.
Pinned directory descriptors and relative filesystem mutations are still needed
to close changes between a check and a pathname syscall. This does not complete
Undo/Trash/restore/custom-action in-flight storage handling, persistent original
media associations for history and jobs, source clipboard/drag and chooser
bindings, unavailable browsing/preview, exact persisted unfinished jobs, or actual
kernel-mount/physical unplug verification. Partially committed cross-filesystem
moves still require phase-aware continuation/conflict work. Those requirements
remain open before complete device-loss safety can be claimed.
