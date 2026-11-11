# Notify core-experience audit

Status: needs-triage

Audited 2026-11-11 on `main` at `20e1bf5`. I verified claims against the code in
`crates/canopa-notify/`, `crates/canopa-notify-client/`, the Shell
(`crates/canopa-shell/src/modules/notifications.rs`, `app/module_graph.rs`,
`launcher/controls.rs`), Settings and packaging. Where docs and code disagree, the code wins.

Tests run (focused, no desktop):
- `bash scripts/cargo.sh test -p canopa-notify --lib`: **154 passed, 3 failed** (the three layout tests below).
- `dbus-run-session -- bash scripts/cargo.sh test -p canopa-notify --locked --test integration`: **278 passed, 0 failed**.
- No workspace check, desktop run or native acceptance was run. Focused success does not establish either.

## Summary

- The freedesktop 1.3 wire contract is solid and well tested. That covers the four methods, the signals, the close reasons, ordered `ActivationToken` → `ActionInvoked` → `NotificationClosed`, safe IDs, bounded input, markup, links, raw images and persistence. All 278 integration tests pass.
- Day-to-day use is weaker than the protocol layer. There is no DND schedule, no fullscreen or screen-share suppression, no lock-screen presentation, no Notifications page in Settings, no inline reply, no grouping and no translation. **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c): DND timer/schedule/break-through, fullscreen and sharing suppression, per-app sound and lock-screen rules, the lock-screen feed, portal inline replies and app grouping exist, with a control API for Settings. The Settings page itself and translation remain open.
- **Flatpak/GNotification banners never hide by themselves.** **Fixed in 4459406.** The portal backend sets `expire_timeout: Never` (`notification_server/portal/intake.rs:229`).
- **Notifications that never expire can exhaust fixed caps, and after that new notifications are refused.** **Fixed in 0bfe3e7.** Silenced, DND and overflow notifications stay live, and the live cap is 1,024 (`runtime_resources.rs:13`). Expired-but-unread notifications stay pending, and the pending cap is 4,096 (`sqlite_persisted_state.rs:846`). Past either cap, apps get `LimitsExceeded` errors.
- Visual interop gaps that users will see:
  - Only PNG/JPEG files that have a file extension are accepted (`presentation.rs:581`). SVG app icons and Chromium/Electron's extension-less temp images are probably lost.
  - The icon theme is fixed to `hicolor` (`configuration.rs:301`).
- Compatibility gaps with real clients:
  - Reusing a closed ID through `replaces_id` returns `InvalidArgs` (`notification_server/ids.rs:160`), which breaks `notify-send -r`/`dunstify -r` OSD scripts. **Fixed in 78cd3b6.**
  - `int:transient:1` and int32 urgency are silently ignored (`notification_server_intake.rs:77-104`). **Fixed in 78cd3b6.**
- The freedesktop-completion plan is complete for its selected static profile. Only W15 is open (native portal v2 and inline replies). Code has since moved past the plan: a v2 portal backend now exists, without replies, display hints or categories.
- The 3 failing layout tests are stale fixtures. Commit `2345fc5` deliberately shrank banners without updating these unit tests. Fix the tests, not the code. **Fixed in 7f5eddb.**

## Feature table

Statuses: **works**, **partial**, **missing**, **broken**, **rough**.

| Item | Status | Evidence | What a user notices |
| --- | --- | --- | --- |
| Capabilities | works | `notification_server.rs:29-75`; exact-set tests in `tests/dbus_contract.rs` | Advertises actions, action-icons, body, body-images, body-markup, icon-static and persistence. Adds body-hyperlinks, sound and activation-token only when available. Apps adapt correctly. |
| Actions and default action | works | `presentation.rs:501-511`, `presentation/renderer.rs:664-676` | Clicking the card runs `default`. All named actions become wrapping buttons. Without a default action, a click only dismisses (`renderer.rs:668`); GNOME would focus the app. |
| `body-markup`, hyperlinks | works | `intake.rs:98+`, compliance matrix; HTTP(S)-only typed links | b/i/u and `<a href>` render. Non-http(s) links stay plain text. Links need a working OpenURI portal. |
| `image-data` | works | `notification_server_intake.rs:128`, `domain.rs:491-538` | Firefox/Thunderbird/libnotify raw images show. |
| `image-path`, `app_icon`, body `<img>` | partial | `presentation.rs:412-425`, `:581-583` (absolute path **and** `.png/.jpg/.jpeg` extension), `renderer/images.rs:99,117` (PNG/JPEG only) | SVG icons (most `hicolor/scalable/apps`) fall back to an initial badge. Extension-less temp files (Chromium/Electron) are probably dropped. Needs a hands-on check. |
| Themed icon names | partial | `configuration.rs:301` defaults to `hicolor`; no desktop icon-theme tracking | Generic names (`dialog-information`, `mail-unread`, …) rarely exist in hicolor, so they show badges. |
| App icon/name fallback | rough | `presentation.rs:544-570`: uses the app identity as the icon name; never reads the `.desktop` file's `Icon=`/`Name=` | Portal apps show their reverse-DNS ID (e.g. `org.gnome.Fractal`) as the app label (`portal/intake.rs:213`). |
| Urgency levels | partial (**Fixed in 78cd3b6**: any integer width) | `notification_server_intake.rs:77-86`: only a `u8` urgency is honoured | `-u critical` works. Clients that send urgency as int32/uint32 get Normal, so their critical alerts time out and DND silences them. |
| `expire_timeout` -1/0/n | works | `domain.rs:175-193`; 5 s default for low/normal, critical never (`domain.rs:41-47`); timer starts at display (ADR 0028) | Behaves as expected. There is no hover-to-pause (no hover handling in `renderer.rs`). |
| `replaces_id` | rough (**Fixed in 78cd3b6**) | `notification_server/ids.rs:156-161`: an issued-but-closed ID → `InvalidArgs` | In-place replacement while the notification is live works. `notify-send -r 4242` volume/brightness scripts fail on the second call after the first one expires. |
| `CloseNotification`, reasons | works | `notification_server.rs:144-153`; reasons 1–4 tested in `tests/dbus_contract.rs` | — |
| `ActionInvoked`, `ActivationToken` | works | xdg-activation token from the click serial with fallback provider (compliance "Compatibility rules"); ordered publisher | Apps raise themselves correctly on Wayland. |
| `category` | partial | `notification_server_intake.rs:52`; only drives per-app/category rules (`delivery_policy.rs:45-56`) | No category-specific presentation (calls, alarms, im). |
| `desktop-entry` | works | `intake.rs:88-96` | Preferred source for app identity and rules. |
| `resident`, `transient` | partial (**Fixed in 78cd3b6**: integer or boolean) | `notification_server_intake.rs:55,96-104`: boolean only | `notify-send -e` works. The common `-h int:transient:1` is ignored, so the notification lands in history. |
| `sound-file`/`sound-name`/`suppress-sound` | works | `notification_server_intake.rs:56,106-110`, `runtime.rs:224` | Plays theme sounds. `suppress-sound` is honoured only as a boolean. |
| `x`/`y` | partial (by design) | `notification_server_intake.rs:113-120`, decoded and ignored | Fine on Wayland. |
| `action-icons` | works | `notification_server_intake.rs:39` | Only PNG/JPEG icons render (same limit as above). |
| Portal Notification v2 (Flatpak) | partial / rough (banner timeout **Fixed in 4459406**; display hints, purposes, SupportedOptions, replies and a banner-hide timer that keeps callbacks live **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c)) | Backend `org.freedesktop.impl.portal.desktop.canopa.notify`, `notification_server/portal.rs:424-468`, `packaging/system/canopa-portals.conf:3`. Gaps: `expire_timeout: Never` (`portal/intake.rs:229`); `display-hint` and button `purpose` ignored; `high` priority mapped to Normal (`:132`); SupportedOptions advertises no categories or purposes (`portal.rs:466`) | GTK4/libadwaita Flatpaks deliver with typed actions and survive app exit. **Every portal banner stays on screen until clicked or dismissed.** |
| GNotification (unsandboxed GTK) | works | GLib uses its freedesktop backend (no `org.gtk.Notifications` owner) | Ordinary freedesktop path. |
| KDE extras (`inline-reply` cap, `NotificationReplied`, `x-kde-*`, JobView) | missing | No matches in `crates/canopa-notify/src`; the compliance doc says `inline-reply` is not advertised | KDE apps fall back to plain notifications. No job-progress view. The SNI tray exists in the Shell (`crates/canopa-shell/src/dbus/sni_watcher.rs`). |
| Inline reply | missing (**Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c) for portal `im.reply-with-text`: banner composer, Center "Reply…"; KDE `inline-reply` still missing) | Issue `.scratch/notify/canopa-notify/issues/32-…` is ready-for-human; no reply widget; banner `KeyboardInteractivity::None` (`renderer.rs:105`) | Chat apps cannot offer reply-from-notification. |
| Notification Center and history | works / rough (Mark all read, Clear all with undo, grouping by app with collapse **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c)) | Provider `control_adapters.rs:415-545`, paging `control_adapters/pages.rs`; Shell launches it (`canopa-shell/src/app/module_graph.rs:115-130`); launcher entry `launcher/controls.rs:28` | Chronological reading list with unread dots, actions and links. No "mark all read"; "Clear active notifications" (`control_adapters.rs:540`) does **not** clear expired unread items. The badge (`modules/notifications.rs:65-85`) keeps growing. The only bulk fix is "Delete notification history". The title reads "Canopa Notify" (`:417`). |
| Do-not-disturb | works (timer, schedule, break-through, state in bar and Center **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c)) | `delivery_policy.rs:210`; Shell right-click toggle and quick-settings panel (`module_graph.rs:121-127`, `modules/notifications.rs:176-200`); persisted | Toggle only. |
| DND schedules / timed DND | missing (**Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c): `for`, `until`, `until-tomorrow`, daily schedule with DST-safe edges) | No schedule or "until" anywhere in `configuration.rs`/`control.rs` | No "for 1 hour" option and no night schedule. |
| Per-app rules | partial (mute/Center-only/sound/lock-screen per App Identity, configurable before an app notifies, control API **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c); the Settings page is pending) | Center Settings view: Show banners / Save quietly / Block (`control_adapters/pages.rs:177-179`), only for apps on the current page; KDL `rule` nodes (`examples/config.kdl`); no Notifications page in Settings (`canopa-settings/src/lib.rs:3-11`) | An app cannot be configured before it notifies. There is no per-app sound toggle and no per-app "hide content". |
| Critical in DND | works | `delivery_policy.rs:210`; sound plays (`tests/sound.rs:176`) | — |
| Critical on lock screen | missing (**Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c): read-only lock-screen feed rendered by Canopa Lock's `notifications` widget) | No notification code in `crates/canopa-session` (canopa-lock) | Alarms and calls are invisible while locked. |
| Lock-screen privacy | missing (private by omission; "App — New notification" by default, per-app `content`/`hidden`, portal lock hints **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c)) | ext-session-lock hides overlay banners; no `hide-content` policy | Nothing leaks, and nothing shows either. The timeout behaviour across a lock is unverified (see the hands-on checks). |
| Grouping / stacking | partial (Center grouping by app with collapse **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c); conversation grouping open) | At most 3 banners (`presentation.rs:739`), critical first, then a "+N more" pill (`renderer.rs:2120-2170`). No app or conversation grouping (issues 30/31 open; ADR 0052 is a flat list) | The pill is not clickable. Demoted or never-shown overflow notifications never get a banner later and never expire (ADR 0028). |
| Flood / rate limiting | rough (cap refusal **Fixed in 0bfe3e7**; no per-app cooldown) | No per-app cooldown (issue 29 open). Global live cap of 1,024 (`runtime_resources.rs:13-48`) includes silenced/DND/overflow threads, which never expire | A day of DND or a "Save quietly" app can exhaust the cap. **All** apps then get `LimitsExceeded`. |
| Long text | works | Summary/action wrapping (`renderer.rs:1745,1895`); 120 px body preview (`renderer.rs:14,2029`); internally scrollable oversized cards (ADR 0046) | — |
| Multi-monitor placement | partial | Compositor-selected output per new stack (`renderer.rs:85-108`, ADR 0047); fallback on output loss; evidence on Sway; not verified on physical multi-output Niri | Position is fixed top-right with no setting. |
| Fullscreen apps | missing (fullscreen and screen-sharing suppression through Niri IPC; content hidden while sharing **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c)) | Always on `Layer::Overlay` (`renderer.rs:99`); no fullscreen or screencast check | Banners cover fullscreen video, games and presentations, including during screen sharing. |
| Sounds | partial / rough (per-app sound off, quiet portal replacements **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c)) | Bounded rodio playback; default `message-new-instant`. A sound plays on **every** shown receipt, including replacements (`runtime.rs:224`, `tests/sound.rs:203` `shown_replacement_plays_sound_again`) | Progress or download updates chime repeatedly. The only control is a global KDL `sound enabled`. No volume or per-app setting. |
| Persistence across restarts | works | ADR 0049; pending attention survives; old sender actions are disabled (`tests/dbus_restart.rs`, `tests/history.rs`) | Transient notifications vanish as designed. |
| Crash recovery | partial | `Restart=on-failure` (`packaging/systemd/canopa-notify.service`). Renderer death is terminal until a manual restart (`presentation.rs:659`, ADR 0043) and is not exposed in the Shell summary (`canopa-notify-client/src/lib.rs:14-19`). No D-Bus activation file for `org.freedesktop.Notifications` (only for the portal) | If the renderer dies, banners stop silently and timeouts never start. |
| AccessKit / Orca | works / partial | Native AT-SPI tree; Alert role, Polite live region (`renderer/accessibility.rs:128-134`); Orca pass recorded in `docs/notify/notification-desktop-acceptance.md` | Critical notifications are announced politely, not assertively. |
| Keyboard interaction | partial | Banners never take focus (by design). The Center has a keyboard surface. No default shortcut to open the Center or dismiss the latest banner (`packaging/niri/defaults.kdl`) | Keyboard-only users need the launcher ("notifications") or a custom binding (`canopa-shell --toggle-menu`). |
| Translatability | missing | No gettext/fluent in the workspace. Hard-coded English (`renderer.rs:2128`, `control_adapters.rs:410,469,518,540`). Fixed `%Y-%m-%d · %H:%M` (`control_adapters.rs:406`) | Non-English users see English UI and ISO dates. |
| Theming | works | Shared `canopa-theme` appearance, live `reload-canopa`, `banners { ui … }` override (`configuration.rs:152-170`), text size scaling via Settings | Icon theme does not follow the desktop (see above). |

## Remaining freedesktop-completion work

Source: `.scratch/notify/freedesktop-completion-review/PLAN.md` and `IMPLEMENTATION.md`, checked against the code.

- **W01–W14: complete for the selected static profile.** Every acceptance box is checked (PLAN.md lines 82–295 and 336–343), and all 14 issue files say "complete". The code confirms it: the capability list is `notification_server.rs:29-37`, and the persistence, links and body-images paths are present and tested.
- **W15 (native portal v2 + inline replies): all four acceptance boxes unchecked** (PLAN.md:307-310). The plan calls it "optional, not selected". The PRD still says "Native portal v2 is not selected".
  - **The code has moved past this.** Root ADR 0031 added a Notify-owned v2 backend (`notification_server/portal.rs`, `version = 2`). It is advertised in `packaging/system/canopa-portals.conf`. ADR 0031's status is "shared validation pending".
  - What remains of W15 in code:
    - (a) inline replies (`im.reply-with-text` purpose, reply UI, reply parameter, never persisted); **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c).
    - (b) `display-hint` handling (`transient`, `tray`, `persistent`, `hide-on-lock-screen`, `hide-content-on-lock-screen`, `show-as-new`); **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c).
    - (c) advertising categories and button purposes in `SupportedOptions` (`portal.rs:466` lists only a Canopa extension key); **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c).
    - (d) a banner-hiding timer for portal notifications (ADR 0031 "Current boundary"); **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c); Center actions stay live after the banner hides.
    - (e) the final "full consumer, shared and core validation" that ADR 0031 records as pending. Still open: no real-frontend or native-session run.
- **Verification limits listed as not done** (PLAN.md:345-347, `notification-release-verification.md` "Explicit profile limits"):
  - abrupt migration/import interruption and SQLite busy injection (partly addressed later in `project-completion-verification.md`);
  - power-loss certification;
  - compositor occlusion feedback;
  - a complete scale/font/locale matrix;
  - physical multi-output Niri hotplug and zero-output recovery (IMPLEMENTATION.md:340).
- **Hosted CI gate is open.** There is no repository/runner and no run URLs (`docs/notify/project-completion-verification.md:146-151`).
- **Stale documentation to reconcile:**
  - PRD.md, PLAN.md and the compliance matrix still name the external Niribar host at `/home/kevinard/projects/niribar`. The host is now Canopa Shell in this workspace.
  - Compliance matrix line 210 says "existing single-line/empty action-row geometry is preserved". Commit `2345fc5` removed the empty action row.
  - IMPLEMENTATION.md's test counts (430/331) predate the consolidated `integration` target (278 cases) and the lib suite (157).
- **Backlog deferred by the plan** (`.scratch/notify/canopa-notify/issues/`):
  - 26 trustworthy app identity: ready-for-human;
  - 27 enforce resolved identity: ready-for-agent;
  - 29 burst cooldown: ready-for-agent;
  - 30/31 conversation grouping: ready-for-human / ready-for-agent;
  - 32 portal inline replies: ready-for-human;
  - 33 snooze: ready-for-agent.

## Layout test diagnosis

**Verdict: the tests are wrong (stale fixtures). The code change was intentional.** **Fixed in 7f5eddb** (tests measure natural heights; the oversized card measured 324 px).

Commit `2345fc5` ("Polish Canopa desktop components", 2026-11-08) changed `notification_banner_content` in `presentation/renderer.rs` (around lines 1721-1806):
- It dropped the fixed 28 px placeholder action row for banners without actions, which also removes one 8 px card gap (Notification `gap` = 8, `canopa-theme/src/customization.rs:383`).
- It moved the app name above the title and shows it only when it differs from the title.
- It raised content column spacing from 1 to 3 px.

The commit message says native acceptance fixtures were updated, but these unit tests were not.

Arithmetic check on the fixture `banner_view(_, "Queued")` (summary "Download", app name "Downloader", no actions):
- old height = 132 px;
- new height = 132 − 28 (empty action row) − 8 (card gap) + 4 (two content gaps 1→3 px) = **100 px**.
- This matches the observed `[…, 276, 100]` and `[…, 360, 100]`.

1. `queued_input_regions_use_the_latest_viewport_and_remain_empty_after_clear` (layout.rs:291) expects a 132 px region height. The actual is 100.
2. `late_layout_from_lost_output_cannot_restore_its_input_region` (layout.rs:1342) has the same stale 132.
3. `oversized_card_is_bounded_and_acknowledged_without_losing_natural_content` (layout.rs:1614): the card has actions, so only the +4 px spacing applies. Its natural height grew from about 320 px to about 324 px. The "grown" viewport of 320 px no longer fits the card, so the clamped scroll offset is correctly 4.0, not 0.0.
   - The region assertion `<= 320` still passes only because the region is clamped to the viewport.
   - I inferred the 324 px figure from the 4.0 px offset. Confirm it by running the test after the edit.

The behaviour under test is intact: latest-viewport regions, stale-callback rejection and scroll clamping all work. Only the hard-coded geometry is outdated. All other 21 layout tests and the 278 integration tests pass.

Proposed fix (not applied), in `crates/canopa-notify/src/presentation/renderer/tests/layout.rs`:
- **Minimal:**
  - Replace `132` with `100` at lines 293, 299, 1346 and 1632. Line 1632 is the replacement assertion in test 3, which the current failure never reaches.
  - In test 3, grow the viewport to 360 instead of 320 at lines 1601, 1615 and 1624, and change the bound at line 1607 to `<= 360`.
- **Better (prevents the next theme tweak from breaking these):**
  - Add a helper that lays out `notification_banner_widget(&banner_view(42, "Queued"), &Default::default())` at the given width and returns its natural height. Assert regions against that value.
  - In test 3, read the natural content height from `ScrollBounds` in the 150 px frame. Grow the viewport to `content.height.ceil()`, and assert region height == natural height and offset == 0.
- Also update the compliance-matrix sentence about preserved empty action-row geometry (`docs/notify/freedesktop-notification-compliance.md:210`).

## Ranked gaps

Ranked by impact on a distro default where users bring their own apps.

1. **Portal banners never auto-hide** (`portal/intake.rs:229`, ADR 0031). **Fixed in 4459406** (server-default timeout; urgent stays). `display-hint` and the banner-hide timer that keeps callbacks live **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c); `high` → sound remains open. Every GTK4/libadwaita Flatpak notification stays on screen. *Approach:* keep sender callbacks live, but give portal banners the server-default banner timeout through a banner-visibility timer separate from the sender lifecycle. Honour `display-hint` `persistent`/`transient` and map `high` → Normal + sound. **M**
2. **Never-expiring notifications exhaust the caps and new notifications are then refused** (`runtime_resources.rs:13,47`; `sqlite_persisted_state.rs:842-853`). **Fixed in 0bfe3e7** (ADR 0054; no Center warning yet). *Approach:*
   - Let silenced, DND and overflow threads finish their sender lifecycle after the default timeout, or stop counting them as live (convert them to retained-only), while preserving attention.
   - At the 4,096 pending cap, auto-acknowledge or archive the oldest pending item instead of rejecting new receipts.
   - Show a warning in the Center.

   **M**
3. **Image and icon interop** (`presentation.rs:581`, `renderer/images.rs:99,117`, `configuration.rs:301`). *Approach:*
   - Detect the format from file content instead of the extension.
   - Enable the SVG rasterizer that `canopa-assets` already provides (it is already used for portal icons, `portal/intake.rs:357`).
   - Take the icon theme from the shared appearance or GTK settings, with hicolor and Adwaita as fallbacks.
   - Resolve `Icon=`/`Name=` from the desktop entry through `canopa-apps`.

   **S–M**
4. **No everyday notification controls:** **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c), except the Settings page, which another agent builds on the documented control API.
   - DND schedule or timer;
   - automatic DND while fullscreen or screen sharing;
   - a Settings → Notifications page (app list incl. not-yet-seen apps, banners/sound/lock-screen per app, banner position);
   - "mark all read".

   *Approach:* add a Settings page that writes the existing user-rule store and KDL. Add DND `until`/schedule to the policy state. Get fullscreen state from Niri IPC (or the portal's screencast state). **M–L**
5. **`replaces_id` on a retired ID returns an error** (`ids.rs:160`). **Fixed in 78cd3b6** (the closed token is reused). This breaks OSD scripts and clients that keep stale IDs. *Approach:* treat a retired or unknown ID as a new notification. Return a fresh ID, or reuse the token while it is not live, matching dunst/mako/GNOME. Keep the collision protections. **S**
6. **Inline reply missing** (portal `im.reply-with-text` and KDE `inline-reply`/`NotificationReplied`). Portal replies **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c); KDE remains open. *Approach:* on demand, give the banner a keyboard-interactive text field (`OnDemand` interactivity), route the reply through portal ActionInvoked / KDE signal, and never persist it. Issue 32 decision first. **L**
7. **Lenient hint types** (`notification_server_intake.rs:77-104`). **Fixed in 78cd3b6.** *Approach:* accept any integer type for urgency and `value`, and int or boolean for `transient`/`resident`/`suppress-sound`. **S**
8. **Sound on every replacement** (`runtime.rs:224`). Per-app mute and quiet portal replacements **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c); freedesktop replacements still chime, and there is no volume setting. *Approach:* play only for a new thread, or for a replacement that explicitly carries `sound-*` or a changed urgency. Add a per-app mute and a volume setting. **S**
9. **Unread badge grows forever; "Clear active" does not clear it** (`control_adapters.rs:540`, `lifecycle.rs:505-512`). **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c): Mark all read and Clear all with undo. *Approach:* add "Mark all as read" and per-item "Remove". Make "Clear" acknowledge everything visible. **S**
10. **No lock-screen story** (critical/alarm/call on the lock screen, privacy options). **Fixed in Tier 3 item 5** (ADR 0055; 77cc95e…0c7ba7c): read-only feed and Canopa Lock widget. *Approach:* a minimal lock-screen list in canopa-lock that reads Notify's bounded summary, with a content-hiding option. **M–L**
11. **Overflow UX**: the 4th and later notifications never get a banner and never expire, and the pill is inert (`presentation.rs:735-747`, `renderer.rs:2120`). **Expiry fixed in 0bfe3e7**; the pill is still inert. *Approach:* promote overflow notifications when a slot frees, or expire them silently. Make the pill open the Center. **S–M**
12. **Translatability** is missing workspace-wide. *Approach:* adopt gettext or fluent across the workspace and use locale-aware time formatting (relative times in the Center). **L**
13. **Renderer death is silent and terminal** (`presentation.rs:659`). *Approach:* exit non-zero (systemd restarts the service) or reconnect in-process. Expose renderer health in `canopa-notify-client::Summary` so the bar shows a warning. **S–M**
14. **No grouping or burst cooldown** (issues 29–31). **M**
15. **Polish:**
    - hover-to-pause expiry;
    - a default shortcut for the Center;
    - assertive announcement for critical notifications;
    - clicking a card without a default action focuses or launches the app instead of dismissing;
    - Center title "Notifications" instead of "Canopa Notify".

    **S each**

## Bugs or risks found

- **Hard failure mode by policy:** **Fixed in 0bfe3e7.** `LimitsExceeded` after 1,024 live or 4,096 pending notifications applies to every app, not just the noisy one (see gap 2). DND, "Save quietly" and overflow threads never expire (ADR 0028). This is probably the most likely way a long-running session ends up with "notifications stopped working".
- **Stale unit tests on main** (layout diagnosis above). **Fixed in 7f5eddb.** `cargo test -p canopa-notify --lib` is red, so CI-style checks fail until the fixtures are updated.
- **Probable Chromium/Electron image loss.** Chromium writes notification images to extension-less temp files. The extension check at `presentation.rs:581-583` rejects them, even though the decoder could sniff the content. Needs a hands-on check.
- **`replaces_id` reuse error** **Fixed in 78cd3b6** (`ids.rs:160`) deviates from what other servers do. The spec leaves the case open, but users experience it as "the notification didn't appear".
- **Urgency sent as int32 silently becomes Normal.** **Fixed in 78cd3b6.** For a client that does this, a "critical" alert expires and DND suppresses it.
- **Portal `bytes` icons are rejected** (`portal/intake.rs:296-304` accepts only `themed`/`file-descriptor`). If the frontend fork passes deprecated `bytes` icons through to the backend instead of converting them to a sealed fd, the whole `AddNotification` call fails with InvalidArgs. Verify with a v1-style GNotification Flatpak.
- **No D-Bus activation for `org.freedesktop.Notifications`.** Notifications sent before the unit is up, or during a restart, fail. If another daemon (mako/dunst) is installed, it can win the name through its own activation file. Ship `org.freedesktop.Notifications.service` with `SystemdService=canopa-notify.service`. Size S.
- **Timeouts across a lock or screen-off depend on compositor frame callbacks** (display-based timers). This is untested. Banners may silently expire behind the lock surface, or wait correctly until unlock.

## Hands-on checks needed

Run these in a Canopa session. `ctl` means `canopa-notify ctl`.

1. Baseline: `notify-send "Hello" "World"`. Expect a top-right banner that disappears after about 5 s, an unread dot in the Center and the bar count +1.
2. Critical through DND:
   ```
   ctl dnd on
   notify-send -u critical "Critical" "shows"
   notify-send "Normal" "quiet"
   ctl dnd off
   ```
   Expect only the critical banner, with sound.
3. int32 urgency:
   ```
   gdbus call --session --dest org.freedesktop.Notifications \
     --object-path /org/freedesktop/Notifications \
     --method org.freedesktop.Notifications.Notify \
     t 0 "" "Int urgency" "critical?" "[]" "{'urgency': <int32 2>}" -1
   ```
   Expected today: it behaves as Normal.
4. Actions: `notify-send -A default=Open -A reply=Reply -A archive=Archive "Mail" "Pick"`. Click the card body, then a button. The printed key must match.
5. Retired ID: `notify-send -r 4242 "Vol" "10%"; sleep 7; notify-send -r 4242 "Vol" "20%"`. Expected today: the second call errors.
6. Progress chimes:
   ```
   id=$(notify-send -p -h int:value:0 "Download" "0%")
   for i in 20 40 60; do
     notify-send -r "$id" -h int:value:$i "Download" "$i%"
     sleep 1
   done
   ```
   Count the sounds.
7. int transient: `notify-send -h int:transient:1 "Transient?" "x"`, then open the Center after it expires. Expected today: it is retained.
8. Images:
   ```
   cp /usr/share/icons/hicolor/48x48/apps/*.png /tmp/noext 2>/dev/null
   notify-send -h string:image-path:file:///tmp/noext "No extension" "image?"
   ```
   Also try `notify-send -i org.gnome.Nautilus "SVG icon" "x"` and `notify-send -i dialog-information "Generic" "x"`.
9. Chromium/Electron: allow notifications at a web-notification test page in Chromium (or trigger one in Slack/Discord/VS Code). Check whether the site image or icon appears.
10. Flatpak portal: with any GNOME-runtime Flatpak installed:
    ```
    flatpak run --command=gdbus <app-id> call --session \
      --dest org.freedesktop.portal.Desktop \
      --object-path /org/freedesktop/portal/desktop \
      --method org.freedesktop.portal.Notification.AddNotification \
      t1 "{'title': <'Portal'>, 'body': <'Does this hide?'>}"
    ```
    Wait 30 s. Also check the app label shown.
11. Flood and caps: `for i in $(seq 1 30); do notify-send "Flood $i" "b"; done`. Check for 3 banners plus "+N more", click the pill, and check `ctl status` / `ctl active --json`. Then repeat with `ctl dnd on` and watch the live count stay up after DND is turned off.
12. Fullscreen: run `mpv --fs <video>` (or press Mod+Shift+F on a browser) and send a notification. Note whether it covers the video. Repeat while screen sharing.
13. Lock: run `sleep 3; notify-send "While locked" "x"` in one terminal and press Mod+L immediately. After 10 s, unlock and check whether the banner is still visible or has already expired.
14. Multi-monitor: focus each output and send a notification. Unplug the output that shows a banner, then confirm that the banner moves to the other output and that new banners still appear (`ctl status` shows no renderer error).
15. Restart and crash:
    ```
    notify-send -u critical "Persist" "x"
    systemctl --user restart canopa-notify
    ```
    Open the Center: the item should be present with no stale action buttons. Then run `systemctl --user kill -s KILL canopa-notify` and confirm systemd restarts the service. Also check `ls /usr/share/dbus-1/services | grep -i notif` for competing daemons.
16. Orca: start `orca` and send a normal and a critical notification. Note whether each is announced and whether the critical one interrupts. Navigate the Center by keyboard (Tab/arrows/Enter/Escape).

## Tier 3 item 5 hands-on checks (ADR 0055)

Not yet run in a live session. `ctl` means `canopa-notify ctl`.

17. Timer: `ctl dnd on --for 1`, then `notify-send "Quiet" "x"` (no banner) and
    `notify-send -u critical "Loud" "x"` (banner). The bar tooltip says "Until HH:MM";
    after a minute it returns to "DND off" without a command.
18. Schedule: `ctl dnd-schedule 22:00 07:00 --alarms --calls`, `ctl preferences`.
    Then `notify-send -h string:category:alarm.ringing "Alarm" "x"` during the window
    shows a banner; `-h string:category:im.received` does not.
19. Fullscreen: `mpv --fs <video>` and `notify-send "Hidden" "x"` (no banner, in the
    Center); `notify-send -u critical "Shown" "x"` shows. Repeat with a maximized,
    non-fullscreen window: banners show.
20. Sharing: start a screencast (OBS, or a browser's screen share through the Canopa
    portal). Visible banners switch to "New notification"; normal ones go to the
    Center quietly; critical ones show without content. Check `ctl status --json`
    has no error, and `niri msg casts` lists the stream.
21. Per-app: `ctl app notify-send --delivery silence`, `ctl app Firefox --sound false`;
    send from each.
22. Center: send three notifications from two apps, open the Center, press
    "Group by app", collapse one, then "Mark all read" (badge 0, live items stay),
    "Clear all" and "Undo clear" within 30 s.
23. Lock screen: `notify-send -u critical "Bank" "Code 1234"`, lock (Mod+L): the lock
    screen shows "notify-send — New notification". `ctl app notify-send
    --lock-screen content` shows the summary; `hidden` shows nothing.
24. Portal reply (needs Canopa's Notification backend selected): from a GNOME-runtime
    Flatpak shell,
    ```
    gdbus call --session --dest org.freedesktop.portal.Desktop \
      --object-path /org/freedesktop/portal/desktop \
      --method org.freedesktop.portal.Notification.AddNotification chat \
      "{'title': <'Alice'>, 'category': <'im.received'>, 'buttons': <[{'purpose': <'im.reply-with-text'>, 'action': <'reply'>}]>}"
    gdbus monitor --session --dest org.freedesktop.portal.Desktop
    ```
    Click Reply, type, press Enter: `ActionInvoked` carries `('chat', 'reply',
    [<{'activation-token': …}>, <'text'>])`. Let the banner hide, then use "Reply…" in
    the Center. Check `SupportedOptions` with `busctl --user get-property
    org.freedesktop.impl.portal.desktop.canopa.notify /org/freedesktop/portal/desktop
    org.freedesktop.impl.portal.Notification SupportedOptions`.
25. Banner-hide timer: a portal notification hides after about 5 s but stays in the
    Center with its buttons; clicking one still reaches the application.
