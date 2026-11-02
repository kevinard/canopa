# 0026: Independent wallpaper and lock processes; shared widget data

Status: implemented, validation recorded in `.scratch/wallpaper-lockscreen/PRD.md`.

Canopa needs a low-resource dynamic wallpaper daemon and a configurable lockscreen
inspired by Veila. `canopa-session` provides two independent binaries, shared CPU
image/text preparation and Wayland plumbing. Neither starts the shell/Iced renderer.
Wallpaper uses wlr-layer-shell background surfaces with no input region or exclusive
zone, one private per-Wayland-display IPC endpoint, exact output assignments and
transactional configuration reload. There is no animation or idle redraw timer.
The compositor retains submitted shared-memory buffers; source pixels are dropped.

Lock uses ext-session-lock-v1 with one surface per output, PAM authentication plus
account validation, asynchronous one-at-a-time authentication, bounded/erased secret
input and compositor-confirmed readiness. Only explicit successful authentication
requests unlock. Authentication and graphics failures fail closed. Automatic idle
or suspend coordination remains external. This supersedes ADR 0014's default
`swaylock` command with `canopa-lock`; installation includes distro-appropriate PAM.
Veila is a behavioral reference; its GPL source is not incorporated.

`canopa-config` admits `wallpaper` and `lockscreen` namespaces. The owning binaries
validate them, using the shared appearance snapshot and configuration source rules.
The product CLI delegates commands to sibling binaries and includes both schema
owners in validation. Dynamic wallpaper assignments remain runtime state; reload
restores the selected configuration. Session processes and stores stay independent.

Shell widgets contained useful data tied to Iced adapters. `canopa-widgets` now
owns toolkit-independent battery/media models, UPower observation and a passive
selected-media observer, plus a bounded local weather adapter. Shell retains Module,
Panel, selection/service and control lifetimes. The lockscreen owns its own layout,
visibility/privacy choices and keyboard/authentication state. Shared widget data does
not transfer shell actions into the locked session. Explicit passive custom commands
and interactive authentication are added by ADR 0027.
Future surfaces can consume these types without depending on shell rendering.

Follow-on wallpaper persistence, the picker and event-driven source reload are
specified in [ADR 0034](0034-wallpaper-state-picker-and-source-events.md).
Shared session input/accessibility is specified in
[ADR 0032](0032-shared-authentication-input-and-accessibility.md).
