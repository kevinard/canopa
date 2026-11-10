# 0018: Files uses Iced and shared Canopa controls

Status: accepted following the user's consistency correction, 2026-10-17.
Supersedes the GTK toolkit choice in [ADR 0015](0015-files-native-widgets.md).
The supported default build now uses Iced and `canopa-ui`. GTK remains historical
reference code while feature migration continues; this does not claim feature parity.

Canopa Files must use the workspace's Iced stack and `canopa-ui`, as Shell and
Notify do. Sharing palette values through a GTK styling adapter does not provide
the same controls, behavior, focus treatment or maintenance model. The earlier
decision gave too much weight to available GTK integrations and too little to
consistency across Canopa.

The original reason was specific: the installed Iced 0.14 `window::Event` source
documents file-hover and file-drop events as unimplemented on Wayland. Files
requires interoperable drag-and-drop under Niri. GTK also supplied clipboard,
accessible list/grid, transient-window and media widgets. Those were useful
implementation conveniences, but they do not justify a separate GUI toolkit.

Build ordinary Iced application windows for Browser Windows, source-bound floating
Quick Look and auxiliary Operations/Trash/Recovery windows. Reuse `canopa-ui` for
appearance and control treatments, including `Appearance::style(UiKind)`, shared
fonts, focus indicators and live configuration. Shell's layer surfaces remain
owned by Shell; Files does not become a shell panel or acquire layer-shell
placement merely to reuse Iced.

Preserve the full product brief through the migration: default responsive columns,
identical keyboard navigation in narrow tiles, independent windows/tabs, floating
previews, custom actions, real clipboard and drag-and-drop, devices, operations,
Undo and notifications. Keep the toolkit-independent browsing, filesystem,
configuration, storage-requirement and request-state code and its public tests.
Treat the existing GTK adapter and its acceptance evidence as migration references;
they do not prove behavior in the new renderer.

Wayland file exchange and media preparation remain explicit integration work.
Use focused platform services behind the Iced presentation where necessary;
GIO or other non-widget services do not determine the GUI toolkit. Investigate
the supported shared stack before selecting a protocol bridge or dependency
change. Shared forks, if needed, follow `docs/vendor-maintenance.md` and live only
in `vendor/`. Do not silently omit drag-and-drop or retain a second GTK GUI to
claim completion.

Record and verify migration slices against real Niri windows in both themes and
narrow/wide tiles. Re-run the complete native acceptance matrix for Iced and
update packaging/runtime dependencies after the replacement is working. The full
file-manager goal and the outstanding durability/ownership gates remain active.

Update, 2026-11-10: the Iced and winit changes now live in the kevinard/iced and kevinard/winit GitHub forks rather than `vendor/`; see ../vendor-maintenance.md.
