# Settings architecture

Canopa Settings (`canopa-settings`, crate [`crates/canopa-settings`](../../crates/canopa-settings/))
is an ordinary Iced application. It shows preferences and hands each change to the
component or system service that owns it. It runs no daemon of its own: when the
window closes, only the session observer `canopa-settings watch-display-profiles`
(started by `canopa-display-profiles.service`) keeps running. User guide:
[Settings](../settings.md).

Earlier decision records were consolidated into this document; the originals are in
the git history at commit cb300936 under docs/adr/.

## Build and entry points

- Default feature `desktop` (Iced, `canopa-ui`). Without it, only the subcommands
  (`init-session`, `check`, `outputs`, `pages`, `apply-input`) are built.
- Desktop entry `packaging/applications/org.canopa.Settings.desktop`; D-Bus activation
  file `packaging/settings/org.canopa.Settings.service`, installed in the public
  session service directory.
- `canopa settings ...` forwards to `canopa-settings`. The Shell launcher lists pages
  from `canopa-settings pages`.
- Hidden subcommands: `display-guard` (the display trial helper) and
  `watch-display-profiles` (the session observer).

## Page registry

Every page is declared once in [`src/registry.rs`](../../crates/canopa-settings/src/registry.rs):
a `Page` variant and an `Entry` with a stable id, English title, freedesktop icon name,
group, extra search keywords, `Kind` and a hidden flag. Titles, descriptions and
translated keywords are Fluent messages (`settings-page-<id>-title`, `-description`,
`-keywords`). The sidebar, its groups, Ctrl+1–9, Ctrl+Page Up/Down, search, headings,
command-line ids, `pages` JSON and the D-Bus `Pages` property all come from it.

- Groups have a fixed order: Devices, Input, Appearance, System, Desktop (Developer in
  debug builds). Empty groups are hidden; the Input group currently has no pages.
- `navigation()` sorts visible pages by group with a stable sort, so pages keep
  declaration order within a group. Ctrl+N opens the Nth entry of that list; adding a
  page to an earlier group renumbers later pages.
- Ids are public (launchers, notifications, scripts) and never change.
- Hidden pages are reachable by id only. `live-example` (a live page on a fake
  service) exists only in debug builds.
- Search matches every query word against title and keywords in the selected language
  and in English, folding case and Latin accents; title matches rank first.

The visible order is: Displays, Keyboard & pointer, Network, Bluetooth, Printers,
Sound (Devices); Personalization, Accessibility (Appearance); Power, Date & time,
Region & language, Users, Privacy, About (System); Notifications, Default
applications, Launcher, Shortcuts, Widgets (Desktop).

## Page patterns

`Kind` selects one of two building blocks.

**File-backed** (`Kind::File { documents, apply }`, [`src/file_page.rs`](../../crates/canopa-settings/src/file_page.rs)).
A `FileModel` reads its document, provides a fallback when it cannot be read,
validates, writes and asks the owner to reload, and names the section of a shared
document it writes. A `FileDraft` keeps the draft and the snapshot it was read from.
Saving (`storage::Snapshot::save`) re-checks the snapshot, writes a private candidate
in the same directory, runs the owner's validator on it and renames it into place.
`Apply::Trial` (Displays) applies temporarily and saves only after **Keep**.

The shared-document rule: when one page saves a document, every other page that lists
the same document keeps its draft and continues from the new version, but only if it
had read the replaced version and the save left its own section unchanged. Otherwise
its next save fails as an outside change and it must be reloaded. Personalization and
Widgets share `config.kdl` this way; a motion change or a shortcut save refreshes the
other pages' view of `niri.kdl`.

**Live** (`Kind::Live`, [`src/live.rs`](../../crates/canopa-settings/src/live.rs),
`src/ui/live_page.rs`). A service adapter implements `LiveService`: a name, a snapshot,
a stream of updates and `apply` for one change. `Live` is a toolkit-free state
machine: "Connecting…" until the first reply; "Service not available" with Try again
and automatic reconnection; per-control changes with at most one in flight and the
latest queued (so a dragged slider sends its final value); a refused change leaves an
error with Retry and Dismiss on that control only. The window subscribes before
reading the snapshot, and only while the page is shown.

### Adding a page

1. Add a `Page` variant and its `Entry` in `src/registry.rs` with a new id, group,
   keywords and kind, and its Fluent messages in `i18n/{en,fr}/canopa_settings.ftl`.
2. Write the model: a `FileModel` for a file, or a `LiveService` adapter over a
   `canopa-services` client.
3. Hold its `FileDraft` or live page in `ui::State`, with a `Message` variant for its
   edits or `live::Event`s.
4. Add its arms to the page matches in `ui.rs`: view, save (file pages), reload, the
   shared-document rule if it shares a file, and its subscription (live pages).
5. Add tests: model round trips, or the state machine against a fake service from
   `canopa-services`' `test-support` feature.

## Documents and owners

| `Document` | File (under `~/.config/canopa/` unless noted) | Validated by | Applied by |
| --- | --- | --- | --- |
| `Common` | `config.kdl` (`appearance`, `lockscreen`, `shell`, `notifications` sections) | `canopa config check --file CANDIDATE` (every owner) | Shell live reload; `canopa-notify ctl reload`; `systemctl --user reload xdg-desktop-portal-canopa.service`; lock screen at next lock |
| `Input` | `niri-input.kdl` | `niri validate` | Niri `LoadConfigFile` over IPC |
| `Niri` | `niri.kdl` (motion override, window layout, shortcut include) | `niri validate` | Niri `LoadConfigFile` |
| `Displays` | `niri-displays.kdl`, `display-profiles.kdl` | `niri validate`, profile schema | display trial helper; profile watcher |
| `Idle` | `stasis.rune` | Settings' Rune token editor | `canopa-idle reload` (previous file restored on refusal) |
| `Power` | `power.kdl` (`lid-close`) | Settings | profile watcher holding logind's lid inhibitor |
| `Launcher` | `launcher.json`; `~/.local/state/canopa/shell/clipboard.json` | `canopa-launcher` schemas | Shell (rereads preferences) |
| `Shortcuts` | `shortcuts.kdl` → generated `niri-shortcuts.kdl` | `niri validate` | Niri `LoadConfigFile` |

Niri is `CANOPA_NIRI_BIN`, else `/usr/lib/canopa/niri`, else `niri` from `PATH`.
`session::Paths::active()` requires `NIRI_CONFIG` to point at
`~/.config/canopa/niri.kdl`; otherwise compositor-writing controls refuse with
"needs the Canopa session". An unrelated Niri configuration is never written.

Live pages and their clients:

| Page | Client | Service |
| --- | --- | --- |
| Network | `canopa_services::network` (+ `secret_agent`) | NetworkManager |
| Bluetooth | `canopa_services::bluetooth` (per-attempt pairing agent) | BlueZ |
| Printers | `canopa_services::cups_pk_helper`; `lpoptions`, `lpadmin`, `ippfind` | CUPS, cups-pk-helper, Avahi |
| Sound | `canopa_services::audio` (libpulse) | PipeWire's PulseAudio server |
| Power (live part) | `upower`, `power_profiles`, `backlight` (logind `SetBrightness`) | UPower, power-profiles-daemon, logind |
| Date & time | `timedate`, `geoclue` | systemd-timedated, GeoClue |
| Region & language | `accounts`, `locale`; writes `~/.config/locale.conf` | AccountsService, systemd-localed |
| Users | `accounts`; `passwd` | AccountsService, PAM |
| Privacy | `permission_store`; autostart `.desktop` overrides | xdg-desktop-portal permission store |
| About | `hostname`; `/etc/os-release`, `/proc`, `/sys` | systemd-hostnamed |
| Notifications | `canopa_notify_client::preferences` over Notify's control socket | Canopa Notify ([API](../notify/settings-control-api.md)) |
| Default applications | `canopa_apps::mime_apps` | `mimeapps.list`, `xdg-terminals.list` |

The Sound page's notification-sound switch writes `config.kdl` (validated by
Notify), and the Date & time page's clock format writes the Shell clocks and the lock
screen clock in `config.kdl`.

## Single instance

[`src/instance.rs`](../../crates/canopa-settings/src/instance.rs): the first launch
that opens a window requests `org.canopa.Settings` on the session bus without queueing
and serves, at `/org/canopa/Settings`:

```text
interface org.canopa.Settings
  Activate()                        raise the window
  OpenPage(s page, a{sv} options)   open a page id, then raise
    "display-profile" s             Displays only: select a saved profile
  readonly property as Pages        visible page ids, in sidebar order
```

An unknown page or a wrongly typed option returns `InvalidArgs`; unknown options are
ignored. Later launches send `OpenPage` (or `Activate`) and exit. The window raises
itself with Niri's `FocusWindow` on its own window, falling back to toolkit focus
requests. A request during a save or display trial only raises the window. Without a
session bus, or if the running window does not answer within 10 seconds, a launch opens
its own window; snapshot checks still prevent two windows from overwriting each
other's saves.

## Session initialization

`canopa-settings init-session` (run by the `canopa-session` launcher at every login;
see [session architecture](session.md)) creates missing files with mode 0600:
`niri.kdl`, which includes the shipped defaults (or the generated
`niri-shortcuts.kdl`), `niri-input.kdl`, `niri-displays.kdl` and `niri-custom.kdl`,
plus those includes and `stasis.rune` (an embedded copy of
`packaging/stasis/stasis.rune`). It then regenerates `niri-shortcuts.kdl`, migrates a
copied shortcut block if one is found, validates with Niri and prints the path of
`niri.kdl`. A shortcut regeneration failure keeps the last valid files and does not
block login.

## Design decisions

**Settings presents; owners apply.** Settings is an independently launched window that
does not own the compositor, wallpaper, authentication, idle or notification
lifetimes. Every file change is validated by the owning program (or `niri validate`)
on a private candidate before an atomic rename, and other namespaces, comments and
unknown fields are preserved. Consequence: owners keep their own reload behavior
(Shell live reload, lock at next lock, portal reload request), and Settings can be
closed at any time.

**One registry, two page patterns.** A page used to be spread over a dozen places and
every page was "edit a file, then Save". The registry makes navigation, search,
shortcuts, ids and D-Bus metadata one declaration, and the two patterns (file draft
with Save, live service with immediate apply) cover all pages. Adding a page in an
earlier group renumbers Ctrl+N for later pages; ids, not numbers, are the stable
interface.

**A single window with deep links.** Every launch path (launcher, notification
actions, Shell configuration notices, command line) goes through `OpenPage` on a name
Canopa owns, so the D-Bus activation file is installed globally without an opt-in.
The Shell opens Widgets for errors in its `shell` section and Personalization for
syntax, schema-version and appearance errors.

**A separate session configuration.** Settings writes only under
`~/.config/canopa/` and only when `NIRI_CONFIG` points there, so an ordinary Niri
session's configuration is never touched and Settings never implies persistence it
cannot provide.

**Display changes are trials in a helper process.** A short-lived `display-guard`
process snapshots the outputs, applies the change over IPC and restores it on timeout
(20 s), window disconnect or partial failure. A crashing Settings window cannot leave
an unusable layout. Only Keep writes the display include.

**Display profiles in their own file, with a session watcher.** Named layouts live in
`display-profiles.kdl` and match the whole connected set with unique selectors
(make/model/serial when unique, connector otherwise), in the style of Kanshi. The
watcher reads Niri's topology after a quiet period, offers or applies the highest
priority match, enables a screen when a change leaves all screens off, and switches
with the same leased transaction as manual trials.

**Shortcut changes as a delta over the shipped defaults.** Niri can override an
inherited key but not remove it, so Settings stores only added, changed and removed
bindings in `shortcuts.kdl` and generates `niri-shortcuts.kdl` from the current
shipped defaults at each login. Unchanged shortcuts follow updates, a user binding
wins its key, and a removed action stays removed even if its default key changes.
Retired shipped actions are mapped to their replacements. A full copied binding block
in `niri.kdl` is migrated only when the result reproduces it exactly; the previous
file is kept as `niri.kdl.before-shortcut-changes`.

**The Widgets editor edits the owners' layouts.** Widgets edits Shell bar zones and
lockscreen regions in `config.kdl` with the same snapshot and validation as
Personalization, materializing the Shell defaults on the first bar edit. Its diagram
uses the lockscreen layout planner from `canopa-session-ui` and sample data; it never
starts providers, scripts or authentication.

**Shared system-service clients.** NetworkManager, BlueZ, audio, UPower,
power-profiles-daemon, logind, MPRIS, AccountsService, hostnamed, localed, timedated,
GeoClue, the permission store, cups-pk-helper and the power switches live in
`canopa-services`, shared with the Shell and its providers. Each client has a typed
snapshot, a `watch` stream where "unavailable" is a normal state with bounded retry,
and typed errors; commands address the owner and object the snapshot came from.
System services keep state and policy; no Canopa daemon sits in between.

**Network secrets through NetworkManager and a Settings agent.** Canopa never stores
network secrets. A secret typed while creating a profile goes into the profile with
NetworkManager's flags ("remember" = stored by NetworkManager, "ask every time" = not
saved). Later requests reach a secret agent that Settings registers as
`org.canopa.Settings` on the system bus from the first time the Network page opens:
each `GetSecrets` from NetworkManager's current owner becomes a question on the page
(one at a time, two-minute expiry), non-interactive requests get `NoSecrets`, and
`SaveSecrets`/`DeleteSecrets` keep nothing. Consequence: "ask every time" connections
work only while Settings is open. Plugin VPN files are imported with `nmcli connection
import`; WireGuard files are parsed by Canopa.

**Lid and power button through logind inhibitors.** Writing `logind.conf.d` would need
a new privileged helper for a per-person preference. Instead the session watcher holds
logind's `handle-lid-switch` inhibitor for any lid choice other than `system` and asks
logind for the chosen action on each close (nothing while docked); the lid state comes
from UPower, or logind polled every second. The power button uses Niri's own
`handle-power-key` handling: Suspend (Niri's default) or `disable-power-key-handling`
in `niri-input.kdl` to fall back to logind. Outside the Canopa session the system
policy applies.

**Passwords without hashes.** AccountsService's `SetPassword` expects a crypt hash
computed by the client, which would bypass PAM's quality and history rules and the
current-password check. Your own password is changed by running `passwd` with
`LC_ALL=C` and the three lines on its standard input; its outcome is mapped to a few
messages and the typed text is wiped. For other accounts and new accounts Settings
uses `SetPasswordMode(1)` (choose a password at next login), so it never sets another
person's password. Account type, name, picture (checked against the greeter's limits)
and removal go through AccountsService and polkit. Automatic login is not offered,
because greetd and the Canopa greeter do not use AccountsService's setting.

**The 12/24-hour clock lives in the format strings.** There is no separate clock
preference. The Date & time page reads each Shell clock's `time-format` and the
lockscreen `clock-format`; it recognizes `%H:%M`, `%R`, `%k:%M`, `%-I:%M %p`,
`%I:%M %p`, `%l:%M %p` and their seconds forms, writes `%H:%M[:%S]` or
`%-I:%M[:%S] %p`, and never rewrites any other pattern. **Automatic** removes the
recognized patterns so clocks follow the locale. Consequence: readers need no change
and the strings stay the single source of truth.

**Launcher and clipboard preferences are edited, not owned.** The Launcher page edits
extension enablement, keywords and web search templates in `launcher.json` and the
clipboard options in the Shell's clipboard state file, using the `canopa-launcher`
crate's lock, reread and atomic write shared with the Shell. Stale drafts are refused,
and the Shell's pins and aliases are preserved. The extension catalog comes from the
Shell's published `launcher-extensions.json`. Matching, execution and clipboard
capture stay in the Shell; see [the launcher guide](../launcher.md).

**Default applications follow GLib's records.** The page uses `canopa_apps::mime_apps`,
which implements the MIME Applications Associations specification and writes a default
the way GLib does (`[Default Applications]`, `[Added Associations]`, removing a
matching `[Removed Associations]` entry, keeping every other line), so other toolkits
and `xdg-open` agree.

**Idle and lockscreen preferences delegate to their owners.** The Power page edits
only numeric timeout tokens in Stasis's own Rune file and asks `canopa-idle` to
reload; ambiguous plans are read-only. The wallpaper button runs the wallpaper picker.
Native Rune and the owners' schemas remain the sources of truth.

## Network stack ownership

Two network stacks are common, and each owns different state:

| Concern | NetworkManager + systemd-resolved | systemd-networkd + iwd + systemd-resolved |
| --- | --- | --- |
| IP configuration | NetworkManager | networkd |
| Wi-Fi association and authentication | NetworkManager (wpa_supplicant, or experimental iwd backend) | iwd |
| DNS | resolved (configured by NetworkManager) | resolved |
| VPN and connection profiles | NetworkManager | not covered |

The Settings Network page speaks only NetworkManager (`canopa-services::network`);
the Shell's Wi-Fi provider supports both. A Network page for networkd + iwd is
tracked in [issue #38](https://github.com/kevinard/canopa/issues/38).

## Known limitations

- "Ask every time" network secrets are answered only while Settings is open.
- The power button offers Suspend or the system setting only.
- Page icons are declared and published by `pages` but not drawn in the sidebar.
- Region & language changes reach Files, Notify and the Shell only after they restart.
