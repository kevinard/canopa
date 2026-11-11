# Supported Arch installation

Arch Linux x86-64 is the first package target. The system installation is separate
from the existing `~/.local/bin` companion layout. Build packages as an ordinary
user from the checkout, including its current uncommitted source changes:

```sh
python3 scripts/package-arch.py --output "$PWD/.scratch/artifacts/canopa-arch" --build
```

The output directory must be new. The script creates a reproducible source archive,
an exact SHA-256 PKGBUILD and `.SRCINFO`, then runs makepkg without installing or
fetching system dependencies. Install missing build dependencies through pacman
before retrying makepkg in that directory. Cargo may fetch locked Rust sources.
makepkg downloads and checksums the pinned kevinard/niri and
kevinard/xdg-desktop-portal fork archives (and the portal's release tarball for
its bundled Meson subprojects); Cargo fetches Smithay from the kevinard/smithay
revision in Niri's lockfile. The guarded build reuses root `target/`, with
separate caches for the independently locked compositor and the Meson frontend.
Verified prepared sources remain beneath that target root, one directory per
fork commit, for reuse.

The build produces these packages.

- `canopa`: applications, Settings, services, session entry, PAM lock policy,
  portal defaults and the private XDG portal frontend.
- `canopa-compositor`: Niri built from the pinned kevinard/niri fork, installed
  privately at `/usr/lib/canopa/niri`, with Canopa screen sharing, input leases and
  the Files drag lifetime fixes. See [the fork builds](vendor-maintenance.md#niri-compositor)
  and [the combined companion decision](adr/0045-combined-niri-desktop-companion.md).
- `canopa-login`: optional greetd/Cage integration, a dedicated unprivileged
  `canopa-greeter` account and administrator-owned greeter settings/examples.
- `canopa-oo7`: optional Secret Service setup for greetd logins, with oo7's Secret
  portal and a keyring prompter that can use Canopa's dialog.

Install the desktop and compositor packages together with `pacman -U`, using the
exact generated package filenames. A distribution package manager owns system
updates and removal; `scripts/update.sh` continues to serve only the separate
local companion layout. Existing local user unit overrides take precedence over
system units: move/disable those overrides before switching installation methods,
keeping a backup for rollback. Do this after logging out, not during an active
transfer or capture session.

Select **Canopa** in the existing login manager. A TTY session can run
`canopa-session`. The launcher refuses an already active compositor, initializes
only missing Canopa files, validates them, imports the session environment and
waits for its dedicated compositor unit. Its target starts the companions after
compositor readiness. Logout stops the graphical target and clears the imported
session variables. Interrupt, hangup and termination of the launcher also stop its
owned session and reap its waiting client. No companion unit needs enabling globally; other desktop
sessions do not inherit a Canopa autostart link.

The initial compositor files are under `~/.config/canopa/` (or XDG_CONFIG_HOME):
`niri.kdl` includes system defaults, `niri-input.kdl`, `niri-displays.kdl` and
`niri-custom.kdl`. Settings owns the first two preference includes. Put additional
compositor options and key bindings in the custom file. After you change a shipped
shortcut, `niri.kdl` includes the generated `niri-shortcuts.kdl` in place of the
system defaults, and your changes live in `shortcuts.kdl`; see
[shortcuts](settings.md#shortcuts). Existing
`~/.config/niri/config.kdl` is never replaced or imported silently.

Useful defaults: Super+Space opens applications, Super+E opens Files,
Super+Comma opens Settings, Super+L locks, and Super+Shift+E requests logout.
Arrow keys with Super navigate windows; adding Shift moves them. Foot is the
default terminal. The shell also provides session/power confirmation menus.

Stasis is an optional external dependency for automatic idle policy. Install the
tested Stasis 1.7 implementation separately, as described in [idle integration](idle.md).
The packaged idle unit remains inactive when its executable is absent. Manual
locking works independently. Wi-Fi needs NetworkManager or the supported
networkd/iwd combination. Quick settings uses the system BlueZ service for
Bluetooth and Power Profiles daemon for power profile selection. Both are
optional Arch dependencies. Secret storage remains the chosen system provider;
packaging does not create or replace a password vault.

## Desktop portals

The Canopa session uses Canopa's portal backends by default, including its Iced
file chooser and screen sharing consent, and uses Canopa Notify for portal
notifications. ScreenCast, RemoteDesktop and Clipboard use the packaged Niri
and XDG frontend builds. GTK supplies interfaces such as Print and Email. The
Secret portal uses the selected keyring provider. Arch's GNOME portal and
Nautilus are outside the package dependency chain.

The public XDG portal service selects the private frontend for the dedicated
Canopa session. Other sessions use the distribution frontend. A user's
`canopa-portals.conf` takes precedence over the packaged routing, so review
older overrides if an application still opens a different dialog. Log out of
all sessions for the account after upgrading so the user services load the new
frontend and WirePlumber policy. If the user manager persists, restart
`wireplumber.service` before sharing.

## Optional FileManager1 activation

Canopa Files is available from the launcher and Super+E. The system package
keeps its FileManager1 activation file under `/usr/share/canopa/` so it can
coexist with Nautilus and other file managers. To select Canopa for on-demand
D-Bus requests such as Show in Folder, opt in for your account:

```sh
service=org.freedesktop.FileManager1.service
services="${XDG_DATA_HOME:-$HOME/.local/share}/dbus-1/services"
mkdir -p "$services"
ln -s "/usr/share/canopa/dbus-1/services/$service" "$services/$service"
```

Review any existing file at that user path before replacing it. The link
applies to the account's session bus across desktops, separately from the
default folder application. It takes effect on the next D-Bus activation after
the current FileManager1 owner exits. Remove the link to undo the opt-in.

## Optional Canopa login screen

Install `canopa-login` if you want the Canopa greetd frontend. Review
`/usr/share/canopa/greetd/canopa.toml`, then merge/select that configuration using
the distribution's greetd procedure. It uses the dedicated account and the
packaged executable. `/etc/canopa/greeter.kdl` controls presentation. Greetd's PAM
stack owns login policy; `/etc/pam.d/canopa-lock` is only the locker's policy.
Package installation does not enable greetd or replace the running login manager.
See [greeter setup](greeter.md) before changing display managers.

## Optional oo7 keyring

Choose `canopa-oo7` alongside `canopa-login` to use oo7 for desktop credentials.
Install the Arch `oo7` and `gcr` dependencies and the matching `canopa-oo7` package.
The `gcr` package provides the prompt protocol and the stock dialog. Choose one
Secret Service provider for the account before enabling the daemon. If switching
providers, back up the existing keyring and verify migration on a disposable
account or copy first. This setup is intended for fresh oo7 keyrings and existing
oo7 collections. Legacy GNOME Keyring migration has a separate lifecycle in oo7.

1. Enable `oo7-daemon.service` for each account that wants automatic unlock.
   From that account run `systemctl --user enable oo7-daemon.service`. The Canopa
   drop-in starts the service early and waits up to five seconds for its PAM
   listener. Use the packaged socket defaults in both the daemon and PAM.
2. Merge these optional hooks into `/etc/pam.d/greetd`, after the existing
   policy for each phase. In particular, the session hook follows the include
   that runs `pam_systemd.so`. A customized password authentication stack must
   reach the auth hook with `PAM_AUTHTOK` set.

   ```pam
   auth       include    canopa-oo7
   session    include    canopa-oo7
   ```

   The supplied `/etc/pam.d/canopa-oo7` uses optional oo7 hooks. Arch's greetd
   0.10.3 checks account policy and leaves password changes to a separate PAM
   service. When the login password changes, update the keyring passphrase
   separately to restore automatic unlock.
3. Log out and log back in. The package selects `oo7-portal` for the Secret
   interface in `/etc/xdg/xdg-desktop-portal/canopa-portals.conf`. Back up any
   administrator-owned file at that path before installing the package. The
   portal reads the first matching configuration file, so update the Secret
   entry in any higher-priority user override as well. The packaged configuration
   uses the same Canopa and GTK routing as the desktop package.

A matching login password opens the keyring automatically. If the keyring is
locked after a hardware-only login, the first application that needs it can ask
for its passphrase. Use both login methods on a test account to
verify the chosen PAM policy. `journalctl --user -u
oo7-daemon.service -u oo7-portal.service` shows the user-service logs. Check
`journalctl -u greetd.service` for PAM delivery and login errors.

To roll back, remove the `canopa-oo7` includes from greetd before removing the
opt-in package. Disable `oo7-daemon.service` for
the account and restore any portal override and prior provider. Keep the keyring
backup until applications can read their credentials again. Remove the user
prompter link described below if you enabled it.

[oo7 PAM documentation](https://github.com/linux-credentials/oo7/blob/main/pam/README.md) ·
[Arch oo7 package](https://archlinux.org/packages/extra/x86_64/oo7/files/) ·
[Arch GCR package](https://archlinux.org/packages/extra/x86_64/gcr/files/) ·
[Portal configuration precedence](https://man.archlinux.org/man/portals.conf.5.en)

### Canopa keyring dialog

To enable desktop-aware prompting for your account, link Canopa's activation
file into your user D-Bus service directory.

```sh
service=org.gnome.keyring.SystemPrompter.service
services="${XDG_DATA_HOME:-$HOME/.local/share}/dbus-1/services"
mkdir -p "$services"
ln -s "/usr/share/canopa/dbus-1/services/$service" "$services/$service"
```

If that path already contains a custom service file, review it before replacing
it. The new prompter takes effect the next time D-Bus activates the service
after its current owner exits. Logging out of all sessions for the account and
logging back in normally starts a fresh user bus.

Canopa's dialog appears in the account's active Canopa desktop. Other desktops
use the stock GCR dialog. Routing follows the active desktop even when an
application in another session requested the prompt. To restore GCR directly,
remove the link and let the service activate again.

## Upgrade, recovery and validation

Build the new revision into a new package directory, log out, then install the
matching desktop/compositor versions together. Include `canopa-login` and
`canopa-oo7` in the transaction if you use those packages. Pacman's backup handling preserves
modified PAM and greeter configuration and offers `.pacnew` files for review.
Per-user preferences are never installed over or reset. Keep the previous matching
package pair for `pacman -U` rollback. Keep the existing Niri session available as
an independent recovery route.

For a failed login, use a separate TTY and inspect
`journalctl --user -u canopa-compositor -u canopa-shell -u canopa-lock`.
`canopa-settings check` validates the user compositor configuration. Moving a bad
managed include aside and running `canopa-settings init-session` recreates only
missing files, apart from regenerating `niri-shortcuts.kdl` from `shortcuts.kdl`;
preserve the original for diagnosis. A locked compositor remains
locked if its locker dies; do not use process termination as an unlock method.

Private staging and nested compositor tests do not establish physical DRM/VT,
kernel suspend/resume, distribution login policy or real package-manager
upgrade acceptance. Those require a test installation and retain separate results.
