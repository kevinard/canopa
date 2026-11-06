# Canopa session tools

`canopa-wallpaper` and `canopa-lock` are independent processes with a shared
software renderer and Wayland adapters. They do not start Iced or a GPU renderer.
Wallpaper sleeps on Wayland/control events; the locker starts only when invoked.
An optional `canopa-lock daemon` listens for logind's session-specific lock signal.
See [setup and configuration](../../docs/session-tools.md).

Wallpaper owns background layer-shell surfaces, output assignments and its
session-scoped command socket. Lock owns ext-session-lock surfaces, secret input,
PAM transactions, readiness and lock presentation. Neither owns idle/suspend policy.
`canopa-widgets` supplies passive data; consumers own privacy, rendering and actions.

Presentation now lives in `canopa-session-ui` and is also used by Canopa Greeter.
This crate retains the wallpaper/session-lock surface roles, PAM worker and logind
integration; the greeter uses greetd instead. Existing public module paths remain
available through reexports.
