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
before retrying makepkg in that directory. Cargo may fetch locked Rust sources;
the compositor recipe verifies the pinned Niri/Smithay archives and canonical
patch before building. Both builds use the permanent disk guard. The application
build reuses root `target/`; the independently locked compositor uses
`target/niri-compat-release/`. Verified prepared sources remain beneath that
target root and are checked again before reuse.

The three packages are:

- `canopa`: applications, Settings, services, session entry, PAM lock policy,
  portal selection and defaults.
- `canopa-compositor`: the pinned compatible Niri executable installed privately
  at `/usr/lib/canopa/niri`. It preserves the distribution's `/usr/bin/niri` and
  the existing ordinary Niri session. See [the compatibility decision](adr/0024-niri-drag-source-lifetime.md).
- `canopa-login`: optional greetd/Cage integration, a dedicated unprivileged
  `canopa-greeter` account and administrator-owned greeter settings/examples.

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
compositor options and key bindings in the custom file. Existing
`~/.config/niri/config.kdl` is never replaced or imported silently.

Useful defaults: Super+Space opens applications, Super+E opens Files,
Super+Comma opens Settings, Super+L locks, and Super+Shift+E requests logout.
Arrow keys with Super navigate windows; adding Shift moves them. Foot is the
default terminal. The shell also provides session/power confirmation menus.

Stasis is an optional external dependency for automatic idle policy. Install the
tested Stasis 1.7 implementation separately, as described in [idle integration](idle.md).
The packaged idle unit remains inactive when its executable is absent. Manual
locking works independently. Wi-Fi needs NetworkManager or the supported
networkd/iwd combination. Secret storage remains the chosen system provider;
packaging does not create or replace a password vault.

## Optional Canopa login screen

Install `canopa-login` if you want the Canopa greetd frontend. Review
`/usr/share/canopa/greetd/canopa.toml`, then merge/select that configuration using
the distribution's greetd procedure. It uses the dedicated account and the
packaged executable. `/etc/canopa/greeter.kdl` controls presentation. Greetd's PAM
stack owns login policy; `/etc/pam.d/canopa-lock` is only the locker's policy.
Package installation does not enable greetd or replace the running login manager.
See [greeter setup](greeter.md) before changing display managers.

## Upgrade, recovery and validation

Build the new revision into a new package directory, log out, then install the
matching desktop/compositor versions together. Pacman's backup handling preserves
modified PAM and greeter configuration and offers `.pacnew` files for review.
Per-user preferences are never installed over or reset. Keep the previous matching
package pair for `pacman -U` rollback. Keep the existing Niri session available as
an independent recovery route.

For a failed login, use a separate TTY and inspect
`journalctl --user -u canopa-compositor -u canopa-shell -u canopa-lock`.
`canopa-settings check` validates the user compositor configuration. Moving a bad
managed include aside and running `canopa-settings init-session` recreates only
missing files; preserve the original for diagnosis. A locked compositor remains
locked if its locker dies; do not use process termination as an unlock method.

Private staging and nested compositor tests do not establish physical DRM/VT,
kernel suspend/resume, distribution login policy or real package-manager
upgrade acceptance. Those require a test installation and retain separate results.
