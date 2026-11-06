# 0116: Portal none veto and ordinary Qt file dialogs

Status: generator correction and scoped ordinary Qt/Iced verification pass,
2026-11-05.

Testing ordinary QFileDialog callers exposed an error in the new configuration
generator. The frontend treats `none` anywhere in a preferred list as a veto for
the entire interface. Prefixing a disabled list with Canopa therefore still
disables it. For a selected interface whose explicit/inherited list contains
`none`, replace the disabled list with Canopa alone. Its inactive backend names
must not become enabled fallbacks. Preserve every unselected route and default.

The original phase90 Settings fixture generated its configuration outside the
directory selected by XDG_DESKTOP_PORTAL_DIR. Its green activation result used
legacy desktop matching and did not prove that the generated file worked. Put
the generated file in the actual lookup directory and assert that the frontend
reports loading that exact path. Both the new unit regression and this corrected
frontend fixture fail before the fix, then pass. Twelve generator tests pass.
Keep those red logs and the original generator source as counterexamples.

A new Qt caller uses only QApplication/QFileDialog/QFile APIs; it has no explicit
portal or D-Bus calls. With the xdgdesktopportal theme selected on Wayland, it
verifies Open/read, Save/write and Cancel through the normal backend and actual
Iced chooser. Bus traces observe both public and implementation FileChooser
methods. Text filters return to Qt correctly, Unicode/space paths are preserved,
the application performs Save itself and the original file stays unchanged.
Loaded mappings prove the Qt portal plugin and no GTK in caller/helper.

Qt6.12.0 also has an observed startup boundary. Constructing QFileDialog before
the event loop starts chooses Qt's own widget dialog, even with the portal plugin
loaded and a healthy frontend advertising FileChooser v4. Constructing the same
dialog from an event-loop callback reaches Iced. The scoped acceptance models a
user opening a dialog after startup; the early-construction result remains a
counterexample, not a claimed fix to Qt. Its original caller source and tiny
cached executable remain available. No Qt fork or Print integration is added.

The maintained fixture caches clients by source/compiler/Qt/flags recipe under
root target, preserving checksums and current loaded-library identities. It owns
a private bus, nested desktop, files, metadata and generated configuration. It
does not build Rust, download a runtime or change live selection. Current native
receipt: target/portal-filechooser-qt-91/native-dark-04/result.json, three groups.
The screenshot was inspected and shows the actual Iced chooser with Qt's title
and selected text filter. Source/compositor/client identities stay in the receipt;
this is not a new whole-workspace or universal toolkit acceptance claim.

The behavior is consistent with the pinned Qt
[theme discovery](https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/plugins/platformthemes/xdgdesktopportal/qxdgdesktopportaltheme.cpp),
[file-dialog adapter](https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/plugins/platformthemes/xdgdesktopportal/qxdgdesktopportalfiledialog.cpp)
and upstream [portal selection](https://flatpak.github.io/xdg-desktop-portal/docs/portals.conf.html).

## Ordinary GTK4 routing

Phase92 additionally verifies the public GTK4 GtkFileDialog API on installed
GTK4.24.1. Open/read, Save/write and Cancel reach the same Iced helper, with
offered text filters and Unicode/space filenames. GTK_USE_PORTAL and GDK_DEBUG
are unset. The caller links GTK normally; helper mappings remain GTK-free.
The screenshot was inspected. This is host application routing, separate from
the real Flatpak GIO client and without a universal GTK/runtime claim.

The first private test failed because its bus advertised only the backend as
activatable. GTK checks ListActivatableNames for org.freedesktop.portal.Desktop,
even when the frontend is already running. Supplying the standard frontend
activation entry fixes the fixture; no Canopa runtime change or portal-forcing
flag is required. The shared harness now asserts both frontend activatability
and public FileChooser v4 before launching callers. This matches the pinned
[GTK discovery code](https://raw.githubusercontent.com/GNOME/gtk/4.24.1/gdk/gdk.c).

Qt/GTK wrappers share scripts/file_chooser_toolkit.py for private bus, metadata,
configuration generation, compositor, input, tracing and cleanup. Separate small
public-API callers retain their source/compiler/version/flags cache recipes.
The existing Qt executable is reused unchanged; its three cases pass again on
the shared harness. Three GTK cases also pass. Evidence:
target/portal-filechooser-gtk-92/native-gtk-dark-02/result.json and
native-qt-regression-01/result.json. The first failed fixture and original Qt
harness remain preserved. No Rust, Cargo, vendor, SDK or live selection change.
