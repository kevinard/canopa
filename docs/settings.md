# Canopa Settings

Open **Canopa Settings** from the launcher, press Super+Comma in the packaged
session, or run `canopa settings`. The window uses the shared Canopa appearance.
Closing the window leaves the independent desktop services running.

The sidebar groups pages under Devices, Appearance, System and Desktop. Type in
**Search settings** to list the pages whose name or keywords match, for example
`touchpad`, `wallpaper` or `suspend`; Enter opens the best match. Ctrl+F moves to
the search field. Tab/Shift+Tab navigate controls. On a sidebar page, Up and Down
move to the previous and next page; Ctrl+Page Up and Ctrl+Page Down do the same
from anywhere. Ctrl+1 through Ctrl+9 open the pages in sidebar order (currently
Ctrl+1 Displays to Ctrl+7 Widgets), and Ctrl+S saves the selected page.

Only one Settings window runs at a time. Opening Settings again, from the
launcher, a notification or the command line, brings the existing window
forward at the requested page, keeping unsaved edits on other pages. A page
request that arrives while a change is being applied or a display test is
running only brings the window forward.

## Displays

Connected displays expose enablement, available resolution/refresh modes, scale,
rotation and horizontal/vertical position in logical pixels. At least one display
must remain enabled. **Test display changes** applies temporary compositor settings
and offers **Keep changes** or **Revert** for 20 seconds. Escape also reverts.
Only Keep writes the managed display include. The separate helper restores the
previous live state if the window closes, crashes or misses the deadline, and
rolls back a partially failed apply. Concurrent trials are refused. Disconnected
or changed output identities and changed source files require reloading.

Canopa's session configuration is required for persistent display/input settings.
An ordinary Niri session remains separate. **Reload connected displays** discards
the display draft and uses the current compositor state. Output/workspace events
refresh an unedited draft; a pending draft is retained for deliberate review.

### Display profiles

Set up the connected displays in **Displays**, then enter a name under **Display
profiles** and choose **Save layout as profile**. This saves the current draft
without applying it. To switch manually, select a saved profile, choose **Test
saved profile**, then **Keep changes**. The same rollback window applies. You can
open a specific profile for review with `canopa-settings --display-profile "Desk"`.

**Suggest matching profiles** is the default. When you dock or undock, the
Canopa session offers the matching layout in a notification. **Review layout**
opens Displays with that profile selected. Choose **Switch automatically** and
save the policy to apply matching profiles without prompting. **Manual only**
disables offers and automatic switching.
If unplugging the dock leaves every connected display off, Canopa turns on one
available display so the desktop and any review prompt remain visible.

The saved profiles persist across logins. Switching one changes the live
layout while leaving the default Niri display include intact. The startup
policy controls whether Canopa offers or applies a matching profile when you
log in.

Edit `~/.config/canopa/display-profiles.kdl` (under `$XDG_CONFIG_HOME` if set)
to customize selectors, layouts, priorities and timing.

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

Use `canopa-settings outputs` to find connector names and monitor identities.
Each profile must cover the complete connected set, including any displays it
turns off, and enable at least one display. A saved layout uses make, model and
serial where that combination identifies a single monitor. With a missing or
duplicated serial, it also records the connector. In hand-written rules, combine
any of `connector`, `make`, `model` and `serial`. Use `"*"` to match any value
in a field. Each rule must resolve to a unique display across the entire profile.

An output defaults to enabled, preferred mode, scale 1, normal rotation and
position `x=0 y=0`. Specify all of `width`, `height` and `refresh-mhz` to
select a particular advertised mode. `custom-mode=#true` preserves an advertised
custom mode. `transform` accepts `normal`, `90`, `180`, `270`, `flipped`,
`flipped-90`, `flipped-180` and `flipped-270`.

Higher `priority` wins. Equal highest priorities leave the layout for manual
selection. Set `hotplug=#false` on a profile to keep it manual under any policy.
`settle-ms` coalesces hotplug events and accepts 100–5000 ms. `offer-ms` accepts
1000–120000 ms. File and policy edits take effect on the next topology change.

## Keyboard and pointer

Choose XKB layout identifiers in switch order, optional matching variants and XKB
options. For example, layouts `us,fr` and option `grp:alt_shift_toggle` allow
Alt+Shift switching. Keymaps are compiled before saving. Repeat delay/rate, mouse
speed/buttons/scroll direction and touchpad speed/tap/scroll direction/typing
suppression are applied through Niri's validated input include and reload.
Custom device rules belong in the separate custom include; ambiguous edited
managed input files are read-only rather than silently losing extra settings.

## Shortcuts

Press Ctrl+6 to find packaged keyboard actions by name or current key. Enter a
Niri combination such as `Mod+Shift+Space`, then use **Save changes**. In the
physical Canopa session, Mod is normally Super. Leave a shortcut empty to turn
it off. **Reset to defaults** stages the original keys for review before saving.
Save rejects duplicate assignments and new conflicts with included bindings.

Settings stores only your changes, in `~/.config/canopa/shortcuts.kdl`. Shortcuts
you have not changed follow Canopa updates. Because Niri cannot remove an
inherited key, Settings combines the shipped defaults with your changes into
`niri-shortcuts.kdl` and includes that file in `niri.kdl` in place of the
defaults. Canopa regenerates it at each login. If an update assigns a default to
a key you use, your binding wins and the new default appears unassigned on this
page. Resetting every shortcut restores the original defaults include. Other
includes stay in order. Keep your own bindings in `niri-custom.kdl`. Custom main
configurations with a different binding set are edited in KDL.

`shortcuts.kdl` lists added and changed bindings under `binds`, and the shipped
actions you moved or turned off under `remove`:

```kdl
defaults "/usr/share/canopa/niri-defaults.kdl"
binds {
    Mod+Shift+Space { spawn "canopa-shell" "--toggle-menu" "Applications"; }
}
remove {
    spawn "canopa-shell" "--toggle-menu" "Applications"
}
```

Earlier versions of Settings copied all shipped bindings into `niri.kdl` on the
first save. At the next login, Canopa moves your changes from that copy into
`shortcuts.kdl` and drops the bindings identical to the copied defaults, so they
follow updates again. Edited copies of other default sections stay in `niri.kdl`.
The previous file is kept as `niri.kdl.before-shortcut-changes`. Saving on the
Shortcuts page does the same.

The volume and microphone keys once ran `wpctl`; they now run Canopa Shell
device keys, beside new brightness, keyboard-backlight and media keys. At the
next login, saved changes that name the old `wpctl` actions follow them: a moved
volume key keeps its key with the new command, and a turned-off key stays off.

## Personalization

Light/dark appearance and an optional accent update the shared configuration.
The wallpaper button opens the existing picker, including per-monitor previews
and saved assignments. Its default library is `~/Pictures/Wallpapers`; add local
images there when no wallpaper folder is configured. Picker startup errors appear
in Settings. Lockscreen fields include clock/date formats, font, avatar and
background dimming. The owning application schemas
validate the entire candidate; unrelated namespaces, custom fields and comments
are retained. The Settings portal receives its explicit reload request; other
applications use their existing live-reload contracts. A currently active lock
screen keeps its own authentication/presentation lifetime.

**Text size** sets 100–150% for Canopa's shared typography in Shell, Files and
Notify. It also scales the Settings interface. **Reduce Shell panel motion**
disables the Shell's panel open and close transitions. Save changes to apply
both preferences.

**Reduce window and workspace motion** applies independently in the Canopa
session. It adds an animation override to the compositor configuration, which
Niri validates before applying. **Use Niri animation settings** removes the
override and restores the effect of your existing Niri settings.

## Widgets

Open **Widgets** in Settings or press Ctrl+7. Choose **Bar layout** or **Lock
screen layout**. Select a widget in the diagram or with **Placed widget**. Use
**Zone** or **Region** to move it, **Earlier** and **Later** to reorder it, and
**Add widget** or **Remove widget** to change what appears. On the lock screen,
you can also set the selected region to flow in a row. **Preview monitor** uses
that monitor's proportions and visibility. **Preview scale** changes the size
of the sample widgets in the diagram.

For the bar, select the screen edge and choose **All monitors**, **Selected
monitors**, or **All except selected monitors**. Duplicate a bar to give another
monitor its own layout. A duplicate starts with an empty monitor selection so
you can assign it before saving. Give each bar a distinct monitor target if you
want one bar per screen. The editor lists built-in modules with defaults
and modules already declared in your configuration. Module styling, actions and
panel settings remain editable in KDL.

For the lock screen, select a monitor, select a placed widget, then choose
**Hide this widget on the preview monitor**. The authentication box can move
between regions and remains available on every screen. The diagram uses sample
data. Configured script widgets become available once defined in KDL. See
[lockscreen layout and widgets](session-tools.md#layout-and-custom-widgets).

**Save changes** validates and saves the drafts. Shell applies the bar layout
through its live reload. Lock screen changes take effect the next time you lock.

## Power and idle

Stasis status, manual pause/resume and native profile selection use Canopa Idle.
Standard AC/battery lock, display-power and suspend timeout fields are in seconds;
they are sequential waiting periods, not cumulative times from the last input.
Saving changes only existing numeric timeout tokens, retaining commands, custom
profiles and inhibition rules. Stasis owns reload validation. A rejected reload
restores the previous file. Custom nonstandard plans that cannot be edited without
guessing are read-only; status and manual controls still work.

## Configuration safety

Saving checks the original source again, validates a same-directory private
candidate, then atomically replaces the file. External edits are rejected; use
**Reload this page and discard its edits** to adopt them. It affects only the
selected page; drafts on other pages remain. Personalization and Widgets share
`config.kdl`: saving one keeps the other's unsaved edits on top of the saved
file, but a change made outside Settings still has to be reloaded. Owned regular files are required for
editing; symlink-managed configuration remains read-only. Legacy-only application
configuration must be explicitly migrated before creating a common Canopa file,
so a new appearance file cannot silently hide existing shell/notification settings.
See [configuration](configuration.md) and [installation](installation.md).

## Opening a page

`canopa-settings PAGE` opens a page; `--page PAGE` is the same. The page ids are
`displays`, `input` (Keyboard & pointer), `personal` (Personalization), `power`,
`launcher`, `shortcuts` and `widgets`. When Settings is already open, the window
switches to the page and the command exits. Opening a page never saves
anything; the page's own controls remain responsible for writes. A page cannot
be combined with command-line operations such as `outputs` or `check`.

Programs can do the same on the session bus. Settings owns `org.canopa.Settings`
and serves the `org.canopa.Settings` interface at `/org/canopa/Settings`:

```sh
busctl --user call org.canopa.Settings /org/canopa/Settings org.canopa.Settings OpenPage 'sa{sv}' power 0
busctl --user call org.canopa.Settings /org/canopa/Settings org.canopa.Settings Activate
busctl --user get-property org.canopa.Settings /org/canopa/Settings org.canopa.Settings Pages
```

`OpenPage` takes a page id and options; `display-profile` (a string) selects a
saved layout on `displays`. `Activate` only brings the window forward, and
`Pages` lists the page ids. The installed D-Bus service file starts Settings
when no window is open. Without a session bus each launch opens its own window.

The Shell's configuration-error notification opens Widgets for a problem in its
`shell` section and Personalization for syntax, schema-version and appearance
problems.

The Launcher page manages extension enablement/keywords and web search templates,
and shows extension actions and last status. Save applies preferences to the
existing launcher panel automatically; concurrent pins and aliases are preserved.

In the Settings window, Ctrl+5 selects Launcher and Ctrl+S saves extension and
web search edits. The **Clipboard history** section has its own **Save clipboard
settings** button. It controls recording, whether to include images, and the
maximum number of recent copies. Browse those copies with `clip` in Applications.
