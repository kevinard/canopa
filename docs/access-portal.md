# Canopa Access portal

The Rust/Iced Access backend presents an explicit grant/deny question with shared
Canopa appearance, optional symbolic icon and boolean/combo choices. The standard
XDG frontend retains permission persistence and resource policy. The backend
returns a decision and never opens camera/device resources itself.

## Selection

Build with `cargo build --locked -p canopa-portal` and follow the
[portal installation guide](settings-portal.md#installation-and-backend-selection).
Add this to the existing `[preferred]` group in the session's selected portal
configuration:

```ini
org.freedesktop.impl.portal.Access=canopa
```

For Niri, normally use `~/.config/xdg-desktop-portal/niri-portals.conf`, preserving
the distribution's other routes. Configurations are selected as a whole. Settings,
FileChooser, AppChooser and Access can be selected independently; other interfaces
still need their working backends. Development checks do not install files or
change the live selection.

## Decisions and windows

Opening the dialog or pressing an unhandled initial Enter never grants access.
Only an explicit Allow/Grant action returns success and validated choices. Deny,
Escape, window close, Request.Close and caller loss do not grant. Invalid options,
invalid helper results and helper failures cannot produce a grant. Long text and
choices scroll while both decision buttons stay reachable.

Titles, subtitles, bodies and labels are plain text. Icons are prepared on a
worker through the shared bounded PNG/SVG loader. Missing icons leave the question
usable. Dialogs use the backend's shared configuration selection; each helper
retains its opening appearance snapshot.

Exported Wayland parents attach before initial configure. Optional modal hints
remain subject to compositor support/policy. Without an exported parent, the
window is independent and Niri may tile it. The public Camera API has no parent
window option. For floating placement of all Canopa portal windows, an optional
Niri rule is:

```kdl
window-rule {
    match app-id=r#"^org\.canopa\.Portal$"#
    open-floating true
}
```

The rule passes the installed Niri's configuration validator; add it only to the
user's own Niri configuration if that placement is wanted. X11 parenting and a Canopa permission-management settings UI are
not implemented by this backend.

## Validation

```sh
cargo test --locked -p canopa-portal --lib --test integration
python3 scripts/test-access-portal.py --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-access-dark-new
python3 scripts/test-access-portal.py --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-access-light-new --color-scheme light --scale 1.25
```

The fixture uses private D-Bus/Weston/Niri instances, the shipped descriptor and
activation file, private configuration, a private PipeWire daemon and a private
permission store. Direct backend requests exercise decisions, edited choices,
Close/caller loss, concurrency and long text. A public Camera request verifies
that the standard frontend presents Canopa's consent question and receives the
explicit grant. The fixture normalizes that independent window's geometry for
input/capture; it does not claim automatic transient placement without a parent.

Dependencies match the [FileChooser fixture](file-chooser-portal.md#validation),
plus PipeWire and the standard `xdg-permission-store` service. No GTK renderer or
caller is used. This verifies consent routing, not camera hardware, media flow,
real Flatpak sandbox access or a permission-management settings application.
See [ADR0029](adr/0029-access-portal.md) and the
[evidence ledger](../.scratch/xdg-portals/IMPLEMENTATION.md) for actual results.
