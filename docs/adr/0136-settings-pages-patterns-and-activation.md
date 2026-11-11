# 0136 Settings page registry, page patterns and activation

Status: accepted. Extends [ADR 0129](0129-supported-session-and-settings.md);
keeps the shared-file behaviour of [ADR 0134](0134-widget-layout-editor.md).

Settings is growing from 7 pages to about 15, and most new pages (network,
Bluetooth, sound, power, date and time) are backed by system services rather
than files. A page used to be spread over a dozen places, navigation was a
fixed column of buttons, every page was "edit a file, then Save", and every
launch opened another window editing the same files.

## Page registry

Every page is declared once, in `crates/canopa-settings/src/registry.rs`: a
`Page` variant and an `Entry` with a stable id, title, description, freedesktop
icon name, sidebar group, search keywords, page kind and a hidden flag. The
sidebar, its groups, Ctrl+1–9, Ctrl+Page Down/Up, search, page headings, the
footer, command-line page ids and the D-Bus `Pages` property all come from it.

- Ids are public names used by launchers, notifications and scripts. They never
  change; the original seven are `displays`, `input`, `personal`, `power`,
  `launcher`, `shortcuts` and `widgets`.
- Groups are shown in a fixed order (Devices, Input, Appearance, System,
  Desktop); empty groups are hidden. Within a group, pages keep declaration
  order. Ctrl+N opens the Nth page in sidebar order, so adding a page to an
  earlier group renumbers later pages.
- Search matches every query word against the title and keywords, ignoring
  case, and ranks title matches first. Enter opens the best match; Ctrl+F
  focuses the field.
- Sidebar focus ids use the page id, not the English title, so translated
  titles keep focus and tests stable.
- Hidden pages are reachable by id only. The live-page example is hidden and
  exists only in debug builds.

## Page patterns

The registry's `Kind` selects one of two building blocks.

**File-backed** (`Kind::File { documents, apply }`, `file_page.rs`): the
existing edit-then-Save flow. A `FileModel` says how a page reads its document,
what it shows when the document can't be read, how it validates, writes and
asks the owner to reload, and which section of a shared document it writes.
`FileDraft` keeps the draft with the snapshot it was read from, so saving still
detects outside edits, and "Reload this page" replaces only that page's draft.
The footer offers Reload and Save, or Test for Displays (`Apply::Trial`).

The shared-document rule: when a page saves a document, each other page the
registry lists for that document keeps its unsaved draft and continues from the
new version, but only if it had read the version that was replaced and the save
left its own section unchanged. Otherwise its next save fails as changed
outside the editor and it must be reloaded. A page without a rebase rule simply
gets that safe failure. Personalization and Widgets share `config.kdl` this way
(unchanged from ADR 0134), and a Personalization motion change or a shortcut
save refreshes the other page's view of `niri.kdl`.

**Live service** (`Kind::Live`, `live.rs`, `ui/live_page.rs`): state comes
from a system service and each change applies at once, with no Save footer. A
service client implements `LiveService` through a small adapter: a name, a
snapshot, a stream of updates, and `apply` for one change. Settings does not
depend on the shared services crate; that crate's clients get adapters as pages
are added. `Live` is a toolkit-free state machine:

- the page shows "Connecting…" until the first reply;
- an unavailable service shows "Service not available" with Try again, and the
  window reconnects by itself after the service returns;
- each change belongs to a control. At most one change per control is in
  flight and the latest further change is queued, so a dragged slider sends
  its final value. A refused change leaves an error on its control with Retry
  and Dismiss, while other controls keep working. A service that goes away
  during a change makes the page unavailable.

The window subscribes to updates before reading the snapshot, so no change in
between is lost, and only while the page is shown. A fake service drives the
state-machine tests and the hidden debug page `canopa-settings live-example`,
whose simulation controls stop the service, refuse a change or slow replies.

Widgets and styling still come only from `canopa-ui`, `canopa-theme` and the
crate's existing shared-control helpers. Icon names are declared but not drawn
until the shared icon lookup exists.

## Single instance and deep links

The first launch that opens a window owns `org.canopa.Settings` on the session
bus (requested without queueing) and serves this interface at
`/org/canopa/Settings`:

```text
interface org.canopa.Settings
  Activate()                        raise the window
  OpenPage(s page, a{sv} options)   open a page id, then raise
    "display-profile" s             Displays only: select a saved layout
  readonly property as Pages        visible page ids, in sidebar order
```

An unknown page or a wrongly typed option is
`org.freedesktop.DBus.Error.InvalidArgs`. Unknown options are ignored so newer
callers work with an older window.

`canopa-settings [PAGE]`, `--page PAGE` and `--display-profile NAME` build the
same request. When the name is already owned, the launch sends `OpenPage` (or
`Activate` without a page) and exits, so launcher entries, the display profile
review notification and the Shell's configuration notice open the requested
page in the existing window. The Shell opens Widgets for problems in its own
section and Personalization for syntax, schema-version and appearance problems.

The window raises itself with Niri's `FocusWindow` on the window its own process
mapped with the Settings app id, and falls back to the toolkit's focus and
attention requests elsewhere. A page request during a save or a display trial
only raises the window and explains why. Without a session bus, or when the
running window does not answer within 10 seconds, a launch opens its own window
as before; the snapshot checks still keep two windows from overwriting each
other's saves.

Packaging installs a D-Bus activation file for `org.canopa.Settings` in the
public session service directory. The name is Canopa's own, unlike
FileManager1, so no opt-in is needed. An activated Settings owns the name and
then receives the queued call. No resident service runs after the window
closes.
