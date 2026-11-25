# Canopa Shell architecture

Canopa Shell (`crates/canopa-shell`) is the desktop shell for Niri: bars, Quick
settings, panels and menus, the launcher, the taskbar, the on-screen display, the
system tray and the Wallpaper portal's background surfaces. It is written in Rust
with Iced and `iced_layershell`. This document describes how it is built; the user
guide is [shell.md](../shell.md), the launcher guide is [launcher.md](../launcher.md)
and the configuration reference is [configuration.md](../configuration.md).

Domain vocabulary (Bar, Bar Surface, Module, Foreground, Panel Menu, External Panel
Provider, ...) is defined in [docs/shell/CONTEXT.md](../shell/CONTEXT.md). Shell
decision records live in [docs/shell/adr/](../shell/adr/). Earlier shared decision
records were consolidated into this document; the originals are in the git history
at commit cb300936 under docs/adr/.

## Process model

| Binary | Source | Role |
| --- | --- | --- |
| `canopa-shell` | `src/main.rs` | The shell daemon, its CLI clients and several internal helper modes |
| `canopa-wifi-provider` | `src/bin/canopa-wifi-provider.rs` | Wi-Fi panel provider (NetworkManager or iwd) |
| `canopa-audio-provider` | `src/bin/canopa-audio-provider.rs` | Sound panel provider (libpulse; PipeWire-pulse works) |
| `canopa-media-provider` | `src/bin/canopa-media-provider.rs` | Media panel provider (MPRIS through the Shell's selection) |

The `canopa-shell` binary needs the `desktop` feature (on by default). Without it
the library still builds the providers, device-key actions and protocol code; the
`--core` check builds that way.

One `canopa-shell` process runs per session, started by
`packaging/systemd/canopa-shell.service` (part of `canopa-session.target`, restart
on failure). The same executable is also:

- an IPC client: `reload`, `--open-menu`, `--toggle-menu`, `--open-template`,
  `--dmenu`, `--toggle-panel` and the device-key commands (`volume`, `microphone`,
  `brightness`, `keyboard-backlight`, `media`) connect to the running instance;
- `canopa-shell lock`, which execs `canopa-lock` (or `loginctl lock-session`), or
  with `--swaylock` Swaylock with the Shell's lock image;
- `--check-config [--show-source]`, a validator that prints one JSON line;
- internal helpers that other processes spawn: `--hardware-panel bluetooth|power`
  (built-in providers), `--launcher-extension web|date-time ...` (bundled launcher
  extensions), `--clipboard-frame` and `--clipboard-image-frame` (`wl-paste`
  callbacks) and `--app-scope ID -- PROGRAM...` (launch helper, see
  [Application launches](#application-launches)).

The CLI is hand-written in `src/cli.rs`, not clap. The instance owns
`$XDG_RUNTIME_DIR/canopa-shell.sock` (owner-only, guarded by a lock file; a
second instance cannot take it over and runs without IPC, logging that the
listener is unavailable). Requests are framed JSON (`src/ipc/protocol.rs`:
`Reload`, `DmenuSelection`, `OpenMenu`, `ToggleMenu`, `OpenTemplate`, `DeviceKey`);
the line command `toggle-panel` is also accepted. At most 64 clients are served at
once, each with two-second read and write deadlines.

Built-in providers are separate processes started on demand. The Shell resolves a
companion binary (`canopa-wifi-provider`, `canopa-audio-provider`,
`canopa-media-provider`, `canopa-notify`, or itself) next to its own executable
first, then on `PATH` (`external_panel.rs` `companion_binary`).

### Source map

| Path | Contents |
| --- | --- |
| `src/app.rs`, `src/app/` | The Iced daemon: update loop, surfaces, Foreground, panels, launcher UI, taskbar, OSD, wallpaper surfaces, accessibility |
| `src/config/`, `config/default.kdl` | `shell` namespace parser and the embedded default configuration |
| `src/modules/` | Module kinds, data sources and bar renderers |
| `src/surface/` | Bar geometry, placement, panel and popup views |
| `src/external_panel.rs`, `src/external_panel/` | Provider host: process lifetime and the v1/v4/v5 adapters |
| `src/niri.rs`, `src/niri/` | Niri IPC event stream and state |
| `src/launcher.rs`, `src/launcher/` | Launcher sources: applications, files, windows, controls, calculator, characters, clipboard, history, extensions |
| `src/device_keys.rs`, `src/osd.rs` | Device-key actions (library) and OSD model |
| `src/session.rs` | Session actions through logind |
| `src/notice.rs` | The Shell's own notifications (configuration errors, failed session actions) |
| `src/wallpaper/` | Wallpaper store, D-Bus owner and lock handoff |
| `src/dbus/` | StatusNotifierWatcher and host, DBusMenu, UPower and NetworkManager status |
| `src/media.rs`, `src/media_provider.rs` | MPRIS selection and the `org.canopa.Shell.Media` interface |
| `src/wifi_provider.rs`, `src/audio_provider.rs`, `src/hardware_panel/` | Provider implementations |
| `src/command/` | Bounded commands, application launches and the scope helper |

Shared crates it uses: `canopa-config` (configuration discovery and the
`appearance` block), `canopa-theme` and `canopa-ui` (theme, panel chrome),
`canopa-services` (D-Bus and Pulse clients), `canopa-panel-protocol`,
`canopa-launcher`, `canopa-notify-client`, `canopa-widgets`, `canopa-assets` and
`canopa-apps`. See [foundations.md](foundations.md).

## Configuration and live reload

The Shell reads the `shell` namespace of `$XDG_CONFIG_HOME/canopa/config.kdl`
(through `canopa-config`; the legacy `$XDG_CONFIG_HOME/niribar/config.kdl` is used
only when the shared file is absent). `--canopa-config PATH`, `--config PATH`
(legacy flat format) and `--defaults` select a source explicitly. The embedded
defaults are `config/default.kdl`.

Reload (`src/app/configuration.rs`, [Shell ADR 0021](../shell/adr/0021-live-configuration-reload.md)):

- A runtime polls the selected source's metadata every 250 ms off the UI thread
  and accepts a change after two equal observations. `canopa-shell reload` (or
  `--reload`) requests the same path and waits up to 10 s for the result.
- One candidate is prepared at a time on a blocking worker; newer requests
  coalesce. Read, parse or validation failure keeps the last good configuration.
- Installation builds the new Module Runtime, then commits in one update.
  Unchanged modules keep their readings and subscriptions; changed bars replace
  only the affected windows; open panels and pending provider sessions close.
  The shared `appearance` block is reinstalled as a new theme snapshot.
- An invalid configuration at startup starts the Shell on the embedded defaults
  and keeps watching the file. While the selection is rejected, every bar shows a
  warning badge and the Shell keeps one notification naming the file, line and
  parser message, with **Open file** and **Open Settings** buttons.

## Surfaces

All surfaces are layer-shell surfaces owned by the one Iced daemon:

| Surface | Layer | Namespace | Input |
| --- | --- | --- | --- |
| Bar Surface (one per Bar per eligible output) | configured `layer` (default `top`) | `canopa-shell` | Pointer and keyboard; reserves space unless `reserve-space #false` |
| Panel, Panel Menu, drawer | `overlay` | `canopa-shell` | Keyboard on demand |
| Dismissal surface (one per output while a panel is open) | `top` | `canopa-shell-dismissal` | Pointer only; transparent; consumes the first outside click |
| Tooltip and menu popups | popups of the bar surface | — | Menus use the popup grab |
| On-screen display | `overlay` | `canopa-shell-osd` | None (empty input region) |
| Wallpaper (one per output while a background is selected) | `background` | `canopa-shell-wallpaper` | None |

Bar Surfaces open only after Wayland reports named outputs. A Bar is a named
configuration and a Bar Surface is its instance on one output; output-specific
behaviour is another named Bar with an `output` target
([Shell ADR 0001](../shell/adr/0001-named-bars-for-output-specific-behavior.md)).
Wayland, not Niri, decides which outputs exist
([Shell ADR 0002](../shell/adr/0002-niri-as-first-compositor-integration.md)).

## Modules and bar layout

A bar has `start`, `center` and `end` zones of module names. Module kinds
(`src/modules.rs`): `battery`, `button`, `clock`, `focused-window`,
`idle-inhibitor`, `media`, `minimap`, `network`, `notifications`, `script`,
`taskbar`, `tray`, `volume` and `workspaces`. A module named after a kind infers
that kind.

- **Module Data Sources** own one subscription per source (Niri, UPower, Pulse,
  MPRIS, Notify summary, Stasis, ...) shared by every instance of that kind.
- **Dynamic modules** (workspace buttons, tray items) are children with a
  structural identity separate from configured names
  ([Shell ADR 0019](../shell/adr/0019-structural-module-identities.md)).
- Ordinary modules render through one shared presentation path that handles edge
  orientation, templates and content rotation
  ([Shell ADR 0020](../shell/adr/0020-shared-module-presentation.md)).
- Command-backed modules (`script`, `button`, `command` actions) run explicit
  user commands under `command-policy`: shell form is allowed by default and every
  command has a timeout (default 3000 ms)
  ([Shell ADR 0003](../shell/adr/0003-allow-explicit-shell-commands-by-default.md)).

Built-in default actions (`src/app/module_graph.rs`): `volume` opens the `audio`
provider, `media` opens `media`, `network` opens its `provider` (default `wifi`),
and `notifications` opens the Center on click and runs `canopa-notify ctl dnd
toggle` on right-click. Configured actions always win.

## Foreground: panels, menus and drawers

The active interactive overlay is the **Foreground**: nothing, a Menu, or a Panel.
It owns the window identity together with interaction state, so stale closes and
replacements cannot affect newer content
([Shell ADR 0008](../shell/adr/0008-model-interactive-overlays-as-foreground.md);
ADR 0007 is superseded).

- Panels present as popovers or edge drawers (`presentation auto|drawer|adaptive`,
  `drawer-mode overlay|push-bar`). Shared chrome and metrics come from
  `canopa-ui::panel`; open and close use a short slide and fade (`animation
  { open-ms 180; close-ms 160 }`), rendered from a captured image of the panel
  ([Shell ADR 0014](../shell/adr/0014-share-panel-chrome-and-bound-transition-lifetime.md)).
- The first click outside an overlay dismisses it without reaching the window
  underneath ([Shell ADR 0015](../shell/adr/0015-dismiss-foreground-on-outside-click.md)).
- Panel Menus (`menus { menu "NAME" { ... } }`) hold items or a `source`
  (`installed-apps` or `launcher`). Icons are prepared off the UI thread with a
  bounded cache ([Shell ADR 0013](../shell/adr/0013-prepare-panel-menu-icons-off-the-ui-thread.md)).
- `canopa-shell --dmenu` sends entries to the running instance, which shows them
  in a Panel Menu and returns the selection over IPC
  ([Shell ADR 0004](../shell/adr/0004-broker-dmenu-selection-through-running-instance.md),
  [ADR 0005](../shell/adr/0005-rich-dmenu-input-uses-json-lines.md)).
- Tray menus (DBusMenu) and taskbar application menus use the same menu popup.

## Panel providers

Rich panels are out-of-process **External Panel Providers** that send declarative
content; the Shell owns rendering, focus, sizing and input
([Shell ADR 0010](../shell/adr/0010-external-panel-providers-use-declarative-content.md)).
A provider is started when its panel opens and closed when it closes.

| Name | Command the Shell runs | Backend |
| --- | --- | --- |
| `wifi` | `canopa-wifi-provider --protocol canopa-v5` | NetworkManager, else iwd (`canopa-services`) |
| `audio` | `canopa-audio-provider --protocol canopa-v5` | libpulse |
| `media` | `canopa-media-provider --protocol canopa-v5` | `org.canopa.Shell.Media` |
| `bluetooth` | `canopa-shell --hardware-panel bluetooth` | BlueZ |
| `power` | `canopa-shell --hardware-panel power` | logind backlight, UPower, power-profiles-daemon |
| `notifications` | `canopa-notify panel-provider --protocol canopa-v5` | Notify control socket |

Built-in names use `canopa-v5`. A `panel-providers { provider "NAME" { ... } }`
declaration overrides a built-in with the same name; a configured provider's
protocol defaults to `niribar-v1`, and `canopa-v4` and `canopa-v5` are selected
explicitly. The host never guesses a protocol
([Shell ADR 0011](../shell/adr/0011-explicit-canopa-panel-protocol-adapter.md)).
On close the host sends a close message and waits a bounded grace before killing
the process group: 250 ms in general, 3 s for `bluetooth`, 35 s for `wifi` (so a
pending NetworkManager profile call can be rolled back).

The provider-author reference is
[external-panel-providers.md](../shell/external-panel-providers.md); the wire
contracts are described in [panel-protocol.md](panel-protocol.md). The Wi-Fi
backend bundle rule (never NetworkManager status with iwd control) is
[Shell ADR 0009](../shell/adr/0009-bundle-wifi-control-with-network-status-integration.md);
the audio and media providers are
[Shell ADR 0016](../shell/adr/0016-audio-device-panel-provider.md) and
[ADR 0018](../shell/adr/0018-mpris-media-controls.md).

Quick settings also has in-process controls (`src/app/quick_toggles.rs`): Wi-Fi
and Bluetooth radio switches (NetworkManager, BlueZ), the Sound card and the
brightness slider.

## Niri integration

The Shell talks to Niri through `niri-ipc`, pinned to the `kevinard/niri` fork in
the root `Cargo.toml`. `src/niri/subscription.rs` keeps one event-stream
connection and maintains `NiriState`: workspaces, windows, the focused window,
overview visibility, output sizes and window layout positions. Consumers:

- `workspaces`, `focused-window`, `minimap` and `taskbar` modules;
- the launcher's open-windows source and the overview action (`niri msg action
  toggle-overview`);
- application launches through `Action::Spawn`, and focus through
  `Action::FocusWindow`.

Features that need the fork (stock Niri lacks them; the Shell degrades without
them):

- `Event::OverviewTyped`: text typed in the overview opens the Applications
  launcher with that text (Niri config `overview { emit-typed-text; }`, set in
  `packaging/niri/defaults.kdl`).
- `Action::SpawnPlaced`: window session restore (`windows { restore #true; }`)
  and workspace templates (`--open-template NAME`) start applications at a given
  workspace, column and width (`src/app/session_restore.rs`, record in
  `$XDG_STATE_HOME/canopa/shell/windows.json`).
- `org.canopa.Niri.WindowPreview1` on the session bus: taskbar tooltip
  thumbnails, through the same `canopa-services` client used by the portal
  window pickers.

Log out (`session "log-out"`, `PanelAction::Compositor`) sends `Action::Quit` with
`skip_confirmation` over the Shell's Niri IPC socket from the default Power menu, as
`toggle-overview` sends `ToggleOverview`; the Shell never runs a `niri` executable.

## Taskbar

The taskbar (`src/modules/taskbar.rs`, `src/app/taskbar.rs`) shows pinned
applications, then other running applications in screen order (output, workspace,
column, tile; floating windows after columns). A click focuses an application or
cycles its windows in that order; middle-click launches a new instance; right-click
opens an application menu with Open or New window, the desktop file's actions and
Pin or Unpin. While Niri's overview is open, the taskbar shows running
applications with a neutral marker and the compositor highlights the preview
selection. Pins changed from the bar are Shell state, stored as edits to each
taskbar's configured `pinned` list in `$XDG_STATE_HOME/canopa/shell/taskbar-pins.json`;
the Shell never rewrites the KDL file
([Shell ADR 0026](../shell/adr/0026-taskbar-menus-and-saved-pins.md)).

## Application launches

`launch "program" args...` actions, desktop entries and launcher results are
handed to the session, not run as bounded commands
([Shell ADR 0012](../shell/adr/0012-session-owned-application-launches.md)):

- With a Niri socket, the Shell asks Niri to spawn `canopa-shell --app-scope ID --
  PROGRAM...`. The helper moves itself into `app-canopa-<desktop id>-<random>.scope`
  and execs the program, so applications survive a Shell restart and get their own
  systemd scope. Without a systemd user manager the scope step is skipped.
- Without a Niri socket, `systemd-run --user --scope` is used.
- Desktop entries launch through GIO (`gio launch`); `DBusActivatable` entries get
  `org.freedesktop.Application.Activate` or `Open` directly. A failed handoff is
  reported and never retried through another launcher.

Niri's activation tokens shape focus on launch: a `Spawn` creates an external
token (without the urgent-only marker, so the new window takes focus) that lives
10 seconds. Niri accepts an input serial only if it is no older than the last
keyboard or pointer enter, and layershellev's `request_activation_token` needs
input context from the last 2 seconds, so a keyboard-only activation long after
the last pointer event gets no token.

## Launcher

The launcher is the `launcher` Panel Menu source used by the default Applications
menu ([Shell ADR 0023](../shell/adr/0023-unified-launcher-providers.md)). It
introduces no new window or placement: the Panel Menu and Foreground own those.

- **Session model.** A launcher session has an identity, a query generation and
  displayed result revisions. Asynchronous results must match the current session
  and query; configuration changes and dismissal retire them. In-process sources
  debounce 100 ms, process-backed sources 250 ms.
- **Sources** (`src/launcher/`): applications (`canopa-apps`), open windows (Niri),
  file and folder names, desktop controls (open a provider panel), session actions
  (logind `Can*`), Settings pages (`canopa-settings pages`, opened through
  `org.canopa.Settings` `OpenPage`), calculator, emoji and symbols, clipboard
  history and snippets, and extensions. Scoped keywords (`apps`, `files`,
  `windows`, `settings`, `clip`, `snip`, `emoji`, `symbol`, `=`) restrict a query.
- **Files.** A blocking worker scans the configured roots (default `~`) without
  following symlinks, crossing devices or listing hidden entries, bounded by
  `max-paths` (default 100,000) and 10 s. The index is kept across opens and
  refreshed every minute while the launcher is open. It is a name index, not
  content search.
- **Calculator.** A closed local grammar validates arithmetic and unit conversions,
  then `qalc` evaluates them with personal definitions, currencies and network
  updates disabled and bounded time and output.
- **File actions.** Open with (GIO MIME recommendations first), Reveal
  (`org.freedesktop.FileManager1.ShowItems`), Quick Look (`canopa-files
  --quick-look`) and Copy path. Local file URIs keep the original Unix path bytes.
- **Extensions.** Configured executables receive one routed query per debounced
  input as version-1 JSON lines on stdin and return bounded typed results (open,
  copy, launch, open URL). Each query runs in its own process group, cancelled when
  the query changes. The bundled Web and Date/time extensions are executable modes
  of `canopa-shell` (`--launcher-extension`) using the same contract.
- **History.** Selected applications and files are ranked by a private, bounded
  recent/frequent history (`$XDG_STATE_HOME/canopa/shell/launcher-history.json`).
  Query text is never stored. `history #false` disables it.
- **Preferences** (`crates/canopa-launcher`): `$XDG_CONFIG_HOME/canopa/launcher.json`
  holds extension enablement and keywords, the web search template, search
  shortcuts, pins and aliases. Shell and Settings take the same owner-only lock,
  reread before a field-level edit and publish atomically; the Shell rechecks the
  file every second off-thread and keeps the last valid snapshot. The Shell also
  writes a read-only extension catalog
  (`$XDG_STATE_HOME/canopa/shell/launcher-extensions.json`) for Settings.
- **Clipboard.** When recording is turned on (off by default), the Shell runs
  separate bounded `wl-paste --watch` subscriptions for text and, if enabled,
  images. History (up to 64 entries) lives in memory only; saved snippets and the
  options persist in `$XDG_STATE_HOME/canopa/shell/clipboard.json`. Selecting an
  image hands its original bytes and MIME type to `wl-copy`.

## Device keys and OSD

Device keys are typed IPC requests from `canopa-shell volume|microphone|brightness|keyboard-backlight|media ...`,
bound in `packaging/niri/defaults.kdl`
([Shell ADR 0025](../shell/adr/0025-device-keys-and-on-screen-display.md)). The
actions live in the library (`src/device_keys.rs`) and use `canopa-services`:

- volume through the Pulse protocol (raise caps the loudest channel at 100 % and
  unmutes);
- display brightness through logind `SetBrightness` with perceptual steps and a
  1 % floor; keyboard backlights through logind on `*::kbd_backlight` LEDs;
- media through `org.canopa.Shell.Media`, so keys, bar and panel agree on the
  selected player.

When no Shell owns the socket, the command performs the action itself without an
OSD. The OSD is a passive overlay surface on Niri's focused output with one
indicator at a time (default timeout 1500 ms, configurable under `osd`). It also
follows outside changes from event sources (Pulse default devices,
`actual_brightness`, `brightness_hw_changed`) without polling; the first reading is
a baseline, so nothing shows at login. It has its own AccessKit live region.

## Session actions

`session "suspend|hibernate|reboot|power-off"` calls the logind Manager on the
system bus with interactive authorization, through `canopa_services::logind`
([Shell ADR 0024](../shell/adr/0024-session-actions-through-logind.md)). Items whose
`Can*` answer is `no` or `na` are hidden; Hibernate needs a definite `yes`. Reboot
and Power off open confirmation menus that list blocking inhibitors. Failures are
reported through the notice service.

## Notifications integration

- **Summary.** All `notifications` module instances share one
  `canopa-notify-client` `SummarySubscription` on Notify's control socket. It
  carries the attention count, DND state and reason, and the suppression state; the
  Shell never reads notification history. On error it retries after one second and
  replaces its state from a fresh snapshot.
- **Center.** The built-in `notifications` provider is `canopa-notify
  panel-provider --protocol canopa-v5`. The Shell sizes it 480 logical pixels wide
  and full height. Center actions that ask for an activation token get one from
  the Shell's own input serial and surface (see [notify.md](notify.md)).
- **Notices.** `src/notice.rs` is one long-lived session-bus client of
  `org.freedesktop.Notifications` with one slot per purpose (configuration error,
  failed session action). It never D-Bus-activates a server; it waits for the name
  to appear and re-sends current notices to a restarted server.

## Other integrations

| Feature | Owner in the Shell | External service |
| --- | --- | --- |
| Tray | `src/dbus/sni_watcher.rs`, `src/dbus/sni/` | Owns `org.kde.StatusNotifierWatcher` while a `tray` module is placed; DBusMenu for menus |
| Network status | `network` module, `canopa-services` network bundle | NetworkManager, or networkd for status |
| Battery | `battery` module, `canopa-widgets` | UPower |
| Volume | `volume` module | Pulse protocol, event-driven ([Shell ADR 0022](../shell/adr/0022-event-driven-volume-observation.md)) |
| Media | `src/media.rs` exports `org.canopa.Shell.Media` at `/org/canopa/Shell/Media` | MPRIS 2 players |
| Idle inhibitor | `idle-inhibitor` module | `zwp_idle_inhibit_manager_v1` per bar surface, or `canopa-idle toggle` with `backend "stasis"` ([Shell ADR 0017](../shell/adr/0017-manual-idle-inhibitor.md)) |
| Lock | Quick settings `session-lock` and `canopa-shell lock` run `canopa-lock`; `canopa-shell lock --swaylock` execs Swaylock | See [session.md](session.md) |

Runtime diagnostics go through `tracing` to stderr (journal); `RUST_LOG` raises
detail and `CANOPA_SHELL_PERF=1` adds periodic performance summaries. Logs never
include user content such as window titles, commands or search text
([Shell ADR 0006](../shell/adr/0006-use-tracing-for-runtime-diagnostics.md)).
Bars, panels and the OSD expose AccessKit trees over AT-SPI.

## Wallpaper

The Shell owns the desktop and lock-image selection made through the Wallpaper
portal. It is separate from `canopa-wallpaper` in `canopa-session`, which has its own
configuration and outputs (see [session.md](session.md)).

- D-Bus: `org.canopa.Shell.Wallpaper`, interface `org.canopa.Shell.Wallpaper1` at
  `/org/canopa/Shell/Wallpaper` (`src/wallpaper/runtime.rs`). `Apply` takes a PNG
  descriptor and a destination mask (1 desktop, 2 lock image, 3 both); the
  `Destinations` property offers the lock image only when `canopa-lock` or
  `swaylock` is installed; `canopa-lock` reads it from the store. The portal backend in `crates/canopa-portal` calls it after the user
  accepts a preview.
- Store: canonical PNGs named by SHA-256 plus an atomic versioned `selection.json`
  in `$XDG_STATE_HOME/canopa/shell/wallpaper/`, all under one file lock. Input is
  bounded (128 MiB PNG, 16,384 pixels per side, 64 Mi pixels).
- Surfaces: while a desktop image is selected, one `background`-layer surface per
  output (`canopa-shell-wallpaper`; Niri's `place-within-backdrop` rule keeps it
  behind the overview). When a new wallpaper surface is configured, bars on the
  `background` layer of that output are recreated so they stack above it.
- `canopa-shell lock` execs `canopa-lock`. With `--swaylock` it opens the lock
  image under the state lock and execs `swaylock --image /proc/self/fd/N`; if the
  selection is unreadable or its lock is busy, it starts Swaylock without an image
  rather than waiting.

## State files

| Path | Contents |
| --- | --- |
| `$XDG_STATE_HOME/canopa/shell/taskbar-pins.json` | Pins changed from the bar |
| `$XDG_STATE_HOME/canopa/shell/windows.json` | Window session record |
| `$XDG_STATE_HOME/canopa/shell/launcher-history.json` | Launcher ranking history |
| `$XDG_STATE_HOME/canopa/shell/launcher-extensions.json` | Extension catalog for Settings |
| `$XDG_STATE_HOME/canopa/shell/clipboard.json` | Clipboard options and snippets |
| `$XDG_STATE_HOME/canopa/shell/wallpaper/` | Wallpaper images and selection |
| `$XDG_CONFIG_HOME/canopa/launcher.json` | Launcher preferences (shared with Settings) |

## Design decisions

**Shell owns wallpaper images and surfaces.** Writing GNOME preferences would not
change the Niri desktop or the Swaylock image, so the Shell owns the
portal-selected images independently of bar configuration and draws its own
background surfaces. Images are copied from the caller's descriptor into a
content-addressed store before publication, so no caller path is kept and an
in-progress locker cannot lose its image. Consequence: compositor bindings and
custom lockers must opt into `canopa-shell lock` to use the lock image, and the
Wallpaper portal depends on a running Shell.

**Launcher file actions reuse their owners.** Reveal uses the standard
FileManager1 interface and Quick Look is a Files activation intent
(`canopa-files --quick-look PATH_OR_URI`); the Shell adds no preview renderer or
placement. Settings shortcuts launch `canopa-settings` pages. Consequence: Quick
Look needs Canopa Files, and an older Files owner may refuse the intent rather than
start a second runtime.

**Launcher preferences are a shared library, not a service.** `canopa-launcher`
holds the preference and catalog schemas and the locked atomic store; Shell owns
searching, extension processes and presentation, and Settings edits preferences.
Settings never installs or runs discovered extension code: executable extensions
stay in the Shell's KDL configuration. Bundled Web and Date/time providers are
Shell executable modes using the ordinary extension contract, so no network
request happens during matching. Consequence: configured keywords win over bundled
defaults, and conflicts are shown rather than resolved silently.

**Clipboard history is Shell-owned and opt-in.** Capture, in-memory history and
thumbnails stay in the Shell; only options and explicitly saved snippets are
persisted. Image capture is a separate opt-in that depends on recording, and
offers marked sensitive are ignored. Consequence: history does not survive a Shell
restart.

**The notifications module reads a summary, never history.** A small client crate
carries a bounded summary stream so the bar does not depend on Notify's storage or
policy, and Notify keeps delivering banners while the Shell is stopped. Each
process restarts independently. Consequence: an open Center ends when Notify
restarts and must be reopened; queued actions are not replayed.

## Known limitations

- Window previews, overview typing and window session restore need the Niri fork.
- Window session restore does not restore stacked columns after their first window or dialogs.
- The launcher's file search is a name index refreshed every minute, not a content index.
- External monitor brightness (DDC/CI) is not controlled by the brightness keys.
- Log out uses the `niri msg` CLI rather than `niri-ipc`.
