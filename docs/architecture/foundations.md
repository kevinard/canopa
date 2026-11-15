# Shared foundations

The applications share a set of small library crates. Each library owns a
mechanism (reading the configuration, resolving a palette, finding an icon,
talking to a system service) and leaves policy, state and process lifetime to
the application that calls it. This page describes those crates and the window
conventions the Iced forks provide. For the crate list and the running session,
see [overview.md](overview.md).

Earlier decision records were consolidated into this document; the originals are
in the git history at commit `cb300936` under `docs/adr/`.

## Configuration

Crate: [`canopa-config`](../../crates/canopa-config/src/lib.rs). User reference:
[../configuration.md](../configuration.md).

### Source and discovery

There is one user file, `$XDG_CONFIG_HOME/canopa/config.kdl`. `XDG_CONFIG_HOME`
is used only when it is set and absolute; otherwise `$HOME/.config`, and `HOME`
must be absolute (otherwise the diagnostic code is `location`).

A consumer asks for a source with `Request`:

| Request | Meaning |
| --- | --- |
| `Discover` | The common file if it exists, otherwise a legacy file (Shell and Notify only), otherwise embedded defaults |
| `File { path, format }` | An explicit file in an explicit format (`canopa-v1` or `legacy`); never falls back |
| `Defaults` | Embedded defaults, no file read |

A present path, including a dangling symlink, is selected and must be valid;
discovery never falls back past an invalid file. The legacy fallback applies
only when `canopa/config.kdl` is absent, and only to two components:
`$XDG_CONFIG_HOME/niribar/config.kdl` for the Shell and
`$XDG_CONFIG_HOME/canopa-notify/config.kdl` for Notify. A legacy file is the
bare namespace content without an envelope. `SharedConfiguration` (used by the
Settings portal) reads only the common file or defaults, and rejects legacy input.

### Envelope and namespaces

The file must be a regular, readable UTF-8 file of at most 1 MiB (`MAX_BYTES`);
the main file and its theme file share that budget. Its top level contains
exactly `schema-version 1` and, at most once each, argument-free blocks:

| Block | Owner | Validator run by `canopa config check` |
| --- | --- | --- |
| `appearance` | `canopa-config` (shared) | parsed for every consumer |
| `shell` | Canopa Shell | `canopa-shell --check-config` |
| `notifications` | Canopa Notify | `canopa-notify check-config` |
| `files` | Canopa Files | `canopa-files --check-config` |
| `wallpaper` | `canopa-wallpaper` | `canopa-wallpaper check-config` |
| `lockscreen` | `canopa-lock` | `canopa-lock check-config` |
| `greeter` | `canopa-greeter` | `canopa-greeter check-config` |
| `polkit` | `canopa-polkit` | `canopa-polkit check-config` |
| `pinentry` | `canopa-pinentry` | `canopa-pinentry check-config` |

Any other root node, a duplicate, an argument or a type annotation is an
`envelope` error. There is no namespace for Settings, the launcher, idle or the
portal: Settings edits other owners' namespaces, launcher preferences live in
`$XDG_CONFIG_HOME/canopa/launcher.json`, idle policy is Stasis's
`~/.config/canopa/stasis.rune`, and the portal reads only `appearance`.

`Document::load(request, component, defaults)` reads the source, checks the
envelope, parses `appearance` (with its theme file), and selects the
component's namespace as a borrowed KDL document whose spans point into the
original text. If the namespace is absent, the component gets its embedded
defaults together with the shared appearance. The owner then builds and
validates a complete typed candidate before installing anything.

Error scope follows the layers: syntax, envelope and appearance errors affect
every consumer; a schema error inside one namespace affects only its owner.
Diagnostics carry a code (`location`, `read`, `syntax`, `envelope`, `schema`,
`appearance`), an optional byte offset and an optional `source: "theme"`. They
never contain source text, so they are safe for logs and service status.

Every load produces a `SourceInfo`: the selected path and format, the SHA-256 of
the whole main file, `defaults: true` when the namespace or the whole file was
absent, and, with a theme file, its path and fingerprint plus a combined
fingerprint. Fingerprints are the same across consumers, which makes skew between
processes visible; they do not imply a cross-process atomic reload.

### `canopa config`

`canopa config check [--file PATH] [--show-source]` copies the selected file (and
its theme file) into a private snapshot once, then runs each owner's validator
from the same installation directory against that snapshot and prints one JSON
line per component. Schema logic therefore stays in each application, and the
command needs none of their runtime dependencies. `--show-source` passes through
the owners' local diagnostics, which may include configuration text.

`canopa config migrate --output NEW [--shell FILE] [--notifications FILE]` wraps
explicit legacy files into a new shared file, checks every schema against it,
and creates the output with private permissions. It never overwrites the output
or the originals, and touches no running service or state.

The `canopa` command also forwards `settings`, `wallpaper`, `lock`, `greeter`,
`polkit`, `pinentry` and `idle` to the sibling executables by `exec`, so
`canopa lock` is the same process as `canopa-lock`.

### Reload contract

`canopa-config` has no reload of its own. Each owner decides when to read again,
prepares a complete candidate off its UI or async executor, and keeps its last
good configuration when the candidate is invalid.

| Owner | When it rereads |
| --- | --- |
| Shell | Polls the source and theme metadata every 250 ms and loads after two equal observations; `canopa-shell reload` forces it. Discovery repeats each time. |
| Notify | On `canopa-notify ctl reload [PATH]` or `ctl reload-canopa PATH`; there is no file watching. Settings calls `ctl reload` after saving. |
| Files | Polls every 250 ms with the same two-observation rule. |
| Settings portal | On SIGHUP (`systemctl --user reload xdg-desktop-portal-canopa.service`); emits `SettingChanged` only for values that changed. |
| `canopa-wallpaper` | Watches the configuration, theme and image paths with inotify (`canopa_file::watch`). |
| `canopa-lock` | Reads the configuration each time it locks. |
| `canopa-greeter` | Reads `/etc/canopa/greeter.kdl` (or defaults) at start; never the user's file. |
| `canopa-polkit`, `canopa-pinentry` | Read at process start. |

### Writing the file

Settings and the Files preferences editor change `config.kdl` as a source edit.
Settings takes a snapshot of the exact text and file identity, edits only its own
nodes with the `kdl` crate (comments and other namespaces stay as written),
takes a lock file beside `config.kdl`, writes a private temporary candidate,
validates it with `canopa config check --file`, checks that the file has not
changed meanwhile, and renames the candidate into place. After saving it asks
the owners with explicit reload contracts (Notify, the portal) to reload.

## Appearance and themes

Crates: [`canopa-theme`](../../crates/canopa-theme/src/lib.rs) (data),
`canopa-config` (parsing, in `appearance.rs` and `theme_source.rs`) and
[`canopa-ui`](../../crates/canopa-ui/src/lib.rs) (Iced). User reference:
[../theme-customization.md](../theme-customization.md).

`canopa-theme` holds immutable, toolkit-free values: `Appearance` (color scheme,
optional accent, and a shared `Customization`), the semantic `Palette`, the
17 `ColorRole`s, `Typography`, the ten `UiKind`s (`bar panel menu tooltip
notification card` surfaces and `module button input menu-row` controls),
`UiStyle`/`ResolvedStyle`, logical metrics and WCAG contrast helpers. It reads no
files or environment.

Resolution order is fixed: built-in preset (dark or light), then the theme file
named by `appearance { theme "PATH" }`, then inline `appearance`, then a
component's local `ui` block. Within a layer, the `surface` and `control` groups
apply before specific kinds, and state colors apply last. Unsupported properties
and states are errors, not no-ops, and an explicit zero differs from an omitted
property. A theme file holds the contents of an `appearance` block; it cannot
include another theme or set the main-file-only keys `theme`, `text-scale`,
`high-contrast` and `reduced-motion`. A missing or invalid theme file is an
error reported with `source: "theme"`.

The presets are Canopa's own palettes: deep blue-green and mint in dark, pale
green and forest green in light, with `spine`, `accent-surface` and `brand` roles
for the bar, quiet selection and secondary identity. Typography defaults to
Inter with a system sans fallback at 14/12/20 logical pixels (body, label,
title); `text-scale` (100–150) multiplies the resolved sizes. A custom accent
gets an automatically contrasting `on-accent` unless one is set.
`high-contrast #true` replaces surfaces with black or white and forces text,
borders, accent and status colors to at least 7:1.

Consumers: Shell, Notify (GUI build), Files, Settings, the portal dialogs, and
the session surfaces through `canopa-session-ui`. The Settings portal publishes
`color-scheme`, `accent-color`, `contrast` and `reduced-motion` in
`org.freedesktop.appearance` for other toolkits. Reduced motion is a published
preference; each consumer decides what it animates.

## Shared Iced controls

[`canopa-ui`](../../crates/canopa-ui/src/lib.rs) turns an `Appearance` into
Iced's palette (`theme`) and supplies shared treatments. Callers keep messages,
focus, identity and validation; nothing in the crate knows a domain.

| Item | Use |
| --- | --- |
| `theme(appearance)`, `colors(theme)` | The full Iced theme and Canopa's role colors |
| `action(content, on_press, Action, focused)`, `action_style` | Ordinary Iced buttons with `Primary`, `Normal`, `Quiet` or `Destructive` treatment and a shared focus outline |
| `styled_action`, `frame`, `font`, `padding`, `content_padding` | Apply a configured `UiKind` style to a control or surface |
| `surface`, `muted`, `link`, `panel::surface`, `panel::row` | Container and text treatments |
| `switch_style`, `checkbox_style`, `scrollbar` | Shared on/off switch, check box and quiet scrollbar |
| `scroll_speed(content)` | Touchpad scroll gain for scrollable descendants |
| `direction::*` | Right-to-left rows, alignment, padding, chevrons and icon names |
| `icons::handle`, `icons::trimmed_handle` (feature `icons`) | Iced image handles for rasters prepared by `canopa-assets` |
| `accessibility::*` (feature `accessibility`) | AT-SPI trees; see [Accessibility](#accessibility) |

The Shell's surfaces and Notify's banners are layer surfaces (`iced_layershell`).
Files, Settings and the portal dialogs are ordinary Iced (winit) windows. The
session surfaces (lock, greeter, polkit, pinentry, wallpaper) do not use Iced;
they draw with the software renderer in `canopa-session-ui`.

## Window conventions

The iced, iced_winit and winit forks add a few operations for ordinary Wayland
windows. Each is renderer-neutral, knows nothing about any application, and is
declined by layer surfaces. See [../vendor-maintenance.md](../vendor-maintenance.md)
for the fork branches.

**Transient toplevels.** `iced::window::open_transient(parent, settings)` opens a
non-modal auxiliary window whose `xdg_toplevel` parent is set before its first
configure, so the compositor sizes and places it as a dialog from the start
(setting the parent after creation is too late: Niri has already tiled it).
It returns `None`, and shows nothing, if the parent has closed or the backend
cannot parent windows. Niri floats windows that have a parent by default, so no
window rule is needed. Files uses this for its dialogs and Quick Look.
`open_dialog(parent_handle, modal, settings)` is the out-of-process variant used
by portal dialogs, taking the requesting application's exported Wayland handle.

**Input transition boundary.** Iced normally delivers a whole batch of queued
events against the widget tree it had when the batch started. The iced_winit
fork stops a window's batch at the first event after which the interface is
outdated (a widget requested a rebuild), applies the resulting messages, rebuilds,
and then replays the untouched tail before newer events. A widget opts in by
invalidating widgets when it publishes a page, focus or input change; ordinary
traversal does not rebuild. This keeps fast key sequences such as a shortcut
followed by Tab and typing on the intended page.

**Focus.** `iced::window::gain_focus(id)` on Wayland requests an
`xdg_activation_v1` token bound to the most recent keyboard, pointer-press or
touch input on that connection, from the surface that received it. Input older
than two seconds is not used, a token reply must arrive within one second, and at
most 64 requests are pending per connection. The compositor still decides.
Applications request focus for a parent before closing the child that received
the input, so the closing window can still vouch for it. Background updates do
not request focus.

**Drag and drop.** `iced::window::drag_drop` gives ordinary windows a native
data-device transport. An outgoing drag needs a capability from the exact
left-button press that starts it; a later click cannot lend its grab. Copy or
Move is frozen at drop, each dropped offer is kept separately until it is
finished, and the destination finishes only after its own accepted work succeeds.
Data pipes run off the event loop with a 16 MiB and five-second bound per
transfer. Escape cancels an undropped source. Source handles outlive their window,
so a receiver can complete after the origin window closed. The transport never
deletes a file on Move; the receiving application owns the operation. The Niri
fork carries a Smithay fix that retires a drag grab when its source is destroyed,
so a cancelled drag does not deliver a Drop.

**Window shape.** Client windows draw square, edge-to-edge surfaces. Corner
radius, clipping, shadows and the floating-window hairline border come from the
compositor defaults in [`packaging/niri/defaults.kdl`](../../packaging/niri/defaults.kdl),
so every application in the session gets the same shape. Unsupported compositors
show square windows, which is still correct.

## Assets, icons, MIME types and thumbnails

| Piece | Crate | Notes |
| --- | --- | --- |
| Bounded raster/SVG preparation | `canopa-assets` (`RasterBatch`, `Formats`, `svg`) | Caller supplies per-image policy, aggregate budget and cancellation |
| Icon lookup | `canopa-assets::icons` (`IconLookup`, `IconResolver`, `IconSource`) | freedesktop Icon Theme lookup with bounded index parsing and a cache |
| Selected icon theme | `canopa-file::icon_theme` | `gtk-icon-theme-name` from GTK 4, then GTK 3 `settings.ini` |
| MIME detection | `canopa-file::mime` (`MimeDatabase`) | shared-mime-info `globs2`, `magic`, aliases, subclasses, icons |
| Applications for a type | `canopa-apps::mime_apps` | MIME Applications Associations: read, default, `set_default` |
| Thumbnail cache | `canopa-assets::thumbnails` | Thumbnail Managing Standard storage and `.thumbnailer` discovery |
| Iced handles | `canopa-ui::icons` | Built from prepared pixels |

A `RasterBatch` charges every file attempt, byte read and decoded pixel to the
batch, checks dimensions before decoding, and returns owned RGBA pixels or
nothing. Callers run batches on admitted blocking workers and own fallbacks;
failures never log paths or content. SVG is detected from content, never from
the file name.

Icon lookup tries the selected theme, its parents depth-first, `hicolor`, then
unthemed pixmaps, trying every candidate name in a theme before the next theme,
and ranks by exact size and scale, then nearest size. It finds PNG, SVG and XPM
paths (XPM is not decoded). `IconSource::parse` accepts names, absolute paths and
local `file://` URLs. Generic-name fallbacks (`a-b-symbolic` to `a-symbolic`,
`a-b`, `a`) are opt-in; Notify uses them for sender icons. `IconResolver` caches
hits and misses and drops the cache when the settings files, icon roots or loaded
theme directories change (checked at most every two seconds). Notify instead
runs fresh per-banner lookups so it sees files when a banner is prepared.

MIME detection follows the shared-mime-info order: a unique glob wins without
reading content; otherwise magic runs on at most a 4 KiB prefix; otherwise
`text/plain` or `application/octet-stream` from the first 128 bytes, and
`application/x-zerosize` for empty files. Icon names come from the type's `icon`,
then the type with `/` replaced by `-`, then its generic icon.

`mime_apps` reads every `mimeapps.list` (desktop-specific lists first) in XDG
order, applies added and removed associations, and picks the default the way
GLib's `g_app_info_get_default_for_type` does, walking the type's ancestors but
leaving out `application/octet-stream`. `set_default` records a choice like GLib
does, keeps every other line byte for byte, and writes atomically through a
symlinked list.

Thumbnails use GLib's URI escaping and `MD5(uri).png` names in the `normal`,
`large`, `x-large` and `xx-large` directories, so other desktops' thumbnails are
reused and Files' thumbnails are reused by them. A thumbnail is valid only when
its URI and modification time match; invalid ones are skipped, never deleted.
Running thumbnailers is the consumer's job; Files runs them in a Bubblewrap
sandbox.

## Desktop applications

[`canopa-apps`](../../crates/canopa-apps/src/lib.rs) discovers desktop entries in
XDG order with locale fallback and desktop-file ID shadowing, reading each entry
with a 256 KiB bounded read. It returns `DesktopApplication` metadata (name,
generic name, keywords, icon, desktop actions, D-Bus activation,
`StartupWMClass`, `SingleMainWindow`) and launch checks (`check_launch`,
`TryExec`, terminal launcher). `discover_installed_applications` applies menu
visibility for the launcher; `discover_choice_metadata` keeps names and icons for
hidden handlers that a portal request names explicitly. The crate does not
launch, choose or authorize anything: the Shell owns launches and menus, the
portal owns AppChooser selection. Call it on a worker.

## File primitives

[`canopa-file`](../../crates/canopa-file/src/lib.rs) depends only on `rustix`, so
headless profiles can use it.

| Module | Provides |
| --- | --- |
| crate root | `read_bounded`, `read_canonical_bounded`, `read_prefix`: nonblocking, descriptor-checked reads of regular files with an overflow probe; errors carry no content |
| `mime` | shared-mime-info detection (above) |
| `icon_theme` | The desktop's selected icon theme |
| `gtk_bookmarks` | Read, add and remove lines of `~/.config/gtk-3.0/bookmarks`, keeping other lines byte for byte |
| `private_state` | Descriptor-checked private state files with atomic durable replacement |
| `watch` | inotify watching of paths, including atomic replacement and missing parents |
| `executable` | `current()` and `installed()`: paths that still start the right release after a package upgrade replaced the running executable |

## Translations

Crate: [`canopa-i18n`](../../crates/canopa-i18n/src/lib.rs). How to add strings
and languages: [../translations.md](../translations.md).

Messages use Project Fluent through `i18n-embed`. Each crate with user-visible
text has `i18n.toml` and `i18n/<language>/<crate>.ftl`, embedded at build time,
and declares `canopa_i18n::localization!()` in its `src/i18n.rs`, which defines
`LOADER` and a crate-local `fl!` checked at compile time against the English
file. English (`en`) is the source and fallback; `fr` is also shipped.

Processes use English until they call `canopa_i18n::init()`, so tests are
deterministic. Negotiation, first match wins: `CANOPA_LANGUAGE`, `LC_ALL`, the
person's `$XDG_CONFIG_HOME/locale.conf` (which Settings writes), then the
inherited `LC_MESSAGES`/`LANG` with `LANGUAGE`. Date, time and number formats
follow the same order with `LC_TIME` and `LC_NUMERIC`. The selection is
process-wide with a generation counter: after `select`, every crate's loader
reloads on its next lookup, so windows switch language at their next redraw.

`canopa_i18n::format` formats dates, times, weekdays, first weekday, hour cycle,
integers, decimals, percentages, sizes, durations and relative times with ICU4X
and compiled CLDR data (Gregorian calendar only). Right-to-left direction applies
to the negotiated language; `canopa_ui::direction` mirrors Iced layouts, and
cosmic-text shapes bidi text in Iced and in the session renderer.
`CANOPA_PSEUDO_LOCALE=1` produces accented, lengthened, right-to-left English for
layout testing. `check_catalog` runs in each crate's tests, and
`scripts/check-i18n.py` rejects untranslated word-like literals in UI code.

Clocks: an explicit Chrono pattern (`time-format`, `lockscreen { clock-format }`)
is the person's own and is kept; without one, the Shell clock, calendar and lock
screen use the locale's styles. Settings' Date & time page switches recognized
12- and 24-hour patterns and never rewrites a custom one.

## Accessibility

Crate: `canopa-ui`, features `accessibility` and `accessibility-testing`.

Views describe controls declaratively: `semantic(Node, element)` wraps a widget
with a role, stable key, name, value, states, relations and the messages for
click, increment, decrement, expand, collapse and set-value. The wrapper shares
the wrapped widget's tree state, so it never resets focus, scrolling or drafts.
One `Collect` widget operation reads the nodes, the text drawn inside them, the
focused widget and each node's bounds; an unnamed slider, pick list, field or
switch is named by the text drawn just before it. `Bridge` publishes one AT-SPI
frame per window through `accesskit_unix`, keeps node IDs stable by key, exposes
password fields only as bullets, and turns incoming requests back into the
messages ordinary input sends. Requests for nodes that are no longer in the last
published tree do nothing.

Nothing is collected until an assistive technology enables the accessibility bus.
Keys are explicit and independent of displayed text, so translation never changes
focus or AT-SPI identity.

Settings, the launcher, Shell bar surfaces, Files and the portal dialogs use the
shared layer. Three integrations keep dedicated trees on the same adapter crate:
Shell provider panels, Notify banners (for announcement ordering) and the
session surfaces in `canopa-session-ui` (for secret handling).
`accessibility::testing` runs a private AT-SPI bus for native tests.

## System-service clients

Crate: [`canopa-services`](../../crates/canopa-services/src/lib.rs); its crate
documentation has the module table.

Toolkit-free clients for NetworkManager (with systemd-networkd and iwd), BlueZ,
audio (PulseAudio protocol, feature `audio`), UPower, power-profiles-daemon,
logind (capabilities, inhibitors, session actions, brightness), backlights,
timedated, hostnamed, localed, AccountsService, the portal permission store,
cups-pk-helper, GeoClue and MPRIS. The Shell, its headless providers and Settings
share them. Every client has the same shape:

- a typed snapshot, with a one-shot read where useful;
- a `watch` stream of `ServiceState<T>`, where `Unavailable` (not installed, not
  running, or the last read failed) is a normal state; streams report an outage
  once, retry with bounded backoff and follow owner changes;
- commands with typed errors (`Unavailable`, `NotAuthorized`, `TimedOut`,
  `Changed`, `Invalid`, `Refused`, or a service-specific type).

Commands address the owner, device or connection the snapshot came from and
revalidate before acting. Wording stays with the callers. Fake services for
tests (private bus, logind, UPower, BlueZ, MPRIS, NetworkManager, iwd, a private
Pulse server) are behind the `test-support` feature. `canopa-session` keeps its
own logind client for locking.

## Widget data

[`canopa-widgets`](../../crates/canopa-widgets/README.md) holds passive data
sources for widgets in the Shell, the lock screen and the greeter: battery
(UPower), the Shell's selected media player (`org.canopa.Shell.Media`), Stasis
idle state (`canopa-idle watch`), bounded script output and a weather snapshot
file. It has no Iced dependency and no actions; consumers own layout, privacy on
the lock screen and control.

## Other shared crates

- [`canopa-notify-client`](../../crates/canopa-notify-client/src/lib.rs): bounded
  clients for Notify's control socket: the summary subscription
  (`summary-subscribe-v1`, 512-byte frames), the lock-screen feed
  (`lock-screen-subscribe-v1`, at most four entries) and the preferences API used
  by Settings. No notification bodies, storage or policy cross it. See
  [../notify/settings-control-api.md](../notify/settings-control-api.md).
- [`canopa-launcher`](../../crates/canopa-launcher/README.md): launcher
  preferences (`$XDG_CONFIG_HOME/canopa/launcher.json`), pins, aliases and
  extension discovery shared by the Shell and Settings, through a bounded,
  private, locked store.
- [`canopa-session-ui`](../../crates/canopa-session-ui/README.md): the
  software-rendered presentation of the lock screen, greeter, polkit agent,
  pinentry and wallpaper picker, with erased secret input and its own AT-SPI
  tree. See [session.md](session.md).
- [`canopa-panel-protocol`](../../crates/canopa-panel-protocol/README.md): see
  [panel-protocol.md](panel-protocol.md).

## Design decisions

**One configuration file, independent schema owners.** Users edit one file, but
each application validates its own namespace with its own schema and installs it
on its own schedule. `canopa config check` runs each owner's validator from the
same installation against one immutable snapshot, which keeps schema code in its
owner and the command free of GUI dependencies. The consequence is that there is
no cross-process atomic reload; fingerprints in each owner's status make skew
visible instead.

**Present invalid files never fall back.** A file that exists but cannot be read
or parsed, including a dangling symlink, is an error rather than a reason to try
the legacy file or defaults. Silently running on another source would hide the
user's mistake and could apply settings they deleted. Owners keep their last good
configuration at runtime.

**Content-free diagnostics.** Configuration errors carry a code and an offset,
never source text, so they can go to the journal, service status and JSON output.
Source excerpts appear only with an explicit local `--show-source`.

**Migration only wraps.** `canopa config migrate` wraps legacy files into
namespaces without rewriting their content, checks the result, and creates a new
file. It never overwrites, so an interrupted or mistaken migration loses nothing.

**Appearance is parsed once and is toolkit-free.** `canopa-config` parses
`appearance` for every consumer and `canopa-theme` holds the result as plain
data, so headless and software-rendered processes use the same palette as Iced
ones, and there is one definition of every role. `canopa-ui` only adapts it to
Iced and carries no domain behavior.

**Strict theme grammar.** Unknown properties and states are errors and explicit
zero differs from omission. Accepting unsupported settings as no-ops would make a
theme look applied when it is not.

**Bounded, caller-owned asset work.** Asset crates return paths or owned pixels
under budgets chosen by the caller, on the caller's workers. They hold no
path-keyed pixel cache, so a later load sees file changes, and they never decode
during rendering.

**GTK's settings name the icon theme.** Canopa's configuration does not own an
icon theme. Reading GTK's `settings.ini` means Canopa and GTK applications in the
session always agree. dconf and KDE settings are not read.

**Own MIME and thumbnail code, no GIO.** The shared-mime-info and thumbnail
formats are small; reading them with Canopa's bounded reads keeps every input
size-limited and adds no crate dependency. Compatibility with GLib (glob
precedence, default-application order, thumbnail names) is deliberate so Canopa
and GTK applications see the same results.

**Desktop metadata without shared lifetimes.** The Shell, the portal, Notify and
Files read desktop entries through `canopa-apps` instead of depending on each
other. The crate offers metadata and checks only; each consumer keeps its own
launch, selection and caching policy.

**Minimal fork operations for window behavior.** Transient parents, the input
boundary, input-bound focus and drag transport are each one focused,
application-neutral operation in the Iced and winit forks, rather than
application-specific workarounds or Niri IPC. Layer surfaces keep their own
adapter. Each change is kept as a fork topic so it can be proposed upstream.

**Compositor-drawn window shape.** Transparent client windows with their own
radius and shadow would need alpha surfaces and margins the compositor counts as
part of the window. Square client surfaces with compositor-drawn shape work on
every renderer and give every application the same look.

**Declarative accessibility beside the widgets.** Hand-building a tree per view
describes every view twice; adding roles to every Iced widget is a large fork
divergence. Semantic wrappers read from the laid-out tree cover every view built
from shared controls, at the cost that a raw widget without a wrapper is exposed
only as text.

**Fluent and ICU4X.** Fluent gives compile-time checked message IDs and
arguments and grammatical selectors; ICU4X gives dates, times, numbers and week
data for every locale without depending on installed glibc locales or
process-global `setlocale`. Binaries carry the CLDR data they use (a few MB); this
keeps correct formats for regions people use with an English or French interface.

**The clock's hour cycle lives in its format string.** A separate 12/24-hour key
would need a precedence rule against existing patterns and would silently change
what a custom pattern means. Settings edits only the patterns it recognizes.

**System services own state.** The shared clients read, observe and ask; no
Canopa daemon sits in between. Providers stay separate processes, which isolates
crashes and keeps service permissions out of the bar.

## Known limitations

- AccessKit exposes only the default action over AT-SPI, so secondary actions
  (right click, scroll on a bar module) are not reachable from a screen reader.
- Wayland gives no global window position, so accessible bounds are
  window-relative and magnifiers follow focus only within a window.
- Portal Screenshot, ScreenCast, GlobalShortcuts, Account and Print dialogs have
  no shared accessibility tree yet.
- dconf and KDE icon-theme settings are not read.
- The Shell and Files use fixed icon source scales rather than per-output scale.
- The tray ignores a StatusNotifierItem's `IconThemePath`.
- Only the Gregorian calendar is used for display, and no right-to-left
  translation is shipped.
