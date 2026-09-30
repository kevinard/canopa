# Canopa Settings portal

`xdg-desktop-portal-canopa` exposes Canopa's dark/light preference and effective
accent color to applications through `xdg-desktop-portal`. It reads the same
`appearance` block as Canopa Shell and Notify. Applications choose how to use these
preferences; the portal does not repaint their UI itself.

```kdl
schema-version 1
appearance {
    color-scheme "dark"
    accent "#89b4fa"
}
```

The common file is `$XDG_CONFIG_HOME/canopa/config.kdl`, with the usual
`~/.config` fallback. Without it, the shared dark preset applies. Application legacy
files are not portal sources. `--canopa-config /absolute/file.kdl` chooses an
explicit common file; `--defaults` ignores files. Invalid configuration prevents
startup. Other applications' namespace options are left to their own validators.

## Installation and backend selection

The release archive includes the executable under `bin/`, D-Bus activation metadata
under `share/dbus-1/services/`, the user unit under `share/systemd/user/`, and
`canopa.portal` under `share/xdg-desktop-portal/portals/`. The portal frontend must
already be installed. Package managers should install the descriptor in the
frontend's data directory, usually `/usr/share/xdg-desktop-portal/portals`, alongside
the other backend descriptors. This is separate from selecting a backend.

For the existing local companion layout, put the binary in `~/.local/bin`, the
activation file in `~/.local/share/dbus-1/services`, and the user unit in
`~/.config/systemd/user`. Reload the user unit definitions with
`systemctl --user daemon-reload`. The unit is activated on demand; it does not need
enabling. For another binary location, adjust the unit's `ExecStart`. On a session
bus without systemd activation, the fallback command finds the binary through the
bus daemon's PATH; use an absolute executable path in the activation file if needed.

In the portal configuration selected for the session, add this entry to its
existing `[preferred]` group:

```ini
org.freedesktop.impl.portal.Settings=canopa
```

Preserve the existing `default` and other interface entries. For a Niri session,
start from its current `niri-portals.conf` when making a user override at
`~/.config/xdg-desktop-portal/niri-portals.conf`. The frontend reads one selected
configuration file; an override containing only Settings does not merge the rest
of the distribution file. The [upstream selection guide](https://flatpak.github.io/xdg-desktop-portal/docs/portals.conf.html)
describes desktop-name precedence. Restart the portal frontend or log in again to
apply selection changes, at a time when no portal dialogs or capture sessions are
needed. Canopa never changes this selection automatically.

## Reload and verification

After editing the common file, reload the running backend:

```sh
systemctl --user reload xdg-desktop-portal-canopa.service
```

For a manually launched process, send SIGHUP to that process. Changed values emit
`SettingChanged`; unchanged values emit no signal. An invalid candidate leaves the
last valid values available and records `event=reload_rejected` in the service log.
The systemd reload command acknowledges signal delivery, not successful validation;
check the journal or read the portal value when confirming a change.
Discovery is repeated on reload: creating the common
file adopts it, and removing it restores shared defaults. Explicit paths never
fall back if their file disappears. Restart Canopa Shell and reload Notify separately
to apply their own settings; this is not an atomic desktop-wide reload.

Read the value applications receive through the frontend:

```sh
gdbus call --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.portal.Settings.ReadOne \
  org.freedesktop.appearance color-scheme
```

Dark is unsigned integer 1 and light is 2. `accent-color` returns three doubles in
[0,1]. Canopa does not currently publish contrast or reduced-motion preferences.
The backend's interface version is 1; the frontend supplies public Settings version
2 and `ReadOne`. This distinction follows the separate upstream interfaces.

The developer fixture tests actual activation and public values on a private bus:

```sh
python3 scripts/test-settings-portal.py --binary target/debug/xdg-desktop-portal-canopa
```

It needs `xdg-desktop-portal`, `dbus-daemon` and Python GI, and never touches the live
desktop bus, user configuration, or service manager. See [ADR 0008](adr/0008-settings-portal.md).
