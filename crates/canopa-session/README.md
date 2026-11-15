# Canopa session tools

Three programs that run alongside the Canopa desktop:

- `canopa-wallpaper`: wallpaper daemon on wlr-layer-shell background surfaces, with a
  picker and an optional slideshow.
- `canopa-lock`: lock screen on ext-session-lock-v1 with PAM authentication (service
  `canopa-lock`), plus the logind lock listener `canopa-lock daemon`.
- `canopa-idle`: supervisor for the external Stasis idle daemon, coordinated with the
  lock listener before sleep.

The lock screen and picker use Iced’s standard wgpu compositor with a tiny-skia
software fallback. Rendering remains event driven.
The wallpaper daemon submits prepared images directly. The `canopa wallpaper`,
`canopa lock` and `canopa idle` commands forward to them. Presentation is shared with
the login screen, polkit agent and pinentry through `canopa-session-ui`.

- [Wallpaper, lock screen and idle guide](../../docs/session-tools.md)
- [Session architecture](../../docs/architecture/session.md)
