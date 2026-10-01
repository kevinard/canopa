# Canopa configuration

Both applications discover `~/.config/canopa/config.kdl`, or
`$XDG_CONFIG_HOME/canopa/config.kdl` when that variable is an absolute path.
An empty or relative XDG value falls back to `$HOME/.config`.

```kdl
schema-version 1
appearance {
    color-scheme "dark"
    // accent "#89b4fa"
}
shell {
    bar "main" { edge "top"; }
    // Existing Canopa Shell modules, menus, command-policy and panel-providers go here.
}
notifications {
    sound enabled=#true theme="freedesktop" default="message-new-instant"
    rule "quiet-chat" app="org.example.Chat" outcome="silenced"
}
```

See [the shared example](../config/canopa.kdl), [shell options](../crates/canopa-shell/README.md),
and [notification options](../crates/canopa-notify/README.md). Unknown root nodes and duplicate
namespaces are errors.

`appearance` is shared by both applications, including when `shell` or `notifications`
is omitted. `color-scheme` accepts `"dark"` (default, Catppuccin Mocha) or `"light"`
(a contrast-adjusted Latte preset). Optional `accent` accepts an opaque `"#rrggbb"`
color; omit it to use the selected preset's accent. Each setting may appear once.
Font, icon-theme and motion settings are not part of this block yet. Existing shell
style tokens resolve against this palette; explicit color overrides still win.
Presets have contrast checks; custom colors can reduce contrast.

A missing namespace uses that application's embedded defaults. An empty `shell {}`
uses the shell schema's empty-document defaults (one bar and no configured modules);
omit the namespace to get Canopa Shell's complete embedded configuration. Settings are
never merged with legacy files. Syntax/envelope/appearance errors affect both consumers;
invalid options inside a namespace affect only its owner.

If the common file is absent, Canopa Shell reads `niribar/config.kdl` and Notify reads
`canopa-notify/config.kdl` under the same config directory. If both are absent, that
application uses embedded defaults. A present invalid common file is an error,
including a broken symlink. Files must be readable regular UTF-8 files at most
1 MiB; symlinks to regular files work.

Build/install `canopa`, `canopa-shell`, and `canopa-notify` in the same binary directory.
Validation opens no desktop surfaces, control socket, or history database:

```sh
canopa config check
canopa config check --file /path/to/canopa.kdl
canopa config check --file /path/to/canopa.kdl --show-source
```

The checker reports one JSON line per component and exits nonzero if either fails.
It validates the same immutable common-file snapshot in both executables. Normal
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
The command preserves source comments, validates both schemas, and creates a mode
0600 file without replacing an existing output. After reviewing, place the file at
`~/.config/canopa/config.kdl` (or your XDG equivalent). Originals are left intact.

Shell settings require restarting Canopa Shell. Notify reloads with:

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
are local to each Notify process. Canopa Shell reports its installed source at startup;
`canopa config check` reports candidates, not the running shell's state.

Appearance changes repaint Notify's existing banners after a successful reload,
without resetting notification timeouts. Restart Canopa Shell to apply the same changes
to bars and the Notification Center. These are independent installations; reload
success does not promise that a disconnected renderer has repainted. Rendering
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

The optional declaration supports `format`, `expanded-format`,
`horizontal-format`, `content-rotation`, `tooltip`, `icons`, and the usual
`actions` overrides. Format placeholders are `{icon}`, `{value}` (`On`/`Off`),
`{state}`, and `{label}` (status and help). Icon and segment style states are
`active`, `inactive`, `pending`, and `unavailable`.

The module uses the compositor's Wayland idle-inhibit protocol, with inhibitors
attached to live bar surfaces. Output changes preserve the selected toggle;
switching off, removing the module, or exiting the shell releases its inhibitors.
An unsupported compositor displays `unavailable`. There is no polling process.

Inhibition applies while a bar surface is visible and depends on the compositor
and idle daemon honoring the protocol. It does not block explicit lock, suspend,
or shutdown commands. See the [Wayland protocol contract](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/unstable/idle-inhibit/idle-inhibit-unstable-v1.xml).

## Settings portal

The optional Settings backend consumes only the common file's shared appearance;
it does not consult application legacy files. Send SIGHUP (or use its systemd unit's
reload command) after editing the file. Invalid candidates retain the previous
values; unchanged effective values emit no portal signal. Discovery repeats on
reload, while an explicit path remains selected. Canopa Shell still requires a restart.
See [Settings portal installation and selection](settings-portal.md).
