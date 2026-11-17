# Desktop portals

Applications, including sandboxed Flatpak apps, open file dialogs, take
screenshots and read desktop preferences through XDG desktop portals. The public
`xdg-desktop-portal` frontend receives each request, identifies the application,
stores permissions and exports documents into sandboxes. It then forwards the
request to a backend. Canopa ships two backends:

- `xdg-desktop-portal-canopa` shows Canopa's own dialogs. It also hosts the
  screen sharing interfaces described in [Desktop sharing](desktop-sharing.md)
  and the optional Print interface described in [Printing](printing.md).
- Canopa Notify serves the Notification portal, so portal notifications appear
  as ordinary Canopa banners and Notification Center entries.

Every dialog is a Rust/Iced window that uses the shared Canopa appearance. No
dialog loads GTK.

## What Canopa provides

| Interface | What the user sees | Packaged Canopa session |
| --- | --- | --- |
| Settings | Applications follow Canopa's dark/light scheme, accent, contrast and reduced-motion preferences | Canopa |
| FileChooser | Canopa's Open, Save and folder chooser | Canopa |
| AppChooser | "Open with" application list | Canopa |
| Access | Allow/Deny questions, such as camera access | Canopa |
| Account | Review of the name, username and picture shared with an app | Canopa |
| Background | A notification and review dialog for apps running without windows | Canopa |
| DynamicLauncher | Review before an app installs a launcher or web app | Canopa |
| Lockdown | Switches that turn off saving, printing, app handlers, camera and other operations | Canopa |
| Wallpaper | Review before an app sets the wallpaper | Canopa |
| Screenshot | Screen, area or window screenshot review, and the color picker | Canopa |
| Notification | Banners and Notification Center entries | Canopa Notify |
| ScreenCast, RemoteDesktop, Clipboard | Screen sharing and remote control consent | Canopa; see [Desktop sharing](desktop-sharing.md) |
| InputCapture, GlobalShortcuts, Usb | Consent dialogs | Opt-in; see [Desktop sharing](desktop-sharing.md) |
| Print | Print dialog | Opt-in; see [Printing](printing.md) |

Interfaces that Canopa does not advertise, such as Print and Email in the
packaged session, fall back to `xdg-desktop-portal-gtk`. Secret goes to the
keyring provider selected by the packages (see [Installation](installation.md)).

## How routing works on a packaged install

The Arch packages configure everything; nothing needs enabling.

1. `canopa-session` starts the session with `XDG_CURRENT_DESKTOP=canopa:niri`
   and `XDG_SESSION_DESKTOP=canopa`.
2. The user unit `xdg-desktop-portal.service` has a drop-in,
   `/usr/lib/systemd/user/xdg-desktop-portal.service.d/50-canopa.conf`, that
   starts `/usr/lib/canopa/portal-frontend`. When `CANOPA_SESSION=1` and
   `XDG_SESSION_DESKTOP=canopa`, this wrapper exports
   `XDG_CURRENT_DESKTOP=canopa:niri` again (Niri started with `--session`
   replaces it) and runs Canopa's frontend build,
   `/usr/lib/canopa/xdg-desktop-portal`. In any other session it runs the
   distribution frontend, `/usr/lib/xdg-desktop-portal`.
3. Because the current desktop is `canopa`, the frontend reads
   `canopa-portals.conf`:

   ```ini
   [preferred]
   default=canopa;gtk;
   org.freedesktop.impl.portal.Notification=canopa-notify;
   org.freedesktop.impl.portal.Secret=gnome-keyring;
   ```

   The `canopa-oo7` package installs the same file with
   `Secret=oo7-portal;` at `/etc/xdg/xdg-desktop-portal/canopa-portals.conf`.
4. `default=canopa;gtk;` sends every interface listed in Canopa's descriptor,
   `/usr/share/xdg-desktop-portal/portals/canopa.portal`, to Canopa, and
   everything else to GTK. The installed descriptor lists the ten dialog
   interfaces plus ScreenCast, RemoteDesktop and Clipboard. Notify's
   descriptor, `canopa-notify.portal`, lists Notification.
5. The backends start on demand through D-Bus activation:
   `org.freedesktop.impl.portal.desktop.canopa` runs
   `xdg-desktop-portal-canopa.service`, and
   `org.freedesktop.impl.portal.desktop.canopa.notify` runs
   `canopa-notify.service`, which the session already starts.

### Configuration file precedence

The frontend uses the first `canopa-portals.conf` it finds and does not merge
files:

| Path | Provided by |
| --- | --- |
| `~/.config/xdg-desktop-portal/canopa-portals.conf` | You |
| `/etc/xdg/xdg-desktop-portal/canopa-portals.conf` | `canopa-oo7` package |
| `/usr/share/xdg-desktop-portal/canopa-portals.conf` | `canopa` package |

A user file therefore replaces the packaged routing completely. If an
application still opens another desktop's dialog, check for an older user file.

### Route one interface elsewhere

Copy the file currently in effect and add a line for the interface. For example,
to use the GTK file chooser:

```sh
mkdir -p ~/.config/xdg-desktop-portal
cp /usr/share/xdg-desktop-portal/canopa-portals.conf ~/.config/xdg-desktop-portal/
printf 'org.freedesktop.impl.portal.FileChooser=gtk;\n' \
  >> ~/.config/xdg-desktop-portal/canopa-portals.conf
systemctl --user restart xdg-desktop-portal.service
```

Copy from `/etc/xdg/xdg-desktop-portal/` instead if `canopa-oo7` is installed.
Restart the frontend when no portal dialog or screen share is open, or log out
and back in.

The special value `none` disables an interface entirely, wherever it appears in
the list: `org.freedesktop.impl.portal.Screenshot=none;` turns screenshots off
for portal clients. It is not a fallback.

## Opt-in interfaces

The backend always serves InputCapture, GlobalShortcuts and Usb on the bus, but
the frontend routes an interface only when the descriptor advertises it. The Arch
packages do not advertise these three. A release archive advertises them when it
is built with `scripts/package-release.sh ... --desktop-niri RECEIPT
--desktop-frontend RECEIPT --extra-portals`. See
[Desktop sharing](desktop-sharing.md).

Print is served only when the backend runs with `--experimental-print`, and it
is not in the shipped descriptor. See [Printing](printing.md) for the full setup.

## Using Canopa portals without the packages

The release archive contains:

| Archive path | Purpose |
| --- | --- |
| `bin/xdg-desktop-portal-canopa` | Backend |
| `bin/canopa-portal-config` | Routing generator (below) |
| `share/dbus-1/services/org.freedesktop.impl.portal.desktop.canopa.service` | Backend activation |
| `share/dbus-1/services/org.freedesktop.impl.portal.desktop.canopa.notify.service` | Notify portal activation |
| `share/systemd/user/xdg-desktop-portal-canopa.service` | On-demand user unit |
| `share/xdg-desktop-portal/portals/canopa.portal`, `canopa-notify.portal` | Descriptors |

The archive's `canopa.portal` lists the ten dialog interfaces. It also lists
ScreenCast, RemoteDesktop and Clipboard, plus InputCapture, GlobalShortcuts and
Usb, when the archive was built with those components.

1. Put `xdg-desktop-portal-canopa` in `~/.local/bin`. The supplied unit runs
   `%h/.local/bin/xdg-desktop-portal-canopa`. Edit its `ExecStart` for another
   location.
2. Copy both activation files to `~/.local/share/dbus-1/services/` and the unit
   to `~/.config/systemd/user/`, then run `systemctl --user daemon-reload`. The
   unit has no `[Install]` section and needs no enabling. Without systemd
   activation, the bus runs `/usr/bin/env xdg-desktop-portal-canopa` through its
   own `PATH`. Use an absolute path in the activation file if needed.
3. Install the two descriptors in the frontend's portal directory, normally
   `/usr/share/xdg-desktop-portal/portals/`.
4. Select Canopa in the portal configuration your session reads. A
   `niri-session` login runs Niri with `--session`, which sets
   `XDG_CURRENT_DESKTOP=niri`, so the frontend reads `niri-portals.conf`. The
   descriptors' `UseIn=canopa` does not match that desktop, so routing must be
   explicit. Generate the file with `canopa-portal-config`, review it, and
   install it as `~/.config/xdg-desktop-portal/niri-portals.conf`.
5. Restart the frontend (`systemctl --user restart xdg-desktop-portal.service`)
   or log in again.

Canopa Notify serves the Notification portal only in service mode
(`canopa-notify daemon --service`); see [Notifications](notifications.md).
`scripts/update.sh` does not build or install `xdg-desktop-portal-canopa`, so
install a new backend by hand after updating.

### Generate a routing file

`canopa-portal-config` (from `scripts/portal-config.py`, Python 3 standard
library only) reads the Canopa descriptors next to it and merges Canopa routes
into a complete configuration you choose. It never installs files or restarts
services.

| Option | Meaning |
| --- | --- |
| `--base FILE` | The complete configuration your session uses now. Required unless `--list`. Never modified. |
| `--interface NAME` | Interface to route to Canopa, short (`FileChooser`) or full name. Repeat for more. Default: Settings and FileChooser. |
| `--portal-dir DIR` | Descriptor directory. Default: the archive's `share/xdg-desktop-portal/portals`. |
| `--output FILE` | Write a new file with mode 0600. Refuses existing files and symlinks. Default: standard output. |
| `--list` | Print the interfaces the descriptors advertise and their backend. |

```sh
bin/canopa-portal-config --list
bin/canopa-portal-config --base /usr/share/xdg-desktop-portal/niri-portals.conf \
  --interface Settings --interface FileChooser --interface Notification \
  --output /tmp/niri-portals.conf.canopa
diff -u /usr/share/xdg-desktop-portal/niri-portals.conf /tmp/niri-portals.conf.canopa
```

Selected interfaces get Canopa first, followed by the interface's previous
backends (or the `default` list) as fallbacks. Notification routes to
`canopa-notify`. Comments, other groups and unselected entries stay as they
were. A selected interface whose list contains `none` is replaced by Canopa
alone, because `none` would veto the whole list. Unselected `none` entries stay.

## Backend command line

| Option | Effect |
| --- | --- |
| `--canopa-config FILE` | Read the shared appearance from this file instead of discovering `$XDG_CONFIG_HOME/canopa/config.kdl`. Conflicts with `--defaults`. |
| `--defaults` | Use the embedded appearance and read no configuration file. Lockdown policy stays active. |
| `--no-file-chooser-history` | Do not read or write per-application FileChooser folders. |
| `--lockdown-policy FILE` | Lockdown database to use. Default: `$XDG_STATE_HOME/canopa/portal/lockdown.sqlite3`. |
| `--lockdown-read-only` | Publish an existing Lockdown database and deny every change. |
| `--experimental-print` | Also serve Print and the printer job review. See [Printing](printing.md). |
| `--printer-jobs` | Open the printer job review of a running backend started with `--experimental-print`. |
| `--recover-print` | Review retained print files and unresolved printer jobs. |
| `--print-recovery-store DIR` | Override the print recovery store. |
| `--version`, `--help` | Print version or usage. |

The backend also has hidden modes that it uses to run its own dialogs and
workers. Do not start those directly.

On a packaged install, add options with a drop-in:

```sh
systemctl --user edit xdg-desktop-portal-canopa.service
```

```ini
[Service]
ExecStart=
ExecStart=/usr/bin/xdg-desktop-portal-canopa --no-file-chooser-history
```

Then run `systemctl --user restart xdg-desktop-portal-canopa.service`.

### Reload

```sh
systemctl --user reload xdg-desktop-portal-canopa.service
```

Reload sends SIGHUP. The backend reloads the shared appearance and the Lockdown
policy independently. A rejected candidate keeps the previous values and logs
`event=reload_rejected` (appearance) or `event=policy_reload_rejected`
(Lockdown) in the journal. The command itself only confirms that the signal was
delivered. Saving the Personalization page in Canopa Settings reloads the
backend automatically. Invalid configuration at startup stops the backend from
starting.

## Portal notes

### Settings

Settings publishes four keys in the `org.freedesktop.appearance` namespace,
taken from the `appearance` block of `config.kdl` (see
[Configuration](configuration.md) and
[Theme customization](theme-customization.md)):

| Key | Value |
| --- | --- |
| `color-scheme` | 1 for `color-scheme "dark"`, 2 for `"light"` |
| `accent-color` | The effective accent as three doubles in 0–1, including the theme's accent when no `accent` is set |
| `contrast` | 1 with `high-contrast #true`, otherwise 0 |
| `reduced-motion` | 1 with `reduced-motion #true`, otherwise 0 |

A change emits `SettingChanged` for each value that actually changed.
Applications decide how to use these preferences. Read what applications see:

```sh
gdbus call --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.portal.Settings.ReadOne \
  org.freedesktop.appearance color-scheme
```

When `config.kdl` does not exist, the shared dark defaults apply. Creating or
removing the file takes effect at the next reload. A file chosen with
`--canopa-config` stays selected; if it disappears, reloads are rejected.

### File chooser

The chooser opens files or folders (one or several), saves a file, or picks a
folder for several files (SaveFiles). It honors the application's title, accept
label, initial folder, file and name, filename and MIME filters, extra choices
and the read-only option. Places lists Home, Documents, Downloads and Pictures
(when those XDG folders exist) and Filesystem.

| Key | Action |
| --- | --- |
| Ctrl+L | Focus Location |
| Ctrl+F | Focus the name filter |
| Ctrl+H | Show or hide hidden files |
| Ctrl+A | Select all visible items (multiple selection only) |
| Alt+Up | Parent folder |
| Up, Down | Move through the list |
| Enter | Open the focused folder, or accept |
| Ctrl+Shift+N | New folder (Save dialogs) |
| Escape | Cancel, or close the New folder or Replace prompt |

- **Saving.** The chooser never creates or truncates the file; the application
  writes it. An existing destination needs Replace. A destination that is a
  folder, symlink or special file is refused.
- **New folder.** Available in Save dialogs. Type one name, then Create or Enter.
  The folder is created and opened, and the file name you typed is kept. Existing
  entries are never replaced. A folder you created stays if you then cancel the
  save.
- **Filenames** keep their original bytes, including names that are not valid
  UTF-8. MIME filters match by filename only (shared-mime-info glob rules), not
  by content.
- **Folder memory.** When an identified application gives neither an initial
  folder nor an initial file, the chooser opens in the last folder you accepted
  for that application. Cancelling changes nothing. Only application IDs and
  folder paths are stored, in `$XDG_STATE_HOME/canopa/portal-file-chooser.db`
  (normally `~/.local/state/canopa/portal-file-chooser.db`), for at most 128
  applications and 2 MiB. Remembering a folder does not grant the application
  access to it. Turn this off with `--no-file-chooser-history`; to forget all
  folders, stop the backend and delete the file.
- **GTK 4 applications** using `GtkFileDialog` use the portal automatically when
  the frontend is D-Bus activatable (`org.freedesktop.portal.Desktop.service`
  is installed). Setting `GTK_USE_PORTAL` is not required. Applications that
  build their own GTK file widgets keep them.
- **Qt applications** use the portal with the Qt `xdgdesktopportal` platform
  theme plugin installed and selected, for example:

  ```sh
  QT_QPA_PLATFORM=wayland QT_QPA_PLATFORMTHEME=xdgdesktopportal YOUR_QT_APPLICATION
  ```

  The packaged Canopa session sets `QT_QPA_PLATFORMTHEME=xdgdesktopportal` unless
  you already chose a platform theme (ADR 0154); other sessions need it set by
  hand, and the plugin must be installed (Arch: part of `qt6-base`). A `QFileDialog`
  created before the application's event loop starts uses Qt's own dialog.
  Dialogs opened after startup reach Canopa.

### App chooser

Shows the applications the frontend offers, with names and icons from their
desktop entries, including handlers hidden from the launcher. Search covers
names, IDs, generic names and keywords (Ctrl+F). Up and Down move, Enter
chooses, Escape cancels. The list updates live when the offered applications
change. The chooser asks the compositor for a fresh activation token after you
choose, so the selected application can take focus. The frontend launches the
application and sets defaults; the dialog does neither.

### Access

A plain question with Allow and Deny, an optional icon and optional checkboxes
or drop-downs. Only an explicit Allow grants. Deny, Escape, closing the window
and the requesting app quitting all deny. Opening the dialog or pressing Enter
does not grant. The frontend stores the decision.

### Account

Shows your username, name and profile picture from AccountsService (with an NSS
fallback for missing fields and `~/.face` or `~/.face.icon` when AccountsService
is unavailable). You can edit or exclude each field and choose or clear the
picture before pressing Share. Excluded fields are sent as empty strings.
Choose image opens the file chooser inside the same window. Clearing the picture
only changes what is shared, not your account.

- Source pictures are PNG, JPEG, GIF, WebP or SVG, up to 8 MiB (SVG 256 KiB).
  They are converted to a PNG of at most 256 px with metadata removed.
- Only Share publishes the picture, into
  `$XDG_CACHE_HOME/canopa/portal/account/`. The cache keeps at most 64 images
  and 16 MiB, and keeps each recent image for at least 60 seconds. Applications
  that need the picture longer should copy it.

### Background

When a sandboxed application keeps running without windows, the frontend asks
the backend. Canopa shows a notification "*App* is running in the background"
with three actions:

| Action | Result |
| --- | --- |
| Allow | Permission to run in the background |
| Allow once | Keep the current run, permission unchanged |
| Block | Refuse; the frontend can stop the current run |

Clicking the notification opens a review dialog with the same choices. Closing
the dialog, Escape or no answer keep the current run without changing the
permission. Questions expire after 20 seconds, also under Do Not Disturb.

Background needs Canopa Notify, which provides the timed questions, and the
session's `NIRI_SOCKET` in the activation environment, which Niri imports. It
tracks windows only of verified Flatpak applications (by their
`.flatpak-info`), never by the Wayland app ID a client declares. When Niri
cannot be observed, the backend reports a failure and the frontend skips that
check. The frontend stores permissions, stops applications and writes autostart
files.

### Dynamic launcher

Before an application or web app installs a launcher, Canopa shows the requesting
app, the icon, the name (editable when the app allows it) and, for a web app, its
address. Only Install approves. The icon is shown as sent and cannot be edited.
The frontend writes and removes the desktop entry.

- Icons: PNG or JPEG up to 512×512, SVG up to 4096×4096, at most 1 MiB.
- Names: at most 1024 bytes, no control characters, not blank.
- Unattended installs (`RequestInstallToken`) are always denied. The frontend
  can still let unidentified host processes install without asking the backend.

### Lockdown

Lockdown has no dialog. It holds seven switches that the frontend consults
before running the matching operations:

| Property | When `true` |
| --- | --- |
| `disable-printing` | Printing and print preparation are refused |
| `disable-save-to-disk` | SaveFile and SaveFiles are refused |
| `disable-application-handlers` | Opening URIs, files and folders in other applications is refused |
| `disable-location` | Location access is refused |
| `disable-camera` | Camera access is refused |
| `disable-microphone` | Published for consumers that support it |
| `disable-sound-output` | Published for consumers that support it |

Read all switches:

```sh
gdbus call --session \
  --dest org.freedesktop.impl.portal.desktop.canopa \
  --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.DBus.Properties.GetAll \
  org.freedesktop.impl.portal.Lockdown
```

Turn off camera access (use `'<false>'` to turn it back on):

```sh
gdbus call --session \
  --dest org.freedesktop.impl.portal.desktop.canopa \
  --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.DBus.Properties.Set \
  org.freedesktop.impl.portal.Lockdown disable-camera '<true>'
```

Changes take effect immediately and persist in
`$XDG_STATE_HOME/canopa/portal/lockdown.sqlite3` (or
`~/.local/state/canopa/portal/lockdown.sqlite3`). The file is created on the
first change; without it every switch is `false`.

- `--lockdown-policy FILE` selects another database (an absolute path).
- `--lockdown-read-only` requires an existing valid database, publishes it and
  answers every change with AccessDenied. An administrator can supply a fixed
  policy this way.
- A malformed, symlinked or oversized (over 1 MiB) database stops the backend
  from starting. On reload it is rejected and the previous switches stay.
- A tool that edits the database directly must use SQLite transactions and then
  reload the backend.

### Wallpaper

Canopa shows the requesting app, the picture, its size and where it will be
set: desktop background, lock screen or both. A preview always appears, even if
the app asks for none. Only Apply changes anything. When the app does not say
where, Canopa sets the desktop background and leaves the lock image alone.

The backend copies the picture before showing it, so changing or deleting the
original afterwards does not change what is applied. Canopa Shell stores the
selection under `$XDG_STATE_HOME/canopa/shell/wallpaper` and draws it on every
output, so Shell must be running. The selection survives restarts.

- Local files arrive through `SetWallpaperFile`; `SetWallpaperURI` accepts
  `http`/`https` addresses only.
- PNG, JPEG, GIF and WebP up to 32 MiB; animated images use a still frame.
- Downloads verify TLS, follow at most five redirects and stop after 20 s.
- The lock-screen image appears on `canopa-lock` unless the `lockscreen` block
  sets its own `image` (see [Session tools](session-tools.md)), and on
  `canopa-shell lock`.
- Wallpapers set from Canopa Settings belong to `canopa-wallpaper` and are
  stored separately from portal selections.

### Screenshot

Screenshot offers Screen (all outputs), Area, Window and the color picker. The
pixels are frozen before the review opens, so later changes on screen do not
change the result.

- **Review.** Share publishes the image; Use color returns the selected pixel's
  color. Cancel, Escape or closing the window publish nothing.
- **Area.** Drag in the preview, or move with the arrow keys and press Space to
  start and finish (Shift moves ten pixels).
- **Color.** Click a pixel or use the arrow keys; Next display switches outputs.
- **Window.** Alt+W, or an app's request for a window, opens a window picker.
  The image contains the whole window, even on another workspace. The active
  window option captures the focused window and fails when there is none.
  Window capture needs the Canopa Niri build (packaged as `canopa-compositor`)
  and PipeWire. Without them only Screen, Area and the color picker are offered.
- **Permission.** For a non-interactive request the frontend asks for
  permission (in the Access dialog) and remembers the answer. A permitted
  non-interactive full-screen request then completes without review.
  Interactive requests, areas, windows and colors are always reviewed.

Screen and Area need a compositor with `wlr-screencopy` v3 and `xdg-output` v2.
Several outputs are combined at the highest pixel density with transparent gaps.
Screenshots never touch the clipboard.

Images are stored as read-only PNGs in `$XDG_CACHE_HOME/canopa/portal/screenshots`
(mode 0700, files 0400). Each is kept for at least 24 hours; the cache holds at
most 128 files and 1 GiB. When recent screenshots fill it, new requests fail.
Applications that keep screenshots should copy them.

Niri's own screenshot keys (Print, Ctrl+Print, Alt+Print) are separate and save
to `~/Pictures/Screenshots`.

### Notification

Canopa Notify implements Notification version 2 under
`org.freedesktop.impl.portal.desktop.canopa.notify` while it runs as the session
service. Portal notifications follow Notify's Do Not Disturb, sounds and history
(see [Notifications](notifications.md)).

- An application ID plus notification ID identifies a notification. Sending the
  same pair again replaces it; withdrawing it closes it. Notifications survive
  the sending app quitting.
- The banner hides after Notify's normal timeout; the entry stays actionable in
  the Center until you dismiss it or the app withdraws it. `urgent` priority
  stays on screen.
- Priority `low` maps to low urgency, `normal` and `high` to normal, `urgent` to
  critical.
- Buttons and the default action keep their targets. Actions named `app.*` are
  delivered by activating the application; others return through the portal.
- `markup-body` supports bold, italic and links. Icons can be themed names or
  PNG, JPEG or SVG data up to 1 MiB; sounds can be default, silent, or Ogg/WAV
  data up to 8 MiB.
- Categories `im.received`, `alarm.ringing` and `call.incoming` and button
  purposes for replying and call controls get Canopa-specific presentation.
  Display hints such as `transient`, `tray`, `persistent`,
  `hide-on-lock-screen` and `show-as-new` are honored within Notify's policy.

## Dialog windows on Niri

Portal dialogs use the Wayland app ID `org.canopa.Portal`. When an application
passes its window as the parent, the dialog attaches to it (xdg-foreign) and
Niri opens it floating, like other parented dialogs. Without a parent, or when the
parent handle has expired, the dialog is an independent window. The packaged
session's Niri defaults float it anyway (ADR 0154). In your own Niri
configuration, outside the packaged session, add the same rule to float them:

```kdl
window-rule {
    match app-id=r#"^org\.canopa\.Portal$"#
    open-floating true
}
```

## Troubleshooting

Check which desktop the frontend sees, and that the Canopa frontend is in use:

```sh
systemctl --user show-environment | grep -E 'XDG_CURRENT_DESKTOP|CANOPA_SESSION'
systemctl --user cat xdg-desktop-portal.service
systemctl --user status xdg-desktop-portal.service xdg-desktop-portal-canopa.service
```

Read the logs:

```sh
journalctl --user -b -u xdg-desktop-portal.service -u xdg-desktop-portal-canopa.service
```

List the portal names on the bus and the interfaces each side exposes:

```sh
busctl --user list | grep -i portal
busctl --user introspect org.freedesktop.impl.portal.desktop.canopa /org/freedesktop/portal/desktop
busctl --user introspect org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop
```

The backend serves more interfaces than the descriptor advertises. Only
advertised interfaces appear on the public side.

| Symptom | Check |
| --- | --- |
| A GTK or GNOME dialog opens instead of Canopa's | A user `canopa-portals.conf` or `niri-portals.conf` override, or the interface is not in `canopa.portal` |
| Nothing happens for an interface | A `none` entry in the configuration in effect |
| Settings changes do not reach apps | Run the reload and check the journal for `reload_rejected` |
| Background questions never appear | Canopa Notify is running, and `NIRI_SOCKET` is in `systemctl --user show-environment` |
| Window screenshots are not offered | The session runs `canopa-compositor`, and PipeWire is running |
| Wallpaper requests fail | Canopa Shell is running |

After changing routing, restart the frontend with
`systemctl --user restart xdg-desktop-portal.service` or log in again.

## Known limitations

- InputCapture, GlobalShortcuts and Usb are not advertised by the Arch packages.
- Print is opt-in: start the backend with `--experimental-print`; Print otherwise goes to GTK.
- Parent windows are supported only for Wayland (`wayland:` handles); X11 parents open an independent dialog.
- Niri has no `xdg-dialog-v1`, so the modal hint has no effect.
- The Screenshot review cannot be confirmed from the keyboard.
- The Account and Screenshot dialogs publish no AT-SPI accessibility tree.
- The file chooser has no previews, remote locations or content-based type detection.
- Dynamic launcher icons cannot be edited, and Canopa has no trusted-installer policy.
- Background observation relies on numeric process IDs reported by Niri.
- Background can observe only Flatpak applications; other sandbox formats report a failure.
- The Wallpaper lock-screen image applies to `canopa-shell lock` only, not to `canopa-lock`.
- Canopa Settings has no page for the Lockdown switches.
- Qt applications need the `xdgdesktopportal` platform theme, which the session does not set.
