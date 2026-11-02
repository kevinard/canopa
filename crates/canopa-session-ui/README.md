# Canopa session presentation

Shared software backgrounds/text, layout planning, shaping/font fallback, bounded erased secret
input with cursor/selection and best-effort memory locking, authentication prompt presentation and scoped widget observation. Used by
Canopa Lock, Canopa Greeter, Canopa Polkit and Canopa Pinentry, with no PAM execution or Wayland surface roles.
Native AT-SPI shares passive semantics and scoped action delivery.
Applications own authentication, configuration selection, input actions, privacy and
session lifetime. See [session tools](../../docs/session-tools.md) and
[greeter setup](../../docs/greeter.md).
