# Login screen

`canopa-greeter` is a graphical login screen for [greetd](https://sr.ht/~kennylevinsen/greetd/).
It shares its look, layout, widgets and password field with the Canopa lock screen.
greetd owns PAM authentication, account policy, the switch to the user's account and
the desktop session's lifetime. The greeter runs as an unprivileged account inside a
compositor started for it (Canopa's Niri build by default, or Cage or Weston) and
shows one fullscreen window.

This guide is for administrators. The user-facing input keys are in
[shared authentication input](session-tools.md#shared-authentication-input).

## Install on Arch

The optional `canopa-login` package depends on `canopa`, `greetd` and `polkit`, and
installs:

| Path | Purpose |
| --- | --- |
| `/usr/bin/canopa-greeter` | The greeter (shipped in the `canopa` package) |
| `/usr/lib/sysusers.d/canopa-login.conf` | System account `canopa-greeter`, home `/var/lib/canopa-greeter` |
| `/usr/lib/tmpfiles.d/canopa-login.conf` | Creates `/var/lib/canopa-greeter` with mode 0700 |
| `/etc/canopa/greeter.kdl` | Login screen settings (kept on upgrade) |
| `/usr/share/canopa/greetd/canopa.toml` | greetd configuration to merge (Niri) |
| `/usr/share/canopa/greetd/canopa-cage.toml`, `canopa-weston.toml` | The same with Cage or Weston |
| `/usr/lib/canopa/greeter-displays` | Helper that sets the login screen's display layout |
| `/usr/share/polkit-1/actions/org.canopa.greeter.policy` | Administrator authorization for that helper |

The login screen runs in the bundled compositor `/usr/lib/canopa/niri` from
`canopa-compositor`, which `canopa` depends on. Cage and Weston are optional; install
one only to use its configuration.

The `canopa` package installs the session entry
`/usr/share/wayland-sessions/canopa.desktop`, which runs `/usr/bin/canopa-session`
with the bundled compositor `/usr/lib/canopa/niri`. The greeter lists it with the
other installed sessions.

`/usr/share/canopa/greetd/canopa.toml` contains:

```toml
[terminal]
vt = 1

[default_session]
command = "dbus-run-session /usr/bin/canopa-greeter session --compositor niri"
user = "canopa-greeter"
```

The package does not change `/etc/greetd/config.toml`, does not enable greetd and does
not replace the running display manager. To switch:

1. Check the settings and look at the result:

   ```sh
   canopa greeter check-config --show-source
   canopa greeter preview /tmp/login.png --width 1920 --height 1080
   canopa greeter list-sessions
   ```

2. Merge `canopa.toml` into `/etc/greetd/config.toml` (as root). The VT must not be
   used by another display manager or getty.
3. Make sure you can still log in another way (another TTY, or SSH) before rebooting.
4. Disable the current display manager and enable `greetd.service`, following the
   distribution's procedure.

greetd's PAM stack is `/etc/pam.d/greetd`. It must contain the normal
authentication, account and session policy, including `pam_systemd`.
`/etc/pam.d/canopa-lock` belongs to the lock screen and is not a login stack. For
unlocking the keyring at login, see the `canopa-oo7` package in
[installation](installation.md).

## Install manually

For other distributions or the release archive:

1. Install greetd, Niri (or Cage, or Weston 15 or later with `kiosk-shell.so`),
   fonts and the Wayland runtime libraries. greetd's PAM stack must run
   `pam_systemd`, which gives the greeter account the `XDG_RUNTIME_DIR` Niri needs.
2. Install `canopa-greeter` as a root-owned executable. The samples use the
   packaged path `/usr/bin/canopa-greeter`; adjust it for another prefix such as
   `/usr/local/bin`.
3. Create an unprivileged system account for the greeter with a private writable
   home. The samples use the packaged `canopa-greeter` account
   (`packaging/system/canopa-login.sysusers`); adjust `user` if you reuse the one
   your greetd package provides (often `greeter`).
4. Install `packaging/greetd/greeter.kdl` (release archive:
   `share/canopa/greetd/greeter.kdl`) as root-owned `/etc/canopa/greeter.kdl`.
5. Install a Canopa session entry in `/usr/local/share/wayland-sessions/` or
   `/usr/share/wayland-sessions/`. Outside the Arch package, use
   `packaging/wayland-sessions/canopa.desktop` (release archive:
   `share/wayland-sessions/canopa.desktop`), which starts Canopa through
   `niri-session`.
6. Merge one of the examples into `/etc/greetd/config.toml`, adjusting the path and
   account: `packaging/greetd/config.toml` (Niri), `config-cage.toml` (Cage) or
   `config-weston.toml` (Weston), shipped in the release archive under
   `share/canopa/greetd/`. Without `/usr/lib/canopa/niri`, the launcher runs
   `niri` from `PATH`.

```toml
[terminal]
vt = 1

[default_session]
command = "dbus-run-session /usr/local/bin/canopa-greeter session --compositor niri"
user = "greeter"
```

Images shown before login (backgrounds, avatars) must be readable by the greeter
account, for example under `/usr/local/share/canopa/`. Users' home folders are not
available.

## Compositor

`canopa-greeter session` starts the login screen's compositor. Niri is the default:
the launcher writes a configuration for it to a private directory under
`XDG_RUNTIME_DIR`, starts it, runs the greeter as its own child, and stops Niri and
exits when the greeter exits, so greetd can start the desktop. That configuration
binds no keys, starts no other program, turns off Xwayland and the hot corner, leaves
the power button to logind and opens the greeter fullscreen. The greeter appears on
the focused output; other outputs show its background colour. Cage and Weston replace
the launcher, run the greeter and exit with it.

Niri needs noticeably more memory than Cage or Weston for the same login screen, and
idles slightly higher. On a memory-constrained machine, choose Cage; it gives up the
kept last frame and the shared display layout.

| Option | Meaning |
| --- | --- |
| `--compositor niri\|cage\|weston` | Default `niri`: `/usr/lib/canopa/niri` when installed, otherwise `niri` from `PATH`, with `--config` and without `--session`. Cage runs as `cage -s -m last` (last connected monitor). Weston runs with `--shell=kiosk-shell.so --no-config --idle-time=0`. |
| `--renderer auto\|software` | `software` asks for software rendering: `LIBGL_ALWAYS_SOFTWARE=1` for Niri, `WLR_RENDERER=pixman` for Cage, `--renderer=pixman` for Weston. Default `auto`. |
| `--compositor-bin PATH` | Use another compositor executable. |
| `--compositor-arg ARG` | Extra compositor argument, repeatable, passed literally. |
| `--displays FILE` | Niri only: the display layout to include instead of `/etc/canopa/greeter-displays.kdl` (see below). |
| `--print-command` | Print the command (for Niri also the greeter's command and the generated configuration) and exit without starting anything. |

`--canopa-config` and `--defaults` are forwarded to the greeter. Plain
`canopa-greeter` (no subcommand) connects to an existing compositor; it needs
`GREETD_SOCK` and refuses to run as root.

### Switching to the desktop

When greetd accepts the session, the login screen replaces itself with its plain
background colour (the `view` `color`, by default the theme background) and exits
once that frame is on screen, so no account name or picture remains visible. With
Niri, the launcher asks the compositor to leave that frame on screen when it exits
(`NIRI_KEEP_LAST_FRAME_ON_EXIT=1`; Linux 6.9 or later), and the Canopa session's
Niri starts on the same colour until the wallpaper appears. Whether the frame
survives until the desktop draws depends on the kernel and on the text console:
greetd switches the VT to text mode between the two sessions, so a console drawn on
that VT can still show briefly.

### Display layout

With Niri, the login screen uses `/etc/canopa/greeter-displays.kdl` when it exists
and it and its directories are root-owned and not writable by group or others;
otherwise Niri picks each output's preferred mode. Using the same modes as the
desktop also avoids a mode change when the desktop starts. To copy your current
layout, open Settings → Displays and choose **Use on the login screen**; it asks
for an administrator password and copies what is applied, not unsaved changes.
The file holds only `output` blocks, for example:

```kdl
output "DP-1" {
    mode "2560x1440@143.912"
    scale 1.25
    transform "normal"
    position x=0 y=0
}
```

Remove it to go back to the defaults (`pkexec /usr/lib/canopa/greeter-displays
reset`, or delete it as root). The file is included in the login compositor's
configuration, so only root may write it: anything else a Niri configuration can
contain would run as the greeter account.

### Interface

The login screen uses the shared Iced session controls: hover feedback,
release-to-activate buttons, scrollable choosers and pointer selection in text
fields. Its wgpu renderer falls back to tiny-skia when GPU initialization is
unavailable. The `--renderer` launcher option selects the login compositor's
renderer; `ICED_BACKEND` selects the interface renderer. See
[shared authentication input](session-tools.md#shared-authentication-input).

## Configuration

The greeter reads `/etc/canopa/greeter.kdl`, never a user's `~/.config`. When the file
is missing it uses built-in defaults; when it is present it must be valid, and the
file and its parent directories must be owned by root and not writable by group or
others. `--canopa-config FILE` selects another file (also accepted from non-root
owners, for development); `--defaults` uses the built-in defaults.

The file uses the shared format: `schema-version 1`, an optional `appearance` block
(see [Configuration](configuration.md) and [Theme customization](theme-customization.md))
and a `greeter` block. `canopa config check` checks your own `config.kdl`; use
`canopa greeter check-config` for the system file.

```kdl
schema-version 1
appearance { color-scheme "dark"; }
greeter {
    discover-users #true
    user-source "auto"
    users "alice" "bob"
    default-user "alice"
    discover-sessions #true
    view {
        widgets "avatar" "keyboard" "battery"
        image "/usr/local/share/canopa/login.jpg"
        auth-timeout 120
        layout {
            top-left "clock" "date"
            center "avatar" "authentication"
            bottom-center "keyboard" "battery" flow="row"
        }
    }
}
```

| Key | Value | Default |
| --- | --- | --- |
| `discover-users` | boolean | `#true` |
| `user-source` | `auto`, `accountsservice`, `nss` | `auto` |
| `nss-uid-min`, `nss-uid-max` | integer | `UID_MIN`/`UID_MAX` from `/etc/login.defs`, else 1000–60000 |
| `nss-include-nologin` | boolean | `#false` |
| `users` | usernames, at most 64, listed first | — |
| `default-user` | username, preselected | — |
| `remember-selection` | boolean | `#true` |
| `user-avatars` | boolean | `#true` |
| `avatar-dir` | path | `/var/lib/AccountsService/icons` |
| `user-avatar "USER" "PATH"` | repeatable, at most 64 | — |
| `power-controls` | boolean | `#true` |
| `discover-sessions` | boolean | `#true` |
| `session-dirs` | up to 8 absolute paths | `/usr/local/share/wayland-sessions`, `/usr/share/wayland-sessions` |
| `xsession-dirs` | up to 8 absolute paths | `/usr/local/share/xsessions`, `/usr/share/xsessions` |
| `x11-wrapper { argv ...; }` | enables X11 sessions | — |
| `session "ID" { label; command; env ...; }` | manual session, at most 32 | — |
| `view { ... }` | presentation | see below |

`view` takes the lock screen's presentation keys (`image`, `fit`, `color`, `output`,
`dim`, `blur`, `font`, `clock-format`, `date-format`, `clock-size`, `text-size`,
`auth-timeout`, `avatar`, `weather-file`, `widgets`, `layout`, `widget`); see the
[`lockscreen` reference](session-tools.md#lockscreen). Differences from the lock
screen:

- Default widgets are `avatar`, `keyboard` and `battery`; the default layout centers
  clock, date, username, avatar, the password field and keyboard, with battery (and
  weather, when enabled) in a row at the bottom.
- `media` is refused and `use-wallpaper #true` is refused; set an explicit `image`.
  Without one, the login screen shows Canopa's built-in light or dark wallpaper,
  softened like the lock screen's; an explicit `color` or a high-contrast
  appearance shows a solid background.
- Custom script widgets run as the greeter account, without greetd's socket or
  authentication descriptors. Treat them as administrator configuration.
- `clock-format` and `date-format` default to the system language's formats.

## Users

A username can always be typed. In addition, the chooser lists accounts:

- `users` come first, in the given order.
- With `discover-users #true`, discovered accounts follow, sorted by display name.
  `user-source "auto"` asks AccountsService and falls back to NSS when it is missing,
  denied or fails. A successful but empty AccountsService answer stays empty.
  `accountsservice` or `nss` use only that source.
- AccountsService omits system accounts and root. NSS (libc `getpwent`, so
  `/etc/nsswitch.conf` applies) keeps UIDs between `nss-uid-min` and `nss-uid-max`,
  omits root, and omits `nologin`/`false` shells unless `nss-include-nologin #true`.
  Directory services that disable enumeration list nobody; typing still works.

These are display filters only. greetd and PAM decide who may log in.

Each source has a two-second deadline (up to four seconds in `auto` with fallback),
and the screen does not wait for it. At most 128 accounts are listed. The greeter
follows AccountsService signals and changes to `/etc/passwd`, `/etc/nsswitch.conf` and
`/etc/login.defs`. Press **F5** to refresh, for example for remote NSS sources.

```sh
canopa greeter list-users
canopa greeter list-users --source nss
```

## Sessions

With `discover-sessions #true`, the session chooser lists the `.desktop` entries in
`session-dirs`, sorted by name. Earlier directories hide entries with the same file
name in later ones. Entries with `Hidden`, `NoDisplay`, `Terminal` or
`DBusActivatable` set are skipped, as are entries whose `TryExec` or executable is
missing. Names and `Comment` follow the system language. Outside `--canopa-config`
development mode, entries and their directories must be root-owned and not writable
by group or others.

`Exec` follows the desktop-entry rules without running a shell. `DesktopNames`
becomes `XDG_CURRENT_DESKTOP`, the file name `XDG_SESSION_DESKTOP`, and
`XDG_SESSION_TYPE` is `wayland`. The list updates when entries are installed or
removed. Discovery reads at most 256 files of 64 KiB and lists at most 128 sessions.

Manual sessions appear first and override a discovered entry with the same ID (the
file name without `.desktop`):

```kdl
greeter {
    session "niri" {
        label "Niri with local setup"
        command { argv "/usr/local/bin/my-niri-session"; }
        env "XDG_SESSION_TYPE=wayland" "XDG_CURRENT_DESKTOP=niri"
    }
}
```

`command` also accepts a single string, split into words without a shell. Set `discover-sessions
#false` for a fixed list. With no session at all, the screen explains it and does not
start authentication.

X11 sessions are listed only with an `x11-wrapper`, a program that starts the X server
and runs the session command it receives as arguments:
`x11-wrapper { argv "/usr/local/libexec/my-xsession"; }`. X11 entries get IDs prefixed
`x11:`, "(X11)" labels and `XDG_SESSION_TYPE=x11`.

```sh
canopa greeter list-sessions    # resolved sessions as JSON
```

## Avatars

With `user-avatars #true` and the `avatar` widget placed, the selected user's picture
is the first readable image from: `user-avatar`, AccountsService's `IconFile`,
`avatar-dir/USERNAME`. Otherwise the view's `avatar` image or the user's initials are
shown. Files may lack an extension; each must be a raster image of at most 2 MiB,
2048 pixels per side and 4 megapixels. Images are read with the greeter account's
permissions and decoded in a separate helper; home folders are never searched.
Relative paths are relative to the configuration file. `user-avatar` entries do not
add users to the chooser.

```kdl
greeter {
    avatar-dir "/usr/local/share/canopa/faces"
    user-avatar "alice" "/usr/local/share/canopa/alice.png"
}
```

Users set their own picture on the Settings Users page, which stores it through
AccountsService.

## Using the login screen

| Key | Action |
| --- | --- |
| Enter | Continue or submit |
| Escape | Clear the field, then cancel the attempt and return to the account list |
| Up / Down | Move in the account list or chooser |
| Tab | Next desktop session |
| F5 | Refresh users, sessions and avatars |
| F6 | Next keyboard layout |
| F10 | Power menu (shut down, restart) |

With known accounts, the screen opens on a **Choose an account** card listing up to
four of them; clicking one or pressing Enter on it starts signing in. Typing or
**Use another account** shows a username field. Below the field, **Switch account**
(or **Choose an account** while typing a name) opens the full list, which also offers
**Use another account**; it is unavailable while a response is being checked. After
a rejected password the field stays: type again and press Enter. The desktop chooser
is in the bottom-left corner; keyboard layout, battery and power are in the
bottom-right, and they stay in place, unavailable, while a chooser or the power
options are open. When a layout region uses those corners, they move into the
sign-in column.

The power options open with keyboard focus on **Back**: Enter returns to sign-in,
and the arrow keys or Tab move to Shut down and Restart. The confirmation step also
starts on Back.

With `remember-selection #true` the greeter saves the last user who logged in and
each user's last session, only after greetd confirms the session started. At the next
start it opens directly on that user's password prompt with their session selected.
`default-user` takes priority but is only preselected. The file is
`$XDG_STATE_HOME/canopa/greeter.json` of the greeter account
(`/var/lib/canopa-greeter/.local/state/canopa/greeter.json` for the Arch account). It
holds no credentials or commands.

The power menu appears when logind answers `yes` to `CanPowerOff` or `CanReboot` for
the greeter account. Each action asks for confirmation. `power-controls #false` hides
it.

## Authentication

The greeter passes greetd's questions through in order: passwords, PINs, one-time
codes, visible questions and messages. Each prompt gets a fresh field. A fingerprint
or security key can complete without typing. `auth-timeout` (5–600 s, default 120)
limits each attempt; after a failure the next attempt waits two seconds from the start
of the previous one. A desktop starts only for the attempt currently on screen, and
the session command chosen at the start of the attempt is the one greetd receives; if
that entry changes or disappears meanwhile, the attempt is cancelled.

## Troubleshooting and recovery

- **The screen does not appear.** Check `journalctl -b -u greetd.service`. Run
  `canopa greeter check-config` as root to check `/etc/canopa/greeter.kdl`; an
  invalid file, or one owned by a non-root user, stops the greeter.
- **Niri does not start.** Check that the greeter's PAM session provides
  `XDG_RUNTIME_DIR` (`pam_systemd`), and run `canopa-greeter session --print-command`
  as the greeter account to see the generated configuration. To rule out the display
  layout, move `/etc/canopa/greeter-displays.kdl` away. Cage and Weston remain
  available with `--compositor cage` or `--compositor weston`.
- **Black screen or GPU errors.** Add `--renderer software` to the greetd command.
  Set `ICED_BACKEND=tiny-skia` in that command's environment to force software
  rendering for the login interface too. Use `--print-command` to see what is run.
- **No sessions listed.** Run `canopa greeter list-sessions`. Check that the
  session directories and entries are root-owned and not group- or world-writable,
  and that `TryExec` exists.
- **No users listed.** Run `canopa greeter list-users`. Typing a username always
  works.
- **Back to the previous login manager.** From another TTY (Ctrl+Alt+F2) or SSH,
  disable `greetd.service` and re-enable the previous display manager, or restore the
  previous `/etc/greetd/config.toml`.
- **Forget remembered choices.** Delete the greeter account's `greeter.json`.

## Known limitations

- The `canopa-login` package does not edit `/etc/greetd/config.toml` or enable greetd.
- Expired-password changes at login work only if greetd's PAM conversation offers them.
- Some greetd versions wait for a blocked PAM module before cancelling; give device modules finite timeouts.
- `packaging/greetd/config.toml`, `config-cage.toml` and `config-weston.toml` use `/usr/local/bin` and the `greeter` account, not the Arch layout.
- With Niri, the login screen shows on one output only, and touchpad workspace swipes can move it out of view until you swipe back.
- The last frame is kept only by the Canopa Niri build on Linux 6.9 or later; the text console of greetd's VT can still appear between the login screen and the desktop.
- Display layouts from Settings name outputs by connector (`DP-1`); after moving a monitor to another port, apply the layout again.
