# Settings core-experience audit

Status: needs-triage

Audit date: 2026-11-11, at `20e1bf5`. This was a read-only code review. The only
command run was the focused Settings test build
(`bash scripts/cargo.sh test --locked -p canopa-settings --test integration --lib`).
It passed: 6 unit tests, 17 integration tests, and 2 tests ignored because they
need a native Niri. The desktop was not run.

Sizes: **S** is under a week, **M** is 1–3 weeks, **L** is more than 3 weeks of focused work.

## Summary

- Settings has **7 pages**: Displays, Keyboard & pointer, Personalization,
  Power & idle, Launcher, Shortcuts and Widgets (`crates/canopa-settings/src/lib.rs:3-36`).
  None of them talks to a system service. The only zbus code in the crate is a
  notification proxy in `profile_watch.rs:251-271`.
- What exists is well engineered. Displays has a test/keep/revert trial run in a
  separate process with rollback. Config files are saved atomically, validated,
  and checked for outside edits. Display profiles and the widget layout editor are solid.
- Of the 18 baseline areas, only **Displays** and **Mouse/touchpad** are near
  complete. Appearance, Keyboard and Power are partial or rough. Network,
  Bluetooth, Sound, Notifications, Default apps, Date/time, Region, Users,
  Privacy, Printers and About have **no Settings page**.
- Network, Bluetooth, Sound, brightness and power profiles exist only as
  **Shell quick-settings providers**. Their zbus and libpulse clients live inside
  `canopa-shell` and can't be reused by Settings.
- Cross-cutting gaps: no search inside Settings, no single instance, no AccessKit
  tree, no translation support, no polkit flows. Deep links work only as
  `--page <7 values>`.
- The architecture can't take 15+ pages without groundwork first: a shared service
  crate, a page registry, a "live service" page type, scrollable/grouped navigation,
  and a single-instance D-Bus entry point.
- There is an upgrade trap in the shortcut editor. Once a user saves one shortcut,
  later changes to the shipped bindings never reach them, and the editor becomes
  read-only. Fixed in 2ac9fc7.

## Page/capability table

"Where" means where the capability lives today: **S** = Settings app,
**Sh** = Shell quick settings or launcher provider, **K** = hand-edited KDL
(`config.kdl`, `niri-custom.kdl`, theme files), **CLI** = terminal only, **–** = nowhere.

| Area | Capability | Status | Where | Evidence | What a user notices |
| --- | --- | --- | --- | --- | --- |
| Appearance | Light/dark | works | S | `ui.rs:1180-1187`, `preferences.rs:40-52`, portal `canopa-portal/src/lib.rs:103-112` | Toggle plus Save. Shell, Notify and portal reload. The Settings window previews immediately |
| | Accent | rough | S | `ui.rs:1188-1190`, `ui.rs:1500-1515` | Free-text `#RRGGBB` field. No swatches or preview |
| | Contrast / high contrast | missing | – | portal publishes only `color-scheme` and `accent-color` (`canopa-portal/src/lib.rs:103-124`) | No high-contrast option. Apps get no `contrast` key |
| | Fonts | missing | K | typography only in `config.kdl`/theme files (`docs/theme-customization.md`). The lock-screen font is a text field (`ui.rs:1216`) | Must edit KDL to change the UI font |
| | Text scaling | works (Canopa only) | S | `ui.rs:1191-1198`, `ui.rs:1639` | 100–150 % applies to Canopa apps only. GTK/Qt apps aren't scaled |
| | Display scaling | see Displays | S | | |
| | Wallpaper | works | S (external picker) | `ui.rs:768-787` | Opens `canopa-wallpaper pick` |
| | Icon theme | missing | – | none | Can't change |
| | Cursor theme/size | missing | K (`niri-custom.kdl` `cursor {}`) | none in Settings | Must edit Niri KDL |
| | Reduced motion | works | S | `ui.rs:1199-1204`, `ui.rs:1245-1258`, `niri.rs:109-147` | Two separate controls with different apply models (Save vs instant button). Not published to the portal |
| | Lock-screen look | rough | S | `ui.rs:1209-1243` | chrono format strings and avatar as a typed absolute path |
| Displays | Resolution/refresh | works | S | `ui.rs:1019-1026`, `display.rs:92-132` | 20 s Keep/Revert (`display.rs:25`). Rollback survives a crash |
| | Scale | partial | S | `ui.rs:1042-1049` | Slider from 0.5 to 4 in 0.25 steps. No 1.33/1.1 and no "automatic" |
| | Arrangement | rough | S | `ui.rs:1050-1064` | Typed logical x/y only. No drag canvas and no automatic edge snapping |
| | Rotation | works | S | `ui.rs:1027-1039` | |
| | Enable/disable | works | S | `ui.rs:1013-1018`, `display.rs:164-191` | Refuses to turn off the last display |
| | Primary output | missing | – | Niri has no primary. `focus-at-startup` isn't exposed | No way to choose the main screen |
| | VRR | missing | K | not in `actions` (`display.rs:92-132`). A hand-written `variable-refresh-rate` is preserved by `display.rs:267-343` | Gamers must edit KDL |
| | Saved profiles | works | S | `profiles.rs`, `ui/profiles.rs`, `profile_watch.rs`, ADR 0133 | Hotplug offers and auto-switch. Physical dock testing is still unverified (`.scratch/display-profiles/PRD.md`) |
| | Night light | missing | – | none (PRD W3) | |
| Network | Wi-Fi | partial | Sh | `canopa-shell/src/wifi_provider/backends.rs` (NM and iwd), enterprise refused at `:415-418` | Scan, connect open/WPA-Personal/hidden from the panel. WPA-Enterprise/eduroam: "unsupported network". No forget or saved-network list **Fixed in 77f2b41 (Settings Network: list, join incl. hidden and WPA-Enterprise PEAP/TTLS/TLS, forget, auto-connect, Wi-Fi switch; secrets ADR 0139), 50a3626 and 3861301 (Shell).** |
| | Wired | missing | – | Shell only shows connectivity (`dbus/networkmanager.rs`) | No static IP, no 802.1X wired **Fixed in 77f2b41:** wired status and IPv4/IPv6 method, addresses and DNS (802.1X wired still open). |
| | VPN | missing | – | none | Needs `nmcli`/`nm-connection-editor` **Fixed in 77f2b41:** connect/disconnect, WireGuard create and `.conf` import, plugin VPN import through `nmcli connection import`. |
| | Hotspot | missing | – | none | **Fixed in 77f2b41.** |
| | Proxy | missing | – | none | **Fixed in 77f2b41** (connection proxy: none, PAC URL, or manual through a PAC script; metered flag too). |
| Bluetooth | Devices | partial | Sh | `canopa-shell/src/hardware_panel/bluetooth.rs` (raw zbus, agent `:358`, power `:629`, connect/disconnect `:668-671`) | Can power, scan, pair and connect. No forget/remove, battery, rename or audio profile **Fixed in c8d9e06:** Settings Bluetooth page: adapter power and visibility, search while shown, paired and nearby lists with kind and battery, pair with code confirmation or PIN/passkey entry, connect, disconnect, trust, forget (audio profile on the Sound page; rename still open). |
| Sound | Output/input device | partial | Sh | `canopa-shell/src/audio/devices.rs:55-60` (libpulse) | Can set default, volume and mute per device **Fixed in 1e58759:** also in Settings Sound, with balance. |
| | Per-app volume | missing | – | no sink-input handling | **Fixed in 43b9907 and 1e58759.** |
| | Card profiles | missing | – | none | Can't switch HDMI/analog or A2DP/HFP **Fixed in 43b9907 and 1e58759.** |
| | Input level meter / alert sound | missing | – | none | **Fixed in 1e58759:** level meter while the page is shown; Notify notification sounds switch. |
| | Volume keys | works | K (shipped defaults) | `packaging/niri/defaults.kdl:39-42` (`wpctl`) | No on-screen display (PRD W3) |
| Power | Battery status | partial | Sh (bar) | `canopa-shell/src/dbus/upower.rs` | Bar indicator only. No health or details **Fixed in 53776d4 and 101c4ba:** Settings Power shows charge, estimates, health and cycles. |
| | Power profiles | partial | Sh | `hardware_panel/power.rs:11-19` (power-profiles-daemon) | Panel only, not in Settings **Fixed in 101c4ba.** |
| | Brightness | partial | Sh | `hardware_panel/power.rs` (logind `SetBrightness`) | Slider works. **No brightness keys** in `defaults.kdl` **Fixed in 101c4ba:** Settings slider and the Shell slider use the perceptual mapping (keys: 004f775). |
| | Lid / power-button actions | missing | – | none | logind defaults only **Fixed in 101c4ba** (ADR 0140: lid choice through logind's inhibitor in the session; power button Suspend or the system setting). |
| | Idle lock/screen-off/suspend timeouts | rough | S | `ui.rs:1267-1337`, `preferences.rs:166-316` | Times in seconds as text fields. **Save is disabled unless Stasis is running** (`ui.rs:618-622`, `:915-918`). Custom plans are read-only **Fixed in 101c4ba:** duration pickers, Save works without Stasis, idle daemon state shown. |
| | Idle profile, pause/resume | rough | S | `ui.rs:1291-1318` | Profile name is free text with default `"none"` (`ui.rs:228`) |
| Keyboard | Layouts | rough | S | `ui.rs:1082-1096`, validation `input.rs:39-75` | Must type XKB ids (`us,fr`) and options (`grp:alt_shift_toggle`). No picker or search |
| | Repeat | works | S | `ui.rs:1097-1115` | |
| | Shortcuts editor | partial | S | `shortcuts.rs`, `ui/shortcuts.rs:45-51` | Only the shipped set. Typed key names, no key capture, can't add custom bindings. See upgrade risk below |
| | Compose / Caps behaviour | rough | S (XKB options text) | `ui.rs:1092-1096` | Only with knowledge of XKB option names |
| Mouse & touchpad | Speed, natural scroll, tap, disable while typing, left-handed | works | S | `ui.rs:1118-1169`, `input.rs:3-16` | Settings apply to all devices. No scroll speed, click method, accel profile or middle-click emulation. Per-device rules only in KDL |
| Notifications | Do not disturb | works | Sh (`notifications` provider) + CLI | `canopa-shell/src/launcher/controls.rs:28-34`, `canopa-notify/README.md:73` | Not in Settings. No schedule **Fixed in f82cffa:** Settings Notifications page: on, for a while, until tomorrow, daily schedule with days, alarms/calls break-through, fullscreen and sharing suppression (Notify ADR 0055 control API). |
| | Per-app rules / sounds | missing (KDL only) | K | `docs/configuration.md:25` (`rule … outcome="silenced"`) | Must edit KDL **Fixed in f82cffa:** per-app delivery (banners, Center only, mute), sound and lock-screen visibility for apps with rules and any installed app found by name. |
| Default apps | mimeapps.list | missing | CLI | `docs/files.md:146` (`xdg-mime default …`). `canopa-apps` has no `MimeType` field (`canopa-apps/src/lib.rs:7-16`) | Can't choose browser, mail or terminal. `Mod+Return` is hard-wired to `foot` (`defaults.kdl:13`) **Fixed in e7c5f15:** Settings Default applications page (browser, mail, calendar, music, video, photos, text editor, file manager, terminal through `xdg-terminals.list`, other file types by name or extension). `Mod+Return` still runs `foot`. |
| | Autostart | missing | – (Background portal owns modern autostart, `canopa-portal/src/background/service.rs:1`) | none | No list or toggle of login apps |
| Date & time | Time zone, NTP, 12/24 h | missing | K (clock format strings) | none | **Fixed in ef645c5:** Settings Date & time page: time zone search, zone from location once (GeoClue), NTP, manual date and time, 12/24 h in the Shell and lock clock formats with custom patterns kept (ADR 0142). Continuous automatic time zone still open. |
| Region & language | Locale, formats | missing | – | none (PRD W5) | |
| Users | Accounts | missing | – | AccountsService reader exists only in `canopa-greeter/src/accounts.rs` | |
| Privacy | Lockdown switches | missing | CLI (`busctl`) | `docs/lockdown-portal.md` | Switches exist, but there's no UI |
| | Portal permissions / screen-share history | missing | – | none | |
| Accessibility | Text size, reduced motion | partial | S | see Appearance | |
| | High contrast, cursor size, sticky/slow keys, screen reader, magnifier, OSK | missing | – | none | |
| | Settings itself usable with Orca | missing | – | no `accesskit` dependency in Settings or `canopa-ui`. Only Shell, Notify and session-ui have one | A screen-reader user can't use Settings |
| Printers | Queues, jobs, discovery | missing | – | Print portal is opt-in and unrelated (ADR 0070–0072) | |
| About | Hostname, OS, hardware, firmware | missing | – | none | |
| Extra (not in baseline) | Launcher extensions, web search, clipboard | works | S | `ui/launcher.rs`, `launcher.rs` | Clipboard has its own Save button |
| | Bar / lock-screen widget layout | works | S | `ui/widget_layouts.rs`, ADR 0134 | |

### Cross-cutting

| Capability | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Search within Settings | missing | no search field in `ui.rs:816-982` | The launcher has 9 Settings shortcuts plus provider entries (`canopa-shell/src/launcher/controls.rs:8-133`). The Widgets page has no launcher entry |
| Deep links | partial | `main.rs:14-19`, `--page` with 7 values, `--display-profile` | No positional form, no D-Bus activation and no single instance. Every launch opens a new window (`profile_watch.rs:355-357` spawns another process) |
| Live apply / revert | mixed | Displays: trial (`display.rs:370-…`). Files: validate → atomic replace → roll back if the owner's reload fails (`input.rs:181-…`, `preferences.rs:222-234`, `niri.rs:148-165`) | Safe, but four apply models: footer Save, instant button (compositor motion), separate Save (clipboard), external app (wallpaper). No unsaved-changes indicator. Closing the window drops drafts silently (no close handler in `ui.rs:1625-1651`) |
| Errors when a service is missing | partial | Stasis status line (`ui.rs:613-616`), not-in-session banner (`ui.rs:898-905`), Niri errors put in `notice` | Messages are generic. There's no service layer, so no consistent "service not installed or not running" state |
| Polkit for privileged changes | n/a / missing | no polkit or `pkexec` in Settings | Nothing privileged exists yet. Date/time, locale, hostname, users and printers will all need it |
| AccessKit tree | missing | see Accessibility | |
| Keyboard navigation | works | `ui/focus.rs`: Tab/Shift+Tab, arrows on sliders and choices, Ctrl+1–7 (`focus.rs:233-239`), Ctrl+S (`:252`), Esc reverts (`:254`) | Ctrl+1–7 is hard-coded and won't extend to 15 pages |
| Translatability | missing | English literals throughout. Focus IDs are built from English labels (`ui.rs:1397-1402`, `ui.rs:978`) | Translating labels would change focus IDs and break default focus and tests |
| Window follows Canopa theme and text scale | works | `ui.rs:1638-1639` | |

## Architecture assessment

**What is reusable**
- `storage::Snapshot` (`storage.rs:14-93`) handles file-backed preferences well:
  owned regular files only, lock file, outside-edit detection, validation before
  publish, atomic rename. Every KDL-backed page (Notifications rules, Default apps
  via `mimeapps.list`, fonts, cursor) can reuse it.
- The display trial helper (`display.rs:370-695`) is a good model for any risky
  live change.
- `profiles.rs` and `widget_layouts.rs` already use a **per-page sub-enum
  (`Action`) plus `impl State` in a submodule**. That is the right template.
- Shared primitives such as `settings_group`, `field`, `toggle`, `adjust` and
  `choice` (`ui.rs:1339-1479`) and the custom focus frame give consistent keyboard behaviour.

**What doesn't scale**
- **God state and flat messages.** One `State` with about 30 fields for all pages
  (`ui.rs:89-120`) and about 60 `Message` variants (`ui.rs:29-88`), some keyed by
  magic `u8` indices (`InputText(u8)`, `PersonalText(u8)`, `InputFlag(u8)`,
  `ui.rs:494-518`). The "blocked while busy" list (`ui.rs:248-270`) must be kept
  up to date by hand.
- **A page is spread over 6+ places**: the `Page` enum, `ALL` and `label`
  (`lib.rs`), the description match (`ui.rs:848-856`), the view dispatch
  (`ui.rs:906-914`), the Save dispatch (`ui.rs:627-716`), the footer availability
  rules (`ui.rs:915-922`), subscriptions (`ui.rs:799-815`), Ctrl+N shortcuts
  (`focus.rs:233-239`) and the launcher entries in the Shell.
- **Navigation** is a fixed 200 px column of buttons with no scroll, groups or
  search (`ui.rs:817-846`). With 15+ pages and a 600 px minimum height, it overflows.
- **One global `busy` flag** freezes the whole window during any save (`ui.rs:99`, `:1395`).
- **Only one page type exists**: "file-backed draft plus footer Save". Pages backed
  by system services (NetworkManager, BlueZ, PipeWire, UPower, timedated) need a
  second type: state from the service, updated by `PropertiesChanged` and other
  signals, changes applied immediately with busy and error state per control, a
  placeholder when the service is missing, and polkit prompts.
- **No service-proxy layer.** Work runs as blocking closures and subprocesses
  (`ui.rs:121-134`, `storage.rs:124-…`). The proxies Settings needs already exist
  in `canopa-shell`, tied to the panel protocol: NetworkManager/iwd/networkd
  (`network_integration.rs`, `wifi_provider/backends.rs`, about 1.5k lines), BlueZ
  with its pairing agent (`hardware_panel/bluetooth.rs`, 822 lines), logind
  brightness and power-profiles-daemon (`hardware_panel/power.rs`), UPower
  (`dbus/upower.rs`), libpulse devices (`audio/devices.rs`) and WirePlumber
  (`dbus/wireplumber.rs`).
- **No single instance or activation interface.** A Shell panel can't open
  "Wi-Fi settings…" in the existing window. Two windows edit the same files and
  conflict.
- **i18n and a11y hooks are missing.** Translated labels would change focus IDs,
  and there is no AccessKit tree.

**Verdict:** Do the groundwork (**M**, about 2–3 weeks) before adding pages:
1. Create a `canopa-services` crate (or one module per service) with async zbus
   proxies and typed snapshots and streams. Move the Shell's NM, BlueZ, UPower,
   power-profiles-daemon, logind and pulse code into it so both Shell and Settings
   use it. Add `zbus_systemd`-style proxies for timedate1, locale1, hostname1 and login1.
2. Add a page registry: a trait or table with id, label, icon, keywords, group,
   view, update and subscription, plus `Draft` and `Live` page types. Derive
   navigation, deep links, Ctrl+N, search and launcher entries from it.
3. Add a scrollable, grouped sidebar with a search field that searches page
   keywords and section labels.
4. Add a single-instance `org.canopa.Settings` D-Bus service with
   `Activate(page, args)`. Map `canopa-settings <page>` to it.
5. Use stable, non-English focus IDs. Add Fluent (W5) and AccessKit through
   `canopa-ui` (W6) before the page count triples.

## Ranked gaps

Ranked by impact on a new user of a distribution that uses Canopa as its default
desktop. M1/M2 refer to the PRD milestones.

| # | Gap | Impact | Approach (service, crate) | Size | Depends on |
| --- | --- | --- | --- | --- | --- |
| 1 | **Settings groundwork**: service layer, page registry, live page type, navigation, single instance | Blocks every new page and deep link from Shell panels | See the verdict above. Extract the Shell's zbus/libpulse clients into a shared crate | M | – |
| 2 | **Network page** | Can't join WPA-Enterprise/eduroam, set up VPN, use wired 802.1X or static IP, share a hotspot or set a proxy without a terminal. This fails the W3 exit criterion | NetworkManager via zbus proxies (generated from NM introspection; `cosmic-dbus-networkmanager` as reference). Needs a `SecretAgent` for secrets, the VPN plugin import API (`.ovpn`/WireGuard `.conf`), and per-user connection permissions to avoid polkit prompts. Keep iwd as status-only fallback | L | 1 |
| 3 | **Sound page** | Can't pick HDMI vs analog or A2DP vs HFP profiles, control per-app volume or test the microphone | `libpulse-binding` (already in Shell, through pipewire-pulse) for sinks, sources, sink-inputs and card profiles. `pipewire-rs` later for level meters. Alert sound via Notify config | M | 1 |
| 4 | **Power page completion** | Timeouts can't be saved without Stasis. No lid or power-button choice, no battery details, no profiles in Settings, no brightness keys | UPower (battery, health), power-profiles-daemon (existing proxy), logind. For lid and power-button actions, take `handle-lid-switch`/`handle-power-key` inhibitor locks in the session and apply user policy (GNOME approach; `/etc/systemd/logind.conf` would need root). Decide whether Stasis is a hard dependency (PRD W3). Use duration pickers ("5 minutes", "Never"). Add `XF86MonBrightness*` binds plus a Shell OSD | M | 1, Stasis decision |
| 5 | **Accessibility of Settings + Accessibility page** | A blind user can't use Settings. No high contrast or cursor size. Third-party apps don't get contrast or reduced motion | AccessKit in `canopa-ui` (reuse `canopa-session-ui/src/accessibility.rs`). Portal: add `contrast` and `reduced-motion` keys (`canopa-portal/src/lib.rs:103-124`). High-contrast preset in `canopa-theme`. Cursor size through Niri `cursor {}` plus env/gsettings. Orca toggle via autostart | M–L | W6, 1 |
| 6 | **Default applications and autostart** | Can't set the default browser, mail or terminal. The shipped terminal binding is `foot` | Parse `MimeType=` in `canopa-apps`, write `~/.config/mimeapps.list` with the Snapshot pattern, use `xdg-terminal-exec` for the terminal binding. Autostart: list `~/.config/autostart` and XDG dirs, toggle with `Hidden=true`, include Background-portal entries | S–M | 1 (registry only) |
| 7 | **Date & time** | Can't change time zone or NTP after install. No 12/24 h choice | systemd-timedated (`SetTimezone`, `SetNTP`, `SetTime`, polkit) via zbus. Zone list from `timedated.ListTimezones` or tzdata. Automatic zone via geoclue later. Map 12/24 h to Shell and lock clock formats | S | 1, polkit agent (exists) |
| 8 | **Keyboard usability + shortcut editor** | Layout setup needs XKB jargon. Can't add a custom shortcut or record a key. Shortcut upgrade trap | Read `/usr/share/X11/xkb/rules/evdev.xml` (`quick-xml`) into a searchable layout and variant list with a switch-shortcut choice. Key capture widget. Allow added `spawn` bindings in `niri-custom.kdl`. Redesign the materialization (risk 1) | M | 1 |
| 9 | **Bluetooth page** | Can't forget a device, see battery or choose an audio profile | Reuse the Shell BlueZ code via the services crate, or `bluer`. `RemoveDevice`, `org.bluez.Battery1`, `Trusted`/`Blocked`. Audio profile through the Sound page | S–M | 1, 3 for profiles |
| 10 | **Region & language** | System language and formats are fixed at install | systemd-localed (`SetLocale`, `SetX11Keyboard`, polkit). Format locale via `LC_*` in `~/.config/locale.conf`. ICU4X previews | S (page) + W5 | 7's polkit pattern, W5 |
| 11 | **Appearance completion** | Can't change fonts, icon or cursor theme. Accent is a hex string. GTK3/Qt apps don't follow | Fonts: `fontconfig` crate or `fc-list` → typography in `config.kdl`. Icon/cursor: scan XDG icon dirs. Cursor → Niri `cursor {}`. Accent swatches. Theme-file picker. Publish fonts and cursor via portal `org.gnome.desktop.interface` keys or a gsettings bridge | M | 1 |
| 12 | **Notifications page** | DND only in a panel. Per-app rules and sounds are KDL-only | Edit `notifications { rule … }` in `config.kdl` with the Snapshot pattern plus `canopa-notify ctl reload`. App list from Notify history and `canopa-apps`. DND schedule | S–M | 1 |
| 13 | **Displays completion** | Arrangement needs arithmetic. No VRR or main-display choice. No night light | Drag canvas using the widget editor's preview code. Niri `OutputAction::Vrr` plus `variable-refresh-rate` in the include. Finer scale steps. `focus-at-startup`. Night light via Niri gamma or `wl-gammarelay-rs` | S–M | – |
| 14 | **Users** | Can't add a user, set an admin or change a password in a GUI | AccountsService via zbus (reuse `canopa-greeter/src/accounts.rs`). `CreateUser`, `SetAccountType`, `SetPassword` (with `crypt` hashing), avatar using greeter rules. Polkit | M | 1, polkit |
| 15 | **About** | No system info or hostname | hostnamed (polkit), `/etc/os-release`, sysfs/`sysinfo`. fwupd (zbus) for firmware later (W6/M3) | S (+M for fwupd) | 1 |
| 16 | **Privacy** | Lockdown switches need `busctl`. Can't review portal grants | Lockdown properties through the portal's D-Bus. `org.freedesktop.impl.portal.PermissionStore` (xdg-desktop-portal) for per-app grants. Background-app list | M | 1 |
| 17 | **Printers** | Can't add or manage a printer | CUPS via IPP (`ipp` crate) plus `cups-pk-helper` (polkit). mDNS discovery (`mdns-sd`). Or recommend `system-config-printer` for the first release | L | 1, polkit |
| 18 | **Search + translations** | Hard to find settings. Only English | Search derived from the registry's keywords. Fluent via `i18n-embed` (W5) | S + M | 1 |

## Bugs or risks found

1. **Shortcut editor upgrade trap (high).** The first save copies the shipped
   binds into `niri.kdl` (`shortcuts.rs:135-151`). The editor recognizes the
   defaults include only by byte equality with the defaults compiled into the
   binary (`shortcuts.rs:114`). It requires the materialized block to match the
   compiled defaults in length and action order (`shortcuts.rs:157-168`). After
   any package update that adds or changes a default binding (W3 brightness and
   OSD keys, for example), users who changed one shortcut never get the new
   bindings, and the Shortcuts page fails with "Custom top-level binding blocks
   are read-only." A mismatched `/usr/share/canopa/niri-defaults.kdl` during an
   upgrade is also read as a custom include. Fixed in 2ac9fc7: Settings stores
   only the changes, in `shortcuts.kdl`, and regenerates `niri-shortcuts.kdl` from
   the current shipped defaults at login. Legacy copies migrate with a backup
   (ADR 0135).
2. **"Reload and discard edits" discards every page.** It re-runs `State::boot()`
   (`ui.rs:371-377`), which drops drafts on all pages and the selected display profile.
   Fixed in bc71302: it reloads only the selected page.
3. **Pages that edit the same file conflict with each other.** Personalization and
   Widgets both edit `config.kdl` with separate snapshots. `ui.rs:729-731` clears
   the widget editor only when it isn't dirty. Saving one page makes the other's
   save fail as "changed outside this editor". The only fix offered is risk 2.
   Fixed in bc71302: each page rebases its pending edits on the other's save, and
   external edits are still detected.
4. **Power page can't save without Stasis.** `Save` requires `idle_available`
   (`ui.rs:618-622`, `:915-918`), and every page change resets that flag
   (`ui.rs:391`). If Stasis isn't installed or is stopped, the timeouts can't be
   saved even though the policy file exists. Fixed in 101c4ba: Stasis stays optional; Save writes the file without it.
5. **Editable defaults hide read-only state.** When a source fails to load or
   parse, the page shows default values as if they were editable
   (`ui.rs:174-195`). Only Save reveals that the page is read-only. `notice` keeps
   only the last startup error.
6. **Stale notices.** The `notice` text isn't cleared on page change
   (`ui.rs:389-402`), so an error from one page appears on another.
7. **Unchecked indexing.** `ui.rs:458-493` indexes `monitors[index]` and
   `positions[index]`. A refresh of outputs that changes the monitor count, queued
   with a click on a display card (hotplug during interaction), would panic. The
   probability is low.
8. **Startup panics.** `State::boot` panics if the config home can't be found
   (`ui.rs:156`). It also uses `expect` on embedded lock defaults (`ui.rs:172`).
9. **Silent theme reload failures.** Personalization ignores failures of the
   portal reload (`systemctl --user reload xdg-desktop-portal-canopa.service`) and
   the Notify reload (`ui.rs:676-687`). Third-party apps can keep the old theme
   with no feedback.
10. **Multiple windows.** Settings has no single instance. The display-profile
    notification and each launcher entry spawn another window (`profile_watch.rs:355-357`).
11. **Lost drafts.** Closing the window discards unsaved drafts without a prompt.
12. **Fragile motion override.** The compositor motion override is detected with
    `ends_with` (`niri.rs:109-124`). Anything appended after it later turns the
    control into a permanent "edited outside Settings" error.
13. **Duplicate mode labels.** Display modes are identified by their label
    (`ui.rs:462-466`, `:1480-1487`). Two modes with the same size and a refresh
    rate rounded to the same 2 decimals collapse into the first one.
14. **Symlinked configs are read-only everywhere.** This is by design
    (`storage.rs:31-34`) and affects users of stow or home-manager. It needs a
    clear explanation in the UI.
15. **Documentation drift.** `crates/canopa-settings/README.md` documents
    `--test backend`, but the only test target is `integration` (Cargo reports
    "no test target named `backend`"). `crates/canopa-notify/README.md` says the
    Shell applies appearance after a restart, while Shell ADR 0021 describes a
    live reload. The Settings README part is fixed in c992030.
16. **Thin default bindings.** `packaging/niri/defaults.kdl` has no brightness or
    media-player keys and no `Mod+1…9` workspace keys. `Mod+Return` runs `foot`
    and the focus-ring colors are hard-coded, not taken from the accent.
17. **Minor.** The Launcher page re-reads the catalog every 2 s (`ui.rs:801-803`).
    `storage.rs` uses `unsafe` `libc` calls (`geteuid`, `flock`) where `rustix`
    would fit W10's `forbid(unsafe_code)`.

## Hands-on checks needed

1. **Stasis dependency.** Stop Stasis (`systemctl --user stop canopa-idle.service`
   or the Stasis unit), run `canopa-settings --page power`, and change a timeout.
   Expect Save to stay disabled.
2. **Cross-page conflict.** Change the accent on Personalization (don't save), go
   to Widgets and move a bar widget, save Widgets, then save Personalization.
   Expect "changed outside this editor". Press Reload and confirm that the other
   page's edits are lost.
3. **Shortcut upgrade trap.** In a scratch `XDG_CONFIG_HOME`, run
   `canopa-settings init-session`, change one shortcut and save. Rebuild Settings
   with one extra bind added to `packaging/niri/defaults.kdl`, then open
   Shortcuts. Expect it to be read-only and the new bind to be missing.
4. **Screen reader.** Start Orca, open Settings and Tab through it. Expect no
   speech. Check `accerciser` for a missing `org.canopa.Settings` tree.
5. **Portal publication.** Toggle Light and save, then run
   `gdbus call --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop --method org.freedesktop.portal.Settings.ReadAll "['org.freedesktop.appearance']"`.
   Expect only `color-scheme` and `accent-color`. Then open a GTK3 app and a Qt5
   app and note whether they follow.
6. **Single instance.** With Settings open, run `canopa-settings --page power`.
   Expect a second window.
7. **Display arrangement.** With two monitors, change one monitor's scale and Test.
   Check whether the outputs overlap or leave a gap, and that positions need to be
   re-typed by hand. Confirm there is no VRR control.
8. **Enterprise Wi-Fi.** Try a WPA-Enterprise network from Wi-Fi quick settings.
   Expect "unsupported network".
9. **Hotplug while editing.** On Displays, plug or unplug a monitor while clicking
   its Enabled checkbox, to look for the panic in risk 7.
10. **Unsaved drafts.** Edit a keyboard repeat value, close the window, reopen.
    Expect the edit to be gone without a prompt.
11. **Keyboard only.** Use Tab, Shift+Tab, arrows, Ctrl+1…7, Ctrl+S and Esc on every
    page. Note any control focus can't reach, especially in the widget editor diagram.
