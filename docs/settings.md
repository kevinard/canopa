# Canopa Settings

Open **Canopa Settings** from the launcher, press Super+Comma in the packaged
session, or run `canopa settings`. The window uses the shared Canopa appearance.
Tab/Shift+Tab navigate controls; Ctrl+1 through Ctrl+4 select pages, and Ctrl+S
applies the selected page. Closing the
window leaves the independent desktop services running.

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

## Keyboard and pointer

Choose XKB layout identifiers in switch order, optional matching variants and XKB
options. For example, layouts `us,fr` and option `grp:alt_shift_toggle` allow
Alt+Shift switching. Keymaps are compiled before saving. Repeat delay/rate, mouse
speed/buttons/scroll direction and touchpad speed/tap/scroll direction/typing
suppression are applied through Niri's validated input include and reload.
Custom device rules belong in the separate custom include; ambiguous edited
managed input files are read-only rather than silently losing extra settings.

## Personalization

Light/dark appearance and an optional accent update the shared configuration.
The wallpaper button opens the existing picker, including per-monitor previews
and saved assignments. Its default library is `~/Pictures/Wallpapers`; add local
images there when no wallpaper folder is configured. Picker startup errors appear
in Settings. Lockscreen fields include clock/date formats, font,
avatar, background dimming and standard widgets. The owning application schemas
validate the entire candidate; unrelated namespaces, custom fields and comments
are retained. The Settings portal receives its explicit reload request; other
applications use their existing live-reload contracts. A currently active lock
screen keeps its own authentication/presentation lifetime.

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
**Reload and discard edits** to adopt them. Owned regular files are required for
editing; symlink-managed configuration remains read-only. Legacy-only application
configuration must be explicitly migrated before creating a common Canopa file,
so a new appearance file cannot silently hide existing shell/notification settings.
See [configuration](configuration.md) and [installation](installation.md).

The launcher can open a particular page with `canopa-settings --page displays`,
`--page input`, `--page personal`, `--page power`, or `--page launcher`. The option selects the initial
page only; normal Apply controls remain responsible for writes. It cannot be
combined with command-line operations such as `outputs` or `check`.

The Launcher page manages extension enablement/keywords and web search templates,
and shows extension actions and last status. Save applies preferences to the
existing launcher panel automatically; concurrent pins and aliases are preserved.

In the Settings window, Ctrl+5 selects Launcher and Ctrl+S saves extension and
web search edits. The **Clipboard history** section has its own **Save clipboard
settings** button. It controls recording, whether to include images, and the
maximum number of recent copies. Browse those copies with `clip` in Applications.
