# Configuration reference

Canopa's applications share one KDL file:

```text
$XDG_CONFIG_HOME/canopa/config.kdl      (default ~/.config/canopa/config.kdl)
```

`XDG_CONFIG_HOME` is used only when it is set to an absolute path; otherwise
`$HOME/.config` is used. The file is optional: every application has embedded
defaults. A sample with comments is installed as `/usr/share/canopa/canopa.kdl`
(`share/canopa/canopa.kdl` in the release archive; [`config/canopa.kdl`](../config/canopa.kdl)
in the source).

```kdl
schema-version 1

appearance {
    color-scheme "dark"
}

shell {
    bar "main" { edge "top"; }
}

notifications {
    rule "quiet-chat" app="org.example.Chat" outcome="silenced"
}

files {
    view mode="list" sort="modified" descending=#true
}
```

Canopa Settings edits the same file: Personalization writes `appearance` and
`lockscreen`, Widgets writes `shell` bars and the lock screen layout, Date & time
writes clock formats, and Sound writes `notifications { sound … }`. Files'
Preferences window writes `files`. Hand edits and graphical edits can be mixed.

## File format

- The file must be a readable regular UTF-8 file of at most 1 MiB. A symlink to
  such a file works. The main file and its theme file together are limited to 1 MiB.
- `schema-version 1` is required. The other top-level nodes are optional
  blocks, each at most once, with no arguments: `appearance`, `shell`,
  `notifications`, `files`, `wallpaper`, `lockscreen`, `greeter`, `polkit`,
  `pinentry`. Any other top-level node is an error.
- A missing block means that application uses its embedded defaults. It still
  receives the shared `appearance`.
- Syntax errors, envelope errors and `appearance` errors affect every application.
  An error inside a block affects only the application that owns it.
- If the file exists, it is used, even when it is invalid or a dangling symlink.
  There is no silent fallback to another file.

| Block | Owner | Validator | Guide |
| --- | --- | --- | --- |
| `appearance` | every application, plus the Settings portal | all of them | [Theme customization](theme-customization.md) |
| `shell` | Canopa Shell | `canopa-shell --check-config` | [Shell](shell.md), [Launcher](launcher.md) |
| `notifications` | Canopa Notify | `canopa-notify check-config` | [Notifications](notifications.md) |
| `files` | Canopa Files | `canopa-files --check-config` | [Files](files.md) |
| `wallpaper` | `canopa-wallpaper` | `canopa-wallpaper check-config` | [Session tools](session-tools.md) |
| `lockscreen` | `canopa-lock` | `canopa-lock check-config` | [Session tools](session-tools.md) |
| `greeter` | `canopa-greeter` | `canopa-greeter check-config` | [Login screen](greeter.md) |
| `polkit` | `canopa-polkit` | `canopa-polkit check-config` | [Polkit](polkit.md) |
| `pinentry` | `canopa-pinentry` | `canopa-pinentry check-config` | [Pinentry](pinentry.md) |

The greeter does not read your file. It reads `/etc/canopa/greeter.kdl`, which uses
the same format with an `appearance` and a `greeter` block.

## Check the configuration

```sh
canopa config check
canopa config check --file /path/to/candidate.kdl
canopa config check --show-source
```

Without `--file`, the command checks the file applications would select now (the
shared file, a legacy file, or the defaults). `--file` always reads the shared
format. Each application's own validator checks the same snapshot of the file and
its theme, and the command prints one JSON line per component:

```json
{"component":"shell","source":{"path":"/home/alex/.config/canopa/config.kdl","format":"canopa-v1","fingerprint":"…","defaults":false}}
{"component":"files","error":{"code":"schema","offset":412}}
```

The exit status is nonzero if any component fails. Diagnostic codes are `location`,
`read`, `syntax`, `envelope`, `schema` and `appearance`; normal output never
contains configuration values. `--show-source` also prints each validator's
detailed message on stderr, which can quote your configuration.

All validators must be installed in the same directory as `canopa`. The greeter is
checked against your file, not `/etc/canopa/greeter.kdl`; use
`canopa greeter check-config` for the system file.

`canopa` also forwards `settings`, `wallpaper`, `lock`, `greeter`, `polkit`,
`pinentry` and `idle` to the matching `canopa-*` program, so `canopa lock` runs
`canopa-lock`.

## Migrate legacy files

When the shared file is absent, two applications still read their old files:

| Application | Legacy file |
| --- | --- |
| Canopa Shell | `$XDG_CONFIG_HOME/niribar/config.kdl` |
| Canopa Notify | `$XDG_CONFIG_HOME/canopa-notify/config.kdl` |

The other applications use their defaults. Creating the shared file replaces the
legacy files for both; they are never merged. To convert them:

```sh
canopa config migrate --shell ~/.config/niribar/config.kdl \
    --notifications ~/.config/canopa-notify/config.kdl \
    --output ~/canopa-migrated.kdl
canopa config check --file ~/canopa-migrated.kdl
mv ~/canopa-migrated.kdl ~/.config/canopa/config.kdl
```

Either source can be omitted. The command wraps each file in its block, keeps
comments, validates the result with every owner, and writes it with mode 0600. It
never overwrites an existing output, and the output's directory must exist.
Migration does not move notification history, Do Not Disturb or per-app choices
made in Settings.

## Reloading

| Application | When changes apply |
| --- | --- |
| Shell | Automatically, about 0.5 s after a save settles. `canopa-shell reload` (alias `--reload`) asks for a reload and waits up to 10 s for the result. |
| Notify | Only on request: `canopa-notify ctl reload`. Settings runs it after saving. Changes apply to later notifications; appearance changes repaint open banners. |
| Files | Automatically after a settled save, including theme file edits. |
| Wallpaper | Automatically (the file, its theme and the image files are watched). `canopa-wallpaper reload` re-reads it on request and keeps the choices made with `set` and `slideshow`; `canopa-wallpaper reset` clears them. |
| Lock screen | At the next lock. Each lock reads the file again. |
| Greeter | When the greeter starts. |
| Polkit | When the agent starts: `systemctl --user restart canopa-polkit.service`. |
| Pinentry | Each time a pinentry process starts (normally once per prompt). |
| Settings portal (appearance) | On SIGHUP: `systemctl --user reload xdg-desktop-portal-canopa.service`. |

An invalid change never replaces a working configuration: each application keeps
the last valid one. If the Shell starts with an invalid file, it uses its embedded
defaults and keeps watching the file. While the file is invalid, every bar shows a
red badge and Notify shows a persistent notification with the file, line, column
and message, plus **Open file** and **Open Settings**. Click the badge to open the
file; right-click it to open Settings at the page that edits the broken section.
The next valid save removes both. `canopa-shell --check-config --show-source`
prints the full diagnostic.

The Shell repeats discovery on every reload, so creating the shared file takes
over from a legacy file without a restart. Comment-only edits and edits to other
blocks do not disturb open panels. `canopa-notify ctl reload` without a path also
repeats discovery. Files shows a banner in its windows while the file is invalid.

### Explicit sources

For testing, applications accept an explicit file or the embedded defaults instead
of discovery:

| Program | Options |
| --- | --- |
| `canopa-shell` | `--canopa-config PATH`, `--config PATH` (legacy format), `--defaults` |
| `canopa-notify daemon` | `--canopa-config PATH`, `--config PATH` (legacy format) |
| `canopa-files` | `--config PATH` (alias `--canopa-config`), `--defaults` |
| `canopa-wallpaper daemon`, `canopa-lock`, `canopa-greeter`, `canopa-polkit`, `canopa-pinentry` | `--canopa-config PATH`, `--defaults` |

Each validator accepts the same options together with `--show-source`.

## appearance

Shared by every application. Summary:

| Key | Value | Default |
| --- | --- | --- |
| `color-scheme` | `"dark"` or `"light"` | `"dark"` |
| `accent` | `"#rrggbb"` | the scheme's accent |
| `theme` | path to a theme file, relative to this file | none |
| `palette { ROLE "#rrggbb" }` | color roles | the scheme's colors |
| `typography { family …; body-size; label-size; title-size }` | fonts and sizes | `"Inter" "sans-serif"`, 14, 12, 20 |
| `text-scale` | integer percentage, 100 to 150 | 100 |
| `high-contrast` | `#true` / `#false` | `#false` |
| `reduced-motion` | `#true` / `#false` | `#false` |
| `ui { … }` | spacing, borders, radii and colors per element kind | built-in |

See [theme customization](theme-customization.md) for the full schema, theme files
and examples. The icon theme is not part of this block: Canopa follows
`gtk-icon-theme-name` in GTK's `settings.ini` (`gtk-4.0`, then `gtk-3.0`).

## shell

Canopa Shell's block. It accepts `bar` (repeatable), `modules`, `menus`,
`panel-providers`, `osd`, `windows`, `templates` and `command-policy`. Omitting the
block uses the complete embedded layout
([`crates/canopa-shell/config/default.kdl`](../crates/canopa-shell/config/default.kdl),
installed as `/usr/share/canopa-shell/default.kdl`). An empty `shell {}` gives one
bar with no modules. Copy what you need from the embedded layout into your block.
[The Shell guide](shell.md) describes each feature.

### bar

```kdl
bar "main" {
    output "all"
    edge "left"
    thickness 64
    margin 12
    modules {
        start "launcher" "workspaces"
        center "taskbar"
        end "network" "volume" "clock"
    }
    panel { presentation "adaptive"; start "network"; center "volume"; }
    actions { click { open-panel; }; }
}
```

| Key | Value | Default |
| --- | --- | --- |
| `output` | `"all"`, one or more output names, or names prefixed with `!` to exclude (`niri msg outputs` lists them) | `"all"` |
| `edge` | `"left"`, `"right"`, `"top"`, `"bottom"` | `"left"` |
| `layer` | `"background"`, `"bottom"`, `"top"`, `"overlay"` | `"top"` |
| `thickness` (alias `collapsed-width`) | logical pixels, 2 to 32768, including padding and border | 48 |
| `length` | `"fill"`, pixels up to 32768, or `"0.01%"` to `"100%"` | `"fill"` |
| `align` | `"start"`, `"center"`, `"end"` | `"center"` |
| `margin` | 1, 2 or 4 values | 0 |
| `reserve-space` | reserve the bar's space from windows | `#true` |
| `tooltip-delay` | milliseconds before a tooltip appears | 500 |
| `style` | `color=`, `size=`, `weight=` defaults for every module's text | none |
| `ui { … }` | frame style (see [theme customization](theme-customization.md)) | appearance `bar` |
| `modules { start …; center …; end … }` | module names per zone | none |
| `panel { … }` | the bar's Quick settings panel | see below |
| `actions { … }` | actions on the empty bar background | none |

Bar names must be unique. Several bars can share an output.

`panel` keys:

| Key | Value | Default |
| --- | --- | --- |
| `title` / `title-id` | heading text or message ID | Overview |
| `presentation` | `"auto"`, `"drawer"` (all panels in a side drawer), `"adaptive"` (drawer for browsing, popover for a single module) | `"auto"` |
| `drawer-mode` | `"overlay"` or `"push-bar"` (left and right bars only) | `"overlay"` |
| `width`, `height`, `max-width`, `max-height` | logical pixels | automatic |
| `launcher-width` | width of launcher menus opened from this bar | panel width |
| `start`, `center`, `end` | module names | none |
| `animation { open-ms; close-ms }` | transition durations; `0` disables | 180, 160 |
| `ui { … }` | panel frame style | appearance `panel` |

### Actions

`actions` blocks appear on bars and modules. Each child is a trigger with exactly one
operation:

```kdl
actions {
    click { toggle-menu "Applications"; }
    right-click { launch "canopa-settings" "notifications"; }
    Ctrl+E { launch "canopa-files"; }
}
```

Triggers are `click`, `right-click` or a key chord such as `Super+Space`.

| Operation | Effect |
| --- | --- |
| `launch "program" "arg"…` | Start an application in its own session scope. Arguments are literal. |
| `command "text"` | Run a command through the shell, stopped after the command timeout. |
| `command { argv "program" "arg"…; }` | Run a command without a shell, with the same timeout. |
| `session "suspend"` / `"hibernate"` / `"reboot"` / `"power-off"` | Ask logind directly; polkit may ask for a password. |
| `session "log-out"` | End the session through the compositor's IPC socket, without Niri's own confirmation (put it behind a confirmation menu). |
| `toggle-overview` | Open or close Niri's overview through the compositor's IPC socket. |
| `open-panel`, `toggle-panel` | Open the module's own details. |
| `open-bar-panel`, `toggle-bar-panel` | Open the bar's `panel`. |
| `close-panel` | Close the open panel. |
| `open-menu "NAME"`, `toggle-menu "NAME"` | Open a named menu. |
| `open-provider "NAME"` | Open a panel provider (`wifi`, `audio`, `media`, `bluetooth`, `power`, `notifications`, or a configured one). |

Use `launch` for applications and `command` for short automation.

### command-policy

| Key | Value | Default |
| --- | --- | --- |
| `allow-shell` | `#false` rejects `command "text"` (shell strings); argv commands still work | `#true` |
| `action-timeout-ms` | timeout for `command` actions, at least 1 | 3000 |

### menus

```kdl
menus {
    menu "Applications" show-icons=#true prompt-id="shell-menu-launcher-prompt" {
        source "launcher"
    }
    menu "Tools" title="Tools" {
        item "Terminal" { icon "utilities-terminal"; launch "xdg-terminal-exec"; }
        item "More" { item "Calculator" { launch "gnome-calculator"; }; }
    }
}
```

A menu has properties `show-icons`, `title` or `title-id`, and `prompt` or
`prompt-id`. It contains either one `source` or `item` nodes:

- `source "installed-apps"`: the installed applications.
- `source "launcher" { … }`: the unified launcher. Its options (`root`, `exclude`,
  `max-paths`, `history`, `extension`) are described in the [launcher guide](launcher.md).
- `item "Label"` or `item label-id="…"`, with an optional `icon` (icon theme name),
  `keywords`, one action operation, or nested items.

The menu's name is its identifier for `open-menu` and `canopa-shell --open-menu`.

### modules

Declare a module to change its options. A module whose name is a module type gets
that type; any other name needs a `type` child:

```kdl
modules {
    module "clock" { time-format "%H:%M"; }
    module "updates" {
        type "script"
        command "checkupdates | wc -l"
        interval 1800
    }
}
```

Types: `battery`, `button`, `clock`, `focused-window`, `idle-inhibitor`, `media`,
`minimap`, `network`, `notifications`, `script`, `taskbar`, `tray`, `volume`,
`workspaces`.

Keys every module accepts:

| Key | Value |
| --- | --- |
| `type` | module type, when the name is not one |
| `format` | bar content (vertical bars) |
| `expanded-format` | content in expanded or panel views |
| `horizontal-format` | content on top and bottom bars |
| `content-rotation` | `"none"`, `"auto"` (rotate on left/right bars), `"clockwise"`, `"counter-clockwise"` |
| `tooltip` / `tooltip-id` | tooltip format or message ID; `tooltip #false` disables it |
| `actions { … }` | overrides the module's built-in click behavior |
| `ui { … }` | module style (see [theme customization](theme-customization.md)) |

Formats are one or more string segments, or `segment` children with their own
style:

```kdl
format "{icon}" "{value}%"
format {
    segment "{icon}" color="accent" size=18
    segment "{value}%" {
        style weight="bold"
        style state="muted" color="disabled"
    }
}
```

A segment takes either inline style properties or `style` children (one without
`state`, plus one per state), not both.

Segment and bar styles take `color`, `size` and `weight` (`"light"`, `"normal"`,
`"bold"`). `color` is `#rrggbb`, `#rrggbbaa` or one of the appearance roles
`accent`, `text`, `muted`, `background`, `surface`, `raised`, `border`,
`disabled`, `danger`, `warning` and `success`, which follow the current palette.
The older names `subtext`, `base`, `surface0`, `surface1`, `overlay0` and
`critical` remain accepted for the same roles. A
standalone `"{icon}"` segment uses the icon size (20 by default); other segments use
the text size (12). Every module has `{icon}` and `{value}`.

| Type | Own keys | Placeholders | States |
| --- | --- | --- | --- |
| `battery` | `critical-threshold` 0–100 (15); `states { state "NAME" threshold=N }`; `icons { icon "…" state="…"` or `at=N }` | `{percent}`, `{state}` | `charging`, `critical` (15), `low` (30), `normal` (100) |
| `button` | `label` / `label-id`; `icon` (text, or a `canopa:` embedded icon) | `{label}` | |
| `clock` | `time-format`, `date-format` (see [clock formats](#clock-formats)) | `{time}`, `{date}` | |
| `focused-window` | `fallback-icon`; `max-length` (50; 0 disables) | `{title}`, `{app-id}` | |
| `idle-inhibitor` | `backend "wayland"` (default) or `"stasis"`; `icons` | `{state}`, `{label}` | `active`, `inactive`, `pending`, `unavailable` |
| `media` | | `{label}`, `{title}`, `{artist}`, `{player}`, `{state}` | `playing`, `paused`, `stopped` |
| `minimap` | `size` 8–64 (18); `max-length` (240; 0 is unlimited) | | |
| `network` | `provider "NAME"` or `provider #false`; `icons` | `{label}`, `{state}` | `full`, `limited`, `none` |
| `notifications` | | `{value}` (count, or `?`), `{state}`, `{label}` | `available`, `dnd`, `unavailable` |
| `script` | `command`; `interval` seconds, at least 1 | `{output}` | |
| `taskbar` | `pinned "app.desktop"…` | | |
| `tray` | `menu-search #true` (filter field in long menus) | | |
| `volume` | `step` 1–100 (5); `states`; `icons` | `{percent}`, `{state}` | `muted`, `unmuted` |
| `workspaces` | `workspace N { label "…"; icon "…"; }` | `{index}`, `{name}`, `{label}`, `{state}` | `focused`, `active`, `inactive` |

`states` thresholds are upper bounds: the active state is the one with the smallest
threshold at or above the value. Entries with a built-in name replace it. In `icons`,
`at=N` means "at or above N" and `state=` icons take precedence.

### osd

The on-screen display for device keys.

| Key | Value | Default |
| --- | --- | --- |
| `enabled` | `#false` keeps the keys working without the display | `#true` |
| `volume-step` | percent per press, 1–100 | 5 |
| `brightness-step` | perceptual percent per press, 1–50 | 5 |
| `timeout-ms` | how long the display stays, 500–10000 | 1500 |

A step given on the command line (`canopa-shell volume up 10`) overrides these.

### windows

| Key | Value | Default |
| --- | --- | --- |
| `restore` | reopen the session's applications at the next login | `#false` |

### templates

```kdl
templates {
    template "dev" {
        app "code.desktop" width=66.667
        app "foot.desktop" width=33.333
    }
}
```

`width` is a percentage of the working area (5 to 100). Open a template with
`canopa-shell --open-template dev`.

### panel-providers

```kdl
panel-providers {
    provider "weather" {
        command { argv "my-weather-provider"; }
        protocol "canopa-v5"
    }
}
```

`protocol` is `"niribar-v1"` (the default), `"canopa-v4"` or `"canopa-v5"`. A
provider with a built-in name replaces the built-in one. See
[external panel providers](shell/external-panel-providers.md).

### Clock formats

`time-format` and `date-format` (clock modules) and `clock-format` and
`date-format` (lock screen) take [chrono strftime](https://docs.rs/chrono/latest/chrono/format/strftime/index.html)
patterns. Without them, clocks follow the session language's formats and hour
cycle (`14:30` in French, `2:30 PM` in US English). Settings' 12/24-hour switch
writes explicit patterns. Invalid directives fail validation; write `%%` for a
literal percent. Formats with seconds or fractions update every second; others
update at the minute. The clock's calendar starts weeks on the region's first
weekday.

### Translated labels

The Shell's built-in buttons, menus and panel headings name their texts by message
ID, so they follow the session language (see [translations](translations.md)).
Text you write yourself stays as written. Each field takes one form or the other,
not both:

| Your text | Built-in text | Where |
| --- | --- | --- |
| `label "Lock"` | `label-id "shell-module-lock"` | `button` modules |
| `tooltip "Launcher"` | `tooltip-id "shell-tooltip-launcher"` | modules with a tooltip |
| `title "Quick settings"` | `title-id "shell-panel-quick-settings"` | a bar's `panel` |
| `menu "Log out?" title="…"` | `title-id="shell-menu-log-out-title"` | `menu` properties |
| `prompt="Unsaved work may be lost."` | `prompt-id="shell-menu-unsaved-work"` | `menu` properties |
| `item "Cancel" { … }` | `item label-id="shell-menu-cancel" { … }` | menu items |

IDs are the `shell-` messages in
[`crates/canopa-shell/i18n/en/canopa_shell.ftl`](../crates/canopa-shell/i18n/en/canopa_shell.ftl).
An unknown ID fails validation. Variables in a `tooltip-id` message become the
module's placeholders (`shell-tooltip-workspace` is `Workspace {index}`). IDs are
resolved when the configuration loads, in the language the Shell started with.

## notifications

```kdl
notifications {
    rule "quiet-chat" app="org.example.Chat" category="im.received" outcome="silenced"
    app "org.example.Chat" sound=#false lock-screen="hidden"
    do-not-disturb {
        schedule from="22:00" to="07:00" days="mon tue wed thu fri"
        break-through alarms=#true calls=#true
        suppress fullscreen=#true screen-sharing=#true
    }
    sound enabled=#true theme="freedesktop" default="message-new-instant"
    icons scale=1
    banners { position "top-end"; ui { margin 4 0; }; }
}
```

| Node | Properties | Default |
| --- | --- | --- |
| `rule ["ID"]` (repeatable) | `app=` (required), `outcome=` `"allow"`/`"shown"`, `"silence"`/`"silenced"` or `"drop"`/`"dropped"` (required), `category=`, `urgency=` `"low"`/`"normal"`/`"critical"` | none |
| `app "ID"` (repeatable, up to 1024) | `sound=` bool, `lock-screen=` `"hidden"`, `"name-only"` or `"content"` | sound on, `"name-only"` |
| `do-not-disturb { schedule …; break-through …; suppress …; }` | `schedule from="HH:MM" to="HH:MM" days="mon … sun"`; `break-through alarms= calls=`; `suppress fullscreen= screen-sharing=` | no schedule, no break-through, both suppressions on |
| `sound` | `enabled=`, `theme=`, `default=` | `#true`, `"freedesktop"`, `"message-new-instant"` |
| `icons` | `theme=` (icon theme name), `scale=` 1–8 | the desktop icon theme, 1 |
| `banners { position; ui; }` | `position` `"top-start"`, `"top-end"`, `"bottom-start"`, `"bottom-end"` | `"bottom-start"` |

Rules are checked in order; the first matching rule decides. The rule ID defaults
to `config-rule-N`. Apps are identified by the sender's `desktop-entry` hint, or its
application name. Setting `icons theme=` pins that theme instead of following the
desktop. Choices made in Settings, the Notification Center or `canopa-notify ctl
app` are stored in Notify's database, not in this file. See
[notifications](notifications.md).

## files

```kdl
files {
    view mode="columns" sort="name" column-width=240 folders-first=#true {
        details "size" "modified"
    }
    toolbar "back" "forward" "parent" "location" "new-folder" "view"
    sidebar "auto"
    properties "window"
    thumbnails show="local" size=96
    places { place "recent"; place "home"; bookmark "Projects" path="/home/alex/Projects"; }
    shortcut "search" "<Control>f" "F3"
    action "inspect-images" name="Inspect images" command="my-script %F" {
        selection min=1
        patterns "*.png" "*.jpg"
    }
}
```

| Node | Values | Default |
| --- | --- | --- |
| `view` | `mode` `columns`/`list`/`icons`; `sort` `name`/`size`/`modified`/`kind`; `descending`; `column-width` 160–640; `tree` (List view); `folders-first`; child `details` (`size`, `modified`, `kind`) | `columns`, `name`, `#false`, 240, `#false`, `#true`, `size modified` |
| `toolbar "ID"…` | command IDs, `location` (required, once), `view`, `action:ID`; up to 64 | `back forward parent location new-folder view` |
| `sidebar` | `"auto"`, `"always"`, `"never"` | `"auto"` |
| `properties` | `"window"` or `"panel"` | `"window"` |
| `list-columns` | `name=` 160–1200; `size=`, `modified=`, `kind=` 48–480 | follow the window |
| `panes` | `sidebar=` 160–420, `properties=` 260–640 | built-in |
| `places { place "ID"; bookmark "Name" path="/abs"; }` | places `recent`, `home`, `desktop`, `documents`, `downloads`, `music`, `pictures`, `videos`, `templates`, `public`, `filesystem`, `trash`, `recovery`; up to 256 entries | all except `templates` and `public` |
| `thumbnails` | `show=` `"local"`, `"always"`, `"never"`; `size=` 48–256 pixels | `"local"`, 96 |
| `shortcut "COMMAND" "BINDING"…` (repeatable) | GTK-style accelerators, up to 8; no bindings disables the command | built-in keys |
| `action "ID"` (repeatable) | `name=`, `command=`, `icon=`, `shortcut=`, `toolbar=`; children `selection min= max= files= directories=`, `patterns` | |

Unknown nodes and properties are errors. Custom action commands are split with
shell-like quoting but never run through a shell; `%f`/`%F` are the first or all
selected paths, `%n`/`%N` the basenames, `%d` the current folder and `%%` a percent
sign. See [Files](files.md) for command IDs and default shortcuts.

## wallpaper

| Key | Value | Default |
| --- | --- | --- |
| `image` | image path, relative to this file | none |
| `fit` | `"fill"`, `"fit"`, `"stretch"`, `"center"` | `"fill"` |
| `color` | `"#rrggbb"` solid color | the palette background |
| `output "NAME" { image; fit; }` | per-output override, repeatable | |

## lockscreen

The lock screen and other session interfaces use Iced’s standard wgpu compositor
with tiny-skia software fallback. Existing appearance, font, layout and per-output
keys apply; no renderer setting is required. For diagnostics, `ICED_BACKEND=tiny-skia`
forces software rendering, while `ICED_BACKEND=wgpu,tiny-skia` explicitly requests
the default preference order. A single `ICED_BACKEND=wgpu` requests GPU
initialization first; setup and presentation failures still retry per output with
software fallback so authentication remains available.
Startup diagnostics report the selected backend and adapter.

| Key | Value | Default |
| --- | --- | --- |
| `image`, `fit`, `color` | as in `wallpaper` | none, `"fill"`, palette background |
| `use-wallpaper` | use the desktop wallpaper when no lock image is set | `#true` |
| `dim` | background dimming, 0–100 | 65 |
| `font` | font family | the first `appearance` family |
| `clock-format`, `date-format` | strftime patterns; empty follows the language | empty |
| `clock-size`, `text-size` | 8–160 | 72, 19 |
| `auth-timeout` | seconds, 5–600 | 120 |
| `avatar`, `weather-file` | paths, relative to this file | none |
| `widgets "NAME"…` | `username`, `avatar`, `keyboard`, `battery`, `media`, `weather`, `notifications`, or custom names | `battery media keyboard username notifications` |
| `layout { margin; gap; REGION "widget"… flow= x= y=; }` | regions `top-left` … `bottom-right` | see [session tools](session-tools.md) |
| `widget "NAME" { type "script"; command …; … }` | up to 32 custom script widgets | |
| `output "NAME" { image; fit; hide-widgets …; }` | per-output override | |

## greeter

Read from `/etc/canopa/greeter.kdl` only. Keys include `discover-users`,
`user-source` (`"auto"`, `"accountsservice"`, `"nss"`), `users`, `default-user`,
`remember-selection`, `user-avatars`, `avatar-dir`, `user-avatar`, `power-controls`,
`discover-sessions`, `session-dirs`, `xsession-dirs`, `x11-wrapper`, `session` and
`view` (the lock screen presentation keys). See [the login screen guide](greeter.md).

## polkit and pinentry

Both blocks accept only:

| Key | Value | Default |
| --- | --- | --- |
| `font` | font family | the first `appearance` family |
| `auth-timeout` | seconds, 5–600 | 120 |

See [polkit](polkit.md) and [pinentry](pinentry.md).

## Other files

Canopa keeps some settings outside `config.kdl`. Settings is the usual editor.

| File | Contents |
| --- | --- |
| `~/.config/canopa/niri.kdl`, `niri-input.kdl`, `niri-displays.kdl`, `niri-custom.kdl`, `niri-shortcuts.kdl`, `shortcuts.kdl` | Compositor configuration (see [installation](installation.md#first-login)) |
| `~/.config/canopa/display-profiles.kdl` | Saved display layouts |
| `~/.config/canopa/power.kdl` | Lid-close action |
| `~/.config/canopa/stasis.rune` | Idle policy ([session tools](session-tools.md)) |
| `~/.config/canopa/launcher.json` | Launcher extensions, keywords, search shortcuts, pins and aliases ([launcher](launcher.md)) |
| `~/.local/state/canopa/shell/clipboard.json` | Clipboard history options and snippets |

### Compositor titlebars

Canopa's Niri fork enables titlebars for windows that negotiate server-side
decorations. They contain the window title and close/maximize controls; drag the
titlebar to move a window. Controls also accept touch input. Application-provided
headerbars remain intact, and fullscreen windows have no titlebar.

Override the compositor defaults in `~/.config/canopa/niri-custom.kdl`:

```kdl
layout {
    titlebar {
        on
        height 32
        font "Inter 10"
        active-color "#202d31"
        inactive-color "#10191d"
        text-color "#edf4f0"
    }
}
```

Use `off` to disable titlebars. Height accepts 24–64 logical pixels; the font is
a Pango font description. These settings reload with Niri's configuration.
`prefer-no-csd` must also be enabled to expose decoration negotiation to newly
started applications; restart applications after changing that preference.

This block requires the Canopa compositor fork. Titlebar styling currently comes
from compositor configuration and does not follow live `appearance` changes.
Shell-defined titlebar menus and extra actions are not implemented yet.

## Known limitations

- The Shell's error hint for an unknown module name lists only some module types;
  the full list is above.
- Notify does not watch the file; run `canopa-notify ctl reload` after hand edits.
- The Settings portal and the Polkit agent need an explicit reload or restart.
