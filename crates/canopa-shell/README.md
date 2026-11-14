# Canopa Shell

This application is part of the [Canopa workspace](../../README.md). Run the
commands below from the repository root. Cargo uses the root lockfile and writes
normal build outputs to root `target/`.

Canopa's desktop shell for Niri, built with Rust and Iced. It includes configurable bars,
workspaces, a launcher, tray menus, battery, network, volume, clock/calendar,
script modules, dmenu selection, and external panels.

## Build and run

Use Rust 1.95.0 (the CI baseline) or newer. On Debian/Ubuntu, install the native
build dependencies:

```sh
sudo apt-get install build-essential pkg-config libpulse-dev libwayland-dev libxkbcommon-dev libudev-dev dbus at-spi2-core
cargo +1.95.0 build --locked --release -p canopa-shell --bins
target/release/canopa-shell
```

Run inside a Niri/Wayland desktop session with its session D-Bus environment.
Keep `canopa-wifi-provider` and `canopa-audio-provider` alongside `canopa-shell` or on PATH. PulseAudio or
PipeWire's PulseAudio compatibility service supplies audio state and control.
Ordinary desktop-entry launching requires `gio` (`libglib2.0-bin` on Debian/Ubuntu).
D-Bus-enabled entries activate through the session bus. Executable launches use niri's IPC; without `NIRI_SOCKET`, they require
`systemd-run` and a systemd user manager.

Configuration comes from the `shell` namespace of `$XDG_CONFIG_HOME/canopa/config.kdl`
(default `~/.config/canopa/config.kdl`). When that file is absent, the legacy
`niribar/config.kdl` remains supported; otherwise missing namespaces use embedded
defaults. See the [shared configuration guide](../../docs/configuration.md) for validation,
explicit sources, and migration. Saved changes apply within about half a second,
without a restart. An invalid file never stops the Shell: it keeps the last working
configuration (or starts with the embedded defaults), shows a red badge on the bar
and a notification with the file, line and message, and applies the next valid
save. Click the badge to open the file. Existing option
examples in [config/default.kdl](config/default.kdl), [docs](../../docs/shell/) and
[CONTEXT.md](../../docs/shell/CONTEXT.md) go inside the shared `shell` block.
The adjacent shared `appearance` block controls the dark/light palette and accent
for the shell, Notification Center, and Notify banners.

The bundled left bar is Canopa's spine: a 64-pixel rounded frame, 12-pixel outer
spacing and a stable 88-pixel reservation. Launcher, native overview and local
workspaces are at the top, the taskbar in the middle, and status and time below: the system tray,
a status group of network, volume and battery icons, notifications and the clock
(time, day period and date). Spine buttons are icons with tooltips and accessible
names; the one whose panel is open is highlighted. Workspaces are pills: accent for
the focused one, a filled pill when occupied and an outline when empty. The bell
carries the unread count as a badge.

The taskbar shows the applications listed in its `pinned` desktop IDs (missing
ones are skipped), then the other open applications in screen order: by
workspace, then column, as Niri lays them out, grouped by desktop file. A dot on the bar's screen edge marks a running
application, a longer mark several windows, and an accent mark the focused one.
Click focuses the application's most recent window or launches it; clicking it
again cycles through its windows from left to right. Middle-click starts another instance.
Right-click opens the application's menu: Open (or New window while it runs,
unless the desktop file declares a single main window), the desktop file's own
actions such as New Private Window, then, while it runs, layout commands for its
window in front or its most recent one (one-third, half or two-thirds width,
tabs, center, float or tile, move to the workspace above or below), and Pin to
taskbar or Unpin from taskbar.
Pins changed there are saved in `$XDG_STATE_HOME/canopa/shell/taskbar-pins.json`
as changes to the configured list, so `pinned` keeps its order and later
additions to it still appear.

```kdl
module "taskbar" {
    pinned "org.canopa.Files.desktop" "firefox.desktop"
}
```

The minimap draws Niri's scrolling strip for the workspace shown on the bar's
output: one block per column, from left to right, split by the column's stacked
windows. Widths are to scale with the screen, the focused window is in the
accent color and the rest of its column is brighter than the other columns.
Click a block to focus that window; scroll over the strip to move focus to the
next or previous column. It does not mark which columns are on screen yet,
because Niri does not report that for tiled windows. Floating windows are not
drawn; the accessible description counts them. `size` is the strip's thickness
across the bar (8–64, default 18) and `max-length` the longest it grows along
the bar before every column shrinks to fit (default 240, 0 for no limit).

```kdl
module "minimap" {
    size 18
    max-length 240
}
```

Network and volume hide while their service is unavailable, and
battery appears only when UPower reports a present battery. The tray (and the
`org.kde.StatusNotifierWatcher` it owns) runs because the default bar places it.
Only the embedded defaults changed: a `shell` block in your `config.kdl`, including
one saved by Settings, keeps its own layout. Add `"tray" "network" "volume"
"battery"` to a bar's `end` zone, or place them in Settings > Widgets, to match. Inspect the bundled layout
with `target/debug/canopa-shell --canopa-config examples/canopa-desktop.kdl`; the
example selects the shared Canopa palette, with `"light"` available as a companion.
The status group's icons use `toggle-bar-panel` to open that bar's configured
modules; inside Quick settings the same modules open their own providers (Wi-Fi,
Sound). Add the `system` button to a bar for a separate Quick settings button;
`open-panel` and `toggle-panel` on a module still open its own details.
Quick settings starts with two-up tiles: Wi-Fi and Bluetooth (the tile switches the
radio through NetworkManager or BlueZ; its chevron opens the provider), then
Do Not Disturb and Keep awake. A tile that is on uses the accent fill. The Sound
card names the default output and carries a level slider and mute button; Power
& display shows the battery charge and, when a built-in backlight exists, a
brightness slider on the same curve as the brightness keys. Wi-Fi and Sound remain reachable when status
is unavailable; a radio without a service shows a plain row instead of a tile.
Provider Back returns to Quick settings on the same output. Tab/Shift+Tab select
cards and Enter activates them. Battery and media appear when they have content;
Calendar opens separately from the clock.

**Bluetooth** uses the system BlueZ service. Turn on an adapter, choose **Find
devices**, and pair or reconnect from the device list. Passkey and PIN
requests appear in the same panel. Closing the panel ends its discovery and
cancels an unfinished pairing.
**Power & brightness** shows the built-in display backlight when the active
Wayland login can use logind. The power profile list reflects the options
advertised by the system Power Profiles daemon or a compatible service.
The launcher can also open both panels.

Lock, Suspend and Power share the footer. Power opens a menu with Hibernate,
Log out…, Reboot… and Power off…; the hibernate, logout, reboot and poweroff
buttons remain available to place in the panel's end zone for one-click access.
Lock launches `canopa-lock` with session ownership (install its PAM service first; see
[session tools](../../docs/session-tools.md)). Suspend and Hibernate act at once.
Log out, Reboot and Power off open confirmation menus with Cancel selected first;
Cancel returns to Quick settings. Log out asks Niri to quit. The Power menu leaves
out an item whose confirmation logind would refuse.

Suspend, Hibernate, Reboot and Power off ask systemd-logind directly with the
`session "suspend"`, `"hibernate"`, `"reboot"` and `"power-off"` actions, never
through a timed command, so polkit can ask for a password (for example while
other users are logged in). Buttons for actions logind refuses (`CanSuspend` and
friends answer `no` or `na`) are hidden, and Hibernate appears only when
`CanHibernate` answers `yes`. A confirmation lists applications that block the
action, such as "Blocked by Firefox: Downloading files", and a failed request is
reported in a notification instead of failing silently. These ordinary button
actions and menus can be rebound in the configuration; `session` also works in
your own menus, where items for refused actions are left out.

For Swaylock sessions, `canopa-shell lock` passes the lockscreen image selected
through the Wallpaper portal to Swaylock. Custom launch commands remain literal argv.

The bundled Applications menu searches applications, open windows, file/folder
names, desktop controls, arithmetic and unit conversions using the same panel
facilities. Use `files budget`,
`apps firefox`, `12*(8+3)`, `3 x 4` or `10 km to miles` to search or calculate;
the `=` prefix is optional and only adds error explanations. Enter
copies a calculator result. File actions include Copy path, reveal, Open with and
Quick Look. Recent/frequent selections improve ranking; history can be cleared or
disabled. Use `web query`, paste a URL, or try `time in Tokyo` and `timestamp 0`.
Tab searches a result's actions, including Pin and Set alias. The Launcher page
in Settings manages extensions, keywords, search templates and last provider status.
Configured executable extensions add results and named actions. See the [launcher guide](../../docs/shell/launcher.md)
for file coverage, refresh behavior, calculator setup, and extension configuration.

### Bar text and icons

Bars use defaults of 12 logical pixels for status text and 20 for icons, including
workspace icons, tray images and focused-window application icons. Each role
keeps its size on every edge, including rotated titles.
The compositor's output scale applies to all three.

Standalone Nerd Font glyphs use `Symbols Nerd Font Mono` when installed, avoiding
the compressed icon shapes in patched coding fonts. Normal text keeps its default
font, and systems without the symbols font keep normal glyph fallback.

Unrotated icon segments also accept `canopa:mark`, `canopa:overview`,
`canopa:controls`, `canopa:lock`, `canopa:logout`, `canopa:suspend`,
`canopa:hibernate`, `canopa:reboot`, and `canopa:poweroff`: embedded monochrome
SVGs following the configured size/color.

In bar formats, a standalone `"{icon}"` segment uses the icon size; other segments
use the text size. Write `format "{icon}" "{value}"` to keep those roles distinct.
A mixed segment such as `"{icon} {value}"` stays one text run. Workspace `"{icon}"`
uses the text size when it falls back to a workspace number.

Explicit bar `style size=...` and segment sizes override these defaults; segment
styles take precedence over the bar style. Tray images also follow the explicit
bar size.

Focused-window titles are limited to 50 displayed characters by default, including
a middle ellipsis (`…`), so long titles leave room for other modules. The beginning
and end are preserved, and emoji and combining marks stay intact. Set `max-length`
on the module to change this limit, or `0` to disable it:

```kdl
modules {
    module "focused-window" {
        max-length 40
    }
}
```

The limit applies to `{title}` and its `{value}` alias in all display formats,
including rotated bar content. Tooltips keep the full title.

### Panel appearance and motion

Drawers and automatic popovers share headings, close controls, menu rows, spacing,
and focus treatment. Their sizing and anchoring continue to follow the selected
presentation. Panels slide 8 logical pixels from their originating edge and fade
in or out, without changing the desktop's reserved space on each frame. Opening
decelerates into place; closing eases gently to transparent before removal.

The existing per-bar animation settings now control these transitions:

```kdl
panel {
    animation {
        open-ms 180
        close-ms 160
    }
}
```

Set both durations to `0` for reduced motion. Either duration can be disabled
independently. Motion is configured per bar; the shared `appearance` block does not
currently read a desktop motion preference. Saved changes apply without a restart.
Closing ends interaction immediately; the remaining exit image cannot receive input.

### Vertical side drawer

The shipped left bar opens the launcher, Quick Settings, and provider panels in a
416-logical-pixel side drawer with rounded corners and 12-pixel vertical margins.
The `adaptive` presentation opens individual module details, such as Calendar,
in popovers beside their bar controls and fits short search results and standalone
provider status messages to their content. The panels overlay the desktop without
resizing tiled windows. Click the originating module again or use Close to dismiss
the panel. Escape clears an active search before closing its panel.

Use `presentation "adaptive"` in an existing bar's `panel` block to match the
shipped behavior. Use `presentation "drawer"` for a full-height drawer for every panel.
Remove any existing `height` or `max-height` to let the drawer fill the screen's
usable height; for example, an old `height 720` keeps the drawer at 720 logical pixels.

```kdl
panel {
    presentation "adaptive"
    // drawer-mode "push-bar" // drawer at the edge, bar on its inner side
    // width 400       // optional consistent drawer width
    // max-height 800  // optional cap; the drawer stays at the top
    // Keep your existing start / center / end module lists here.
}
```

Explicit `width`/`height` and `max-width`/`max-height` still apply. On top/bottom
bars, the drawer preference uses the existing automatic layout. Omitted
`presentation`, or `presentation "auto"`, keeps the previous content-aware panel
sizing. Tooltips and tray context menus remain small popups. Saved changes apply
without a restart.

`drawer-mode "overlay"` (the default) keeps the bar at the screen edge and opens
the drawer beside it. `drawer-mode "push-bar"` opens the drawer at the bar's edge
and moves the bar inward: on the left, the drawer is on the left and the bar on
its right; on the right, this is mirrored. Closing the drawer returns the bar to
the edge. Both modes overlay application windows without resizing them.
`drawer-mode` applies to drawer surfaces on left and right bars.

For an existing module with no click action, add `actions { click { open-panel; } }`
inside that module to open its details. The shipped clock, sound, battery and
focused-window modules include this binding. Network and notifications retain
their provider actions; workspace switching and custom action bindings still work
as configured.

Use `launch "kitty"` or `launch "firefox" "https://example.org"` in menu items
and module actions to open applications. Arguments are literal strings. Launches
belong to the desktop session and survive bar restarts. Existing `command`
actions remain bounded automation (three seconds by default); migrate any
application-opening `command` actions in your own config to `launch`.

```sh
canopa-shell --version
canopa-shell --toggle-panel
canopa-shell --open-menu apps
canopa-shell --open-menu main
canopa-shell --toggle-menu apps
printf 'First\nSecond\n' | canopa-shell --dmenu --prompt 'Choose' --no-custom
```

The IPC commands require a running bar. Selection input is bounded to 1 MiB.

`--open-menu NAME` opens any named menu in the running bar's configuration,
including menus with `source "installed-apps"` or `source "launcher"`. Names are exact and case-sensitive;
quote names containing spaces and use `--open-menu=NAME` or `--toggle-menu=NAME`
for names starting with `-`. The request uses a bar on Niri's focused output,
falling back to one deterministic bar surface if that output has no bar or focus
is unavailable. Opening a menu replaces the current interactive overlay; repeating
the command opens the menu again instead of toggling it closed.

`--toggle-menu NAME` closes that menu if it is already open, even when it was
opened by a module or on another output. Otherwise it opens the requested menu,
replacing any other interactive overlay. Toggling a menu while its installed-app
entries are still loading cancels that opening request.

Both commands exit as soon as the running bar accepts the request, without waiting
for a selection. Exit status is `0` for acceptance, `1` for an unknown menu, no
available bar surface, or an IPC failure, and `2` for invalid command-line usage.
The running bar must also support this command; restart it after upgrading.

For example, put this inside Niri's existing `binds` block to toggle an `apps` menu:

```kdl
Mod+D repeat=false hotkey-overlay-title="Run an Application: canopa-shell" {
    spawn "canopa-shell" "--toggle-menu" "apps";
}
```

## Install a release

For an existing Niribar installation, first follow the
[shell rename guide](../../docs/shell-rename.md) to migrate services and startup commands.

The Canopa Linux x86-64 archive contains the companion executables, shared
configuration, documentation and user service units. From the extracted directory:

```sh
mkdir -p ~/.local/bin ~/.config/systemd/user
install -m755 bin/canopa-shell bin/canopa-wifi-provider bin/canopa-audio-provider bin/canopa-notify bin/canopa ~/.local/bin/
cp share/systemd/user/*.service share/systemd/user/canopa-session.target ~/.config/systemd/user/
systemctl --user daemon-reload
```

The units use `~/.local/bin`. Once the Niri session exports `WAYLAND_DISPLAY` and
`NIRI_SOCKET` to the user service manager, enable `canopa-session.target` to start
both companions. Use one notification daemon in the session. See the
[notification and session guide](../../docs/notifications.md) for controls and restart
behavior. Canopa Shell can still be launched directly from Niri's startup configuration;
use one startup mechanism for each process.

## Windows, workspaces and the overview

These use the Canopa Niri fork; the packaged compositor provides them.

Typing in Niri's overview searches the launcher: the first key opens the
launcher menu (the one whose source is `launcher`, Applications by default)
over the overview with that text, and closing the launcher closes the overview.
The shipped Niri defaults turn on `overview { emit-typed-text; }`, which this
needs.

With `windows { restore #true; }`, the Shell records the open applications and
where they sit (workspace, column, window width) in
`$XDG_STATE_HOME/canopa/shell/windows.json` and starts them again in the same
places at the start of the next session, if no application windows are open
yet. Only applications with a desktop entry are recorded; the later windows of
a stacked column and dialogs are left out, and an application that declares a
single main window is started once. Window closes are recorded after eight
seconds, so ending the session does not empty the record. Workspaces without a
name are restored by index on the focused output. D-Bus-activated applications
may not carry Niri's activation token to their window and then open in Niri's
usual place.

A taskbar application's tooltip shows thumbnails of up to four of its windows,
in screen order, from Niri's window preview service
(`org.canopa.Niri.WindowPreview1`; `window-previews { off; }` in Niri turns it
off). The taskbar asks for fresh thumbnails each time its tooltip opens and
shows the last ones meanwhile.

A column pinned to an edge of the view (`Mod+P` left, `Mod+Shift+P` right in
the shipped defaults, or the taskbar menu's Pin column entries) stays on screen
while the rest of the strip scrolls. The minimap outlines it and sets it apart
from the strip. Window session restore does not restore pins.

A workspace template opens applications together, one column each, on the empty
workspace at the end of the focused output, and shows it:

```kdl
templates {
    template "dev" {
        app "code.desktop" width=66.667
        app "foot.desktop" width=33.333
    }
}
```

`width` is a percentage of the working area, like Niri's `proportion`; without
it Niri's default column width applies. Open one with
`canopa-shell --open-template dev`, for example from a Niri key binding.

## Device keys and on-screen display

The session's Niri defaults bind the volume, microphone, brightness,
keyboard-backlight and media keys to Canopa Shell commands. The Shell shows the
new level for a moment in a small display above the bottom of the focused output:
an icon, the device, a level bar and the value. Repeated presses update it in
place. It also appears when another application, or the firmware, changes the
volume, the default output, microphone mute or the display brightness, but never
at login. It takes no keyboard focus, lets clicks through, follows the shared
appearance, skips its fade-out with reduced motion and is announced to screen
readers.

```sh
canopa-shell volume up [STEP]       # also: down [STEP], mute
canopa-shell microphone mute
canopa-shell brightness up [STEP]   # also: down [STEP]
canopa-shell keyboard-backlight up  # also: down, toggle
canopa-shell media play-pause       # also: play, pause, stop, next, previous
```

Volume goes through the audio server and stops at 100 %; raising it unmutes.
Brightness goes through logind without root, in perceptual steps that never go
below 1 % of the range. Media keys control the player selected in the media
module, or the playing one. Without a running Shell the commands still work,
without the display. Steps and the timeout are set in an optional `osd` block:

```kdl
shell {
    osd {
        // enabled #false hides the display; the keys still work.
        volume-step 5        // 1-100, default 5
        brightness-step 5    // 1-50, default 5
        timeout-ms 1500      // 500-10000, default 1500
    }
}
```

See [ADR 0025](../../docs/shell/adr/0025-device-keys-and-on-screen-display.md).

## Wi-Fi and external panels

The optional `media` module displays the selected MPRIS player and opens a panel
with player selection and previous/play-pause/next controls. See the
[Media guide](../../docs/media.md) for configuration and selection behavior.

The volume module opens **Sound**, with speaker/headphone and microphone volume,
mute and default-device controls. Existing volume `open-panel` bindings also open
Sound. Other explicit actions remain authoritative. See [Sound controls](../../docs/audio.md)
for keyboard behavior, routing semantics and validation limits.

The built-in Wi-Fi provider prefers NetworkManager; when it is absent, it uses
iwd. It supports scanning, saved/open networks, personal credentials, and
hidden personal networks. The system daemon controls authorization and saved
credentials. New enterprise/EAP provisioning is outside the panel's scope;
provision such profiles with the system's network tools first. iwd network
configuration also depends on the system's existing DHCP/DNS setup.

Built-in providers use `canopa-v5`; configured providers also support explicit
`niribar-v1` and `canopa-v4`. All contracts
are available directly from [canopa-panel-protocol](../canopa-panel-protocol/README.md)
without depending on the shell. See
[the provider guide](../../docs/shell/external-panel-providers.md).

## Development and validation

To update your local desktop installation whenever you choose, run:

```sh
./scripts/update.sh
```

This updates Canopa Shell, its Wi-Fi helper, Notify, and the `canopa` configuration tool
together from the current checkout, including uncommitted changes. It validates
configuration before stopping services, backs up binaries and notification state,
and verifies startup. Intentionally stopped services stay stopped. Configuration
and service definitions are preserved. See the [workspace update guide](../../README.md)
for prerequisites, backups, recovery, and build-job options. The script does not
fetch Git updates.

```sh
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked -p canopa-shell --all-targets -- -D warnings
dbus-run-session -- cargo +1.95.0 test --locked -p canopa-shell --all-targets
python3 scripts/check-contrast.py
python3 scripts/test-update.py
```

For the combined release archive, build all workspace binaries and run the root
packager as described in the [Canopa guide](../../README.md).

The transport suite also requires `libglib2.0-bin`, `pipewire`, `pipewire-bin`, `pipewire-pulse`
and `pulseaudio-utils`. Tests start private D-Bus/audio daemons and Unix sockets;
environments that forbid local socket binding cannot run the transport suite.
Audio fixtures use synthetic sinks without hardware discovery. Tests do not
change your desktop's audio or Wi-Fi services. See [the audit](../../docs/shell/audits/2026-09-23-project-state.md)
and [follow-up evidence](../../docs/shell/audits/2026-09-23-remediation.md).
The [desktop validation harness](../../docs/shell/manual-validation.md) runs a private
compositor session for measurements and panel checks. The
[accessibility assessment](../../docs/shell/ACCESSIBILITY.md) records default contrast ratios,
native AT-SPI coverage, and remaining operator checks. Nix packaging is deferred;
the optional CI job runs only if a flake is added.

Canopa Shell is [MIT licensed](LICENSE). Vendored dependencies retain their own
licenses; see [vendor maintenance](../../docs/shell/vendor-maintenance.md).
