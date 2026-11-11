# Launcher core-experience audit

Status: needs-triage

Audit date: 2026-11-11, at `20e1bf5`. This was a read-only review of the code,
the `canopa/main` forks of Niri and exwlshelleventloop (`1de59e6`, the same
revision as the Cargo git checkout), and the launcher docs and `.scratch` notes.
`.scratch/launcher-capture/` is empty. The only commands run were the focused
Shell test builds:

- `bash scripts/cargo.sh test -p canopa-shell --lib launcher`: 0 matching tests.
- `bash scripts/cargo.sh test -p canopa-shell --bin canopa-shell launcher`: 42 passed.

The desktop was not run.

Sizes: **S** is under a week, **M** is 1–3 weeks, **L** is more than 3 weeks of
focused work. Paths without a crate prefix are under `crates/canopa-shell/src/`.

## Summary

- The launcher covers a lot. It searches apps, windows, files, Settings
  controls, the calculator and unit conversion (qalc), web and date, emoji,
  clipboard history and snippets, and configured extensions, with pins,
  aliases, frecency and an action menu. The docs match the code, and the
  session/revision model is solid (42 focused tests pass).
- **Search breaks for non-Latin scripts.** App, window and control matching
  lowercases only ASCII (`app/panel_search.rs:30,100`). A Russian user typing
  `терм` won't find "Терминал", and there is no accent folding.
- **The xdg-activation bug is real but doesn't affect normal launches.**
  Exec apps go through Niri's `Spawn` action, which gives them its own token
  (`XDG_ACTIVATION_TOKEN`/`DESKTOP_STARTUP_ID`). D-Bus-activated apps and
  "Reveal in file manager" get **no token**. The layershellev keyboard-serial
  gap blocks the obvious fix, and it hits keyboard activation in Notification
  Center today.
- **Missing desktop-entry features:** `Terminal=true` apps are hidden on
  purpose (`canopa-apps/src/lib.rs:231`). `[Desktop Action]` and `TryExec` are
  not supported. Icons come from **hicolor only**, so the user's icon theme
  and absolute `Icon=` paths are ignored.
- **Not usable without sight, and English only.** The launcher panel has no
  AccessKit tree, no strings can be translated, and the scope keywords
  (`apps`, `files`, `settings`, `clip`, `emoji`) are hard-coded English.
- **Weak error feedback.** Once Niri accepts the spawn, a broken `Exec` fails
  silently. The panel then closes, and there is no busy feedback while an app starts.
- The search never offers power or session actions, though they exist as bar
  buttons. An empty query shows at most 20 rows, so you can't browse all apps.
  The two bundled providers start two `canopa-shell` processes each time typing pauses.

## Feature table

| Area | Item | Status | Evidence | What a user notices |
| --- | --- | --- | --- | --- |
| App search | XDG data dirs, desktop-ID precedence, subdirectory IDs | works | `canopa-apps/src/lib.rs:161-213` (dirs, ID = relative path joined by `-`), `:146-149` (first ID wins, before filtering) | User overrides in `~/.local/share/applications` win, as expected |
| App search | Flatpak / Snap | partial | Only through `XDG_DATA_DIRS`. `niri-session` re-execs a login shell (`forks/niri/resources/niri-session:13-18`), so `/etc/profile.d/flatpak.sh` normally applies. There is no built-in fallback | Works on a normal install. Flatpak or Snap apps vanish if the login shell skips `profile.d` |
| App search | `Hidden`, `NoDisplay`, `OnlyShowIn`/`NotShowIn` | works | `canopa-apps/src/lib.rs:227-248`. Session sets `XDG_CURRENT_DESKTOP=canopa:niri` (`packaging/wayland-sessions/canopa.desktop:4`) | Correct |
| App search | `TryExec` | missing (**Fixed in e48a85b**) | Not parsed anywhere in `canopa-apps` | Stale entries for uninstalled binaries still show and then fail silently |
| App search | `Terminal=true` apps | missing (**Fixed in e48a85b**) | Filtered out for the launcher (`canopa-apps/src/lib.rs:231`) | htop, btop, vim, nmtui and ranger can't be found |
| App search | Localized Name, GenericName, Comment, Keywords | works | `canopa-apps/src/lib.rs:274-303`, with `LANGUAGE`/`LC_*` fallbacks (`:46-62`) | Localized names show. Localized `Keywords[xx]` **replace** the English ones, so English keyword search stops working in localized sessions |
| App search | Fields matched | partial (**Fixed in d768cb9**: the desktop-file id and the `Exec` program name are matched with the lowest priority) | Name, then GenericName, Comment, Keywords (`app/panel_menu_session.rs:795-812`). The desktop ID and `Exec` binary are not matched | Typing `nautilus` for "Files" or `code` for some VS Code builds may fail |
| App search | Fuzzy matching | **broken** (non-ASCII) (**Fixed in ce15a6d**, ranking in 40bbd51) | `to_ascii_lowercase` on both query and text (`app/panel_search.rs:30,100`). The later tiering uses `to_lowercase` (`app/launcher.rs:649-690`), but only on entries that already matched | Case-insensitive search fails for Cyrillic, Greek and accented capitals (`éd` doesn't match "Éditeur"), and there is no accent folding |
| App search | `[Desktop Action]` entries | missing (**Fixed in 15fed5b and d768cb9**: parsed with localized names; Tab actions and query-matched rows; `ActivateAction` with a token for D-Bus apps) | Parser reads only `[Desktop Entry]` (`canopa-apps/src/lib.rs:338`) | No "New private window" or "New document" |
| App search | Ranking / frecency | works | Count capped at 20, plus recency by selection order (`launcher/history.rs:68-75`). Ranked by quality tier, then pin, then boost, then source order (`app/launcher.rs:649-720`). Pins and aliases: `app/launcher/personalization.rs:164+` | Frequent apps rise; exact and prefix matches stay on top |
| App search | Empty query / browse all apps | rough (**Fixed in d768cb9**: pins, up to five recent apps, then every app A–Z with no limit; `apps ` lists A–Z only; windows left the empty list) | Native results are cut to `RESULT_LIMIT` = 20 (`app/launcher.rs:770`, `launcher.rs:22`), and open windows come first | Users with more than 20 apps can't scroll an A–Z list and have no categories (confirm hands-on) |
| App search | Icons: theme | partial | Lookup is hard-coded to `theme: "hicolor"`, `scale: 1` (`icon_theme.rs:62-63`). It is rasterized at 72 px (`app/panel_icons/prepare.rs:14`), so it stays sharp up to about 2× | Papirus, Breeze and Adwaita-only icons are ignored. Apps without hicolor icons show a glyph |
| App search | Icons: absolute `Icon=` paths | missing | `is_valid_icon_name` rejects `/` (`icon_theme.rs:5-7`, `app/panel_menu_session.rs:766`) | AppImage, Steam-game and JetBrains Toolbox entries have no icon |
| App search | Icons: fallback | works | Kind glyph when there is no icon (`app/view/launcher.rs:946-956`). Lazy batches of 4 (`app/panel_icons.rs:14`) | Icons appear progressively on first open |
| App search | Refresh on install or remove | works | Full rescan on **every open**, before the panel is prepared (`app/overlay_adapter.rs:183-200`). Again on every launch (`app/launcher.rs:1424-1430`) | New apps appear on the next open, not while the panel is open |
| Launch | Exec apps | works | `gio launch <file>` (`command/applications.rs:108-126`) sent to Niri `Action::Spawn` (`:27-28,64-72`). GIO owns field codes and `Path=` | Correct field codes and working directory |
| Launch | `%f`/`%u` (Open with) | works | URIs appended to `gio launch` (`command/applications.rs:154-176`). D-Bus `Open` for activatable apps | Correct |
| Launch | Environment | works | Apps inherit **Niri's** environment (systemd user env plus Niri `environment {}`), not the shell's. `argv[0]` is resolved against the shell's PATH (`command/applications.rs:13-25`) | Normal. The two PATHs can differ only if they were configured differently |
| Launch | systemd scope | rough (**Fixed in d768cb9**: `canopa-shell --app-scope` moves each app into `app-canopa-<desktop id>-<random>.scope` after Niri places it) | Niri names the scope after the argv basename: `app-niri-<basename>-<pid>.scope` (`forks/niri/src/utils/spawning.rs:428-443`). Every launcher app becomes `app-niri-gio-<pid>.scope`. Without `NIRI_SOCKET` the fallback is a `systemd-run --scope` with no name (`command/applications.rs:29-43`) | Isolated from Shell (good), but you can't tell apps apart in `systemctl --user`, systemd-oomd or by portal host-app-ID parsing |
| Launch | Activation token / focus, Exec apps | works | Niri creates an external token for `Spawn` (`forks/niri/src/input/mod.rs:1102-1105`) and exports it (`spawning.rs:152-155`). External tokens have no `UrgentOnlyMarker`, so they focus (`handlers/mod.rs:809-816`). The lifetime is 10 s (`handlers/mod.rs:86`). This does **not** depend on layershellev | New windows and single-instance apps that forward the token (Firefox, GTK4) are focused |
| Launch | `DBusActivatable=true` | partial (**Fixed in d768cb9**: `platform-data` carries `activation-token` and `desktop-startup-id`; an app with a window also offers Switch to) | `org.freedesktop.Application.Activate`/`Open` with **empty `platform_data`** (`command/applications.rs:182-215`, `:201`). There is no Exec fallback | First start is usually focused by Niri's Smart policy. An **already-running** GNOME app is not raised |
| Launch | `Terminal=true` | missing (**Fixed in e48a85b** and 40bbd51: `gio launch` uses `xdg-terminal-exec`, a missing terminal is reported) | Hidden (see above). Even if shown, no terminal is chosen. GLib uses `xdg-terminal-exec` or its own built-in list | — |
| Launch | Startup feedback | missing | The panel closes once Niri accepts the request. There is no busy cursor or spinner | Slow apps (LibreOffice) look like nothing happened, so users press Enter again |
| Launch | Errors shown | partial (**Fixed in 40bbd51** for a missing `TryExec`/`Exec` program, missing terminal and rejected spawn; a crash after start is still silent) | Handoff errors appear as one generic status line (`app/launcher.rs:1498-1500`). Niri replies `Handled` before `exec`, and the child's stderr goes to `/dev/null` (`forks/niri/src/utils/spawning.rs:114-120`) | A broken `Exec`, missing binary or crash on start closes the panel silently |
| Launch | Reveal in file manager | partial (**Fixed in d768cb9**: the token is the `ShowItems` startup id) | `FileManager1.ShowItems(uris, "")`, with an empty startup ID (`app/launcher.rs:1508-1527`). Files accepts one (`canopa-files/src/file_manager.rs:43-44`) | An existing Files window may not be raised |
| Providers | Calculator, units | works | qalc, sandboxed state (`launcher/process.rs:58-140`). Docs: `docs/shell/launcher.md` | Needs `libqalculate`. No currency |
| Providers | Files (names) | works | `launcher/files.rs:17+`. Home only by default, 100k paths, 10 s, depth 32, 60 s refresh while open (`app/launcher.rs:934-944`) | Good. No recent files (`recently-used.xbel`), no content search (PRD W9) |
| Providers | Running windows | works (**StartupWMClass fixed in d768cb9**; app rows of running apps offer Switch to and New window) | `launcher/windows.rs:16-125`. Focus via Niri `FocusWindow` (`command/applications.rs:73-77`). App lookup by exact ID only (`windows.rs:73-79`); `StartupWMClass` is ignored | Windows whose `app_id` isn't the desktop ID lose their icon, name and "Launch" action |
| Providers | Settings pages / controls | partial (**Fixed in f286c32 and d768cb9**: pages come from the Settings registry through `canopa-settings pages` and open with `OpenPage`; search inside a page is still open) | 13 hard-coded rows (`launcher/controls.rs:8-125`). The Settings **Widgets** page (`canopa-settings/src/lib.rs:10`) is missing. Search doesn't reach inside pages. Settings isn't single-instance (see the Settings audit) | "Wi-Fi" and "volume" work. "night light", "language" and "default browser" find nothing |
| Providers | Power and session actions | missing (**Fixed in d768cb9**: Lock, Log out, Suspend, Hibernate, Reboot, Power off, following `Can*` and the confirmation menus) | Not in `controls.rs`. They exist only as bar buttons and menus (`config/default.kdl:31-42,130`) | Typing "shut down", "lock" or "log out" finds nothing |
| Providers | Command runner | missing | Only an extension's `launch` argv (`launcher.rs:110-112`) | Can't type a shell command |
| Providers | Web search | partial | `web`/`wiki` keywords, URL detection, custom templates (`launcher/bundled.rs:9-70`) | No "Search the web for …" fallback row when nothing matches |
| Providers | Clipboard history, snippets | works | Opt-in, `wl-paste --watch` (`launcher/clipboard.rs:274-302`). ADR 0132 | Needs wl-clipboard and data-control |
| Providers | Emoji and symbols | works | Bundled Emoji 17.0 (`launcher/characters.rs`) | English keywords only |
| Providers | Date/time, extensions | works | `launcher/bundled.rs`, `launcher/process.rs`. Docs list protocol limits | — |
| Keyboard | Full flow | works | Arrows, Enter, Tab for actions, Ctrl+Enter, Ctrl+1–5, Backspace (`surface/input.rs:91-151`). Escape clears, then closes. Default bind `Mod+Space` (`packaging/niri/defaults.kdl:14`) | Good |
| Keyboard | Multi-monitor | works | Opens on the bar of Niri's focused output; falls back to the first bar (`app/surfaces.rs:278-304`, `app/panel.rs:1587`) | Correct. Needs a bar on some output |
| Input methods | fcitx5 / text-input-v3 | partial (unverified) | layershellev binds `zwp_text_input_v3` (`forks/exwlshelleventloop/layershellev/src/lib.rs:1233-1270`). iced_layershell forwards IME requests (`iced_layershell/src/multi_window.rs:1088-1106`). `ime_allowed` is one global flag for all of Shell's surfaces. No Canopa test | Unknown. PRD W5 still lists it |
| Accessibility | AccessKit tree | missing | The Shell bridge publishes only `PanelContent::ExternalProvider` (`app/canopa_accessibility.rs:574-577`). The launcher is a Menu panel | Orca says nothing in the launcher. This blocks the W6 exit criterion "launch apps" **Fixed in 76c0460:** the launcher publishes a search field controlling the results list, the selected row as active descendant (announced as you move), action rows, the browse view and an error live region (ADR 0144). |
| Translations | Strings | missing | No i18n crate in the workspace. About 220 capitalized string literals in launcher files. Scope keywords are hard-coded (`launcher.rs:160-205`, `app/launcher.rs:32-48`) | English only. **Fixed (Tier 3.8 translations):** launcher text in Fluent with French, translated scope keywords accepted beside the English ones, locale digits in calculator results (ADR 0145) |
| Performance | Cold open | rough (unmeasured) | Desktop entries are rescanned before the panel opens (`app/overlay_adapter.rs:183-200`). The file index builds on first open. There is no recorded latency figure in `.scratch/unified-launcher/` | Fine with 78 entries on this host. Not measured with a large Flatpak set |
| Performance | Per-keystroke cost | rough (**Fixed in d768cb9**: `web` and `date` answer in-process; qalc and configured extensions wait 250 ms) | `web` and `date` are `global=true` and run `canopa-shell --launcher-extension` (`launcher/bundled.rs:15-26`). `qalc` runs for math-like queries. All of this follows a 100 ms debounce (`app/launcher.rs:1101`) | Each pause in typing forks up to 3 processes. Matters on low-end and battery hardware |
| Performance | Memory | works (bounded) | 100k-path index, 256 history entries, clipboard images capped at 32 MiB, icon cache 1024 entries (`app/panel_icons.rs:15`) | — |
| Config / theming | Source config, Settings page, JSON prefs, theme | works | `docs/shell/launcher.md`. `panel.launcher-width`. `canopa-settings --page launcher`. ADRs 0131 and 0132 | Good |

## Ranked gaps

1. **Unicode-aware matching.** **Fixed in ce15a6d and 40bbd51** (case and accent folding); desktop ID and `Exec` basename matching **fixed in d768cb9**. *Impact:* app, window and control search fails
   for most non-Latin locales, which matters for any distribution default.
   *Approach:* fold both sides with `str::to_lowercase`. Optionally add NFKD
   accent folding with `unicode-normalization`, or use `nucleo-matcher` (the
   Helix matcher, which handles Unicode case and normalization) inside
   `panel_search`. Also match on the desktop ID and the `Exec` basename as a
   third field. *Size:* S.
2. **Accessibility tree for the launcher.** (**Done** in 76c0460.) *Impact:* a blind user can't start
   an app. *Approach:* extend `PanelAccessibility` to Menu panels. Expose the
   search field as `Role::SearchInput`, results as `ListBox`/`ListBoxItem`
   with the active descendant tracking the selection, and status as a live
   region. Reuse the existing bridge and AT-SPI test fixture. *Size:* M.
3. **Terminal apps and `TryExec`.** **Fixed in e48a85b and 40bbd51** (the package does not yet depend on `xdg-terminal-exec`). *Impact:* TUI apps are invisible, and
   stale entries fail silently. *Approach:* stop filtering `Terminal=true`.
   `gio launch` already wraps these apps in a terminal, and GLib prefers
   `xdg-terminal-exec`, so ship or depend on that and configure a default
   terminal. Honour `TryExec` against PATH. *Size:* S.
4. **Desktop actions.** **Fixed in 15fed5b and d768cb9.** Exec actions run through GIO's `g_desktop_app_info_launch_action` in the `--app-scope` helper (gio 0.22 has no `DesktopAppInfo` binding, so the helper declares the `libgio-2.0` symbols). *Impact:* a common expectation ("New window", "New
   private window") is missing. *Approach:* parse `Actions=` and
   `[Desktop Action *]` in `canopa-apps`, show them as Tab actions, and use
   D-Bus `ActivateAction` for activatable apps. For Exec apps, add a
   `canopa-shell --launch-action <file> <action>` helper spawned through Niri
   that calls `gio::DesktopAppInfo::launch_action` (the `gio` crate is already
   a Shell dependency). That keeps the process out of Shell's cgroup. *Size:* M.
5. **Icon theme and absolute icon paths.** *Impact:* the desktop looks
   inconsistent, and some apps show no icon. *Approach:* pass the user's icon
   theme to `IconLookup`, from Canopa theme config or the
   `org.gnome.desktop.interface icon-theme` setting through the Settings
   portal. Pass the output scale, and accept absolute `Icon=` paths through the
   existing bounded raster loader. Files uses the same `"hicolor"` hard-code
   (`canopa-files/src/iced_desktop/icons_ui.rs:216`), so fix it in `canopa-assets` once. *Size:* S–M.
6. **Activation tokens for D-Bus apps, Reveal and panel actions.** Fork part fixed in `fecb4f1a` (`e489cad`); launcher part **fixed in d768cb9**. *Impact:*
   running GNOME apps and Files aren't raised. Activating a Notification
   Center action from the keyboard doesn't focus the app. *Approach:* in
   layershellev, call `update_activation_context` from `wl_keyboard`
   `Enter`/`Key`. Then request a token before D-Bus `Activate`/`Open` and pass
   it as `platform_data["activation-token"]` and `["desktop-startup-id"]`,
   and as the `ShowItems` startup ID. *Size:* S (fork) + S (Shell).
7. **Launch feedback and errors.** **Errors fixed in 40bbd51** (pre-handoff checks; "Starting X…" feedback and crash-on-start reporting remain open). *Impact:* a launch that fails or starts
   slowly looks like nothing happened. *Approach:* validate with
   `gio::DesktopAppInfo` before handoff (TryExec, Exec binary). Keep a short
   "Starting X…" state until Niri's event stream reports a window with the
   expected app ID (`StartupWMClass` or desktop ID), or a timeout passes. On
   failure, post a Notify notification. *Size:* S–M.
8. **Translations.** *Impact:* English only. *Approach:* PRD W5, with Fluent
   via `i18n-embed`. For the launcher, also translate scope keywords and
   control keywords while keeping English aliases, and localize the emoji
   annotations from CLDR. *Size:* M for the launcher, L for the whole desktop.
9. **Power and session actions in search.** **Fixed in d768cb9.** *Impact:* "shut down" and "lock"
   are common launcher queries. *Approach:* add control rows that open the
   existing confirmation menus (`config/default.kdl:31-42`). Add Lock through
   `loginctl lock-session`, which Canopa Lock already handles, and Suspend
   through logind `Suspend`. *Size:* S.
10. **Browse all apps.** **Fixed in d768cb9** (no categories). *Impact:* discoverability for new users. *Approach:*
    remove the 20-row cap for a blank query and for the `apps` scope
    (rendering is already range-limited). Optionally add categories from
    `Categories=`. *Size:* S.
11. **Per-app systemd scopes.** **Fixed in d768cb9** (the helper route). *Impact:* resource control, oomd, and
    attributing processes to apps. *Approach:* the Niri overlay could accept a
    scope name. Alternatively, Shell can spawn
    `systemd-run --user --scope --unit=app-canopa-<desktop-id>-<rand>.scope gio launch …`
    through Niri, following the XDG app-unit naming convention. *Size:* S.
12. **Window and app matching through `StartupWMClass`.** **Launcher part fixed in d768cb9**; `focused_window_icons` still never refreshes. *Impact:* some
    window rows lack an icon and the Launch action. *Approach:* parse
    `StartupWMClass` in `canopa-apps` and match case-insensitively. Also fix
    `focused_window_icons`, whose catalog never refreshes (`focused_window_icons.rs:40`). *Size:* S.
13. **Bundled providers in-process.** **Fixed in d768cb9.** *Impact:* fewer process forks per
    keystroke. *Approach:* run the `web` and `date` handlers as async
    functions inside Shell. They are already Rust (`launcher/bundled.rs`). Keep
    the JSONL protocol for third-party extensions. *Size:* S.
14. **Smaller items.** A web-search fallback row (S). Recent files from
    `recently-used.xbel` (S). A `>` command runner, optionally in a terminal
    (S). Search into Settings pages, by having Settings publish a page and
    keyword catalog (M). Content search through LocalSearch (`localsearch` or
    the Tracker SPARQL D-Bus API) as an extension (M; PRD W9).

## Bugs or risks found

- **layershellev keyboard serial: confirmed.** `update_activation_context` is
  called only from touch down/up and pointer button events
  (`layershellev/src/seat.rs:450,489,686`). `wl_keyboard` `Enter`/`Key` never
  update it. `request_activation_token` returns `None` if there is no context
  or the last one is older than 2 s (`layershellev/src/lib.rs:1677-1681`). The
  Shell uses it only for external panel provider events
  (`app/panel.rs:563-640`), meaning Notify actions in Notification Center.
  **It doesn't affect launcher launches:** the launcher never requests a token
  and relies on Niri's `Spawn` token. It does block issue 6 above. One more
  limit: Niri accepts a serial only if it is no older than the last
  keyboard or pointer enter (`forks/niri/src/handlers/mod.rs:773-785`).
- **Fixed in d768cb9.** **Empty D-Bus `platform_data`** (`command/applications.rs:201`). A running
  activatable app can't be raised from the launcher.
- **Fixed in ce15a6d.** **ASCII-only case folding** (`app/panel_search.rs:30,100`). This is a bug,
  not just a gap, because the file provider already uses Unicode lowercasing
  (`launcher/files.rs:135-149`).
- **Localized keywords replace the English ones**
  (`canopa-apps/src/lib.rs:256-266` with `localized_field`). `\;` escapes in
  lists aren't handled (`unescape_string`, `:305-328`).
- **Partly fixed in 40bbd51** (missing program, missing terminal, rejected spawn). **Silent launch failures** after the Niri handoff. A missing `gio` binary is
  reported, but a broken target app is not.
- **Fixed in d768cb9.** **Every app scope is named after `gio`**
  (`forks/niri/src/utils/spawning.rs:428-443`). This hurts per-app accounting
  and any tool that parses `app-<launcher>-<id>-…` unit names.
- **Each launch rescans the whole catalog** (`app/launcher.rs:1424-1430`,
  `:1538-1544`). It's correct, but doubles the I/O cost of a launch and can
  resolve a different entry if an override appeared in between.
- **Fixed in d768cb9.** **Process forks while typing.** Up to two `canopa-shell` processes plus
  `qalc` per debounced query (`launcher/bundled.rs:15-26`).
- **The global IME flag** (`ime_allowed`, `layershellev/src/lib.rs:1027,1233`)
  is shared by every layer surface Shell owns (bars, panel, popups). That
  could misroute IME when the focused surface changes. This is unverified.
- **Fixed in f286c32 and d768cb9.** **Settings coverage drift.** The hard-coded control list lacks the Widgets
  page, and future Settings pages need manual entries (`launcher/controls.rs`).
- **The panel needs a bar.** If a user configures no bar on any output,
  `select_global_panel_surface` returns `None` and the launcher can't open
  (`app/surfaces.rs:278-304`).
- **Stale PRD wording.** PRD W5 says "fix the vendored layershell path". The
  layer-shell crate is now a git dependency
  (`Cargo.toml:135-138`, `kevinard/exwlshelleventloop@1de59e6`).

PRD gaps, confirmed or refuted:
- W5 i18n: confirmed missing.
- W5 IME: plumbing exists but is unverified.
- W6 AccessKit "launcher first": confirmed missing. Shell's AccessKit covers
  only provider panels.
- W9 content search through the launcher's provider protocol: confirmed
  missing. The extension protocol can host it.
- W4 "Calculator via libqalculate": confirmed working.
- W1 `niri msg` logout: confirmed in `config/default.kdl:33`. This is the
  bar's session buttons, not the launcher.

## Hands-on checks needed

After d768cb9 the expectations of checks 2, 3, 5, 7, 10 and 13 change: a
running app's result says Switch to and a running D-Bus app is raised; Files is
raised by Reveal; scopes are `app-canopa-<desktop id>-<random>.scope`; Tab on
Firefox lists New Window and New Private Window; the empty query lists every
app; typing starts no `canopa-shell --launcher-extension` processes. Also try
`private`, `shut down`, `night light` (once Settings has such a page) and
`nautilus`.

1. **Exec token.** Launch an Exec app (for example `foot` or Firefox) from the
   launcher with Enter only. Then run
   `tr '\0' '\n' < /proc/$(pgrep -n firefox)/environ | grep -E 'XDG_ACTIVATION|DESKTOP_STARTUP'`.
   Expect both variables to be set.
2. **Raising a running app.** With Firefox running on another workspace,
   launch it again from the launcher; expect focus to move to it. Repeat with
   a running `DBusActivatable` app (`org.gnome.TextEditor` or
   `org.gnome.Calculator`). Expect it **not** to be raised (bug).
3. **Reveal.** With Files open on another workspace, use "Reveal in file
   manager" on a launcher file result. Check whether the Files window is focused.
4. **Keyboard-only token (layershellev).** Leave the mouse untouched for more
   than 2 s. Open Notification Center, select a notification action with the
   keyboard and press Enter. Expect the target app not to be focused. Repeat
   with a click: expect focus.
5. **Scopes.** Run `systemctl --user list-units 'app-niri-*'` after launching
   two different apps. Expect two `app-niri-gio-<pid>.scope` units.
6. **Non-ASCII search.** Create `~/.local/share/applications/t.desktop` with
   `Type=Application`, `Name=Терминал` and `Exec=true`. Search `терм` (expect
   no result) and `Терм` (expect a match). Try `éd` against an app named "Éditeur".
7. **Terminal apps and actions.** Search `htop` and `btop` (expect nothing).
   Press Tab on Firefox (expect no "New private window").
8. **Icon theme.** Install Papirus and select it in GTK settings. Launcher
   icons are expected to stay hicolor. Add a desktop entry with an absolute
   `Icon=/path/icon.png` (expect a glyph).
9. **Silent failure.** Create a desktop entry with `Exec=does-not-exist` and
   launch it. The panel closes with no message. Check
   `journalctl --user -u niri -n 20`.
10. **Blank query.** Open the launcher with no query and count the rows on a
    system with more than 20 apps. Type `apps ` and count again.
11. **IME.** With fcitx5 plus Mozc or Pinyin (`GTK_IM_MODULE` unset; Niri
    text-input-v3), open the launcher, type and convert. Check that preedit
    shows in the field, the commit lands, and Escape cancels composition
    without closing the panel.
12. **Orca.** Start Orca, press `Mod+Space`, type and arrow through the
    results. Expect silence (missing).
13. **Typing cost.** Run `strace -f -e trace=execve -p $(pgrep -x canopa-shell) 2>&1 | grep -c execve`
    while typing a ten-character query with pauses.
14. **Cold open.** On a system with many Flatpaks, screen-record at 60 fps
    (`wf-recorder`) and count the frames from `Mod+Space` to the first
    painted results. Repeat once with an empty file-index cache, after a
    Shell restart.
