# Core-experience audit

Status: in-progress (Tier 1 and Tier 2 done 2026-11-11; Tier 3 open, item 5 done at the code level)

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
  - Accessibility trees cover only the Shell provider panels and Notify.
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

Workspace gate after both tiers: only Files `device_runtime` ("private device
runtime did not settle", pre-existing) and the flaky Files video playback test
(1 in 3 runs alone) fail. The 3 Notify layout tests now pass. None of this has
been run in a live session yet; use the hands-on checks in each report.

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
5. **Notify everyday controls:** DND schedule and timer, suppression over fullscreen and screen sharing, a Settings Notifications page, "mark all read", lock-screen policy. Also finish W15: portal v2 inline replies, `display-hint`, banner timer. **Done in Notify** (ADR 0055, commits above); the Settings page uses [the control API](../../docs/notify/settings-control-api.md) and is still to build. Not yet run in a live session.
6. **Launcher:** desktop actions (M), browse all apps (the empty query shows 20 rows), session actions searchable. **Done in 15fed5b, f286c32 and d768cb9**, with running-app switching, per-app scopes, Settings pages from the registry and in-process bundled providers.
7. **Settings:** default applications and date/time, then users, region, privacy and printers.
8. **Cross-cutting:**
   - AccessKit in `canopa-ui`, so Settings, the launcher and the bar get trees.
   - Fluent translations.
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
