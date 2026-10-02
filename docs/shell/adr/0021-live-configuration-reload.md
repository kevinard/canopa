# Live configuration reload

Date: 2026-10-02
Status: accepted

Canopa Shell applies saved configuration without a process restart. Automatic
reload and `canopa-shell reload` (also `--reload`) share one serialized candidate
preparation and installation path. This supersedes the shell restart requirement
in shared ADRs 0002, 0003 and 0008; Notify and the Settings portal retain their
independent reload contracts. There is no cross-process transaction.

The original selection remains authoritative: discovery is repeated, explicit
paths stay explicit, and embedded defaults never consult ambient files. A private
shell configuration runtime polls selected-source metadata off the UI thread at
250 ms intervals, accepting a change after two equal observations. Re-resolving
discovery and following target metadata handles initially absent configuration,
atomic editor saves, directory replacement and symlinks without attaching a watcher
to a soon-to-be-replaced inode. File bytes are read only to prepare candidates,
through the shared bounded reader and the shell's existing complete validator.
There is no new watcher dependency or shared settings service.

Only one candidate preparation runs at a time, on a blocking worker. Requests
arriving during preparation coalesce into one subsequent preparation; the earlier
candidate is discarded before installation. IPC callers wait for the newest result,
with a bounded responder set. A disconnected caller does not cancel installation.
Read, parse, validation and module construction failures retain the prior complete
configuration, source, generation and foreground. Runtime errors never include
configuration source values. The explicit local checker retains detailed diagnostics.

Installation first constructs the candidate Module Runtime, then commits in one
App update. Effective equality makes comment-only changes and unrelated namespace
edits no-ops for the UI, while still recording the new source fingerprint. Changed
configurations retire foreground content and pending external provider sessions,
clear obsolete focus and hover references, and reconcile Bar Surfaces against the
last complete Wayland Output Snapshot, including Outputs with no existing surfaces.
Unchanged physical plans retain their windows; changes to physical plans or order
replace the affected windows. Wayland effects remain asynchronous: success confirms
local installation, not compositor acknowledgement or a physically atomic repaint.

Unchanged configured Module Instances retain readings and subscription identities.
Changed or newly placed instances receive a new subscription generation; late
emissions from retired instances are ignored. Shared Module Data Sources retain
current readings and remap them using the new configuration, while removed sources
stop subscribing and late events cannot reactivate them. Workspace and Tray children
are rebuilt from their owners' snapshots. Manual idle inhibition retains its wanted
state while configured, and removal releases it through the existing adapter.

Appearance uses a replaceable immutable Iced theme snapshot. All shell palette
consumers read the installed theme; validators continue to have no process-state
side effects. The user guide documents reload timing, foreground dismissal, failure
behavior and explicit source selection.

Regression coverage crosses the configuration preparation, App update, module
subscription and filesystem-monitor interfaces. The maintained native scenario
`scripts/verify-configuration-reload.py` checks actual repaint, window retention,
geometry replacement, foreground behavior, discovery and explicit missing files
on an isolated compositor.
