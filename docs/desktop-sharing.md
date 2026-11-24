# Desktop sharing

Canopa's portal backend, `xdg-desktop-portal-canopa`, shows its own dialogs when
an application asks to share the screen, control the desktop, use the clipboard,
capture input, register global shortcuts or open a USB device. Every request
opens a consent dialog, and nothing is allowed until you choose it. While a
sharing session runs, a small window shows what is shared and lets you stop it.

These interfaces need Canopa's builds of the Niri compositor and the XDG portal
frontend. The Arch packages ship both. For other installs, see
[Portable installs](#portable-installs).

For the general portal setup and routing, see [Desktop portals](portals.md).

## What is available

| Interface | What it does | Arch packages | Portable archive |
| --- | --- | --- | --- |
| ScreenCast | Share displays or windows as video (browsers, meeting apps, recorders) | On | With the desktop-sharing component |
| RemoteDesktop | Let an application send keyboard, pointer and touch input | On | With the desktop-sharing component |
| Clipboard | Let a remote-desktop or input-capture session read and replace the clipboard | On | With the desktop-sharing component |
| InputCapture | Let an application take over your keyboard and pointer when the pointer crosses a screen edge (input-sharing tools) | Opt-in, see below | With `--extra-portals` |
| GlobalShortcuts | Let an application register shortcuts that work while other windows are focused | Opt-in, see below | With `--extra-portals` |
| Usb | Let a sandboxed application open chosen USB devices | Opt-in, see below | With `--extra-portals` |

The backend always serves all six interfaces. Whether applications reach them
depends on the portal descriptor `canopa.portal`, which lists the interfaces the
frontend may route to Canopa. The Arch package installs a descriptor that adds
ScreenCast, RemoteDesktop and Clipboard to Canopa's dialog portals. InputCapture,
GlobalShortcuts and Usb are not listed. GTK's portal does not provide them
either, so by default applications in the Canopa session have no InputCapture,
GlobalShortcuts or Usb portal.

## Sharing your screen

When an application asks to share the screen, a dialog lists your displays and
windows. Window rows include a thumbnail of the current contents and the
application icon when available. Canopa uses the same Niri preview service as
the Shell taskbar. The window title and application name remain available if a
preview cannot be rendered. Nothing is selected at first.

- A search field appears when there are more than three sources. Press Tab to
  move focus.
- Up and Down move through the list; Space selects or clears a source.
- When one source is selected, its name appears above the confirmation buttons.
  With a remote session, View only means the app gets the selected picture
  without input control or clipboard access. Choose Share or press Ctrl+Enter.
- Escape, Cancel or closing the dialog shares nothing.

If the application allows several sources, you can choose up to 16. A source that
closes or disappears while the dialog is open is removed from the selection. If a
display is replaced (for example, a different monitor on the same connector), you
must select it again.

The application can ask for the cursor to be hidden, drawn into the video or sent
separately as metadata. Metadata needs PipeWire 1.4.8 or newer; with an older
PipeWire, the backend offers only the other two modes.

Approval lasts for one session. Canopa does not remember grants, so the next
session asks again even if the application asks to restore a previous one.

## Remote control and clipboard

A remote-desktop request uses the same dialog, with separate choices for each
kind of access the application asks for:

- Allow keyboard control of your desktop
- Allow pointer control of your desktop
- Allow touch control (Alt+T)
- Allow clipboard read and write (Alt+C)

Every choice starts unchecked. Allowing one does not allow the others. Touch
control works only inside a display or window you share, so it also needs a
selected source. Pointer control of exact screen positions likewise needs a
shared source; without one, the application can only move the pointer relative
to where it is.

The clipboard choice lets the application read and replace what you copy. It is
available only together with a remote-desktop or input-capture session, and it
ends with that session.

## Stopping a session

While anything is shared, a floating window titled, for example, "Screen sharing"
or "Remote control" names the application and lists what it has: the shared
displays or windows, the input it may send and whether it can use the clipboard.

- Choose Stop sharing (or Stop control, Stop all sharing), press Escape, or close
  the window to end the session.
- The session also ends when the application closes it or exits, when a shared
  window closes, or when the portal backend stops.

Canopa Notify can hide notification banners while the screen is shared; see
[Notifications](notifications.md).

## Input capture

Input-sharing tools (such as Deskflow or Input Leap) use InputCapture to send your
keyboard and pointer to another computer when the pointer crosses a screen edge.

The dialog "Allow input capture?" has separate, unchecked choices for pointer
input, keyboard input and "Share clipboard while input is captured". Allowing the
session does not start capture by itself; capture starts only when the pointer
crosses an edge the application set up.

While a session exists, its control window shows "Input capture is allowed".
Press Ctrl+Alt+Esc to return input to this computer. Stop or close the window to
end the session. A shared clipboard works only while input is actually captured.
When displays change, the application must set up its edges again before capture
can resume. Touchscreen capture is not available.

## Global shortcuts

When an application asks for global shortcuts, a dialog lists the actions it
offers, each unchecked. The application can suggest a key combination, and
Canopa shows the one you chose last time for that application, but you decide.

- Check the actions you want and give each one a shortcut.
- Type a shortcut such as `CTRL+ALT+a` or `LOGO+F9`, or choose Record keys and
  press the combination; release the keys to finish, or press Escape to cancel
  recording.
- Choose the Allow button for the checked shortcuts, or Allow none to finish
  without registering any. In the editor, choose Save shortcuts. If a shortcut
  is already in use, the dialog stays open so you can pick another.

While the dialog has keyboard focus, existing global shortcuts are paused so you
can record them. Applications can reopen the dialog to change their shortcuts;
canceling keeps the current ones. Your choices are remembered in
`$XDG_STATE_HOME/canopa/portal-shortcuts.db` (default
`~/.local/state/canopa/portal-shortcuts.db`), but only as suggestions: each new
session asks again.

## USB devices

The "Allow USB device access?" dialog names the application and lists the
devices it asks for, with vendor, product ID, serial number and location when
known. No device is selected at first.

- Select devices, or press Alt+S to select or clear all.
- After selecting a device that requests read-write access, choose read-only
  instead. Alt+R applies this to the selected devices.
- Choose Allow selected (Alt+A). Deny, Escape or closing the window grants
  nothing.

The portal frontend opens the devices; Canopa only records your choice.

## Requirements

- The Canopa session from the Arch packages, which runs the Canopa compositor
  (`/usr/lib/canopa/niri`) and selects the Canopa portal frontend
  (`/usr/lib/canopa/xdg-desktop-portal`). Other sessions use the distribution
  frontend and do not use these backends.
- PipeWire and WirePlumber running in the session. The Arch package depends on
  `pipewire-pulse` and `wireplumber`.
- Canopa's WirePlumber access policy, installed by the Arch package as
  `/usr/share/wireplumber/scripts/canopa-screencast.lua` and
  `/usr/share/wireplumber/wireplumber.conf.d/90-canopa-screencast.conf`. It keeps
  sandboxed applications from seeing PipeWire streams they were not granted.
- PipeWire 1.4.8 or newer for the metadata cursor mode.

A stock Niri build lacks the interfaces these portals use. With it, the backend
refuses to start sharing before it shows a dialog.

## Enable InputCapture, GlobalShortcuts and Usb on Arch

These three interfaces are experimental. To try them, give the portal frontend a
per-user descriptor that lists them. The frontend reads
`~/.local/share/xdg-desktop-portal/portals/` before the system directory, and a
user `canopa.portal` replaces the packaged one.

```sh
mkdir -p ~/.local/share/xdg-desktop-portal/portals
sed 's/^Interfaces=/Interfaces=org.freedesktop.impl.portal.InputCapture;org.freedesktop.impl.portal.GlobalShortcuts;org.freedesktop.impl.portal.Usb;/' \
  /usr/share/xdg-desktop-portal/portals/canopa.portal \
  > ~/.local/share/xdg-desktop-portal/portals/canopa.portal
```

The packaged routing (`default=canopa;gtk;`) then sends them to Canopa. Log out
and back in to apply it. Remove the file to undo the change. While the file
exists it also hides later package changes to the descriptor, so recreate it
after upgrading Canopa.

## Portable installs

The portable release archive contains only the dialog portals. Packagers can add
an optional desktop-sharing component with the compositor and frontend builds.
Build both from the pinned forks recorded in `packaging/forks.json`; see
[Upstream dependency forks](vendor-maintenance.md):

```sh
python3 scripts/build-niri.py --archives target/fork-archives --download \
  --work-dir target/niri-fork-sources --fetch --release
python3 scripts/build-xdg-portal-compat.py --archives target/fork-archives --download \
  --work-dir target/portal-frontend-sources --release
```

Each builder writes a `canopa-build.json` receipt in its work directory. Pass
both receipts to the release packager, and check the archive:

```sh
bash scripts/package-release.sh BUILD_DIR OUTPUT_DIR \
  --desktop-niri NIRI_WORK_DIR/canopa-build.json \
  --desktop-frontend FRONTEND_WORK_DIR/canopa-build.json
bash scripts/smoke-release.sh OUTPUT_DIR/canopa-x86_64-unknown-linux-gnu.tar.gz \
  --require-desktop-sharing
```

| Option | Effect |
| --- | --- |
| `--desktop-niri RECEIPT` | Adds the compositor build. Must be given with `--desktop-frontend`. |
| `--desktop-frontend RECEIPT` | Adds the portal frontend build. Must be given with `--desktop-niri`. |
| `--extra-portals` | Also lists InputCapture, GlobalShortcuts and Usb. Needs both receipts. |
| `smoke-release.sh --require-desktop-sharing` | Fails unless the archive contains a complete component. |
| `smoke-release.sh --require-extra-portals` | Also requires the three extra interfaces. |

The packager accepts only builds of the pinned fork commits and refuses changed
sources, mismatched executables and incomplete components. The component adds:

| Path | Contents |
| --- | --- |
| `libexec/canopa/desktop-sharing/niri` | Canopa compositor |
| `libexec/canopa/desktop-sharing/xdg-desktop-portal` | Canopa portal frontend |
| `share/xdg-desktop-portal/portals/canopa.portal` | Descriptor extended with the sharing interfaces |
| `share/canopa/desktop-sharing/portals.conf.fragment` | Routing entries to merge into a `portals.conf` |
| `share/canopa/desktop-sharing/xdg-desktop-portal.service.conf` | systemd override that selects the bundled frontend |
| `share/canopa/desktop-sharing/wireplumber/` | WirePlumber access policy |
| `share/canopa/desktop-sharing.json` | Fork identities, checksums and required shared libraries |
| `share/licenses/canopa-desktop-sharing/` | Original license notices |

Both programs are x86-64 executables that use the distribution's shared libraries,
listed in `desktop-sharing.json`.

### Set up a portable install

These steps assume the archive is installed under `~/.local` and the backend is
already set up as described in [Desktop portals](portals.md). Keep
`libexec/canopa/desktop-sharing/` together.

1. Start the session with `~/.local/libexec/canopa/desktop-sharing/niri`, keeping
   your compositor configuration and the `--session` option. With `niri-session`,
   override the `ExecStart` of `niri.service`.
2. Select the bundled frontend:

   ```sh
   install -Dm644 share/canopa/desktop-sharing/xdg-desktop-portal.service.conf \
     ~/.config/systemd/user/xdg-desktop-portal.service.d/canopa.conf
   ```

   For another prefix, change its `ExecStart` to the installed frontend's absolute
   path.
3. Install the extended descriptor in `~/.local/share/xdg-desktop-portal/portals/`.
4. Merge the entries from `portals.conf.fragment` into the `[preferred]` group of
   the portal configuration your session uses (for a `niri-session` session,
   usually `~/.config/xdg-desktop-portal/niri-portals.conf`). Keep its `default`
   and other entries. The fragment is not a complete file. Alternatively,
   generate a complete file for review with `bin/canopa-portal-config`:

   ```sh
   bin/canopa-portal-config --base CURRENT_SESSION_CONFIG \
     --interface Settings --interface FileChooser --interface ScreenCast \
     --interface RemoteDesktop --interface Clipboard \
     --output ~/niri-portals.conf.canopa
   ```

5. Install the WirePlumber policy:

   ```sh
   install -Dm644 share/canopa/desktop-sharing/wireplumber/canopa-screencast.lua \
     ~/.local/share/wireplumber/scripts/canopa-screencast.lua
   install -Dm644 share/canopa/desktop-sharing/wireplumber/90-canopa-screencast.conf \
     ~/.config/wireplumber/wireplumber.conf.d/90-canopa-screencast.conf
   ```

6. Run `systemctl --user daemon-reload`, end any running sharing sessions, then
   log out and start a new session.

Packaging never changes the live session; each of these steps is explicit.

## Troubleshooting

- **An application opens another sharing dialog.** A user portal configuration
  such as `~/.config/xdg-desktop-portal/canopa-portals.conf` takes precedence
  over the packaged routing. Check it for ScreenCast, RemoteDesktop or Clipboard
  entries that name another backend.
- **Sharing fails right after an upgrade.** Log out of every session for your
  account so the user services load the new frontend and WirePlumber policy. If
  the user manager keeps running, restart `wireplumber.service` before sharing.
- **Sharing fails before any dialog appears.** The compositor is probably not the
  Canopa build. Check that the session runs `/usr/lib/canopa/niri` (or the
  bundled `niri` for a portable install).
- **No metadata cursor.** Check the installed PipeWire version; metadata needs
  1.4.8 or newer.
- **InputCapture, GlobalShortcuts or Usb do nothing.** They are not routed by
  default; see [Enable InputCapture, GlobalShortcuts and Usb on Arch](#enable-inputcapture-globalshortcuts-and-usb-on-arch).
- **Logs.** Run
  `journalctl --user -u xdg-desktop-portal.service -u xdg-desktop-portal-canopa.service`.

## Known limitations

- InputCapture, GlobalShortcuts and Usb are experimental and not routed by the Arch packages.
- Grants are never saved; every session asks again and restore requests are ignored.
- InputCapture cannot capture touchscreens.
- The sharing interfaces work only with the Canopa compositor and portal frontend builds.
- Window streams report a size but no position on the desktop.
- Stopping a session cannot take back clipboard data or USB device handles the application already received.
- The Stop sharing control is an ordinary window, not part of the bar.
- The GlobalShortcuts, ScreenCast, RemoteDesktop and sharing-control dialogs do not yet publish an accessibility tree.

For the design, see [Desktop sharing architecture](architecture/desktop-sharing.md).
