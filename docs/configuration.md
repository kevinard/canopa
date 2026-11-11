# Canopa configuration

The shell's [unified launcher](shell/launcher.md) supports application search,
file/folder names, open windows, desktop controls, arithmetic, unit conversions,
and executable extensions through a `launcher`
menu source. Its guide describes roots, exclusions, refresh behavior, and the
extension protocol.

Canopa Shell, Notify, Files, Wallpaper, Lock, Greeter, Polkit and Pinentry discover `~/.config/canopa/config.kdl`, or
`$XDG_CONFIG_HOME/canopa/config.kdl` when that variable is an absolute path.
An empty or relative XDG value falls back to `$HOME/.config`.

```kdl
schema-version 1
appearance {
    color-scheme "dark"
    // accent "#b0dec3"
}
shell {
    bar "main" { edge "top"; }
    // Existing Canopa Shell modules, menus, command-policy and panel-providers go here.
}
notifications {
    sound enabled=#true theme="freedesktop" default="message-new-instant"
    rule "quiet-chat" app="org.example.Chat" outcome="silenced"
}
files {
    action "inspect-images" name="Inspect images" command="my-script %F" {
        patterns "*.png" "*.jpg"
    }
}
```

See [the shared example](../config/canopa.kdl), [shell options](../crates/canopa-shell/README.md),
and [notification options](../crates/canopa-notify/README.md), plus
[Files interface preferences and custom actions](../crates/canopa-files/README.md). Unknown root nodes and duplicate
namespaces are errors. Wallpaper and lockscreen settings live in optional `wallpaper`
and `lockscreen` blocks; see [session tools](session-tools.md).

`appearance` is shared by all applications, including when their own namespace
is omitted. `color-scheme` accepts `"dark"` (default, Canopa's warm charcoal and mint)
or `"light"` (warm white and forest green). Optional `accent` accepts an opaque `"#rrggbb"`
color; omit it to use the selected preset's accent. Each setting may appear once.
Colors, font families/sizes, borders, corner radii and spacing can be configured in
this block or in a reusable theme file. See [theme and UI customization](theme-customization.md)
for the complete schema, local overrides and bar placement. The icon theme
follows GTK's `gtk-icon-theme-name` ([ADR 0138](adr/0138-shared-icon-theme-and-mime-lookup.md));
motion settings retain their existing owners. Existing shell
style tokens resolve against this palette; explicit color overrides still win.
Presets have contrast checks; custom colors can reduce contrast.

A missing namespace uses that application's embedded defaults. An empty `shell {}`
uses the shell schema's empty-document defaults (one bar and no configured modules);
omit the namespace to get Canopa Shell's complete embedded configuration. Settings are
never merged with legacy files. Syntax/envelope/appearance errors affect all consumers;
invalid options inside a namespace affect only its owner.

If the common file is absent, Canopa Shell reads `niribar/config.kdl` and Notify reads
`canopa-notify/config.kdl` under the same config directory. If both are absent, that
application uses embedded defaults. Files, Wallpaper, Lock, Greeter, Polkit and Pinentry use embedded defaults directly and have no legacy source. A present invalid common file is an error,
including a broken symlink. Files must be readable regular UTF-8 files at most
1 MiB; symlinks to regular files work.

Build/install `canopa`, `canopa-shell`, `canopa-notify`, `canopa-files`, `canopa-wallpaper`, `canopa-lock`, `canopa-greeter`, `canopa-polkit` and
`canopa-pinentry` in the same binary directory.
Validation opens no desktop surfaces, control socket, or history database:

```sh
canopa config check
canopa config check --file /path/to/canopa.kdl
canopa config check --file /path/to/canopa.kdl --show-source
```

The checker reports one JSON line per component and exits nonzero if any fails.
It validates the same immutable common-file snapshot in each owning executable. Normal
reports contain safe diagnostic codes and available byte offsets. `--show-source`
adds local diagnostic detail to stderr and may reveal source values. Diagnostics
refer to the temporary snapshot; the report identifies the original path.

Migrate explicit legacy files to a new output (its parent directory must exist):

```sh
canopa config migrate --shell ~/.config/niribar/config.kdl \
  --notifications ~/.config/canopa-notify/config.kdl \
  --output /path/to/reviewed-canopa.kdl
canopa config check --file /path/to/reviewed-canopa.kdl
```

Either legacy input may be omitted; that namespace then uses embedded defaults.
The command preserves source comments, validates all owning schemas, and creates a mode
0600 file without replacing an existing output. After reviewing, place the file at
`~/.config/canopa/config.kdl` (or your XDG equivalent). Originals are left intact.

Canopa Shell reloads automatically within roughly half a second of a settled save. To request a reload and wait for its result:

```sh
canopa-shell reload
```

`--reload` is an alias. The command uses the running shell's configuration selection;
it does not accept a replacement path. Exit status 0 means the candidate was
installed (or already active); invalid input or unavailable IPC returns 1.

All shell settings and shared appearance can change while running. Content edits
retain existing bar windows; geometry, layer, target and ordering changes reconcile
the affected surfaces. Successful effective changes dismiss panels, menus and
tooltips. Unchanged configurations, including comment-only edits and changes solely
to Notify's namespace, preserve foreground content. Unchanged modules keep their
readings and subscriptions. Changed scripts are cancelled and restarted; their old
results cannot update the replacement. The manual keep-awake preference survives
reload while an idle-inhibitor module remains configured.

Invalid input retains the previous complete configuration and foreground content.
If the configuration is already invalid when Canopa Shell starts, the Shell starts
with its embedded defaults instead of exiting, so the bar and launcher stay
available, and it keeps watching the selected file. In both cases every bar shows
a red warning badge at the top of its end zone, and a persistent Notify
notification names the file, line and column and the parser's message, with
**Open file** and **Open Settings** buttons. Clicking the badge opens the file
(right-click opens Settings). Settings opens at **Widgets** for a problem in the
`shell` section and at **Personalization** for syntax, schema-version and
appearance problems; an open Settings window switches to that page. The first valid save applies the configuration and
removes both. The notification waits for Notify if it is not running yet. Only safe
error codes go to the journal and the explicit command; use
`canopa-shell --check-config --show-source` (with the same source override, if any)
for local diagnostic details. Discovery is repeated, so creating the common file
supersedes legacy input, and removing it reselects legacy input or defaults. A
missing explicit file is an error; `--defaults` stays on embedded defaults.
Atomic replacement and symlink targets are supported. The shell polls source
metadata every 250 ms off the UI thread, debounces changes, and prepares one
bounded candidate at a time.

Canopa Files also reloads automatically after a settled save, including edits to
an external theme. Files settings, custom actions and appearance are validated
together; invalid input retains the complete previous candidate and shows a safe
browser banner until a valid repair. Atomic file replacement, directory replacement
and symlink targets are supported. Discovery remains discovery after graphical
saves; missing explicit files retain the last valid state, and `--defaults` stays
isolated from ambient files.

Comment-only and unrelated-namespace changes preserve browser input and views.
Appearance changes repaint existing windows and preserve Quick Look's current
page, zoom and playback objects. Open Preferences and Custom Actions drafts keep
their fields and show that configuration changed. Stale main-file saves are
refused; Ctrl+R explicitly reloads either editor, asking before discarding a draft.
See [Files live reload](files/adr/0019-live-configuration-reload.md).

Notify reloads with:

```sh
canopa-notify ctl --socket /path/to/control.sock reload
canopa-notify ctl --socket /path/to/control.sock reload-canopa /path/to/canopa.kdl
canopa-notify ctl --socket /path/to/control.sock status --json
```

No-path reload repeats the last successfully installed selection. For a daemon
started with discovery, it discovers again, allowing migration to the common file
without a daemon restart. Explicit file selections stay explicit. Failed reloads
retain the last good source, rules, settings and generation. Source fingerprints
identify the complete source bytes and can be compared across consumers; generations
are local to each process. Canopa Shell logs its installed source and local
generation on reload;
`canopa config check` reports candidates, not the running shell's state.

Appearance changes repaint Notify's existing banners after a successful reload,
without resetting notification timeouts. Canopa Shell applies the same changes
to bars and the Notification Center on reload. These are independent installations;
reload success does not promise that a disconnected renderer has repainted. Rendering
failures remain visible in Notify's presentation status.

For explicit startup overrides, both applications accept `--canopa-config PATH`
for the shared format or `--config PATH` for legacy syntax. Notify places these
options after `daemon`. Its positional `ctl ... reload PATH` remains legacy.
Owner-only validators are `canopa-shell --check-config` and
`canopa-notify check-config`; both support `--defaults`, `--config PATH`,
`--canopa-config PATH`, and local `--show-source`.

Configuration migration does not move or change history, DND, UI-managed permission
rules, or other runtime state. See [the architecture decision](adr/0002-shared-configuration.md)
for ownership and reload guarantees.

## Device keys and on-screen display

Canopa's Niri defaults bind `XF86AudioRaiseVolume`, `XF86AudioLowerVolume`,
`XF86AudioMute`, `XF86AudioMicMute`, `XF86MonBrightnessUp`/`Down`,
`XF86KbdBrightnessUp`/`Down`, `XF86KbdLightOnOff` and `XF86AudioPlay`/`Pause`/
`Stop`/`Next`/`Prev` to `canopa-shell volume up` and similar commands. Volume,
brightness and media keys also work while the screen is locked. Rebind them on
the Settings **Shortcuts** page or in `niri-custom.kdl`; the command list is in
the [shell options](../crates/canopa-shell/README.md#device-keys-and-on-screen-display).
Saved shortcut changes from the release that used `wpctl` are carried over at
the next login.

The optional `osd` block inside `shell` sets the key steps and the display:

```kdl
shell {
    osd {
        enabled #true        // #false: keys work, nothing is shown
        volume-step 5        // percent per press, 1-100
        brightness-step 5    // perceptual percent per press, 1-50
        timeout-ms 1500      // how long the display stays, 500-10000
    }
}
```

Each option may appear once, and the block reloads live. A key's own step
(`canopa-shell volume up 10`) overrides the configured one. The display honors
the shared appearance's colors and reduced motion. See
[Shell ADR 0025](shell/adr/0025-device-keys-and-on-screen-display.md).

## Clock formats

Clock modules accept Chrono output directives in `time-format` and
`date-format`. Without them, the clock follows the session language's formats
(`LC_TIME`): its time format and hour cycle (`14:30` in French, `2:30 PM` in US
English) and its weekday, month and day (`mercredi 11 novembre`, `Wednesday,
November 11`). An explicit pattern is the person's own and is used as written;
Settings' 12/24-hour switch writes one
([ADR 0142](adr/0142-clock-hour-cycle-in-format-strings.md)). The shipped
defaults set neither. Invalid directives, dangling `%`, and parse-only
directives fail configuration validation with a source location. Use `%%` for
a literal percent. Formats showing seconds or fractions update each second;
other formats, and the language's formats, update at minute boundaries.

The calendar in the clock's panel starts weeks on the region's first weekday
(Sunday in the US, Monday in France) and names days and months in the
session language.

## Translated labels

The Shell's built-in buttons, menus and panel headings name their texts by
message ID so they follow the session language
([translations](translations.md)); text you write stays literal. Each field
takes either form, not both:

| Literal (your text) | Message ID (built-in text) | Where |
| --- | --- | --- |
| `label "Lock"` | `label-id "shell-module-lock"` | `button` modules |
| `tooltip "Launcher"` | `tooltip-id "shell-tooltip-launcher"` | every module with `tooltip` |
| `title "Quick settings"` | `title-id "shell-panel-quick-settings"` | a bar's `panel` |
| `menu "Log out?" title="…"` | `title-id="shell-menu-log-out-title"` | `menu` properties; the name stays the identifier for `open-menu` |
| `prompt="Unsaved work may be lost."` | `prompt-id="shell-menu-unsaved-work"` | `menu` properties |
| `item "Cancel" { … }` | `item label-id="shell-menu-cancel" { … }` | menu items |

IDs are the `shell-` messages of
`crates/canopa-shell/i18n/en/canopa_shell.ftl`; an unknown ID fails validation.
A `tooltip-id` message's `{ $name }` variables become the module's `{name}`
placeholders (`shell-tooltip-workspace` is `Workspace {index}`). IDs resolve
when the configuration loads, in the language the Shell started with.

## Idle inhibitor

Add `"idle-inhibitor"` to a shell bar's `modules` list to show a manual keep-awake
toggle. It starts off on every shell launch. Click it, or activate it with the
keyboard, to switch it on or off. All instances share the same toggle.

For example, inside the existing `shell` block:

```kdl
bar "main" {
    edge "top"
    modules { end "idle-inhibitor" "clock"; }
}
modules {
    module "idle-inhibitor" {
        horizontal-format "{icon}" "{value}"
        tooltip "{label}"
    }
}
```

The optional declaration supports `backend` (`wayland` or `stasis`), `format`, `expanded-format`,
`horizontal-format`, `content-rotation`, `tooltip`, `icons`, and the usual
`actions` overrides. Format placeholders are `{icon}`, `{value}` (`On`/`Off`),
`{state}`, and `{label}` (status and help). Icon and segment style states are
`active`, `inactive`, `pending`, and `unavailable`.

The module uses the compositor's Wayland idle-inhibit protocol, with inhibitors
attached to live bar surfaces. Output changes preserve the selected toggle;
switching off, removing the module, or exiting the shell releases its inhibitors.
An unsupported compositor displays `unavailable`. There is no polling process.
Set `backend "stasis"` to control the optional Stasis daemon's manual hold and
consume its streaming state instead. See [idle policy setup](idle.md).

Inhibition applies while a bar surface is visible and depends on the compositor
and idle daemon honoring the protocol. It does not block explicit lock, suspend,
or shutdown commands. See the [Wayland protocol contract](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/unstable/idle-inhibit/idle-inhibit-unstable-v1.xml).

## Settings portal

The optional Settings backend consumes only the common file's shared appearance;
it does not consult application legacy files. Send SIGHUP (or use its systemd unit's
reload command) after editing the file. Invalid candidates retain the previous
values; unchanged effective values emit no portal signal. Discovery repeats on
reload, while an explicit path remains selected. Canopa Shell reloads automatically; the portal still requires its own reload.
See [Settings portal installation and selection](settings-portal.md).

The `greeter` schema is validated by `canopa-greeter`; see [login screen setup](greeter.md).
Its deployed configuration lives in `/etc/canopa/greeter.kdl` and is selected
independently of the authenticated user's shared configuration.

The `polkit` namespace configures the session authorization agent (font and
authentication deadline). Its appearance uses the shared theme; see [agent settings](polkit.md).

Canopa Pinentry provides on-demand GnuPG PIN/passphrase and confirmation dialogs,
using shared session presentation. See [pinentry setup](pinentry.md). The optional `pinentry` configuration namespace owns
`font` and `auth-timeout`; `canopa config check` validates it with the other owners.
