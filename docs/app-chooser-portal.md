# Canopa AppChooser portal

The Rust/Iced AppChooser backend presents application names, bounded icons and
search using Canopa's shared appearance. It handles live choice updates and
returns the selected application's exact ID. The public XDG frontend owns
launching, MIME defaults and document permissions.

## Selection and setup

Build with `cargo build --locked -p canopa-portal` and follow the
[portal installation guide](settings-portal.md#installation-and-backend-selection).
Add this line to the existing `[preferred]` group in the session's selected
portal configuration, alongside Settings and FileChooser if wanted:

```ini
org.freedesktop.impl.portal.AppChooser=canopa
```

For Niri, normally edit `~/.config/xdg-desktop-portal/niri-portals.conf`, preserving
the distribution's other routes. Portal configurations are selected as a whole.
Keep other interfaces on their working backends; Canopa currently advertises only
Settings, FileChooser, AppChooser and Access. The [FileChooser guide](file-chooser-portal.md)
explains per-interface selection and current Wayland parenting limits. Development
fixtures do not install files or change the live desktop.

## Behavior

The chooser uses shared desktop-entry metadata, retaining explicit handlers even
when they are absent from launcher menus. Hidden user overrides still suppress
system entries. Unknown offered IDs keep a text fallback. Search covers names,
IDs, generic names and keywords. Live updates remove stale selections; accepting
an application removed by an update cannot return success.

Incoming activation is applied during helper-window creation. Acceptance asks
Wayland for a new token bound to recent input on this focused chooser. The helper
stays open while waiting, discards late tokens, and invalidates pending acceptance
when choices change. Compositors may decline activation. Without the protocol or
eligible input, selection still completes without a token. The incoming token is
never returned as a substitute after user interaction.

The backend supports local desktop-entry metadata and ordinary Wayland windows.
The shared stack currently has no equivalent X11 parent/token implementation.
The dialog itself neither launches applications nor modifies MIME defaults.

## Verification

```sh
cargo test --locked -p canopa-apps --lib
cargo test --locked -p canopa-portal --lib --test integration
python3 scripts/test-app-chooser-portal.py --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-app-chooser-dark-new
python3 scripts/test-app-chooser-portal.py --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-app-chooser-light-new --color-scheme light --scale 1.25
```

The native fixture uses private D-Bus/Weston/Niri sessions, the shipped descriptor
and activation file, fixture-only desktop entries and a harmless recording
application. It checks public OpenURI selection, exact URI delivery, a fresh token
received and consumed by the chosen Wayland client, focus, native live updates
and Close. It does not exercise real Flatpak document export. Dependencies match
the [FileChooser fixture](file-chooser-portal.md#validation), plus system
GSettings schemas for the standard XDG frontend. No GTK renderer or caller is
used. See [ADR0028](adr/0028-app-chooser-portal.md) and the
[evidence ledger](../.scratch/xdg-portals/IMPLEMENTATION.md).
