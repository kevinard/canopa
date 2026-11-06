# Canopa FileChooser portal

`xdg-desktop-portal-canopa` includes a Rust/Iced file chooser, sharing Canopa's
appearance and Files browsing model without GTK. Applications use the standard
public FileChooser portal; `xdg-desktop-portal` owns sandbox/document access.

## Build and select

```sh
cargo build --locked -p canopa-portal
```

Default features include the Iced helper. `--no-default-features` supports headless
protocol tests and Settings but must not be installed for interactive backends.
Follow the [installation guide](settings-portal.md#installation-and-backend-selection)
for the binary, D-Bus service, on-demand user unit and descriptor. Add this to the
existing `[preferred]` group in the session's selected portal configuration:

```ini
org.freedesktop.impl.portal.FileChooser=canopa
```

For Niri, normally use `~/.config/xdg-desktop-portal/niri-portals.conf`. Copy the
distribution's complete current Niri configuration before adding the entry:
portal configurations are selected as a whole, rather than merged. Keep existing
routing for ScreenCast, Screenshot and other interfaces. Select Settings separately
if wanted. Restart the frontend or log in again after existing portal work ends.
Builds and tests never install or change the live session. Activation needs the
session's Wayland display and XDG paths. The service stays on demand.

## First chooser slice

- Open one/multiple files or folders; optionally request read-only access.
- Save a file or choose a folder for an ordered batch of names (SaveFiles).
- Honor initial folder/file/name, glob/MIME filename filters and combo/boolean choices.
- Preserve original Unix filename bytes, including non-UTF-8 names, in selections
  and URI encoding. Display labels alone use lossy conversion.
- Navigate with Home, Up, Location, personal folders or a folder's Open control.
  Double-click opens a row. Search filters the current folder. Ctrl+L focuses
  Location, Ctrl+F search, Ctrl+H hidden files, Ctrl+A selects visible eligible
  files for multiple selection, and Alt+Up goes to the parent. Escape cancels or
  dismisses overwrite confirmation.
- Require Replace confirmation for existing destinations. The chooser never
  creates/truncates output files; the requesting application performs its save.
- SaveFile and SaveFiles offer **New folder** and Ctrl+Shift+N. Enter one name,
  then Create or Enter to create and open the folder while retaining the Save
  filename. Escape or Cancel dismisses this prompt and keeps the chooser open.
  Existing files, folders and symlinks are never replaced. A folder already
  created by an explicit Create action remains if the save is later cancelled.
- Close owned helpers on Request.Close, caller loss and backend shutdown.

Exported Wayland parents attach through xdg-foreign-v2 before initial configure.
The modal hint uses optional xdg-dialog-v1; compositor policy determines its
effect. Missing/expired handles and unavailable protocols yield an independent
chooser. X11 parenting, remote locations, content-based MIME sniffing, previews
and preferences beyond the last accepted folder remain follow-up work.
MIME filters use advisory shared-mime-info filename rules.
Existing symlink and special-file save destinations are refused in this slice.
The [New folder decision](adr/0113-file-chooser-new-folder.md) records creation
ownership and the focused/native/sandbox verification scope.

Each new chooser reads configuration through the backend's `--canopa-config`,
`--defaults` or discovery mode. Existing windows retain their opening appearance.
Settings retains its SIGHUP reload behavior.

## Remembered folders

An identified application starts in its last accepted folder when it supplies
neither an initial folder nor an initial file. Explicit application hints take
precedence. Cancelling a chooser does not change the remembered location.
Applications have independent preferences; anonymous callers use normal defaults.
Unavailable folders or preference data leave the chooser usable.

The backend stores only application IDs and folder paths in
`$XDG_STATE_HOME/canopa/portal-file-chooser.db` (normally
`~/.local/state/canopa/portal-file-chooser.db`). The private database retains up to
128 applications and is limited to 2 MiB. Selected filenames and access grants are
not stored. Remembering a folder does not grant a sandbox access to it.

Start the backend with `--no-file-chooser-history` to disable these reads and
writes. To reset the preference, stop the backend and remove that database.
See the [folder-memory decision](adr/0114-file-chooser-folder-memory.md) for
ownership, fallback behavior and verification scope.

## Qt file dialogs

Qt applications using native `QFileDialog` can use the portal platform theme.
With Canopa already selected for FileChooser and the distribution's Qt portal
theme plugin installed, a per-application Wayland launch is:

```sh
QT_QPA_PLATFORM=wayland QT_QPA_PLATFORMTHEME=xdgdesktopportal YOUR_QT_APPLICATION
```

On the tested Qt 6.12.0, ordinary Open, Save and Cancel calls made after the
application event loop starts reach the public portal and show Canopa's Iced
chooser. The caller receives its selected filter and paths, reads the chosen
file, and performs its own Save, including spaces and non-ASCII filenames.
The caller and Iced helper load no GTK. This is an ordinary host application
check; the separate real Flatpak evidence above uses GIO.

An observed Qt startup limit remains: constructing the file dialog before the
event loop starts can select Qt's own widget dialog before its asynchronous
portal discovery completes, even with the plugin loaded and FileChooser v4
available. Applications explicitly requesting custom or non-native dialogs also
control their own presentation. This launch setting does not establish automatic
Qt Print routing, which remains deferred. See the upstream
[QFileDialog documentation](https://doc.qt.io/qt-6/qfiledialog.html) and
[Qt portal theme implementation](https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/plugins/platformthemes/xdgdesktopportal/qxdgdesktopportaltheme.cpp).

## GTK file dialogs

The tested GTK 4.24.1 `GtkFileDialog` caller automatically uses the public
FileChooser portal when the standard frontend is D-Bus activatable and exposes
the interface. With FileChooser selected as Canopa, Open, Save and Cancel show
the Iced chooser. The check covers offered text filters, original file reads,
application-owned saves, Unicode/space filenames and cancellation. The test
application links GTK; the Canopa chooser helper loads no GTK library.

GTK checks the session bus's activatable names before using portals. Merely
starting a frontend process without its activation metadata can leave GTK using
its own dialog. The frontend package should provide
`org.freedesktop.portal.Desktop.service`. The installed backend descriptor and
complete configuration must also select Canopa. The
[pinned GTK discovery implementation](https://raw.githubusercontent.com/GNOME/gtk/4.24.1/gdk/gdk.c)
describes this behavior. The native check leaves `GTK_USE_PORTAL` and `GDK_DEBUG`
unset; it does not force portal use or simulate a sandbox.

This is a host GTK4 API check, separate from the existing real Flatpak GIO
evidence. It does not establish every GTK version, custom application dialog or
sandbox runtime. Modern GTK applications use
[`GtkFileDialog`](https://docs.gtk.org/gtk4/class.FileDialog.html); applications
choosing their own dialog widgets control their presentation.

## Validation

```sh
cargo test --locked -p canopa-portal --lib --test integration
python3 scripts/test-settings-portal.py --binary target/debug/xdg-desktop-portal-canopa
python3 scripts/test-file-chooser-portal.py --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-chooser-dark
python3 scripts/test-file-chooser-portal.py --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-chooser-light --color-scheme light --scale 1.25
```

Native checks need Niri, Weston, wtype, grim, wayland-scanner, a C compiler,
Wayland development files, Python GI/Pillow and xdg-desktop-portal. The fixture
owns a private bus, compositor, files, configuration and input. Result JSON,
screenshots and traces remain in the new output directory. See [ADR0026](adr/0026-file-chooser-portal.md)
and [validation evidence](../.scratch/xdg-portals/IMPLEMENTATION.md).
Add `--folder-history` to the native chooser command to exercise separate
application folders across a backend restart, explicit hints and cancellation.

`scripts/test-file-chooser-qt.py` exercises ordinary Qt Open/Save/Cancel through
the public frontend and actual Iced surface, with method tracing and loaded
library checks. Pass `--binary`/`--binary-sha256`, `--frontend`/`--frontend-sha256`
and a new `--output` directory. It compiles a small cached caller using installed
Qt6Widgets development files and a C++ compiler. It downloads no SDK/runtime and
reuses the existing Rust executables. See the
[routing decision](adr/0116-portal-none-veto-and-qt-file-dialogs.md) for the
configuration regression and scoped Qt startup evidence.

`scripts/test-file-chooser-gtk.py` uses the same arguments and private harness
with a public GTK4 `GtkFileDialog` caller. It needs installed GTK4 development
files (4.10 or newer) and a C compiler. The observed runtime is pinned in the
receipt. Both toolkit callers are cached by source/compiler/version/flags recipe;
the harness verifies frontend activation metadata, both public and implementation
method calls, actual Iced windows, caller results and GTK-free helper mappings.

The real Flatpak fixture also verifies Open, Save and Cancel through the normal
backend and the standard Documents portal. A host-only file is inaccessible to
the app before approval; its returned document URI is readable, the Iced
read-only choice denies writes, and an approved Save writes the chosen host file.
The probe checks its actual `/.flatpak-info` identity and the implementation call's
authenticated application ID. The chooser process loads no GTK library.

This check reuses an existing fixture-owned Flatpak store and minimal runtime.
`scripts/prepare-file-chooser-flatpak.py` builds/deploys only the tiny GIO client;
it downloads no SDK or runtime and never uses the real user/system installation.
Use the returned client-build receipt with `scripts/test-file-chooser-flatpak.py`,
passing the backend, compositor and frontend paths/checksums, `--flatpak-store`,
`--client-build` and a new `--output` directory. The native runner creates its
private user/mount namespace automatically so the Documents portal and sandbox
share standard runtime paths. It clears launcher capabilities before Flatpak
starts and retires its Documents mount and services afterward.

Add `--new-folder --modes save` to verify an actual sandbox application writes
through its returned document URI into a folder explicitly created in Iced. The
fixture reuses the installed probe and runtime, preserves the unshared original,
and keeps the shared store plus this evidence below 136 MiB.
Add `--check-folder-history` to verify that the backend stores only that accepted
folder under the sandbox application's authenticated ID.

This is an owned minimal-runtime GIO application check, distinct from automatic
GTK/Qt client routing and wider distribution/runtime acceptance. Exact setup,
failed fixture controls and the final receipt are recorded in
[the sandbox replay notes](../.scratch/xdg-portals/file-chooser-flatpak/README.md).
