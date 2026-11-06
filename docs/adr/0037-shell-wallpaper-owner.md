# 0037: Shell-owned wallpaper images and surfaces

Status: accepted for the Shell owner, with scoped unit/integration/native validation.
The public Wallpaper bridge is now implemented under ADR0038.

Canopa has no wallpaper owner. Writing GNOME preferences would not change the
Niri desktop or the Swaylock image. Shell will own these selections independently
of bar configuration, with full-resolution Iced background surfaces on every
named Wayland Output. They take no keyboard or pointer input and reserve no space.
Wallpaper Surfaces have their own registry; they are not Bar Surfaces.
When a new wallpaper surface receives its first configure, Shell recreates only
the bars on that output using the Background layer, so Niri stacks them above the
wallpaper. The existing retirement path closes foreground content belonging to
those bars. Other bars, later image replacements and scale changes retain windows.

The private `org.canopa.Shell.Wallpaper1` interface at
`/org/canopa/Shell/Wallpaper`, on the `org.canopa.Shell.Wallpaper` session-bus
name, admits a regular PNG descriptor and a destination mask (1 background,
2 lockscreen, 3 both). One bounded worker owns preparation and installation;
queued work survives a disconnected caller. It validates and copies the descriptor
before publication. No caller path is retained. Success confirms durable selection
and local App installation, not compositor acknowledgement or atomic repaint.
An owner failure is an error; a durable commit followed by process failure may
become visible at the next startup.

Images are canonical PNGs addressed by SHA-256 under
`$XDG_STATE_HOME/canopa/shell/wallpaper` (fallback `$HOME/.local/state`). An atomic
versioned selection file changes both requested destinations together, preserving
the other destination. Cooperative readers and writers use one file lock. Images
are installed before selection publication. Cleanup only removes unreferenced
image names under that lock. Limits bound input, dimensions, pixels and codec
allocations (128 MiB PNG, 16,384 pixels per dimension, 64 Mi pixels, 512 MiB
best-effort decoder allocation). These are logical bounds, not filesystem deadlines
or total process/GPU memory caps. Full wallpaper pixels are independent of future
portal thumbnails. Interrupted temporary writes are reclaimed at the next install.
This is runtime state, not user-editable configuration or an external-editor CAS.

`canopa-shell lock` opens the selected lock image while holding the state lock and
executes Swaylock with an inherited descriptor. Unlinking an old image cannot
invalidate a locker already starting. Invalid or unavailable appearance state
falls back to Swaylock without an image; a busy state lock also uses that fallback
instead of waiting behind an image installation. Appearance failure must not
disable locking. The bundled Lock action uses this entry point through the existing
session-owned Launch adapter. Explicit user launch commands remain literal argv.
No Niri, Swaylock or user bar configuration is rewritten.
Existing actions and compositor bindings must opt into `canopa-shell lock` to use
this selection; custom locker routes retain their own appearance behavior.

The portal freezes the caller's image before Iced review and passes the reviewed
owned image to this owner after acceptance. It must honor every requested target
and must not advertise Wallpaper until real output pixels, restart, cancellation,
owner loss and the locker handoff pass isolated native acceptance. These now pass
in ADR0038's scoped frontend integration; this owner remains separate from portal
registration and permissions.
