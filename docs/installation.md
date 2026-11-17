# Installing Canopa

Canopa has two installation layouts:

- **Arch packages** (x86-64): the supported system installation. Binaries live in
  `/usr/bin`, units in `/usr/lib/systemd/user`, and a dedicated **Canopa** login
  session starts everything.
- **Release archive / local companions**: the binaries in `~/.local/bin` with user
  units in `~/.config/systemd/user`, running inside a Niri session you start
  yourself. `./scripts/update.sh` keeps this layout current from a checkout.

Use one layout per account. Local user units in `~/.config/systemd/user` take
precedence over the packaged ones, so remove or disable them (keeping a backup)
before switching to the packages, and do it while logged out of Canopa.

## Arch packages

### Packages

| Package | Contents |
| --- | --- |
| `canopa` | All Canopa programs in `/usr/bin` (`canopa`, `canopa-shell`, the Wi-Fi, audio and media providers, `canopa-notify`, `canopa-files`, `canopa-settings`, `canopa-wallpaper`, `canopa-lock`, `canopa-idle`, `canopa-greeter`, `canopa-polkit`, `canopa-pinentry`, `xdg-desktop-portal-canopa`), the `canopa-session` launcher, user units, the session entry, Niri defaults, portal descriptors and routing, the private portal frontend, the WirePlumber screen-sharing policy, the `canopa-lock` PAM file, desktop entries, the `canopa-notify(1)` man page and the user guides in `/usr/share/canopa/`. |
| `canopa-compositor` | Niri built from the pinned `kevinard/niri` fork, installed privately at `/usr/lib/canopa/niri`. It adds the features Canopa uses: window previews, pinned columns, overview typing, spawn placement, screen sharing and input capture. |
| `canopa-login` | Optional greetd integration: the `canopa-greeter` system account (home `/var/lib/canopa-greeter`), `/etc/canopa/greeter.kdl`, greetd samples at `/usr/share/canopa/greetd/` (`canopa.toml` runs the login screen in the bundled Niri; `canopa-cage.toml` and `canopa-weston.toml` use Cage or Weston, optional dependencies), and the helper and polkit action that let Settings set the login screen's display layout. Depends on `greetd` and `polkit`. |
| `canopa-oo7` | Optional oo7 Secret Service setup for greetd logins: PAM hooks in `/etc/pam.d/canopa-oo7`, an early-start drop-in for `oo7-daemon.service`, Secret portal routing to `oo7-portal`, and a keyring prompter that uses Canopa's dialog. Depends on `canopa-login`, `oo7` and `gcr`. |

`canopa` depends on the matching `canopa-compositor`; Canopa talks to it over its IPC
socket and never needs the distribution's `niri` package. Optional dependencies add
features: `niri` (the stock `niri msg` CLI and a separate Niri session for recovery), `stasis` (automatic idle policy),
`networkmanager` or `iwd` (Wi-Fi), `bluez`, `power-profiles-daemon`,
`xwayland-satellite`, `poppler` (PDF previews in Files), `gnome-keyring`,
`geoclue`, `accountsservice`, `cups-pk-helper`, `avahi`, and the `gvfs-smb`,
`gvfs-mtp`, `gvfs-gphoto2` and `gvfs-nfs` backends for Files.

### Build the packages

Build as an ordinary user from a checkout. The source archive includes the
working tree's uncommitted changes.

```sh
python3 scripts/package-arch.py --output "$HOME/canopa-arch" --build
```

The output directory must not exist yet. The script writes a reproducible source
archive, a `PKGBUILD` with its exact SHA-256 and `.SRCINFO`, then runs `makepkg`
through the disk-space guard. It does not install packages or dependencies: install
the `makedepends` listed in the generated `PKGBUILD` with pacman first. Without
`--build`, run `makepkg` in that directory yourself.

`makepkg` downloads the pinned `kevinard/niri` and `kevinard/xdg-desktop-portal`
fork archives and checks them against the recorded checksums. Cargo fetches locked
Rust sources. The build reuses the checkout's `target/` directory; prepared fork
sources stay there, one directory per fork commit, so later builds reuse them.
Cargo uses one job unless `CARGO_BUILD_JOBS` is set. See [building](building.md)
for the disk-space reserves.

### Install

Install the desktop and compositor packages in one transaction, using the exact
file names `makepkg` produced:

```sh
sudo pacman -U ~/canopa-arch/canopa-VERSION-x86_64.pkg.tar.zst \
    ~/canopa-arch/canopa-compositor-VERSION-x86_64.pkg.tar.zst
```

Add `canopa-login` and `canopa-oo7` to the same command if you use them. Nothing
needs enabling: the units have no `[Install]` section and start only inside the
Canopa session.

## The Canopa session

Select **Canopa** in your login manager. From a text console, run `canopa-session`.

`canopa-session`:

1. Refuses to start if a compositor session (`canopa-compositor.service`,
   `niri.service` or `graphical-session.target`) is already active.
2. Sets `CANOPA_SESSION=1`, `NIRI_CONFIG=$XDG_CONFIG_HOME/canopa/niri.kdl`,
   `XDG_CURRENT_DESKTOP=canopa:niri`, `XDG_SESSION_DESKTOP=canopa` and
   `XDG_SESSION_TYPE=wayland`.
3. Runs `canopa-settings init-session` to create any missing compositor files.
4. Imports the environment into the systemd user manager and the D-Bus activation
   environment, and stops running portal services so they restart with the new
   desktop name.
5. Starts `canopa-compositor.service`, which runs `/usr/lib/canopa/niri --session`
   and pulls in `canopa-session.target`.

`canopa-session.target` starts `canopa-shell`, `canopa-notify`, `canopa-wallpaper`,
`canopa-lock`, `canopa-polkit`, `canopa-idle` and `canopa-display-profiles` (which
applies saved display profiles and the lid-close choice). Each one restarts
independently. The target only starts when `CANOPA_SESSION=1`, so other desktops
on the same account never start Canopa companions.

When the compositor exits, or `canopa-session` receives SIGINT, SIGTERM or SIGHUP,
it starts `canopa-shutdown.target` to stop the session and clears the imported
variables. `canopa-session --print-command` prints the environment and command
without changing anything.

To use the Canopa login screen with greetd, install `canopa-login` and follow
[the greeter guide](greeter.md). The package does not enable greetd or edit
`/etc/greetd/config.toml`.

### First login

The packaged compositor provides titlebars for applications that negotiate
server-side decorations. Applications with their own headerbars keep them.
See [compositor titlebars](configuration.md#compositor-titlebars) for styling and
the option to disable them. This requires the Canopa compositor; distribution
Niri does not understand the `titlebar` configuration block.

`canopa-settings init-session` creates these files in `~/.config/canopa/` (or
`$XDG_CONFIG_HOME/canopa/`) when they are missing, and never replaces existing ones:

| File | Purpose |
| --- | --- |
| `niri.kdl` | Compositor entry point. Includes `/usr/share/canopa/niri-defaults.kdl` and the three files below. |
| `niri-input.kdl` | Keyboard, pointer and cursor settings written by Settings. |
| `niri-displays.kdl` | Display layout written by Settings. |
| `niri-custom.kdl` | Your own compositor options and key bindings. |
| `stasis.rune` | Idle policy for `canopa-idle` (lock, displays off, suspend). |

After you change a shortcut in Settings, `niri.kdl` includes the generated
`niri-shortcuts.kdl` instead of the shipped bindings, and your changes are stored
in `shortcuts.kdl`. An existing `~/.config/niri/config.kdl` is never read or imported.

The shared configuration `~/.config/canopa/config.kdl` is optional. Without it,
every application uses its embedded defaults. Start from the commented sample with
`cp /usr/share/canopa/canopa.kdl ~/.config/canopa/config.kdl`, and see the
[configuration reference](configuration.md).

Useful default keys:

| Key | Action |
| --- | --- |
| Super+Space | Applications (the [launcher](launcher.md)) |
| Super+Return | Terminal (`xdg-terminal-exec`; `foot` is installed as a dependency) |
| Super+E | Canopa Files |
| Super+Comma | Canopa Settings |
| Super+L | Lock the screen |
| Super+O | Overview |
| Super+Q | Close window |
| Super+arrows / Super+Shift+arrows | Focus / move columns and windows |
| Super+Shift+E | Quit Niri (with Niri's confirmation) |

The Settings **Shortcuts** page lists and changes all of them.

Stasis is optional. Without `/usr/bin/stasis`, `canopa-idle.service` stays inactive
and the screen still locks on demand. Wi-Fi needs NetworkManager, or iwd with
systemd-networkd; Bluetooth needs BlueZ; power profiles need power-profiles-daemon.

## Desktop portals

In the Canopa session, xdg-desktop-portal routes requests to Canopa's backends
(`default=canopa;gtk;` in `/usr/share/xdg-desktop-portal/canopa-portals.conf`),
notifications to Canopa Notify, and Secret to `gnome-keyring` (or `oo7-portal` with
`canopa-oo7`). GTK serves the interfaces Canopa does not implement, such as Print and
Email. The packaged backend advertises ScreenCast, RemoteDesktop and Clipboard in
addition to its dialog portals.

The public `xdg-desktop-portal.service` uses a drop-in that starts the patched
frontend in `/usr/lib/canopa/` only for the Canopa session; other sessions keep the
distribution frontend. A user file `~/.config/xdg-desktop-portal/canopa-portals.conf`
overrides the packaged routing. After upgrading, log out of every session of the
account so user services load the new frontend and WirePlumber policy; if the user
manager stays running, restart `wireplumber.service` before sharing a screen.

See [portals](portals.md) and [desktop sharing](desktop-sharing.md).

## Optional FileManager1 activation

Canopa Files opens from the launcher and Super+E. The package keeps its
`org.freedesktop.FileManager1` activation file under `/usr/share/canopa/` so it does
not take over requests such as Show in Folder from another file manager. To select
Canopa Files for your account:

```sh
service=org.freedesktop.FileManager1.service
services="${XDG_DATA_HOME:-$HOME/.local/share}/dbus-1/services"
mkdir -p "$services"
ln -s "/usr/share/canopa/dbus-1/services/$service" "$services/$service"
```

Review any existing file at that path first. The link applies to the account's
session bus in every desktop and takes effect the next time the service is
activated, after the current owner exits. Remove the link to undo it.

## Optional oo7 keyring

`canopa-oo7` sets up oo7 as the Secret Service for greetd logins, unlocking the
keyring with the login password. It is meant for new oo7 keyrings and existing oo7
collections. Choose one Secret Service provider per account; when switching from
another provider, back up the existing keyring and try the migration on a test
account first.

1. Enable the daemon for each account that should unlock automatically:
   `systemctl --user enable oo7-daemon.service`. The Canopa drop-in starts it early
   and waits up to five seconds for its PAM listener. Keep the default socket
   settings in both the daemon and PAM.
2. Add these lines to `/etc/pam.d/greetd`, after the existing lines of each phase.
   The session line must follow the include that runs `pam_systemd.so`, and the
   auth line must be reached with `PAM_AUTHTOK` set.

   ```pam
   auth       include    canopa-oo7
   session    include    canopa-oo7
   ```

3. Log out and back in. The package routes Secret to `oo7-portal` in
   `/etc/xdg/xdg-desktop-portal/canopa-portals.conf`; update the Secret line in any
   user override of that file as well.

If the keyring stays locked after a login without a password (for example a
fingerprint), the first application that needs it asks for the passphrase. When
the login password changes, change the keyring passphrase separately. Logs:
`journalctl --user -u oo7-daemon.service -u oo7-portal.service` and
`journalctl -u greetd.service`.

To roll back, remove the `canopa-oo7` lines from `/etc/pam.d/greetd` before removing
the package, disable `oo7-daemon.service`, restore any portal override and the
previous provider, and remove the prompter link below.

### Canopa keyring dialog

To show keyring prompts with Canopa's dialog, link the prompter's activation file
for your account:

```sh
service=org.gnome.keyring.SystemPrompter.service
services="${XDG_DATA_HOME:-$HOME/.local/share}/dbus-1/services"
mkdir -p "$services"
ln -s "/usr/share/canopa/dbus-1/services/$service" "$services/$service"
```

The prompter shows Canopa's dialog when the account's active session is Canopa and
GCR's stock dialog otherwise. The change applies after the current prompter exits;
logging out of every session of the account starts a fresh bus. Remove the link to
return to GCR. See [pinentry](pinentry.md).

## Upgrading the packages

Build the new revision into a new output directory, log out, then install the new
`canopa` and `canopa-compositor` together (plus `canopa-login` and `canopa-oo7` if
installed). Pacman keeps modified `/etc/pam.d/canopa-lock`,
`/etc/canopa/greeter.kdl`, `/etc/pam.d/canopa-oo7` and
`/etc/xdg/xdg-desktop-portal/canopa-portals.conf` and offers `.pacnew` files for
review. Per-user files are never replaced. Keep the previous package pair for a
`pacman -U` rollback.

## Release archive

`bash scripts/package-release.sh target/release dist` (after a release build of the
workspace binaries) produces `dist/canopa-x86_64-unknown-linux-gnu.tar.gz`:

| Path | Contents |
| --- | --- |
| `bin/` | The same programs as the `canopa` package, plus `canopa-portal-config` (a portal routing merge helper). |
| `share/systemd/user/` | `canopa-session.target`, the Shell, Notify, Wallpaper, Lock, Polkit and Idle units, and `xdg-desktop-portal-canopa.service`. All expect `~/.local/bin`. |
| `share/dbus-1/services/` | Activation files for the portal backend, the Notify portal, Settings and FileManager1. |
| `share/xdg-desktop-portal/portals/` | Portal descriptors. |
| `share/applications/` | Files and Settings desktop entries. |
| `share/wayland-sessions/canopa.desktop` | A session entry that runs `niri-session`. |
| `share/canopa/` | User guides, `canopa.kdl`, examples, `stasis/stasis.rune`, `pam/canopa-lock.{arch,debian}` and `greetd/` samples. |
| `share/man/man1/canopa-notify.1` | Notify manual. |

The archive has no compositor, Niri defaults, `canopa-session` or portal routing
file. It runs on your own Niri session (`niri-session` with `niri.service`), with
your own `~/.config/niri/config.kdl`. Features that need the Niri fork (window
previews, pinned columns, overview typing, window restore) are unavailable on
upstream Niri.

### Install the companions

From the extracted directory:

```sh
install -Dm755 -t ~/.local/bin bin/*
install -Dm644 -t ~/.config/systemd/user share/systemd/user/*
install -Dm644 -t ~/.local/share/applications share/applications/*
install -Dm644 -t ~/.local/share/man/man1 share/man/man1/canopa-notify.1
sudo install -m644 share/canopa/pam/canopa-lock.arch /etc/pam.d/canopa-lock
canopa idle init
systemctl --user daemon-reload
systemctl --user enable canopa-session.target
```

Use `canopa-lock.debian` on Debian-based systems. `canopa-lock.service` and
`canopa-idle.service` do not start without `/etc/pam.d/canopa-lock`; `canopa idle
init` writes `~/.config/canopa/stasis.rune`, which `canopa-idle.service` needs, and
never overwrites it. Make sure `~/.local/bin` is on your `PATH`.

`canopa-session.target` is wanted by `graphical-session.target` and starts the
Shell, Notify, Wallpaper, Lock, Polkit and Idle units. The session launcher must
import `WAYLAND_DISPLAY` and `NIRI_SOCKET` into the user manager before those units
start; `niri-session` does this. In another launcher, run this inside the session
first:

```sh
dbus-update-activation-environment --systemd WAYLAND_DISPLAY NIRI_SOCKET XDG_CURRENT_DESKTOP
```

Stop any other notification daemon first: Notify fails to start when
`org.freedesktop.Notifications` is already owned. Use one startup mechanism per
program; do not also start `canopa-shell` from Niri's `spawn-at-startup`.

The `share/dbus-1/services/` files run programs through `/usr/bin/env`, so they
need `~/.local/bin` on the D-Bus activation environment's `PATH`. Copying
`org.freedesktop.FileManager1.service` to `~/.local/share/dbus-1/services/` makes
Canopa Files the account's file manager for Show in Folder requests. See
[portals](portals.md) for portal descriptors and routing.

### Update with `./scripts/update.sh`

For a local companion installation, `./scripts/update.sh` builds the current
checkout (including uncommitted changes; it does not fetch Git commits) and replaces
the installed binaries:

```sh
./scripts/update.sh            # one Cargo job, or $CARGO_BUILD_JOBS
./scripts/update.sh --jobs 4   # parallel Cargo jobs
./scripts/update.sh --rebuild  # rebuild the workspace packages; registry sources stay cached
```

It installs `canopa`, `canopa-shell`, `canopa-wifi-provider`,
`canopa-audio-provider`, `canopa-media-provider`, `canopa-notify`, `canopa-files`,
`canopa-settings`, `canopa-wallpaper`, `canopa-lock`, `canopa-idle`,
`canopa-greeter`, `canopa-polkit` and `canopa-pinentry` into `~/.local/bin`. It does
not install `xdg-desktop-portal-canopa`, units, D-Bus files or descriptors.

The updater:

1. Requires `canopa-notify.service` and `canopa-shell.service` to be loaded. The
   Wallpaper, Lock, Polkit and Idle units are handled when loaded and skipped
   otherwise.
2. Builds the release binaries and runs the new `canopa config check --show-source`
   before stopping anything.
3. Backs up the previous binaries, `config.kdl`, the service states and Notify's
   state directory under `$XDG_STATE_HOME/canopa-backups/<timestamp>/` (default
   `~/.local/state/canopa-backups/`).
4. Stops the services, swaps the binaries, and restarts the services that were
   running or failed. Stopped services stay stopped.
5. Checks that each service runs the installed executable and that Notify reports
   no startup or rendering error. On failure it restores the previous binaries and
   Notify state.

Keep the backups: Notify's database migrations are forward-only, so an older binary
needs the matching backup. Running Files windows keep the old executable until that
process exits. Only one update runs at a time (lock file
`~/.local/bin/.canopa-update.lock`). The updater assumes the standard
`~/.local/bin` paths and default configuration, socket and state locations; custom
service arguments need a manual update.

## Upgrading from Niribar

Canopa Shell was previously called Niribar. Rename these in your own units, Niri
configuration and scripts:

| Niribar | Canopa |
| --- | --- |
| `niribar` executable | `canopa-shell` |
| `niribar-wifi-provider` | `canopa-wifi-provider` |
| `niribar.service` | `canopa-shell.service` |
| `$XDG_RUNTIME_DIR/niribar.sock` | `$XDG_RUNTIME_DIR/canopa-shell.sock` |
| Layer namespaces `niribar`, `niribar-dismissal` | `canopa-shell`, `canopa-shell-dismissal` |
| `RUST_LOG=niribar=debug` | `RUST_LOG=canopa_shell=debug` |

These remain as fallbacks:

- `~/.config/niribar/config.kdl` is read when `~/.config/canopa/config.kdl` is
  absent. Move it into the shared file with `canopa config migrate`; see
  [configuration](configuration.md#migrate-legacy-files).
- `NIRIBAR_PERF` and `NIRIBAR_WIFI_PROVIDER_BACKEND` are read when
  `CANOPA_SHELL_PERF` and `CANOPA_WIFI_PROVIDER_BACKEND` are unset.
- The `niribar-v1` panel provider protocol name is unchanged.

For a local `~/.local/bin` installation, switch the service once, then use
`./scripts/update.sh` for later updates:

```sh
bash scripts/cargo.sh build --locked --release --bins -p canopa-shell -p canopa-notify -p canopa-cli
target/release/canopa config check --show-source
systemctl --user disable --now niribar.service
systemctl --user stop canopa-notify.service
install -m755 -t ~/.local/bin target/release/canopa-shell target/release/canopa-wifi-provider \
    target/release/canopa-notify target/release/canopa
install -m644 -t ~/.config/systemd/user packaging/systemd/canopa-shell.service \
    packaging/systemd/canopa-notify.service packaging/systemd/canopa-session.target
systemctl --user daemon-reload
systemctl --user enable canopa-session.target
systemctl --user start canopa-shell.service canopa-notify.service
```

Port any `niribar.service.d/` overrides to `canopa-shell.service.d/` first, and
remove a Niri `spawn-at-startup` line that starts the old shell. Notification
history, Do Not Disturb and appearance are unaffected.

## Recovery

If the Canopa session does not start or shows no shell, switch to a text console
(for example Ctrl+Alt+F3) and log in:

- Read the logs: `journalctl --user -b -u canopa-compositor -u canopa-shell -u canopa-notify -u canopa-lock`.
- Validate the compositor files: `canopa-settings check`.
- Validate the shared configuration: `canopa config check --show-source`.
- A broken managed include can be moved aside; `canopa-settings init-session`
  recreates missing files (it also regenerates `niri-shortcuts.kdl` from
  `shortcuts.kdl`). Keep the original for diagnosis.
- An invalid `config.kdl` never stops the Shell: it starts with its embedded
  defaults and shows a red badge until the file is fixed.

The distribution's Niri session remains available from the login manager as an
independent way in. A locked session stays locked if the locker crashes; do not
kill processes as a way to unlock it.

## Uninstall

Arch packages:

```sh
sudo pacman -R canopa-oo7 canopa-login   # if installed; undo the PAM edits first
sudo pacman -R canopa canopa-compositor
```

Local companions:

```sh
systemctl --user disable --now canopa-session.target
systemctl --user stop canopa-shell canopa-notify canopa-wallpaper canopa-lock canopa-polkit canopa-idle
cd ~/.config/systemd/user && rm canopa-session.target canopa-shell.service \
    canopa-notify.service canopa-wallpaper.service canopa-lock.service \
    canopa-polkit.service canopa-idle.service xdg-desktop-portal-canopa.service
systemctl --user daemon-reload
```

Then remove the Canopa programs from `~/.local/bin` and any files you copied into
`~/.local/share`. Neither method removes per-user data. Delete these yourself if
you no longer want them: `~/.config/canopa/` (configuration and compositor files),
`~/.local/state/canopa/` (Shell, wallpaper and portal state),
`~/.local/state/canopa-notify/` (notification history), `~/.local/state/canopa-files/`
and `~/.local/state/canopa-backups/`.
