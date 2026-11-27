# Wallpaper, lock screen and idle

Three programs from the `canopa-session` crate run alongside the Shell:

| Program | Role | Forwarding command |
| --- | --- | --- |
| `canopa-wallpaper` | Wallpaper daemon, picker and slideshow | `canopa wallpaper ...` |
| `canopa-lock` | Lock screen and the logind lock listener | `canopa lock ...` |
| `canopa-idle` | Supervisor for the external Stasis idle daemon | `canopa idle ...` |

`canopa SUBCOMMAND ...` runs the sibling binary installed next to `canopa`, passing
every argument through (including `--help`). The lock screen and picker use Iced’s
standard wgpu compositor with tiny-skia software fallback. The wallpaper daemon
submits static images without a GPU device. The wallpaper needs a compositor
with wlr-layer-shell, the locker one with ext-session-lock-v1. Niri has both.

The login screen, polkit agent and pinentry share the lock screen's presentation;
see [the login screen](greeter.md), [polkit](polkit.md) and [pinentry](pinentry.md).

## Starting the tools

**Arch package.** The binaries are in `/usr/bin` and the user units in
`/usr/lib/systemd/user`. The units have no `[Install]` section: the Canopa session
(`canopa-session.target`) starts `canopa-wallpaper`, `canopa-lock` and `canopa-idle`
with the Shell, Notify and the polkit agent. There is nothing to enable. The package
installs the PAM service `/etc/pam.d/canopa-lock`.

**Local layout** (`~/.local/bin`, installed by `scripts/update.sh`). Copy
`packaging/systemd/*.service` and `packaging/systemd/canopa-session.target` into
`~/.config/systemd/user/`, install the PAM service (see [Lock screen](#lock-screen)),
then:

```sh
systemctl --user daemon-reload
systemctl --user enable canopa-session.target
```

The target is wanted by `graphical-session.target` and wants the Shell, Notify,
wallpaper, lock, polkit and idle units. The lock and idle units stay inactive until
`/etc/pam.d/canopa-lock` exists.

## Wallpaper

```sh
canopa wallpaper set ~/Pictures/mountains.jpg --output DP-1
canopa wallpaper set ~/Pictures/forest.png --all --fit fit
canopa wallpaper set ~/Pictures/test.png --all --temporary
canopa wallpaper status
canopa wallpaper pick
```

| Command | Effect |
| --- | --- |
| `daemon [--canopa-config FILE \| --defaults]` | Run the daemon in the foreground (what the unit runs). |
| `set IMAGE (--output NAME \| --all) [--fit MODE] [--temporary]` | Show an image. `--fit` defaults to `fill`. Stops a running slideshow. |
| `pick [IMAGES...]` | Open the graphical picker. |
| `slideshow DIR (--output NAME \| --all) [--interval S] [--fit MODE]` | Cycle through a folder. |
| `next` | Advance the slideshow now. |
| `stop-slideshow` | Stop the slideshow and keep (and save) the current image. |
| `status` | Print outputs, their size, scale and image/fit, the slideshow and the background color as JSON. |
| `reload` | Re-read the configuration and keep saved choices (see below). |
| `reset` | Return to the configured wallpaper. **This erases saved choices** (see below). |
| `stop` | Stop the daemon. |
| `check-config [--canopa-config FILE \| --defaults] [--show-source]` | Validate the `wallpaper` namespace without connecting to Wayland. |

Output names are the compositor's connector names (`DP-1`, `eDP-1`); `status` lists
them. A choice for a disconnected output is kept and applied when it reappears.
`--all` replaces the default image and clears every per-output choice, so monitors
connected later get it too.

Canopa shows its canopy wallpaper in a light or dark variant when an output has
no image assignment. An explicit `wallpaper { color "#10191d"; }` uses a solid
color on unassigned outputs. High contrast also uses a solid background for those
outputs. Your configured images and saved choices take precedence.

Fit modes: `fill` crops centrally to cover the monitor, `fit` shows the whole image
with letterboxing in the background color, `stretch` ignores proportions, `center`
uses the original pixel size and crops if larger. PNG and JPEG are supported, EXIF
orientation is applied, and transparency is composited onto the background color.

### Saved choices and reload

`set`, the picker and `stop-slideshow` save their result in
`$XDG_STATE_HOME/canopa/wallpaper.json` (else `~/.local/state/canopa/wallpaper.json`),
and the daemon restores it at the next start. The saved choices sit on top of the
`wallpaper` block in `config.kdl`. `set --temporary` changes only the running daemon.

The daemon watches `config.kdl`, its theme file, the images in use and the slideshow
folder. A valid change applies at once and keeps your saved choices. An invalid
configuration or image keeps the current wallpaper.

`canopa wallpaper reload` does the same on request and reports an invalid
configuration as an error. `canopa wallpaper reset` is different: it re-reads the
configuration, **discards every per-output and `--all` choice, stops the slideshow
and overwrites `wallpaper.json` with an empty state**. Use it to return to what
`config.kdl` says.

Limits per image: 64 MiB encoded, 16384 pixels per side, 32 megapixels, 256 MiB of
decoder memory. All prepared monitor surfaces together are limited to 256 MiB.

### Picker

`canopa wallpaper pick` (or **Choose wallpaper…** on the Settings Personalization
page) opens a window that previews the selected monitor, or all of them, at their real
proportions. Previewing and closing do not change anything; **Apply** sets and saves
the choice through the daemon.

| Key | Action |
| --- | --- |
| Left / Right (Page Up / Page Down) | Previous / next image |
| Up / Down | Cycle the monitor selection (one monitor or all) |
| F | Cycle the fit mode |
| Tab | Move between controls |
| Enter | Activate the control |
| F5 | Rescan images and monitors |
| Escape | Close |

Without arguments the picker offers the active wallpapers plus the PNG and JPEG
pictures in the pictures folder (`XDG_PICTURES_DIR` from `user-dirs.dirs`, else
`~/Pictures`) and its `Wallpapers` folder, `~/.local/share/backgrounds` and
`wallpapers`, and the `backgrounds` and `wallpapers` folders of each `XDG_DATA_DIRS`
entry (`/usr/share/backgrounds`…). Arguments (files or folders, up to 256) replace
that list; when the first is a file, the picker starts on it. Folders are searched
two levels of subfolders deep, skipping hidden ones; scans stop after 4096 entries
and 512 images.

### Slideshow

`slideshow DIR` shows the PNG and JPEG files of a folder in name order (at most 256
from the first 2048 entries), changing every `--interval` seconds (10–86400, default
300). The slideshow itself is saved, not each image. An unreadable image keeps the
previous one on screen and is skipped next time.

## Lock screen

### PAM service

Canopa Lock authenticates with the PAM service `canopa-lock`. The Arch package
installs `/etc/pam.d/canopa-lock` (it includes `system-auth` for `auth`, `account`
and `password`). For other layouts, copy the matching example as root:

| Distribution | Example | Release archive path |
| --- | --- | --- |
| Arch | `packaging/pam/canopa-lock.arch` | `share/canopa/pam/canopa-lock.arch` |
| Debian, Ubuntu | `packaging/pam/canopa-lock.debian` | `share/canopa/pam/canopa-lock.debian` |

Other distributions should use their standard screen-unlock stack. There is no setuid
helper. The lock and idle units do not start while the file is missing.

### How locking starts

| Trigger | What runs |
| --- | --- |
| Quick settings **Lock** button | `canopa-lock` |
| Mod+L (packaged Niri bindings) | `canopa-lock` |
| Launcher session action **Lock** | `canopa-lock`, or `loginctl lock-session` when `canopa-lock` is not installed |
| `loginctl lock-session`, logind `Lock` (other programs, the Settings lid choice **Lock the screen**) | the listener `canopa-lock daemon` starts the locker |
| System suspend or hibernate | the listener locks before sleep |
| Idle timeout (Stasis) | `canopa-lock` |
| `canopa-shell lock` | `canopa-lock` (or `loginctl lock-session`); `--swaylock` runs Swaylock with the portal's lock image instead |

Plain `canopa-lock` first attaches to the listener's locker, so every path above
except `canopa-shell lock` shares one lock screen. Without a listener it locks
directly. The command returns after you unlock.

```sh
canopa lock                       # lock now
canopa lock ensure                # lock and wait until the compositor confirms it
canopa lock status                # listener state as JSON
canopa lock preview /tmp/lock.png --width 1920 --height 1080 --monitor DP-1
canopa lock check-config --show-source
```

| Option or command | Meaning |
| --- | --- |
| `--canopa-config FILE`, `--defaults` | Select the configuration (bypasses the listener). |
| `--session ID` | logind session to lock and report to. |
| `--ready-fd FD` | Write one byte to an inherited descriptor once the compositor confirms the lock. |
| `daemon` | Run the logind listener (what `canopa-lock.service` runs). |
| `ensure [--timeout-ms 100..30000]` | Request a lock and wait for confirmation (default 5000 ms). |
| `status` | JSON with the session, readiness, locker PID and sleep coordination. |
| `preview OUTPUT.png [--width] [--height] [--monitor NAME]` | Render without locking or starting PAM. |
| `check-config [--show-source]` | Validate the `lockscreen` namespace. |

An invalid `lockscreen` block never prevents locking: the locker prints the error and
uses the defaults. The configuration is read each time the screen locks.

### Unlocking

Type the response and press Enter. Escape clears the field; on an empty field it
cancels the PAM attempt, and Enter starts a new one. Both PAM authentication and the
account check must succeed. After a failure, the next attempt waits until two seconds
after the previous one started, on top of PAM's own delays. `auth-timeout` (default
120 s) bounds each whole attempt.

PAM can ask several questions in turn (password, PIN, one-time code); each gets a
fresh field. Authentication starts as soon as the lock is confirmed, so a fingerprint
or security key works without typing; the module's message appears while it waits.
Cancellation, timeout or a crashed worker keep the screen locked. If PAM reports an
expired password, the lock screen asks before running the PAM password stack; the
change and a fresh account check must both succeed to unlock. Cancel keeps the
session locked.

Hardware factors come from the installed PAM modules. For example, an Arch policy that
requires both the system password and an enrolled FIDO key
([pam-u2f](https://developers.yubico.com/pam-u2f/Manuals/pam_u2f.8.html)):

```pam
auth     substack system-auth
auth     required pam_u2f.so cue
account  include  system-auth
password include  system-auth
```

`substack` stops a `sufficient` success inside `system-auth` from skipping the key.
For fingerprints use the distribution's `pam_fprintd` stack. PAM runs modules one
after another; the lock screen does not race a password against a fingerprint.

### The logind listener

`canopa-lock daemon` (unit `canopa-lock.service`) serves one logind session. It
selects `--session ID`, else `XDG_SESSION_ID`, else the caller's session, else the
only graphical Wayland session of the user. It then:

- starts the locker on logind's `Lock` signal (`loginctl lock-session`,
  `loginctl lock-sessions`); an `Unlock` signal never unlocks;
- holds a logind sleep delay inhibitor, locks on `PrepareForSleep` and releases the
  inhibitor once the compositor confirms the lock, then takes a new one after resume;
- sets logind's `LockedHint` after the compositor confirms and clears it after unlock;
- restarts a locker that crashed while locked (after 750 ms, up to five times a
  minute), so the screen does not stay locked with no prompt, and stops once another
  locker has unlocked the session;
- reconnects when logind restarts.

Stopping or restarting the listener leaves an active lock screen in place. If the
locker is killed, the compositor keeps the session locked.

### Background and widgets

The lock background is, per output: the `image` of its `output` block, else the
`lockscreen` block's `image`, else a lock-screen picture an application set through
the [Wallpaper portal](portals.md#wallpaper), else, with `use-wallpaper` on (the
default), the wallpaper daemon's current image, including Canopa's built-in
wallpaper. Explicit lock images work without the daemon.
Missing or invalid images fall back to the theme background. `blur` softens the
image, a gentle shade keeps the clock and the bottom controls legible, and `dim`
blends it toward the background color.

Without an `avatar` image the lock shows the account's AccountsService icon or
`~/.face`, else the initials of the account's full name.

Passive widgets, each optional:

| Widget | Shows |
| --- | --- |
| `username` | Your name |
| `avatar` | The `avatar` image, else initials |
| `keyboard` | The active keyboard layout; F6 or a click cycles layouts when there are several |
| `battery` | UPower battery state |
| `media` | The Shell's selected player's title, without controls |
| `weather` | A local JSON snapshot (below) |
| `notifications` | Canopa Notify's lock-screen feed |

`clock`, `date` and `authentication` are placed by `layout`. By default the center
column shows avatar, clock, date, username, the password field and keyboard, and a
column at the bottom center shows notifications, battery, media and weather. The enabled
widgets are `battery`, `media`, `keyboard`, `username` and `notifications`.

The notifications widget shows only what Notify allows on the lock screen: critical
notifications, plus alarms and calls that break through Do Not Disturb, as "App — New
notification" unless the app's rule allows its summary (see
[Notifications](notifications.md)). Up to four lines are shown; with more than four
entries, three lines and "+N more".

The weather widget reads `weather-file` when it redraws:

```json
{"temperature_celsius": 18.5, "description": "Clear", "updated_unix": 1793491200}
```

Files over 16 KiB, invalid temperatures and snapshots more than five minutes in the
future or more than two hours old are ignored. The lock screen makes no network or
location request; something else must write the file.

The clock redraws at each minute, or every second when `clock-format` or `date-format`
contains seconds, fractions or a timestamp. Without a format (or with an empty one)
the clock and date follow your language's formats, for example `2:30 PM` in American
English and `14:30` in French. The Settings Date & time page writes an explicit 12- or
24-hour pattern when you choose one.

### Layout and custom widgets

`layout` replaces the default placement. Each region lists widgets in drawing order:
`top-left`, `top-center`, `top-right`, `center-left`, `center`, `center-right`,
`bottom-left`, `bottom-center`, `bottom-right`. Regions flow as a column unless
`flow="row"`. `x`/`y` offsets are logical pixels (-4096..4096). `authentication` must
appear exactly once. Every enabled widget needs exactly one place; `clock` and `date`
appear when placed. On small screens other widgets overlapping the password field are
hidden. Without `layout`, enabled custom widgets join the last region.

```kdl
lockscreen {
    widgets "username" "battery" "status"
    layout {
        margin 32
        gap 12
        top-left "clock" "date"
        center "username" "authentication"
        bottom-right "battery" "status" flow="row" x=-8 y=-8
    }
    widget "status" {
        type "script"
        command { argv "python3" "/home/me/.config/canopa/lock-status.py"; }
        interval 30
        timeout 5
        format "{output}"
        size 15
        color "muted"
        preview "Example status"
    }
    output "DP-1" { hide-widgets "username" "media"; }
}
```

Custom (`script`) widgets run their command after the screen is locked, as you, with
the configuration file's folder as working directory and no input. `command "uptime
-p"` is split into words without a shell; use `sh -c` explicitly for shell syntax.
Each run must finish within `timeout` seconds and print at most 16 KiB of UTF-8;
the widget shows at most 1024 characters on four lines, and hides on failure. Runs
repeat `interval` seconds after the previous one ends. Previews and `check-config`
never run commands. Only show data you want visible on a locked screen.

`hide-widgets` in an `output` block hides passive, clock, date or custom widgets on
that monitor. The password field stays on every monitor. The Settings
[Widgets page](settings.md#widgets) edits this layout with a diagram.

## Idle (Stasis)

Canopa uses the external [Stasis](https://github.com/saltnpepper97/stasis) daemon
(version 1.7) for idle timeouts. `canopa-idle` starts and supervises it and gives it
Canopa's commands. Stasis is optional: on Arch it is an optional dependency, and the
idle unit only starts when `/usr/bin/stasis` exists.

The policy file is `~/.config/canopa/stasis.rune` (under `$XDG_CONFIG_HOME`). The
Canopa session creates it at login (`canopa-settings init-session`). In the local
layout, create it with `canopa idle init` (mode 0600, never overwrites). The idle
delays can be edited on the Settings Power page, or directly in Rune.

Default policy (steps are sequential):

| Plan | Lock | Displays off after the lock | Suspend after displays off |
| --- | --- | --- | --- |
| Desktop and on AC | 300 s | 60 s | 600 s |
| On battery | 120 s | 30 s | 300 s |

logind and D-Bus inhibitors are honored; game, gamepad and media detectors are off.
Stasis warns before each action and debounces input for 5 s.

```sh
canopa idle info --json
canopa idle pause 1h
canopa idle resume
canopa idle toggle        # Stasis's manual inhibit
canopa idle profile work
canopa idle reload
canopa idle watch
```

| Command | Meaning |
| --- | --- |
| `run [--config FILE]` | Run Stasis under supervision (what `canopa-idle.service` runs). Requires the session's lock listener. |
| `init [--output FILE]` | Write the default Rune file. |
| `suspend` | Lock, wait for the compositor's confirmation (5 s), then ask logind to suspend. Refuses if locking fails. |
| `info [--json]`, `watch`, `pause [DURATION]`, `resume`, `toggle`, `reload`, `profile NAME` | Passed to Stasis. |

Global options: `--stasis PATH` (default `stasis` from `PATH`) and `--session ID`.
`canopa-idle run` gives Stasis `CANOPA_LOCK`, `CANOPA_IDLE` and `CANOPA_NIRI`
(absolute paths used by the Rune commands) and `XDG_SESSION_ID`. `CANOPA_NIRI` is
`CANOPA_NIRI_BIN` when set, else the bundled `/usr/lib/canopa/niri`, else `niri`
from `PATH`, as in Settings. `canopa-lock` must
be installed next to `canopa-idle`. Run one Stasis per user and do not also start
Stasis's own service. Stopping `canopa-idle.service` stops Stasis; an active lock
screen stays.

Suspend from any other source (lid, menu, `systemctl suspend`) is covered by the lock
listener's delay inhibitor. logind limits how long a delay inhibitor can hold sleep.

The Shell's Keep awake tile (`idle-inhibitor` module) uses a Wayland idle inhibitor by
default (`backend "wayland"`). A module with `backend "stasis"` shows Stasis's state
and toggles its manual inhibit instead:

```kdl
shell {
    modules {
        module "awake" {
            type "idle-inhibitor"
            backend "stasis"
        }
    }
    bar "top" { modules { start "awake"; }; }
}
```

The Shell watches Stasis only while such a module exists.
Closing the Shell does not release a manual inhibit.

## Shared authentication input

The lock screen, login screen, polkit agent and pinentry share one input field.

| Key | Action |
| --- | --- |
| Left / Right | Move by character (grapheme); with Shift, extend the selection |
| Home / End | Move to the start or end |
| Backspace / Delete | Delete the selection or the adjacent character |
| Ctrl+A | Select all |
| Ctrl+U | Clear |
| F6 | Cycle the keyboard layouts the compositor provides |

Buttons show hover and pressed feedback and activate on release. Moving away
before releasing cancels the activation. Long choosers scroll with the mouse
wheel or touchpad, and icon controls show a tooltip on hover. Click in a response
field to place the caret, or drag to select complete characters. Unavailable
actions are disabled.

Secret fields have an eye button (also an accessibility action) that shows the typed
text; each new prompt starts masked. The layout selector only changes how this field
interprets keys; configure the layouts themselves in the compositor (Settings,
Keyboard & pointer). Text uses complex-script shaping, bidirectional layout and font
fallback; install fonts for the scripts you need.

Each surface publishes an AT-SPI tree for screen readers: prompt, status, request
context and actions. Secret fields expose only bullets. Typed secrets live in locked,
non-dumpable memory and are erased after use; copies made by PAM, D-Bus, GnuPG or the
compositor are outside Canopa's control.

## Configuration

Both tools read `~/.config/canopa/config.kdl` (see [Configuration](configuration.md))
and the shared `appearance` block. `canopa config check` validates their namespaces
with the other components. Relative paths are relative to the configuration file.
Unknown or repeated keys are errors.

```kdl
schema-version 1
wallpaper {
    image "wallpapers/forest.jpg"
    fit "fill"
    output "DP-1" { image "wallpapers/mountains.png"; fit "fit"; }
}
lockscreen {
    dim 40
    clock-format "%H:%M"
    date-format "%A, %B %-d"
    widgets "username" "keyboard" "battery" "notifications" "avatar"
    avatar "avatar.png"
}
```

### `wallpaper`

| Key | Value | Default |
| --- | --- | --- |
| `image` | path | none (solid color) |
| `fit` | `fill`, `fit`, `stretch`, `center` | `fill` |
| `color` | `#rrggbb` | the theme background |
| `output "NAME" { image; fit; }` | per-monitor override, once per name | — |

### `lockscreen`

| Key | Value | Default |
| --- | --- | --- |
| `image`, `fit`, `color` | as for `wallpaper` | none, `fill`, theme background |
| `output "NAME" { image; fit; hide-widgets ...; }` | per-monitor override | — |
| `use-wallpaper` | boolean | `#true` |
| `dim` | 0–100 | 40 (62 in the light color scheme) |
| `blur` | 0–64 logical pixels; 0 keeps the image sharp | 24 |
| `font` | family name | the first `appearance` typography family |
| `clock-format`, `date-format` | strftime pattern; empty follows the language | empty |
| `clock-size`, `text-size` | 8–160 | 64, 19 |
| `auth-timeout` | 5–600 seconds | 120 |
| `avatar`, `weather-file` | path | none |
| `widgets` | widget names; `widgets;` hides all | `battery media keyboard username notifications` |
| `layout { margin; gap; REGION ... }` | see [Layout](#layout-and-custom-widgets); margin and gap 0–256 | 42 and 18, also inside an explicit `layout` that does not set them |
| `widget "NAME" { ... }` | custom widget, at most 32 | — |

Custom widget fields: `type "script"`, `command` (a string or `command { argv ...; }`,
at most 64 arguments), `format` (with `{output}`), `interval` (1–86400, default 30),
`timeout` (1–60, default 5), `size` (8–160, default 15), `color` (`text`, `muted`,
`accent`, `warning`, `danger`, `success` or `#rrggbb`) and `preview`. Names cannot
reuse built-in widget names.

## Known limitations

- The weather widget only reads a local file; Canopa ships no weather provider.
