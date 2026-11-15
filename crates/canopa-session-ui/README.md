# Canopa session presentation

Library shared by Canopa Lock, the login screen, the polkit agent, pinentry and the
wallpaper picker: software backgrounds and text (complex-script shaping, bidirectional
layout, font fallback), the nine-region layout planner, the Form, Sheet and Dialog
views, bounded secret input in locked non-dumpable memory, keyboard layout selection,
AT-SPI accessibility, passive widget observation and supervised helper processes.

It runs no authentication and owns no Wayland surface roles; each application keeps
its own authentication, configuration and session lifetime.

- [Shared authentication input](../../docs/session-tools.md#shared-authentication-input)
- [Session architecture](../../docs/architecture/session.md)
