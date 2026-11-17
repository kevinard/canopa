# Canopa Shell

Canopa Shell draws the desktop's bar, its panels and menus, the
[Applications launcher](launcher.md), the taskbar, the on-screen display for device
keys and the Notification Center. It runs as `canopa-shell.service` in the Canopa
session (see [installation](installation.md)).

Everything here can be changed in the `shell` block of `~/.config/canopa/config.kdl`,
and the bar layout on the Settings **Widgets** page. The
[configuration reference](configuration.md#shell) lists every key. Saved changes
apply in about half a second; `canopa-shell reload` applies them at once and reports
the result. An invalid file never stops the Shell: it keeps the last working
configuration, shows a red badge on the bar and a notification naming the line, and
applies the next valid save.

## The bar

The default bar is a 64-pixel rounded frame on the left edge of every output, with
12 pixels of margin (88 pixels reserved from windows). From top to bottom:

| Zone | Modules |
| --- | --- |
| start | **Search** (opens Applications), **Overview** (Niri's overview), workspaces |
| center | taskbar |
| end | tray, network, volume, battery, notifications, clock |

Clicking the empty bar opens Quick settings. Every button has a tooltip and an
accessible name, and the button whose panel is open is highlighted.

To move the bar, edit its `edge`, or add more bars for other outputs:

```kdl
shell {
    bar "main" {
        edge "top"
        modules {
            start "launcher" "workspaces"
            center "focused-window"
            end "tray" "network" "volume" "battery" "notifications" "clock"
        }
    }
}
```

A `shell` block replaces the whole embedded layout, so copy the modules, menus and
panel you want to keep from
[`default.kdl`](../crates/canopa-shell/config/default.kdl) (installed as
`/usr/share/canopa-shell/default.kdl`).

### Modules

| Module | What it shows and does |
| --- | --- |
| `workspaces` | One pill per Niri workspace: accent for the focused one, filled when occupied, outlined when empty. Click or scroll to switch. Labels and icons per workspace with `workspace N { label; icon; }`. |
| `taskbar` | Pinned applications, then the other running ones in screen order (see [Taskbar](#taskbar)). |
| `minimap` | Niri's scrolling strip for the bar's output: one block per column, split by stacked windows, widths to scale, focused window in the accent color. Click a block to focus it; scroll to move between columns. Pinned columns are outlined. |
| `focused-window` | Icon and title of the focused window, shortened in the middle to 50 characters (`max-length`). The tooltip has the full title. |
| `tray` | StatusNotifierItem icons. Click activates, right-click opens the item's menu. The Shell owns `org.kde.StatusNotifierWatcher` while a tray is placed. |
| `network` | Connectivity icon (Wi-Fi, wired, limited, offline, VPN). Hidden while network status is unavailable. |
| `volume` | Default output level. Scroll to change it by `step` (5%). |
| `battery` | Charge and charging state from UPower; shown only when a battery is present. Turns critical at 15%. |
| `notifications` | Bell with the count of notifications that still need attention. Click opens the Notification Center; right-click toggles Do Not Disturb. `?` means Notify is unreachable. |
| `clock` | Time, and date in expanded views. Click opens a calendar that starts weeks on the region's first weekday. |
| `media` | The selected media player; hidden when none is open (see [Media](#media)). |
| `idle-inhibitor` | **Keep awake**: prevents idle actions while on. Starts off at every login. |
| `script` | The output of a command, rerun every `interval` seconds. |
| `button` | A label or icon with your own actions. |

The default network, volume and battery icons form one status group that opens
Quick settings. Add the `system` button for a separate Quick settings button.

Text uses 12 logical pixels and icons 20 by default; bar `style` and segment styles
override them. Icon names starting with `canopa:` (such as `canopa:lock`,
`canopa:search`, `canopa:bell`) select monochrome icons built into the Shell.
Standalone Nerd Font glyphs use `Symbols Nerd Font Mono` when it is installed.

### Taskbar

The taskbar shows the desktop files listed in `pinned`, then the other open
applications in screen order (by workspace, then column), grouped by desktop file.
A dot on the bar's edge marks a running application, a longer mark several windows,
and an accent mark the focused one.

- **Click** focuses the application's most recent window, or launches it. Click
  again to cycle through its windows from left to right.
- **Middle-click** starts another instance.
- **Right-click** opens the application menu: Open (or New window while it runs),
  the desktop file's own actions, layout commands for its window (one-third, half
  or two-thirds width, tabs, center, float or tile, move to the workspace above or
  below, pin the column left or right), and **Pin to taskbar** / **Unpin from
  taskbar**.
- **Hover** shows thumbnails of up to four of its windows.

Pins changed from the menu are saved in `$XDG_STATE_HOME/canopa/shell/taskbar-pins.json`
as changes to the configured list, so later additions to `pinned` still appear.

```kdl
modules {
    module "taskbar" {
        pinned "org.canopa.Files.desktop" "firefox.desktop" "org.canopa.Settings.desktop"
    }
}
```

Launched applications each run in their own systemd user scope,
`app-canopa-<desktop id>-<random>.scope`.

## Quick settings

Quick settings is the bar's panel. By default it opens in a 416-pixel side drawer
and contains:

- **Wi-Fi** and **Bluetooth** tiles. The tile switches the radio; its chevron opens
  the full panel. A radio without its service shows a plain row instead.
- **Do Not Disturb** and **Keep awake** tiles. A tile that is on uses the accent fill.
- **Sound**: the default output with a level slider and mute button.
- **Power & display**: battery charge and, with a built-in backlight, a brightness
  slider.
- A footer with media, **Lock**, **Suspend** and **Power** (Hibernate, Log out…,
  Reboot… and Power off…).

Tab and Shift+Tab move between cards and Enter activates them. **Back** in a panel
returns to Quick settings. Edit the bar's `panel { start …; center …; end …; }` to
change the contents; `session-hibernate`, `session-logout`, `session-reboot` and
`session-poweroff` are ready-made buttons for one-click access.

### Panel presentation

| `presentation` | Behavior |
| --- | --- |
| `"adaptive"` (shipped) | Quick settings, the launcher and providers open in a side drawer; single module details, such as Calendar, open in a popover beside the module. |
| `"drawer"` | Every panel opens in a full-height drawer. |
| `"auto"` (default when omitted) | Content-sized panels. |

`drawer-mode "overlay"` keeps the bar at the edge and opens the drawer beside it;
`"push-bar"` opens the drawer at the edge and moves the bar inward. Both overlay
windows without resizing them, and apply to left and right bars. Remove `height` and
`max-height` to let a drawer fill the screen height. Panels slide and fade in
(`animation { open-ms 180; close-ms 160; }`; `0` disables a transition, as does
`appearance { reduced-motion #true; }`). Escape clears an active search, then closes
the panel.

## Panels

Panels are opened by modules, Quick settings, the launcher, or any action with
`open-provider "NAME"`. A provider you configure with the same name replaces the
built-in one.

| Provider | Opened by | Program |
| --- | --- | --- |
| `wifi` | network module, Wi-Fi tile | `canopa-wifi-provider` |
| `audio` | volume module, Sound card | `canopa-audio-provider` |
| `media` | media module | `canopa-media-provider` |
| `bluetooth` | Bluetooth tile | `canopa-shell --hardware-panel bluetooth` |
| `power` | Power & display card | `canopa-shell --hardware-panel power` |
| `notifications` | notifications module | `canopa-notify panel-provider` |

The provider programs must be installed beside `canopa-shell` or on `PATH`; the
packages and the updater install them together.

### Wi-Fi

Lists networks, connects to open and saved networks, asks for personal (WPA-PSK)
passwords and joins hidden personal networks. It uses NetworkManager, or iwd when
NetworkManager is absent. The system service keeps the credentials and decides who
may change them. Enterprise (802.1X) networks are set up in Settings' **Network**
page. `CANOPA_WIFI_PROVIDER_BACKEND=networkmanager|iwd|none` restricts the choice.

### Bluetooth

Uses BlueZ. Turn on an adapter, choose **Find devices**, then pair, connect or
disconnect from the device list. Passkey and PIN requests appear in the panel.
Closing the panel stops discovery and cancels an unfinished pairing. Settings'
**Bluetooth** page manages trust, forgetting and battery levels.

### Sound

Lists **Speakers and headphones** and **Microphones** with a volume slider, mute
button and default marker. Output monitors are not offered as microphones.

- The slider goes from 0 to 100% in one-point steps. Dragging previews the value and
  releasing applies it. With the keyboard, use the arrow keys, Home and End.
- The level is the loudest channel. Changes keep the balance between channels and do
  not unmute. A device already above 100% (amplified by another program) stays as it
  is until you lower it.
- **Mute** / **Unmute** change that device's mute state.
- **Set as output** / **Set as input** make the device the default. Existing streams
  move according to the audio session manager's policy.

The panel follows external changes and shows a reconnecting message if the audio
server goes away. It works with PulseAudio or PipeWire's PulseAudio service
(`pipewire-pulse`), and only reads device information. Per-application volume and
card profiles are on Settings' **Sound** page.

### Media

Shows players that implement [MPRIS 2](https://specifications.freedesktop.org/mpris/latest/).
The player stays in charge of playback; the Shell only displays and controls it.

Add `"media"` to a bar zone or panel to show it (it is in the default Quick
settings footer). On horizontal bars it shows a playback icon and a compact artist
and title label of up to 48 characters; the tooltip has the full label and player. Clicking opens **Media** with the title, artist, player, state, and the
Previous, Play/Pause and Next controls the player supports.

**Automatic** selection prefers a playing player, then a paused one, then a stopped
one, and keeps the current choice among equals. Choosing a player in the panel pins
it until you choose Automatic again, the player exits, or the Shell restarts. With
no player open the module hides. Media keys control the same selected player.

```kdl
modules {
    module "media" {
        horizontal-format "{icon}" "{value}"
        expanded-format "{title}" "{artist}"
        tooltip "{label}" "{player}" "{state}"
    }
}
```

Artwork, seeking, playlists, shuffle, repeat and per-player volume are not
available.

### Power & brightness

Shows the built-in display's brightness (through logind, without root) and the
power profiles offered by power-profiles-daemon: Power saver, Balanced and
Performance. Settings' **Power** page has battery health, the lid action and idle
timeouts.

### Notification Center

The notifications module opens Canopa Notify's Center, 480 pixels wide and the
height of the screen. See [notifications](notifications.md).

## Session actions

| Button | Action |
| --- | --- |
| **Lock** | Runs `canopa-lock`. |
| **Suspend**, **Hibernate** | Ask logind at once. |
| **Log out…**, **Reboot…**, **Power off…** | Open a confirmation menu with **Cancel** selected first. Log out runs `niri msg action quit --skip-confirmation`. |

Suspend, hibernate, reboot and power off use the `session "…"` action, which calls
systemd-logind directly, so polkit can ask for a password (for example while other
users are logged in). Actions logind refuses are hidden; Hibernate appears only when
logind answers `yes`. A confirmation lists applications that block the action, and a
failed request is reported in a notification. The same actions are available from
the launcher by typing `lock`, `suspend`, `reboot` and so on.

## Device keys and on-screen display

The session's Niri defaults bind the hardware keys to Shell commands:

```sh
canopa-shell volume up [STEP]       # also: down [STEP], mute
canopa-shell microphone mute        # "mic" also works
canopa-shell brightness up [STEP]   # also: down [STEP]
canopa-shell keyboard-backlight up  # also: down, toggle
canopa-shell media play-pause       # also: play, pause, stop, next, previous
```

Volume, microphone, display brightness and media keys also work while the screen is
locked. After each press, a small display above the bottom of the focused output
shows the device, a level bar and the value; repeated presses update it in place. It
also appears when another program or the firmware changes the volume, default
output, microphone mute or brightness (but not at login). It takes no focus, lets
clicks through and is announced to screen readers.

- Volume changes the default output through the audio server, stops at 100%, and
  raising it unmutes.
- Brightness uses logind in perceptual steps and never goes below 1%.
- Media keys control the player selected by the media module, or the one Automatic
  would choose. They show nothing on screen and do nothing without a player.
- The commands work without a running Shell, but then nothing is displayed.

Change the steps and timing in the `osd` block (see
[configuration](configuration.md#osd)); a STEP argument overrides it (up to 100 for
volume, 50 for brightness). Rebind the keys on the Settings **Shortcuts** page.

## Windows, workspaces and the overview

These features use the Canopa Niri fork that the `canopa-compositor` package
installs.

- **Overview typing**: typing in Niri's overview (Super+O, or the top-left hot
  corner) opens Applications with that text. Closing the launcher closes the
  overview. This needs `overview { emit-typed-text; }`, which the shipped Niri
  defaults set.
- **Window thumbnails** in taskbar tooltips come from Niri's window preview service.
  `window-previews { off; }` in Niri turns them off.
- **Pinned columns**: Super+P and Super+Shift+P (or the taskbar menu) pin a column
  to the left or right edge of the view while the rest scrolls.
- **Window restore**: with `windows { restore #true; }`, the Shell records open
  applications with their workspace, column and width in
  `$XDG_STATE_HOME/canopa/shell/windows.json`, and reopens them in place at the next
  login if no windows are open yet. Only applications with a desktop file are
  recorded; stacked windows after the first in a column, dialogs and column pins are
  not restored. Applications that declare a single main window start once.
- **Workspace templates** open several applications together, one column each, on
  the empty workspace at the end of the focused output:

  ```kdl
  templates {
      template "dev" {
          app "code.desktop" width=66.667
          app "foot.desktop" width=33.333
      }
  }
  ```

  Run `canopa-shell --open-template dev`, for example from a Niri key binding.

## Wallpaper from applications

Applications that set a wallpaper through the Wallpaper portal hand it to the Shell
(`org.canopa.Shell.Wallpaper`), which stores it in
`$XDG_STATE_HOME/canopa/shell/wallpaper/` and draws it behind windows. The
`canopa-wallpaper` daemon and its picker are described in
[session tools](session-tools.md).

## Command line

| Command | Effect |
| --- | --- |
| `canopa-shell` | Run the Shell (one instance per session). A second instance exits with status 1 and draws nothing while another Shell owns `$XDG_RUNTIME_DIR/canopa-shell.sock`. |
| `canopa-shell --canopa-config PATH`, `--config PATH`, `--defaults` | Run with an explicit shared file, a legacy file, or the embedded defaults. |
| `canopa-shell --check-config [--show-source]` | Validate the configuration and print one JSON line; exit 0 or 1. |
| `canopa-shell reload` (or `--reload`) | Reload the running Shell's configuration and wait for the result (up to 10 s). |
| `canopa-shell --toggle-menu NAME` | Open a named menu, or close it if open. The shipped menus are `Applications`, `Power`, `Log out?`, `Reboot?` and `Power off?`. |
| `canopa-shell --open-menu NAME` | Open a named menu; repeating it reopens instead of closing. |
| `canopa-shell --open-template NAME` | Open a workspace template. |
| `canopa-shell --toggle-panel` | Toggle the bar's panel. |
| `canopa-shell volume …`, `microphone …`, `brightness …`, `keyboard-backlight …`, `media …` | Device keys (above). |
| `canopa-shell --dmenu [-p PROMPT] [-d DELIM] [--json] [--no-custom]` | Choose from lines on stdin; print the choice. |
| `canopa-shell lock` | Run Swaylock with the lock image set through the Wallpaper portal. This is not the Canopa lock screen; use `canopa-lock`. |
| `canopa-shell --version` | Print the version. |
| `canopa-shell --help` (or `-h`) | Print usage and exit. |

Unknown arguments are a usage error (exit 2) and never start the Shell; only
`--dmenu` ignores words it does not know, so dmenu-style callers can keep passing
their launcher's other flags.

Menu names are exact and case-sensitive; quote names with spaces, and use
`--toggle-menu=NAME` for a name starting with `-`. Menus open on the bar of Niri's
focused output. Menu and template commands exit 0 when the running Shell accepts the
request, 1 for an unknown name, no bar or no running Shell, and 2 for a usage error.
For example, in `~/.config/canopa/niri-custom.kdl`:

```kdl
binds {
    Mod+D { spawn "canopa-shell" "--toggle-menu" "Power"; }
}
```

### dmenu mode

`--dmenu` reads up to 1 MiB of entries from stdin, shows them in a panel of the
running Shell and prints the selected entry. `-d` sets the delimiter (`\n`, `\t`,
`\0` and `\\` escapes work). `--json` (or `--entry-format jsonl`) reads one JSON
object per line with `label`, `value`, `icon` and `keywords`, and prints the chosen
entry's `value` (its label when there is none); it cannot be combined with `-d`.
Without `--no-custom`, typed text that matches no entry can be returned as the
choice. Exit status:
0 selected, 1 cancelled, 2 usage error, 3 request failed.

```sh
printf 'First\nSecond\n' | canopa-shell --dmenu --prompt 'Choose' --no-custom
```

## External providers

Any program can supply a panel through the panel protocol. Declare it in
`panel-providers` and open it with `open-provider "NAME"`. See
[external panel providers](shell/external-panel-providers.md) for the protocol and
[configuration](configuration.md#panel-providers) for the declaration.

## Files

| Path | Contents |
| --- | --- |
| `$XDG_RUNTIME_DIR/canopa-shell.sock` | Control socket for `reload`, menus, templates and device keys |
| `$XDG_STATE_HOME/canopa/shell/taskbar-pins.json` | Taskbar pin changes |
| `$XDG_STATE_HOME/canopa/shell/windows.json` | Window restore record |
| `$XDG_STATE_HOME/canopa/shell/launcher-history.json` | Launcher selection history |
| `$XDG_STATE_HOME/canopa/shell/clipboard.json` | Clipboard options and snippets |
| `$XDG_STATE_HOME/canopa/shell/launcher-extensions.json` | Extension catalog published for Settings |
| `$XDG_STATE_HOME/canopa/shell/wallpaper/` | Wallpaper set through the portal |
| `$XDG_CONFIG_HOME/canopa/launcher.json` | Launcher preferences |

Logs: `journalctl --user -u canopa-shell`. Set `RUST_LOG=canopa_shell=debug` in a
unit drop-in (`systemctl --user edit canopa-shell.service`) for more detail.

## Known limitations

- The minimap does not mark which columns are currently on screen.
- Window restore does not restore pinned columns, and D-Bus-activated applications
  may open in Niri's usual place.
- Window thumbnails, pinned columns, overview typing and window restore need the
  Canopa Niri fork.
- `canopa-shell lock` supports only Swaylock.
- The Wi-Fi panel cannot set up new enterprise (802.1X) networks.
- The media panel has no artwork, seeking or per-player volume.
