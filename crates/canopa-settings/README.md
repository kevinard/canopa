# Canopa Settings

An ordinary Iced window for display/input configuration, network, Bluetooth,
printers, sound and power, date and time, region and language, users,
privacy, notifications, default applications and system information, shared
appearance and lockscreen preferences, the existing wallpaper picker and
Stasis idle controls. System services own the state (NetworkManager, BlueZ,
CUPS, timedated, localed, hostnamed, AccountsService, the portal permission
store, Canopa Notify); privileged changes ask polkit.
Run `canopa-settings [PAGE]` or `canopa settings`; a running window switches to
the page instead of opening another. See [the settings guide](../../docs/settings.md).

Display trials run in an independent process and require confirmation before
persistence. A disconnected frontend, timeout or failed partial apply restores
previous live output settings. Niri, Wallpaper, Lock and Stasis retain their own
policy and lifetimes. No settings daemon runs after the window closes.

## Structure

See [ADR 0136](../../docs/adr/0136-settings-pages-patterns-and-activation.md).

- `src/registry.rs` declares every page once: id, title, description, icon
  name, sidebar group, search keywords and kind. Navigation, search, Ctrl+1–9,
  page ids on the command line and over D-Bus come from it.
- `src/file_page.rs` is the file-backed pattern: a `FileModel` reads, falls back
  and writes one document; `FileDraft` keeps the draft and its snapshot and
  applies the shared-document rule when another page saves the same file.
- `src/live.rs` is the live service pattern: a client implements `LiveService`
  (snapshot, updates, apply), and `Live` tracks loading, "service not
  available" and per-control progress, errors and Retry. `ui/live_page.rs`
  runs it in the window. In debug builds, `canopa-settings live-example` opens
  a hidden page on a fake service.
- `src/instance.rs` keeps one window per session on `org.canopa.Settings`
  (`Activate`, `OpenPage`, `Pages`); later launches forward their page and exit.

To add a page:

1. Add a `Page` variant and its `Entry` in `src/registry.rs`, with a new id,
   group, keywords and kind.
2. Write the model: a `FileModel` for a file, or a `LiveService` adapter over a
   service client.
3. Hold its `FileDraft` or `LivePage` in `ui::State`, with a `Message` variant
   for its edits or `live::Event`s.
4. Add its arms to the page matches in `ui.rs`: view, save (file pages),
   reload, the shared-document rule if it shares a file, and its
   subscription (live pages).
5. Add tests: model round trips, or the state machine against a fake service.

The supported Arch installation uses a separate Canopa session and compatible
Niri build; see [installation](../../docs/installation.md). Existing ordinary Niri
configuration is never rewritten. Core adapter tests are in the `integration`
target; select them with
`bash scripts/cargo.sh test --locked -p canopa-settings --test integration`.
The IPC peer tests use a separate validator stub. Three native parser tests are
explicitly ignored in portable checks and run by desktop CI; select them with
`CANOPA_NIRI_BIN=/absolute/path/to/niri bash scripts/cargo.sh test --locked -p canopa-settings --test integration -- --ignored`.
