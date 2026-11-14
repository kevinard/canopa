# Taskbar menus and saved pins

Date: 2026-11-13
Status: accepted

The taskbar launched another instance on right-click, and its pins came only
from the configured `pinned` list. Repeated clicks on an application with
several windows did not cycle: clicking the bar gives the bar keyboard focus,
so Niri reported no focused window and the click refocused the most recent one.

## Cycling

A click treats the window in front as current: Niri's focused window, or, while
a Shell surface holds the keyboard, the focused workspace's active window, as
the taskbar's own focus marks already did. A click that asks Niri to focus a
window remembers it for 1.5 seconds, while Niri still reports the window it
replaced, so a quick second click moves on to the next window. Windows cycle
in a stable order, so repeated clicks visit each one.

## Application menu

Right-click opens the application's menu in the existing menu popup, anchored
to the icon like a tray item's menu. Middle-click still starts another instance.
The menu has Open (nothing runs) or New window (it runs, unless the desktop
file declares `SingleMainWindow`), the desktop file's other `[Desktop Action]`
groups, then Pin to taskbar or Unpin from taskbar. A desktop `new-window`
action stands in for a plain launch, which a D-Bus-activated application would
answer by raising its existing window. Windows without a desktop file get no
menu: there is nothing to launch or pin.

The popup, its keyboard navigation and dismissal stay the Foreground's. A
taskbar menu carries the reserved menu service `canopa-shell/taskbar`, which
cannot be a D-Bus bus name: its selections return to the taskbar runtime, and
no DBusMenu layout watch, `clicked` or `closed` event is sent for it. Tray menu
search does not apply.

## Saved pins

Pins changed from the bar are Shell state, not configuration: the Shell does not
rewrite a person's KDL, which Settings and hand edits share. They are saved in
`$XDG_STATE_HOME/canopa/shell/taskbar-pins.json` (owner-only, written
atomically by one writer thread, at most 64 KiB) as edits to each taskbar's
configured list, by module name: applications pinned from the bar, and
configured ones unpinned there. The pinned set is the configured list without
the unpinned ones, in its order, followed by those pinned from the bar. Edits
that restore the configuration are dropped, so later changes to `pinned`
still apply. An unreadable or foreign file is ignored with a warning and
replaced on the next change.

## Update 2026-11-14: screen order

Running applications that are not pinned now follow screen order instead of
opening order: the output, workspace index, column and tile of their leftmost
window, with floating windows after their workspace's columns. Clicks cycle an
application's windows in that order, left to right, instead of by window ID.
The Shell keeps Niri's `WindowLayoutsChanged` positions for this
(`.scratch/niri-scrolling/issues/02-layout-ordered-taskbar.md`).
