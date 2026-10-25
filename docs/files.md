# Canopa Files runtime and installation

Canopa Files is an independent native application for local files and removable
drives. Run `canopa-files /path/to/folder`; multiple local paths become tabs in a
new window. Local `file://` URIs also work. The included Files README describes
keyboard navigation, Quick Look, operations, Undo, custom actions and preferences.
Implementation of the full product brief is still in progress.

Later launches activate the runtime already owning the same state directory.
`canopa-files --daemon` starts that backend without a browser and keeps it resident
while idle; run it from the graphical session to open windows later. The command
stays in the foreground, so a terminal can use `canopa-files --daemon &`.
Iced initializes on the first window request. `canopa-files --quit` releases
residency and exits after windows, accepted work, pending decisions, clipboard
ownership and unsaved results finish. It does not cancel work or answer questions.
A start without display variables remains headless and refuses browser requests.
Quit it before starting in the graphical session if windows are needed.

The owning backend also implements the standard session-bus FileManager1
interface. Other applications can open folders, reveal selected items in their
parents or show read-only Properties. Multiple folders become tabs; revealing
items groups them by parent and makes explicitly requested hidden files visible
without showing their hidden siblings. Properties is an independent, tilable
window with captured metadata, per-item errors and Copy locations. Escape or
Ctrl+W closes it. Its metadata preparation and URI handling use the shared core.
The resident host and Iced currently share a process; closing windows preserves
accepted work, while frontend crash isolation remains future work.

Ordinary launches inherit the owner's configuration. An explicit selection must
match the owner's original selection. Compatible launches after an
update continue to use the running implementation; finish work, close its windows
and use `--quit` before starting the newly installed executable.

The development executable now uses the workspace's Iced stack with `canopa-ui`,
per [ADR 0018](adr/0018-files-iced-and-shared-controls.md). The first adapter
implements browsing, keyboard navigation, editable locations, tabs, windows,
cancellable filename search and source-bound floating text/source/image/PDF previews.
Space opens Quick Look; its default keyboard focus continues source navigation.
Images support Fit, zoom and 100%; PDFs support Page Up/Down. PDF preparation
requires Poppler's `pdfinfo` and `pdftoppm`. The preview uses the shared Iced
transient-window operation on Wayland; no application-specific Niri rule is needed.
New Folder and Rename use floating Iced dialogs; Ctrl+Z and Operations share
the core filesystem worker and Undo history across windows. Interactive Undo
questions retain their independent operation when a Browser closes. Transfers,
Trash/Recovery, media playback and device controls are connected and undergoing
acceptance. This development slice is not a release of the full
product brief. Source previews include prepared syntax colors. Audio/video use a
renderer-independent GStreamer worker and open paused. Tab enters Play/Pause,
Seek, Mute and Volume after Open; focused seek/volume controls own arrows and
Home/End. Other preview keys continue navigating the source browser.

Copies retain regular-file permission bits and access/modification times; new
files belong to the copying user. Symbolic links remain links, including dangling
links. Hard links become separate files, sparse files allocate their full logical
size, and ACLs and extended attributes are currently omitted. The shared copy
preparation and its verification are documented in
[ADR0153](files/adr/0153-uutils-prepared-copy.md).

The Iced executable uses GIO/GLib 2.58 or newer for live local-volume observation
through a private helper process. These are platform libraries, with no GTK widgets. The
helper is the same executable in an internal mode and requires no separate
installation. If observation stops, Operations reports the failure and refuses
further filesystem changes until Files restarts. Building this feature requires
the distribution's GIO/GLib development libraries.

Local devices appear in the sidebar and in the folder menu's Locations submenu,
including narrow tiles. Open a mounted device directly or select an unmounted
device to mount it. Its removal action follows the platform's Power off, Eject
or Unmount capability. Operations shows progress, cancellation, results and
questions. Password questions float independently of the browser's tile; Files
does not save the credentials. Closing the originating browser dismisses that
dialog while the backend keeps the request. Open Operations and Review to return
to it. Explicit Cancel, Escape or closing the question itself cancels the request.
Successful mounts preserve newer navigation and never reopen a closed origin.
Mount/Unmount and Power off/Eject have private-provider acceptance in both
appearances, including delayed drive removal with every window closed and
same-owner reopening. These checks exercise actual GIO callbacks on simulated
devices; physical-drive acceptance remains pending.

The Iced custom-action editor and command Operations interface are now implemented
and undergoing acceptance. They reuse the shared Canopa controls and configuration,
with configured toolbar order, narrow command strips, keyboard traversal, captured
command arguments, bounded output and cancellation. Optional action icons use shared XDG assets. The complete
configuration/condition/focus matrix and durable restart integration remain unfinished.

Global Preferences opens with Ctrl+comma and edits Views, Toolbar, Locations and
Shortcuts with explicit Save. Default view shortcuts are Ctrl+1 for columns,
Ctrl+2 for list and Ctrl+3 for icons. Folder choices remain separate from global
defaults; Use defaults clears a folder's overrides.

Desktop, Documents, Downloads and Pictures follow the user's XDG directory
definitions. Both sidebar and narrow Locations use the same paths, including
localized names and paths outside Home. Definitions are read at launch; restart
Files after changing them. Missing or invalid definitions use conventional Home
subfolders. See [the personal-folder decision](files/adr/0064-xdg-personal-folder-locations.md).

The Iced executable links GStreamer core, app and video libraries directly and
uses the distribution's decoder, conversion and audio-output plugins. It does
not require GTK's media widget backend. PDF previews use Poppler's
`pdfinfo` and `pdftoppm`. Images and bounded text/code previews are prepared by
Files itself. Preview playback starts only when requested.

On Ubuntu 24.04, install the runtime dependencies with:

```sh
sudo apt-get install libgstreamer1.0-0 libgstreamer-plugins-base1.0-0 \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-libav poppler-utils gvfs \
  desktop-file-utils
```

The [GStreamer libav package](https://packages.ubuntu.com/noble/gstreamer1.0-libav)
supplies FFmpeg decoders. Equivalent packages are required on other distributions;
they retain their distribution license notices. The archive does not bundle
GStreamer, Poppler or a private codec copy. Building from source requires
`pkg-config`, GIO/GLib development libraries and the GStreamer
[core development package](https://packages.ubuntu.com/noble/libgstreamer1.0-dev)
and [base development package](https://packages.ubuntu.com/noble/libgstreamer-plugins-base1.0-dev).
Audio needs the session's working audio service. GIO/GVfs supplies local volume
discovery and mount requests.

The optional `media-playback` Cargo feature is part of the default desktop build.
`local-runtime` can still build the headless application without Iced or media.
Media initialization and decoder/plugin discovery wait for an actual preview;
ordinary browsing and `--daemon` startup create no playback pipeline. Native
codec/control acceptance and release packaging remain in progress.

The archive contains the executable, Files README/licenses, this guide,
`share/applications/org.canopa.Files.desktop` and optional FileManager1
activation metadata. For a per-user installation from
the extracted archive directory:

```sh
install -Dm755 bin/canopa-files "$HOME/.local/bin/canopa-files"
install -Dm644 share/applications/org.canopa.Files.desktop \
  "$HOME/.local/share/applications/org.canopa.Files.desktop"
export PATH="$HOME/.local/bin:$PATH"
update-desktop-database "$HOME/.local/share/applications"
```

The application entry makes Files available in launchers and folder application
choices. Registration preserves the existing default file manager. To choose
Files as the default folder handler explicitly:

```sh
xdg-mime default org.canopa.Files.desktop inode/directory
```

To select Files as an on-demand FileManager1 provider, separately install its
session-bus activation file:

```sh
install -Dm644 share/dbus-1/services/org.freedesktop.FileManager1.service \
  "$HOME/.local/share/dbus-1/services/org.freedesktop.FileManager1.service"
```

This selects the standard desktop presentation provider independently of the
folder MIME default. It starts `canopa-files --daemon` on the first bus request,
using the session PATH and graphical environment. Files leaves another manager's
already owned bus name untouched. Finish and quit an older Files owner before
using newly installed activation support. FileManager1 activation does not set
the desktop entry's distinct `DBusActivatable` contract.

Ordinary CLI launches run on demand. The activation file opts into residency;
`--quit` releases it. Packaging starts no service and installs no autostart or
resident systemd unit; Shell and Notify retain their existing services.
Configuration is shared through `canopa/config.kdl`; `canopa-files --check-config`
validates its Files and appearance settings without opening a window.

A preview that cannot decode a file displays a short explanation and keeps Open
in default application available. Detailed media diagnostics use the
`[canopa-files media]` log prefix. Codec availability is a runtime property;
a successful configuration check or `--help` does not verify decoding. The native
acceptance workflows exercise real decoded frames and isolated recorded audio.
