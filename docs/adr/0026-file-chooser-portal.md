# 0026: Rust and Iced FileChooser portal backend

Status: implementation in progress, 2026-10-29.

Extend the existing Settings backend in `canopa-portal` with
`org.freedesktop.impl.portal.FileChooser`. Retain the public `xdg-desktop-portal`
frontend, which owns application identity, document exports and sandbox grants.
Implement the published contract in Rust; no GNOME implementation is copied.

Support OpenFile (single/multiple files and directories), SaveFile and SaveFiles.
Preserve Unix path bytes through NUL-terminated D-Bus options, private helper
transport and percent-encoded local file URIs. Choices, filters, initial paths,
read-only opening and overwrite confirmation belong to the chooser. Acceptance
never creates or truncates the requested files.

Reuse Files' Directory/Browser through `canopa-files` without default features.
Its operation engine, resident host and GIO runtime are not started. Canopa UI
and shared appearance supply Iced's theme, typography and controls. Filename
globs and shared-mime-info filename rules provide advisory filters. GTK is absent.

The display-independent D-Bus host launches a private mode of the same executable
for each interaction, running Iced on that helper's main thread. JSON travels
over anonymous pipes; no temporary response file, shell interpolation, public
helper service or automatic acceptance is involved. Each request owns one helper;
Settings stays responsive. Sixteen simultaneous interactions are allowed, with
explicit failure beyond that limit.

Export the backend Request at the supplied handle before showing UI. Only the
originating unique sender can Close it. Close, caller loss, backend shutdown and
bus loss retire interactions. Success, cancellation and failure return codes
0, 1 and 2. Unexport on completion and reject duplicate handles. Awaiting a
chooser does not take an exclusive object-server lock.

Extend the shared Iced/Winit forks with `open_dialog`, passing an exported Wayland
parent and modal hint through creation. Optional xdg-foreign-v2 and xdg-dialog-v1
globals establish parenting before the first surface commit and retain it until
child destruction, continuing ADR0019. Missing/expired exports or unavailable
protocols leave a usable independent window. Layer-shell declines the operation.
X11 parent identifiers are outside this Wayland slice. No unsafe pointer bridge
is added. Provenance and patches remain in the one shared vendor tree.

Advertise only Settings and FileChooser. Selection remains explicit per interface;
the companion session target and live portal configuration are unchanged.

Native acceptance covers the public frontend, activation, an exported parent,
rendered Iced dialogs and cancellation/open/save behavior. Protocol/model tests
cover malformed options, concurrency, byte paths, ownership and caller loss.
Shared changes require the workspace gate. See [validation evidence](../../.scratch/xdg-portals/IMPLEMENTATION.md).

Sources:

- [Backend FileChooser](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.FileChooser.html)
- [Backend Request lifetime](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Request.html)
- [Public FileChooser options](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html)
- [Backend registration](https://flatpak.github.io/xdg-desktop-portal/docs/writing-a-new-backend.html)
- [Wayland xdg-foreign source](https://cgit.freedesktop.org/wayland/wayland-protocols/tree/unstable/xdg-foreign/xdg-foreign-unstable-v2.xml)
- [Niri default routing](https://raw.githubusercontent.com/niri-wm/niri/main/resources/niri-portals.conf)
