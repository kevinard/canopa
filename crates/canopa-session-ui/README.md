# Canopa session presentation

Library shared by Canopa Lock, the login screen, the polkit agent, pinentry and the
wallpaper picker: Iced widgets with the standard wgpu compositor and tiny-skia
fallback, prepared backgrounds,
the nine-region layout planner, the Form, Sheet and Dialog views, hover and
press/release feedback, scrollable choosers, icon tooltips and pointer selection.
A dedicated secret-entry widget retains bounded secret input in best-effort locked,
non-dumpable memory, complex-script shaping and font fallback. The library also
provides keyboard layout selection, AT-SPI accessibility, passive widget
observation and supervised helper processes.

It runs no authentication and creates no Wayland surface roles; each application
keeps its own authentication, configuration and session lifetime. Native graphics
handles retain the role and connection until the graphics surface retires.
Graphics setup and presentation failures keep the native host alive and retry;
surface constructor panics select software for the affected output. Hosts call
`SessionView::changed()` after model changes and `tick()` from passive-data timers.
Pointer-only redraws reuse the widget tree. Prepared backgrounds retain a single
shared RGBA allocation, and native cursor shapes are cached until enter or change.

- [Shared authentication input](../../docs/session-tools.md#shared-authentication-input)
- [Session architecture](../../docs/architecture/session.md)
