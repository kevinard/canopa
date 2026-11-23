# Canopa Settings

Open **Canopa Settings** from the launcher, press Mod+Comma in the packaged session,
or run `canopa settings` (`canopa-settings`). The window uses the shared Canopa
appearance. Settings shows and edits preferences; the program or system service that
owns each one applies it. Nothing keeps running after you close the window.

## Pages

The sidebar groups the pages as below. Ctrl+1 to Ctrl+9 open the first nine.

| # | Page (id) | Group | Changes | Applied by |
| --- | --- | --- | --- | --- |
| 1 | [Displays](#displays) (`displays`) | Devices | Test, then keep | Niri |
| 2 | [Keyboard & pointer](#keyboard-and-pointer) (`input`) | Devices | Save | Niri |
| 3 | [Network](#network) (`network`) | Devices | Live | NetworkManager |
| 4 | [Bluetooth](#bluetooth) (`bluetooth`) | Devices | Live | BlueZ |
| 5 | [Printers](#printers) (`printers`) | Devices | Live | CUPS, cups-pk-helper |
| 6 | [Sound](#sound) (`sound`) | Devices | Live | PipeWire (PulseAudio protocol) |
| 7 | [Personalization](#personalization) (`personal`) | Appearance | Save | Canopa components, Niri |
| 8 | [Accessibility](#accessibility) (`accessibility`) | Appearance | Save, some live | Canopa components, Niri |
| 9 | [Power](#power) (`power`) | System | Live and Save | UPower, power-profiles-daemon, logind, Stasis |
| 10 | [Date & time](#date-and-time) (`date-time`) | System | Live | systemd-timedated, GeoClue |
| 11 | [Region & language](#region-and-language) (`region`) | System | Live | `locale.conf`, AccountsService, systemd-localed |
| 12 | [Users](#users) (`users`) | System | Live | AccountsService, `passwd` |
| 13 | [Privacy](#privacy) (`privacy`) | System | Live | Portal permission store, autostart entries |
| 14 | [About](#about) (`about`) | System | Live | systemd-hostnamed |
| 15 | [Notifications](#notifications) (`notifications`) | Desktop | Live | Canopa Notify |
| 16 | [Default applications](#default-applications) (`default-apps`) | Desktop | Live | `mimeapps.list`, `xdg-terminals.list` |
| 17 | [Launcher](#launcher) (`launcher`) | Desktop | Save | Canopa Shell |
| 18 | [Shortcuts](#shortcuts) (`shortcuts`) | Desktop | Save | Niri |
| 19 | [Widgets](#widgets) (`widgets`) | Desktop | Save | Canopa Shell, Canopa Lock |

**Live** pages apply each change at once and show its progress, or an error with
Retry, next to the control. When the service behind the page is not running, the page
says so and reconnects when it returns. Changes that need permission ask through the
[polkit agent](polkit.md).

**Save** pages keep a draft. As soon as it differs from the saved file, a bar at the
bottom of the page says so and offers **Save changes** (Ctrl+S) and **Discard
changes**. Saving checks that the file has not changed since it was read, validates
the candidate with the owning program (or `niri validate`), then replaces the file.
Discarding re-reads the file for that page only. A page without unsaved edits reads
its file again each time it is opened, so changes made elsewhere show up. Personalization and Widgets both edit
`config.kdl`: saving one keeps the other's unsaved edits, but a change made outside
Settings must be reloaded. Symlinked files are read-only.

Pages that write the compositor configuration (Displays, Keyboard & pointer, the
Niri parts of Personalization, Accessibility, Power and Shortcuts) need the Canopa
session, which runs Niri on `~/.config/canopa/niri.kdl`. In other sessions they say
so and do not write. Your own Niri configuration is never rewritten.

## Navigation

| Key | Action |
| --- | --- |
| Ctrl+F | Search settings (page names and keywords, in your language and English, ignoring accents); Enter opens the best match |
| Ctrl+1 … Ctrl+9 | Open page 1 to 9 of the table above |
| Ctrl+Page Down / Ctrl+Page Up | Next / previous page |
| Up / Down (in the sidebar) | Next / previous page |
| Tab / Shift+Tab | Next / previous control |
| Ctrl+S | Save the current page |

Only one Settings window runs at a time. Opening Settings again, from the launcher, a
notification or the command line, brings the window forward at the requested page and
keeps unsaved edits on other pages. While a change is being applied or a display test
runs, a page request only brings the window forward.

## Displays

Displays are numbered; each card's title starts with its number. With several
enabled displays, **Arrangement** draws them where their positions place them, at a
scale that depends only on their sizes, and while the window has focus each screen
shows its number and name in its top left corner. Drag a display to move it: its
edges snap to the other displays' edges, and its label on the screen lights up.
Dropped, it settles at the nearest place where it touches another display without
overlapping one, and a display the move left apart joins the others again. Monitors
the compositor names "Unknown" (it can miss a monitor plugged in later) are named
from the EDID they report. For each connected display: on/off, resolution and
refresh rate, scale (25% steps from 50% to 400%), rotation and, under **Exact
position**, its position in logical pixels from the arrangement's top left corner
at 0, 0. Typed positions that overlap are refused, because Niri would place the
display elsewhere. At least one display stays on. **Test display changes**
applies the layout and offers **Keep changes** or **Revert** for 20 seconds (Escape
reverts). Only Keep writes `~/.config/canopa/niri-displays.kdl`. A separate helper
process restores the previous layout if the window closes or crashes, the time runs
out or the change partly fails. **Reload** (in Arrangement, or beside the one-display notice) discards the draft and
reads the monitors and saved profiles again.

With the `canopa-login` package, **Login screen** › **Use on login screen** copies the applied layout
(not unsaved changes) to `/etc/canopa/greeter-displays.kdl` after administrator
authentication, so the login screen uses the same modes, scales and positions. See
[Login screen](greeter.md#display-layout).

### Display profiles

Set up the displays, enter a name under **Display profiles** and choose **Save layout
as profile**; this saves without applying. To switch, select a profile, choose **Test
saved profile**, then **Keep changes**. `canopa-settings --display-profile "Desk"`
opens Displays with that profile selected.

The policy controls docking and undocking:

- **Suggest matching profiles** (default): a notification offers the matching layout;
  **Review layout** opens it in Displays.
- **Switch automatically**: matching profiles apply without asking.
- **Manual only**: no offers, no switching.

If a change leaves every connected display off, Canopa turns one on (the built-in
panel when there is one). Profiles live in `~/.config/canopa/display-profiles.kdl`:

```kdl
switching "suggest"
startup #true
settle-ms 600
offer-ms 15000

profile "Laptop" {
    output connector="eDP-1" scale=1.25
}

profile "Desk" priority=10 {
    output connector="eDP-1" enabled=#false
    output make="Dell" model="U2723QE" serial="12345" width=3840 height=2160 refresh-mhz=60000 scale=1.5 x=0 y=0
}
```

| Key | Meaning |
| --- | --- |
| `switching` | `manual`, `suggest` or `automatic` |
| `startup` | Also offer or apply a matching profile at login |
| `settle-ms` | Wait for hotplug events to settle, 100–5000 |
| `offer-ms` | How long an offer stays, 1000–120000 |
| `profile "NAME" priority=N hotplug=#false` | Higher priority wins; equal top priorities need a manual choice; `hotplug=#false` keeps a profile manual |
| `output` | Selectors `connector`, `make`, `model`, `serial` (`"*"` matches anything); `enabled`, `width`/`height`/`refresh-mhz` (all three), `custom-mode`, `scale`, `transform` (`normal`, `90`, `180`, `270`, `flipped`, `flipped-90`, `flipped-180`, `flipped-270`), `x`, `y` |

A profile must cover every connected display, each selector must match exactly one
display, and at least one display must be on. `canopa-settings outputs` prints the
connector names and monitor identities. The session unit
`canopa-display-profiles.service` follows hotplug; file edits apply at the next
change.

## Keyboard and pointer

XKB layouts (up to eight, in switching order), listed by name; **Add a layout**
offers the layouts installed on the computer, and the first one is the default.
**Variants and keyboard options** shows the layout codes, variants and options as
text; for example, layouts `us,fr` with option `grp:alt_shift_toggle`. Keymaps are
compiled before saving. Also key repeat delay and rate, mouse speed, left-handed buttons and natural
scrolling, touchpad speed, tap to click, natural scrolling and disable while typing.
Saved to `~/.config/canopa/niri-input.kdl`. Put custom device rules in
`~/.config/canopa/niri-custom.kdl`; a managed file with edits Settings does not
understand is shown read-only.

## Network

- **Wi-Fi**: the switch, nearby networks with security and signal, Connect and
  Disconnect, **Join a hidden network…**. WPA-Enterprise supports PEAP, TTLS and TLS
  with identity, anonymous identity, CA certificate (empty uses the system's),
  server domain, and a password or certificate and key. **Remember the password**
  lets NetworkManager store it; otherwise it is asked each time.
- **Wired**: each Ethernet device's state, connection and address.
- **VPN**: connect and disconnect; **Add a WireGuard tunnel…**; **Import a VPN file**
  reads WireGuard `.conf` files directly and other types through the installed
  NetworkManager plugin (`nmcli connection import`).
- **Hotspot**: share the connection over Wi-Fi with a name and password.
- **Connection settings**: per saved connection, automatic connection, metered,
  IPv4 and IPv6 method, addresses, gateway, DNS, proxy (none, configuration URL, or
  one `host:port`), and Forget.

When NetworkManager needs a secret it does not store ("ask each time", a rejected
password, a VPN password), the question appears at the top of the page while Settings
is open. Settings passes the answer on and keeps nothing.

## Bluetooth

Each adapter has its switch and **Visible to nearby devices**. While the page is
open, Settings searches for devices. **My devices** lists paired devices with kind,
connection, battery level when reported, **Connect**/**Disconnect**, **Allow it to
connect without asking** (trust) and **Forget**. **Nearby devices** lists devices in
pairing mode; **Pair** pairs, trusts and connects. Pairing questions (matching code,
PIN or passkey) appear at the top of the page. Audio profiles are on the Sound page.

## Printers

**Printers** lists queues with state, location and address: **Make default** (your
`lpoptions`), **Print test page**, **Remove**. **Print jobs** lists waiting jobs; you
can cancel yours, an administrator anyone's. **Search the network** finds driverless
printers (IPP Everywhere, AirPrint) with `ippfind`, which needs Avahi; **Add** creates
a driverless queue. Adding and removing use cups-pk-helper and polkit; without
cups-pk-helper they work only for members of CUPS's administrator group.

## Sound

For each output and input: **Use this device**, volume, mute and balance. The default
input shows a level meter while the page is open. **Applications** lists programs
playing or recording with their volume and mute. **Profiles** switches each card's
profile (for example HDMI, or a Bluetooth headset's A2DP and HFP). **Play notification
sounds** writes `notifications { sound enabled=... }` in `config.kdl` at once.

## Personalization

- **Appearance**: light or dark, and the accent color: one of eight presets or any
  `#RRGGBB` under **Custom color**, written to the `appearance` block of
  `config.kdl`. Text size and reduced motion are on the Accessibility page.
- **Choose wallpaper…** opens the [wallpaper picker](session-tools.md#picker) on the
  pictures it finds; **From a file…** asks for a JPEG or PNG picture through the file
  chooser and opens the picker on it, among the pictures of its folder.
- **Lock screen**: background dimming, and under **Formats, font and picture** the
  clock and date formats (empty follows Region & language), font and avatar image,
  written to the `lockscreen` block. **Edit lock screen widgets…** opens Widgets.
- **Window layout**: space around windows (none, small, medium, large), center the
  focused column (never, when it does not fit, always) and new window width (one
  third, half, two thirds, full width). **Use the Canopa layout** restores the
  defaults (medium, when it does not fit, half).

The `config.kdl` fields are written by Save and validated by every owner
(`canopa config check`); the Shell, Notify and the Settings portal reload them, and
the lock screen uses them at the next lock. The motion and window layout controls
apply to Niri at once.

## Accessibility

**High contrast** (`appearance { high-contrast #true }`) makes every Canopa surface
black and white with full-strength text and borders; the Settings portal reports it to
applications. **Larger text** (100–150%) and **Reduce motion in Canopa and in
applications** are written to the `appearance` block of `config.kdl`; these three are
written by Save. **Cursor size** (24, 32, 48, 64 or 96) applies at once through
`niri-input.kdl`; save or reload pending Keyboard & pointer edits first. **Reduce
window and workspace animations** turns Niri's animations off in `niri.kdl` at once,
and turning it off removes the override.
Canopa surfaces expose AT-SPI trees for screen readers such as Orca. When Orca is
installed, **Start Orca** starts it; starting it at every login is an Orca
preference.

## Power

- **Battery**: charge, state, time estimate and, when reported, health and charge
  cycles (UPower). Shown only on computers with a battery.
- **Power mode**: power-profiles-daemon's profiles.
- **Brightness**: the built-in display, on the same scale as the brightness keys.
  Shown only when there is a backlight.
- **When the lid is closed** (computers with a lid): the system setting (default), Suspend, Hibernate, Lock
  the screen, Do nothing or Power off. Saved to `~/.config/canopa/power.kdl`. For any
  choice other than the system setting, the Canopa session holds logind's lid
  inhibitor and asks logind for the action. With an external display or dock, closing
  the lid does nothing.
- **When the power button is pressed**: Suspend (Niri's default) or the system setting
  (logind's `HandlePowerKey`). Applies at once through `niri-input.kdl`.
- **Idle**: whether Stasis is running (through `canopa-idle`), with pause, resume and
  profile controls. Lock, displays-off and suspend delays for AC and battery are
  saved to `~/.config/canopa/stasis.rune`. The delays are sequential. Saving works
  without Stasis; a running Stasis reloads the file, and the previous file returns if
  it refuses. Only the timeout numbers are changed; plans Settings cannot edit safely
  are read-only. See [idle](session-tools.md#idle-stasis).

## Date and time

- **Time zone**: search by city or region and choose **Use**. With GeoClue installed,
  **Use my location** sets the zone once from this computer's location.
- **Set the time automatically from the network** switches NTP. When it is off, set
  the date and time.
- **Clock format**: **Automatic**, 24-hour or 12-hour, for the Shell clocks and the
  lock screen. Automatic follows Region & language (the page shows the result) and
  removes the 12- or 24-hour patterns Settings wrote; the other choices write an
  explicit pattern. A clock with a pattern of your own (for example `%a %H:%M`) keeps
  it, and the page names it.

## Region and language

**Language** and **Formats** (dates, numbers, money, measurement, paper) are saved to
`~/.config/locale.conf` (`LANG` and `LC_*`). Settings switches at once. Files, Notify
and the Shell read `locale.conf` when they start, so restart them or log out; other
applications follow at the next login. AccountsService records the language for
display managers. A preview shows the chosen formats.

**Login screen and new users** sets the system locale through systemd-localed, and
**Use my layouts for the login screen and console** copies your Keyboard & pointer
layouts to the system keyboard. Only installed locales are offered; enable more in
`/etc/locale.gen` and run `locale-gen`.

## Users

- **Your account**: full name, picture (PNG or JPEG, up to 2 MB and 2048 pixels per
  side) and **Change password**, which runs `passwd`, so the system's password rules
  and the check of your current password apply.
- **Other users** (administrators): account type (administrator means the `wheel`
  group), full name, picture, **Ask for a new password at next login** and
  **Remove…** (keeping or deleting the files).
- **Add a user…**: full name, user name and account type. The new user chooses a
  password at the first login; until then the account has no password.

Settings never sets another person's password or handles password hashes. Automatic
login is not offered.

## Privacy

**Application permissions** lists what the desktop portal's permission store records
for sandboxed applications: location, camera, microphone, speakers, screenshots,
background, notifications, and screen-sharing or remote-desktop restore grants.
**Allow** and **Deny** change an answer; **Forget** removes it so the application asks
again. **Start at login** lists autostart entries; turning one off sets `Hidden=true`
in your entry or writes a user entry that hides a system one.

## About

Device name, hardware (model, processor, memory, graphics, disk, firmware) and software
(operating system, kernel, Canopa version). **Rename** sets the device name through
systemd-hostnamed. Without hostnamed the page still reads `/etc/os-release`, `/proc`
and `/sys`.

## Notifications

The page talks to Canopa Notify through its
[preferences API](notify/settings-control-api.md); Notify stores and applies the
preferences.

- **Do not disturb**: on, **For 1 hour**, **For 3 hours** or **Until tomorrow
  morning** (08:00). Turning it off during a scheduled period skips the rest of it.
- **Turn on automatically every day**: start, end and days; an end before the start
  ends the next morning.
- **Alarms** and **Incoming calls** can break through Do not disturb.
- **Hide banners automatically** while an application is fullscreen and while the
  screen is shared or recorded.
- **Applications**: per application, delivery (default, banners, Notification Center
  only, mute), **Play sounds** and what the lock screen shows. **Reset** returns to
  the defaults.

These override the `notifications` section of `config.kdl`; **Use the settings from
config.kdl** returns to the file for Do not disturb. See
[Notifications](notifications.md).

## Default applications

Choose the **Web browser**, **Mail**, **Calendar**, **Music**, **Video**, **Photos**,
**Text editor**, **File manager** and **Terminal**; under **Other file types**, type a
MIME type (`application/pdf`) or an extension (`pdf`). Choices are written at once to
`~/.config/mimeapps.list` as GLib records them, so `xdg-open` and other desktops follow
them; other lines are kept. The terminal goes first in
`~/.config/xdg-terminals.list` (and `canopa-xdg-terminals.list` when it exists), which
`xdg-terminal-exec` reads. Edits made elsewhere appear within a few seconds.

## Launcher

Enable or disable launcher extensions, change their keywords, choose the default search
engine (or type a custom address), edit search keywords,
and see their actions and last status. Saved to `~/.config/canopa/launcher.json`; the
Shell's pins and aliases in that file are kept, and the open launcher picks up the
change. **Save changes** also saves the **Clipboard history** section: recording
(off by default) and, while recording, including images and the number of recent
copies (1–64, default 64). Browse them with `clip` in the Applications menu. See
[Launcher](launcher.md).

## Shortcuts

Find packaged actions by name or key. Each shortcut shows its keys as key caps, named
as keyboards print them (Mod is shown as Super). Select one and press the new keys:
**Esc** keeps the current shortcut and **Backspace** turns it off. While you press
keys, Settings asks the compositor to pass its own shortcuts through (the Wayland
keyboard-shortcuts-inhibit protocol), so combinations Canopa already uses can be
recorded. **Type instead** edits the combination as text, in Niri's syntax such as
`Mod+Shift+Space`, for pointer triggers like `Mod+WheelScrollDown`. A shortcut whose
keys another one also uses says so under its name; saving rejects such duplicates and
conflicts with included bindings. **Reset to defaults**, at the end of the list,
stages the shipped keys for review.

Settings stores only your changes, in `~/.config/canopa/shortcuts.kdl`, so unchanged
shortcuts follow Canopa updates. It generates `niri-shortcuts.kdl` (shipped bindings
plus your changes) and includes it in `niri.kdl` in place of the shipped defaults;
this file is regenerated at every login. If an update gives a default to a key you
use, your binding wins and the new default is shown unassigned. Keep your own extra
bindings in `niri-custom.kdl`.

```kdl
defaults "/usr/share/canopa/niri-defaults.kdl"
binds {
    Mod+Shift+Space { spawn "canopa-shell" "--toggle-menu" "Applications"; }
}
remove {
    spawn "canopa-shell" "--toggle-menu" "Applications"
}
```

`binds` holds added and changed bindings; `remove` lists shipped actions you moved or
turned off. When `niri.kdl` contains a full copy of the shipped bindings, login turns
it into this form and keeps the previous file as `niri.kdl.before-shortcut-changes`.
When a Canopa release replaces a shipped action (for example the volume keys, now
Canopa Shell device keys), your changes follow the replacement.

## Widgets

Choose **Bar** or **Lock screen**. **Arrangement** shows the bar's three zones side by
side, named as they run along the bar (Top, Middle and Bottom on a side bar; Left,
Center and Right along a top or bottom bar), or the lock screen's nine regions as a
grid. Drag a widget to another place: the zone under the pointer lights up and a
line shows where the widget will land. Select a widget to see where it is, move it up
or down, choose its **Position**, or remove it; these work from the keyboard too.
**Add widget** places a widget next to the selected one. **Show for monitor** shows
what that monitor gets (a bar hidden there, or lock screen widgets hidden there); it
is not saved, and **Detect** lists monitors connected since the page opened.

- **Bar**: the screen edge and **Show on** all monitors, selected monitors or all
  except selected ones. Under **More bars**, **Duplicate** a bar to give another
  monitor its own layout; the copy starts with no monitors selected. Module styling and actions stay in KDL.
- **Lock screen**: regions, row flow, and **Hide this widget on the preview monitor**.
  The password field stays on every monitor. Script widgets appear once defined in
  KDL; see [lock screen layout](session-tools.md#layout-and-custom-widgets).

Save writes `config.kdl`. The Shell applies the bar at once; the lock screen uses its
layout at the next lock.

## Command line

```sh
canopa-settings                       # open the window
canopa-settings power                 # open a page (same as --page power)
canopa-settings --display-profile Desk
canopa-settings check
canopa-settings outputs
```

| Command | Meaning |
| --- | --- |
| `[PAGE]`, `--page PAGE` | Open the page with that id (see [Pages](#pages)). A running window switches to it and the command exits. Opening a page never saves. |
| `--display-profile NAME` | Open Displays with that profile selected. |
| `init-session [--directory DIR] [--defaults-file FILE]` | Create the Canopa compositor configuration without replacing files: `niri.kdl` (including the shipped defaults, then `niri-input.kdl`, `niri-displays.kdl` and `niri-custom.kdl`), those three files and `stasis.rune`, then regenerate shortcuts and validate with Niri. Prints the path of `niri.kdl`. The session launcher runs it at every login. |
| `check [--directory DIR]` | Validate `niri.kdl` with Niri without changing the session. |
| `outputs` | Print connected monitors as JSON. |
| `pages` | Print the visible pages (id, title, description, icon, keywords) as JSON, in sidebar order. |
| `apply-input --file FILE` | Apply an input-settings JSON document (at most 16 KiB) in the current Canopa session. |

A page cannot be combined with a subcommand. The directory defaults to
`~/.config/canopa`; the shipped defaults come from `--defaults-file`, else
`/usr/share/canopa/niri-defaults.kdl`, else a private copy written next to `niri.kdl`.

Programs can open pages on the session bus. Settings owns `org.canopa.Settings` and
serves the `org.canopa.Settings` interface at `/org/canopa/Settings`; the installed
D-Bus service file starts it when needed.

```sh
busctl --user call org.canopa.Settings /org/canopa/Settings org.canopa.Settings OpenPage 'sa{sv}' power 0
busctl --user call org.canopa.Settings /org/canopa/Settings org.canopa.Settings Activate
busctl --user get-property org.canopa.Settings /org/canopa/Settings org.canopa.Settings Pages
```

`OpenPage` takes a page id and options; `display-profile` (string) selects a profile
on `displays`. `Pages` lists the visible page ids. Without a session bus each launch
opens its own window.

## Known limitations

- A network secret set to "ask each time" is asked only while Settings is open; otherwise the connection fails.
- Compositor settings (displays, input, shortcuts, window layout, power button) require the Canopa session.
- The power button offers only Suspend or the system setting.
- Language changes reach Files, Notify and the Shell only after they restart.
