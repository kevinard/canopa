# 0166: Open With, programs and desktop-entry trust

Status: implemented with core and Desktop model verification, 2026-11-11.
Native acceptance of the dialogs and of focus handoff is still pending.
Replaces the `gio open PATH` call that Browser windows used for every file
(ADR0004), and uses the shared lookup of
[root ADR 0141](../../adr/0141-mime-applications-and-thumbnail-cache.md).

## Opening a file

Activating a file (double click, Return, **Open**) no longer runs anything on the
UI thread. A worker classifies it with `crate::opening::classify` and resolves its
applications, then the UI decides:

| Classification | What happens |
| --- | --- |
| Folder | Browse it. |
| Document | Launch the default application for its type; with none, open the chooser. |
| Program or script | Ask: **Run**, **Run in Terminal**, **Open** (scripts only), **Cancel**. |
| Trusted desktop entry | `Type=Application`: launch it. `Type=Link`: open its `URL=`. |
| Untrusted desktop entry | Ask: **Launch** (once), **Open as Text**, **Cancel**. |

Nothing executable is ever run without a choice. Cancel has the initial focus.

The default is the one `canopa_apps::mime_apps` resolves: the explicit
`[Default Applications]` choice for the type, then for each ancestor type, then
the most preferred association, which is GLib's order. Types come from
`canopa_file::mime` (ADR 0138), so a file's Open and Open With agree with its icon.

Trash and Recovery show stored copies. They keep GIO's choice of viewer
(`gio open`), now on a worker thread with an activation token, and never run a
stored program.

## Programs and scripts

A file is a program only when all of these hold:

- it is a regular file with an execute bit that the current user owns and may
  execute (`access(X_OK)`);
- its filesystem stores Unix modes and is local, not `noexec`, removable or a
  network share (`crate::mounts`): on FAT, NTFS, ISO 9660 and SMB every file
  looks executable;
- its content starts with an ELF header or `#!`.

An execute bit on a photo, or a script without one, opens as a document. **Run**
starts the file directly in its folder without arguments or a shell. **Run in
Terminal** runs `xdg-terminal-exec PATH` (a package dependency); without it Files
says so. **Open** for a script uses its type's default, else the `text/plain`
default.

## Desktop entries

A `.desktop` file that parses as `Type=Application` or `Type=Link` is trusted
when it lies in an XDG `applications` directory (by its own path or its resolved
path, so a user's link there counts), or when it passes the program test's
ownership, execute-bit and filesystem rules. That is GNOME's rule without its
`metadata::trusted` attribute, which Files cannot store without a journalled
metadata mutation. A launcher on a USB stick or downloaded without an execute bit
is therefore untrusted. **Launch** starts it once; making the file executable
(outside Files for now, pending editable permissions) makes it trusted. **Open as
Text** uses the `text/plain` default.

Before launching an entry Files applies the launcher's checks
(`canopa_apps::check_launch_entry`): `TryExec`, the `Exec` program and a terminal
for `Terminal=true`, so a broken entry reports why instead of failing unseen.

## Open With

The context menu shows **Open With…** for a selection of files. It resolves on a
worker and opens a submenu: the default (marked), the other applications in
association order, then **Other Application…**. Several files offer the
applications every file's types share. **Other Application…** opens a chooser
window: recommended applications first, then every other application menus show,
with search over name, ID, generic name and keywords. When the files share one
type, **Set as default for TYPE** writes the user's `mimeapps.list` before
launching (`canopa_apps::mime_apps::set_default`).

## Launching

Every launch takes an activation token from the window the user acted in (the
dialog, when there is one), then starts on a worker:

- applications and desktop entries: `gio launch ENTRY [FILE…]`, as the launcher
  does, so GIO keeps field codes, `DBusActivatable=` and `Terminal=`;
- links: `gio open URL`;
- programs: the program itself; terminal runs: `xdg-terminal-exec PROGRAM`.

The token is passed as `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`. GIO's
launcher is awaited and its first diagnostic line becomes the error, reported on
the tab that asked. Programs are reaped on a thread and outlive the request.

`gio launch` gives D-Bus-activated applications no activation token in their
platform data; they rely on the compositor's focus policy, like the launcher's.

## Not done

- A persistent "Allow launching" mark and editable permissions (they need a
  journalled metadata mutation).
- Application icons in the menu and chooser.
- Open With for folders.

Verification: `opening` tests (program and trust rules on private files, command
lines, launcher diagnostics), `mounts` tests, the Desktop tests in
`iced_desktop/open_with_ui.rs` (prompts before running, one dialog at a time,
chooser search, errors on the originating tab) and the `canopa-apps`
`mime_apps` tests.
