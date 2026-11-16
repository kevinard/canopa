# Canopa Greeter

`canopa-greeter` is the Canopa login screen, a graphical client for greetd. It runs as
an unprivileged account inside a compositor started by
`canopa-greeter session` (Niri by default, or `--compositor cage|weston`), lists discovered users and
installed sessions, shows avatars, and passes greetd's PAM conversation through. greetd
owns authentication, account policy and the desktop session. Settings come from
`/etc/canopa/greeter.kdl`.

- [Login screen setup and configuration](../../docs/greeter.md)
- [Session architecture](../../docs/architecture/session.md)
