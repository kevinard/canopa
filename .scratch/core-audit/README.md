# Core-experience audit

Status: ready-for-human (Tiers 1–3 done at the code level 2026-11-11; hands-on checks pending)

Code-reading audit, 2026-11-11, of the five components Canopa owns: Shell,
launcher, Settings, Files and Notify. Nothing was run in a desktop session;
each report ends with hands-on checks for the maintainer. Per-component
detail, `file:line` evidence and sizes:

- [shell.md](shell.md)
- [launcher.md](launcher.md)
- [settings.md](settings.md)
- [files.md](files.md)
- [notify.md](notify.md)

## Overall picture

- **Strong:**
  - the Notify freedesktop protocol layer (278 integration tests pass);
  - Files' views, tabs, Quick Look, crash-safe Undo and device mounting;
  - the Shell's bar and providers;
  - launching apps through Niri.
- **Weak:** the everyday controls around those cores.
  - Settings has 7 pages and none of them talks to a system service.
  - Network, sound, Bluetooth and brightness exist only as Shell quick settings.
  - The default bar hides battery, network and volume.
- **Cross-cutting:**
  - No component is translatable.
    **Fixed (Tier 3.8 translations):** Fluent through `canopa-i18n` in every component, a French translation (machine-assisted, needs review), ICU4X formatting, RTL mirroring and a literal lint, ADR 0145.
  - Accessibility trees cover only the Shell provider panels and Notify.
    **Fixed (Tier 3.8 accessibility):** shared AccessKit layer, ADR 0144.
  - Icon lookup is hicolor-only in both the launcher and Notify, and Files has 8
    extension-based icons.

## Progress (2026-11-11)

Tier 1 and Tier 2 are done. The per-component reports mark each fixed row with
its commit. Commits by area:

| Area | Commits | What changed |
| --- | --- | --- |
| Tier 1 Shell | `174ef4d`, `6c1a838`, `f703c23` | Config errors keep the Shell running; logind session actions; default bar |
| Tier 1 launcher | `ce15a6d`, `e48a85b`, `40bbd51` | Unicode and accent matching; terminal apps and `TryExec`; launch errors |
| Tier 1 Files | `de6c744`, `14291e6`, `c3b9ad5` | Live folder refresh; Delete permanently and Empty Trash |
| Tier 1 Settings | `2ac9fc7`, `bc71302` | Shortcut deltas over the shipped defaults; page-scoped reload and shared drafts |
| Tier 1 Notify | `7f5eddb`, `78cd3b6`, `4459406`, `0bfe3e7` | Layout tests; `replaces_id` and hint types; portal timeouts; bounded retention |
| Packaging | `c6ebeed` | Restart limits on the Shell and Notify units; depend on `xdg-terminal-exec` |
| A | `ebb5471`..`c285d03` | `canopa-services` crate (ADR 0137) |
| B | `a0e07dd`..`28272f8` | Settings page registry, file and live page patterns, single instance and D-Bus (ADR 0136) |
| C | `6c4d1f3`..`5675085` | Shared icon-theme and MIME lookup (ADR 0138) |
| D | `b33521c`..`9825b5b` | On-screen indicators and brightness, media and keyboard-backlight keys (Shell ADR 0025) |
| E | `e489cad` | Fork `kevinard/exwlshelleventloop` `fecb4f1a`: keyboard serial for activation tokens |
| Tier 3 #5 Notify | `77cc95e`, `a46341e`, `e9251c3`, `b238418`, `37c919a`, `0c7ba7c` | DND timer/schedule/break-through, fullscreen and sharing suppression, per-app rules and control API, Center bulk controls and grouping, lock-screen feed, portal v2 hints/purposes/replies/banner-hide timer (Notify ADR 0055) |
| Tier 3.1 Network | `50a3626`, `77f2b41`, `3861301` | Settings Network page, NetworkManager settings client and secret agent (ADR 0139), Shell Wi-Fi off/disconnect/forget and wired/VPN state |
| Tier 3.2 Sound | `43b9907`, `1e58759` | Settings Sound page: devices, balance, applications, card profiles, level meter, notification sounds |
| Tier 3.3 Power | `53776d4`, `101c4ba` | Settings Power page: battery health, power modes, perceptual brightness, lid and power button (ADR 0140), idle timeouts without Stasis |
| Tier 3.8 Accessibility | `924d5ed`, `c1cdf06`, `78eb2d7`, `e2dd8a7`, `e04192e`, `099c90a`, `76c0460`…`e70f99d`, `1bea6f7`…`64137dc`, `7cc930a`…`3ba710c` | Shared AccessKit layer in `canopa-ui` (ADR 0144): Settings with stable IDs, launcher, Shell bar/Quick settings/menus/tray, Files, portal FileChooser/AppChooser/Access and five more dialogs; high-contrast palette, portal `contrast`/`reduced-motion`, Settings Accessibility page with cursor size; assertive critical banners. Not yet run with Orca |
| Tier 3.8 Translations | `7d168b6`…`0eb1c1a8` | `canopa-i18n` (Fluent via i18n-embed, ICU4X dates/numbers/sizes, pseudo-locale), `canopa_ui::direction` RTL mirroring, `scripts/check-i18n.py` lint, every component converted with a French translation (machine-assisted, needs review), translated `.desktop` entries, locale-following clocks and Shell calendar, Settings live language switch (ADR 0145). Not yet reviewed by a French speaker nor run in a live session |
| Tier 3.7 Settings pages | `c8d9e06`, `f82cffa`, `e7c5f15`, `ef645c5`, `f1abe0d`, `53a8820`, `27fa4ad`, `97cbb3f`, `03a9256` | Bluetooth, Notifications, Default applications, Date & time (ADR 0142), About, Region & language, Users (ADR 0143), Privacy, Printers; not yet run in a live session |

Workspace gate after both tiers: only Files `device_runtime` ("private device
runtime did not settle", pre-existing) and the flaky Files video playback test
(1 in 3 runs alone) fail. The 3 Notify layout tests now pass. None of this has
been run in a live session yet; use the hands-on checks in each report.

### Tier 3 (2026-11-11)

All Tier 3 items are implemented; each audit file marks its rows with commits.

| Item | What landed | ADRs |
| --- | --- | --- |
| 1–3 Settings Network, Sound, Power | Live pages over NetworkManager (Enterprise Wi-Fi, VPN import, hotspot, proxy), PipeWire-pulse (per-app volume, profiles, meter), UPower/power-profiles-daemon/logind (battery, lid, power button, idle without Stasis) | 0139, 0140 |
| 4 Files | Open With and `mimeapps.list`; Run prompts and `.desktop` trust; sandboxed thumbnails; batched Copy durability (100 small files 16.1 s to 0.8 s) with totals and ETA | Files 0165–0167, root 0141 |
| 5 Notify | Do-not-disturb timer and schedule, fullscreen and sharing suppression, per-app rules, Center bulk actions, lock-screen feed, portal v2 replies and display hints | Notify 0055 |
| 6 Launcher | Desktop actions, browse all apps, session and Settings pages in search, switch to running apps, per-app scopes | — |
| 7 Settings | Bluetooth, Notifications, Default apps, Date & time, About, Region & language, Users, Privacy, Printers | 0142, 0143 |
| 8 Cross-cutting | Files network locations through GVFS; AT-SPI trees for Settings, the launcher, Files, the Shell bar and menus, and 8 portal dialogs; Fluent and ICU4X translations with a complete French catalog (machine-assisted, needs review) and an RTL pseudo-locale | Files 0168, 0144, 0145 |

Workspace gate on the final tree: everything passes except Files
`device_runtime`, which already failed before this work. It sees the machine's
own USB mounts. The Files video playback test failed deterministically for a
while on an incremental build and passed on the clean rebuild; keep watching
it. Nothing has been run in a live session: use the hands-on checks in each
report.

## Ranked plan

### Tier 1: small fixes users hit every day

| # | Component | Problem | Size |
| --- | --- | --- | --- |
| 1 | Shell | Any `config.kdl` error at startup exits the Shell, leaving no bar or launcher; it restarts every 3 s. A rejected live reload only logs. | S–M |
| 2 | Notify | Flatpak (portal) notifications never time out, so banners stay until dismissed. | S |
| 3 | Notify | Silenced and never-shown notifications never expire. Past the 1,024 and 4,096 limits, every app gets `LimitsExceeded`. | S–M |
| 4 | Notify | Replacing an expired ID returns `InvalidArgs`, so `notify-send -r` OSD scripts fail. `transient` and int32 `urgency` hints are ignored. | S |
| 5 | Shell | No Suspend. Reboot and Power off run `systemctl` with a 3 s kill and ignore the result. Use logind over D-Bus. | M |
| 6 | Shell | Default bar has no battery, network, volume or tray, and the tray is off unless added. | S |
| 7 | Launcher | Matching lowercases ASCII only, so non-Latin and accented names don't match. Use Unicode case folding or `nucleo-matcher`. | S |
| 8 | Launcher | `Terminal=true` apps are hidden on purpose, and `TryExec` is ignored. Broken `Exec` lines fail silently. | S |
| 9 | Files | Folders never refresh on outside changes. Wire the existing `canopa_file::watch` inotify watcher. | M |
| 10 | Files | No Empty Trash or Delete permanently; on volumes without a trash folder a file can't be deleted at all. | S–M |
| 11 | Settings | The first shortcut save copies the shipped bindings into the user's `niri.kdl`. Those users stop receiving new bindings and the page turns read-only. | S–M |
| 12 | Settings | "Reload and discard" drops every page's edits. Personalization and Widgets block each other's saves on `config.kdl`. | S |
| 13 | Notify | The 3 failing layout tests are stale expectations after commit `2345fc5`, not a code bug. | S |

### Tier 2: groundwork that unlocks the missing features

| # | Work | Unlocks | Size |
| --- | --- | --- | --- |
| A | Move the zbus and libpulse clients (NetworkManager, BlueZ, UPower, power-profiles-daemon, logind, audio) out of `canopa-shell` into a shared crate. | Settings pages, Shell session actions | M |
| B | Settings architecture: page registry, shared page patterns (live service pages, not only edit-and-save), single instance with a D-Bus entry point and deep links, scalable navigation. | 15 pages | M |
| C | One shared freedesktop icon-theme and MIME lookup in `canopa-ui`/`canopa-file`. Use the user's theme, SVG and absolute `Icon=` paths, and shared-mime-info detection. | Launcher icons, Notify icons, Files file types and thumbnails | M |
| D | Shell on-screen indicators plus brightness, media and keyboard-backlight key bindings, through logind `SetBrightness` and MPRIS. **Fixed in e9f8f9c and 004f775** (Shell ADR 0025). | Daily laptop use | M |
| E | Fix the layershellev keyboard serial for activation tokens, in the fork's activation-token topic. | Keyboard activation in the Notification Center, raising running apps from the launcher | S |

### Tier 3: features, in dependency order

1. **Settings Network** (L): NetworkManager proxies, a secret agent (WPA-Enterprise is refused today), VPN import, wired, hotspot. Shell quick settings also gain Wi-Fi off, disconnect, forget, and wired/VPN state.
2. **Settings Sound** (M): per-app volume, card profiles, microphone meter, using `libpulse-binding`.
3. **Settings Power** (M): battery, profiles, lid and power-button actions. Decide whether Stasis is a hard dependency.
4. **Files Open With and executables** (M), **thumbnails** (M–L, after C), **copy throughput and ETA** (L engine, S UI).
5. **Notify everyday controls:** DND schedule and timer, suppression over fullscreen and screen sharing, a Settings Notifications page, "mark all read", lock-screen policy. Also finish W15: portal v2 inline replies, `display-hint`, banner timer. **Done in Notify** (ADR 0055, commits above); the Settings page over [the control API](../../docs/notify/settings-control-api.md) is in f82cffa. Not yet run in a live session.
6. **Launcher:** desktop actions (M), browse all apps (the empty query shows 20 rows), session actions searchable. **Done in 15fed5b, f286c32 and d768cb9**, with running-app switching, per-app scopes, Settings pages from the registry and in-process bundled providers.
7. **Settings:** default applications and date/time, then users, region, privacy and printers. **Done** (with Bluetooth, Notifications and About): see the Tier 3.7 row above and the Settings audit rows.
8. **Cross-cutting:**
   - AccessKit in `canopa-ui`, so Settings, the launcher and the bar get trees.
     **Done:** see the Tier 3.8 row in Progress.
   - Fluent translations.
     **Done:** see the Tier 3.8 translations row in Progress.
   - Network locations in Files through GVFS.

## Suggested first batch

Tier 1 items 1–13 are mostly S and independent, so they can run in parallel.
Then come Tier 2 A and B, which start the Settings pages, and C and D.

## Hands-on checks

Each report lists short steps to run on the real session: 15 for Shell, 14 for
the launcher, 11 for Settings, 12 for Files and 16 for Notify. The checks that
settle open questions in this ranking:
- **Notify:** a Chromium or Electron notification with an image (temp file with
  no extension).
- **Notify:** `notify-send -r` repeated after expiry.
- **Launcher:** open it with an empty query and check whether all apps can be
  browsed.
- **Files:** copy 100 small files and one large file to the internal disk and to
  a USB stick, and time both.
- **Shell:** start it with a deliberate `config.kdl` typo.
- **Shell:** run Power off while an app holds an inhibitor.
