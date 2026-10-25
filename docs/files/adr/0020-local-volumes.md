# Local volumes and explicit device operations

Status: accepted under delegated implementation authority; implementation and
acceptance in progress, 2026-10-16.

Files uses GIO's volume monitor for local storage discovery. GIO already owns
the platform storage backend and the user-interesting volume/mount catalog;
Files does not parse block devices, call privileged shell commands or introduce
a second system service. Monitor access and signal delivery stay on the GTK
main context, as required by the
[GIO contract](https://docs.gtk.org/gio/class.VolumeMonitor.html). Directory reads
and file operations retain their established worker boundaries.

Devices form a dynamic section after configured locations, available in both the
sidebar and the narrow-window Locations page. Mounted local roots open through
ordinary browser navigation. Unmounted local volumes mount only on explicit
invocation, using the native mount operation for credentials and cancellation.
There is no autorun or application-owned automount policy. Network-only volumes,
non-local mount roots and shadowed mounts are outside the initial local scope.
Mounts associated with a listed volume appear only once.

Mount, unmount, eject and drive stop use asynchronous GIO operations. Removal
prefers the backend's safe drive stop/eject capability and falls back to unmount;
Files never supplies a force flag. Failures remain visible and retryable. Active
or queued Files operations prevent removal, including custom actions whose
arbitrary filesystem effects cannot be bounded to a particular volume. New
Files jobs wait while a removal request is in flight. This conservative first
policy avoids guessing which untracked subprocess handles are safe to interrupt.
The initial implemented adapter currently supports mount and the unmount fallback;
drive stop/eject capability selection remains to be implemented and verified.
Device requests are separate from file-operation Undo.

Backend hotplug signals update the Devices sections without rebuilding directory
views or configured locations. A completed asynchronous mount may navigate its
origin only while that tab still has the same navigation intent; it must not
override a more recent navigation or reopen a closed tab. Device operations hold
the application while pending but monitor subscriptions alone do not.

Native acceptance uses a separately compiled GIO module implementing the public
volume-monitor extension point. It is loaded only in the private desktop's
process environment, with real files in that session's temporary directory.
The fixture exposes backend capabilities, failures and hotplug signals without
accessing any actual block device or changing a real mount. Its scope does not
prove physical unplug, kernel mount or power-off behavior; those require separate
isolated mounted-volume verification. Exact completed evidence and remaining
work belong in the implementation ledger.

The initial native workflow reproduced missing mounted-volume discovery, then
missing unmount and remount controls before implementing them. Acceptance exposed
two adapter defects: child button expansion widened the sidebar into the columns,
and closing a removal menu left focus on its toolbar button, preventing file paste.
The sidebar scroller now explicitly declines horizontal expansion. Device actions
return focus to the current listing; hotplug rebuilding restores a surviving
device control's focus after its new widgets are parented.

`native-devices-13` (dark) and `native-devices-14` (light, 1.25×) pass ten checks
with clean GTK logs: discovery and real-file browsing/preview, unmount without
force, remount, live disconnect/reconnect in the open narrow Locations page,
360-pixel keyboard navigation and floating preview, active-action removal refusal,
mount failure/retry, superseded navigation, zero-window pending-mount lifetime and
closed-origin protection, and transfer/action queue release after delayed removal.
The queue test reads the actual native clipboard URI list and checks independent
file bytes and subprocess output. Both completion notifications are retained.
`native-acceptance-27` passes the existing 27-check desktop regression with a clean
GTK log. Build, all-target Clippy, formatting, Python compilation and maintained
desktop inventory checks pass. The prior 97-test core checkpoint remains intact;
these changes affect the desktop adapter and its native system fixture.

`native-devices-15` additionally waits for both resumed jobs' public completion
notifications before capturing their final Operations state. Rendered inspection
confirms both completed statuses and the copy's Undo control. Visual inspection
also caught excessive device-name truncation in narrow Locations. Its scroller
now propagates natural width within a 200–280 logical-pixel bound.
`native-devices-16` passes the same ten checks at light/1.25× after this adjustment;
its rendered menu shows the full fixture name inside the unchanged 360-pixel
browser, and its settled Operations frame shows both completed jobs. Both final
logs are clean.

Stop/eject, encrypted-volume prompts/cancellation, mounted-root drop targets,
and complete disconnected-root identity/availability guards remain open.
[ADR 0021](0021-storage-placement-preflight.md) adds queued-request protection
against underlying mount-point writes, alias changes and replacement media,
including explicit Retry and restart cases. In-flight changes remain open.
A backend signal
fixture is distinct from physical unplug and kernel mount verification. In
particular, request preflight does not prove safety after a worker has started.
Remaining guards and capability cases must be finished before removable-drive
support is declared complete.
The private provider deliberately leaves its root directory present after
unmount, allowing this failure to be reproduced without mounting anything.
