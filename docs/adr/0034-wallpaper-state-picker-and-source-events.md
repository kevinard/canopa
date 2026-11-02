# Wallpaper state, picker and source events

Status: accepted; private two-output native acceptance recorded in the session-followups PRD.

Canopa Wallpaper owns output assignment policy and static layer surfaces. Its
ordinary picker window shares software text, controls and accessibility with the
session dialogs. It does not share the shell's lifetime or allocate a renderer in
the background daemon.

Saved assignments form a private, atomic user-state overlay over configuration.
An all-output assignment clears earlier output overrides; monitor-specific choices
remain attached to exact output names across disconnection. Temporary changes stay
out of saved state even when another output is saved. Explicit `reload` resets
this overlay and the optional slideshow to configuration; automatic source reload
preserves it. No image or configuration is replaced until all active output
buffers have passed admission. Failed candidates preserve the previous display.

One bounded inotify observer covers images, configuration and theme sources. It
coalesces changes while admission runs and acknowledges the metadata snapshot
observed before reading. Changes during admission remain pending. The observer
sleeps when nothing changes. Static rendering has no frame timer or retained
full-resolution source cache: each distinct source is decoded once per admission,
shared among output preparations, then released. Combined prepared surfaces are
limited to 256 MiB, alongside per-output and decoder limits.

`canopa wallpaper pick` previews all or a selected monitor using its actual aspect,
scale, fit and background color. A latest-request worker bounds directory scans
and preview work; cancelled/stale/failed previews cannot apply. Applying uses the
same daemon IPC and persistence path as the CLI. Cancelling has no daemon effect.

Slideshow is opt-in. Its bounded sorted directory catalog and timer exist only
while enabled; it does not save state on every frame. Invalid slides retain the
previous frame and are skipped on later advancement. Stopping saves the displayed
frame. This is a convenience policy in the wallpaper application, not a new shell
widget or an external process per monitor.

Private Sway acceptance covers landscape/portrait outputs, hotplug, source events,
persistence/temporary isolation, rollback, slideshow and the accessible picker.
Those fixtures do not establish physical DRM, suspend or distribution acceptance.
