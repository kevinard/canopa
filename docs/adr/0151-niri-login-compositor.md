# 0151. Run the login screen in the bundled Niri

Status: accepted

## Context

The greeter is an ordinary xdg toplevel, so it needs a compositor. It ran in Cage
(the packaged default) or Weston's kiosk shell, which made Cage a mandatory
dependency of `canopa-login` although the `canopa` package already installs the
Canopa Niri build at `/usr/lib/canopa/niri`. The switch from the login screen to the
desktop also passed through two unrelated compositors: a blank screen, usually a
modeset, and different output layouts on each side.

greetd owns login and session lifetime, and starts the session only after the
greeter process exits. One compositor spanning both sides would need a privileged
process across that boundary; GDM does not keep one compositor across it either.

## Decision

`canopa-greeter session` defaults to `--compositor niri`. Cage and Weston remain
supported choices, as optional dependencies.

Niri runs its startup command detached and does not exit with it, so for Niri the
launcher stays the parent of both processes instead of exec'ing the compositor. It
writes a generated configuration into a private directory under
`XDG_RUNTIME_DIR` (no key bindings, no startup programs, no Xwayland, no hot
corner, the power key left to logind, the greeter fullscreen), starts Niri with
`--config` and no `--session`, learns the Wayland socket from a reporter that Niri
spawns, runs the greeter as its own child and stops Niri when the greeter exits.
It waits on exactly those two processes through pidfds. Children receive SIGTERM if
the launcher dies.

The handoff is made as smooth as greetd allows:

- after greetd accepts the session, the greeter draws its plain background colour
  and exits once that frame is shown; Niri fills other outputs with the same
  colour, and the session's Niri defaults use it as their background;
- the launcher asks Niri to keep its last frame on exit
  (`NIRI_KEEP_LAST_FRAME_ON_EXIT`, a kevinard/niri change on a kevinard/smithay
  change using `DRM_IOCTL_MODE_CLOSEFB`); other Niri builds ignore the variable;
- `/etc/canopa/greeter-displays.kdl`, included only when root owns it, gives the
  login screen the user's display layout, so both sides can use the same modes.
  Settings writes it through `pkexec` and the `canopa-greeter-displays` helper,
  which accepts structured JSON and generates only `output` nodes, because the
  login compositor would execute anything else a Niri file can contain.

## Consequences

- `canopa-login` no longer depends on Cage; it installs the display helper and
  its polkit action. The greeter's configuration and authentication are unchanged.
- The login screen shows on the focused output only; other outputs show the
  background colour. Built-in touchpad workspace gestures can scroll the greeter
  out of view until the user swipes back.
- Niri uses more memory than Cage for the same greeter.
- Whether the last frame survives until the session's first frame depends on the
  kernel (CLOSEFB, Linux 6.9), on fbcon and on greetd switching the VT to text
  mode between sessions; the change makes the handoff no worse when it does not.
