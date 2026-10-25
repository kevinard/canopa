# Native file manager widgets and shared appearance

Status: superseded by [Files uses Iced and shared Canopa controls](0018-files-iced-and-shared-controls.md),
2026-10-17. The rationale below records the earlier decision; it is not the
current toolkit direction.

Canopa Files adds a third independent application context. It owns file browsing,
selection, transfers, Undo, previews, actions and preferences. Shell retains its
surface and provider ownership; Notify retains notification policy and history.

Files uses GTK4 for its native widgets and consumes the existing toolkit-neutral
`canopa-theme` appearance plus `canopa-config` shared intake. Its GTK styling
adapter stays inside the Files application. Shell and Notify continue using
their shared Iced adapter unchanged. This extends the set of appearance
consumers in ADR 0003 without changing palette, typography or appearance ownership.

The initial proposal reused Iced. Inspection of the workspace's exact upstream
version, `iced_core 0.14.0/src/window/event.rs`, explicitly marks FileHovered,
FileDropped and FilesHoveredLeft as unimplemented on Wayland. Real interoperable
drag-and-drop under Niri is an accepted product requirement. Choosing GTK4
provides native GDK file transfer and clipboard support, transient windows,
accessible list/grid controls and media playback widgets. The existing system
has GTK 4.22, GIO, GStreamer and Poppler development libraries. No additional
shared dependency fork or experimental renderer is introduced.

Core file-manager models and filesystem operations remain independent of GTK.
The crate's optional desktop feature supplies the native application; focused
core tests can run with `--no-default-features`.

GTK controls must resolve Canopa's semantic theme roles and sizes rather than
installing an unrelated application palette. Files preferences use a future
`files` namespace in the common configuration, with application-owned schema
validation. Per-folder views and operation history remain private application
state. The implementation ledger tracks this integration until it is verified.

Primary references:

- [GTK DropTarget](https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/struct.DropTarget.html).
- [GTK Video](https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/struct.Video.html).
- [GTK Application](https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/struct.Application.html).
