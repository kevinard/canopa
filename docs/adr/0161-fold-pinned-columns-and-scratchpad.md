# 0161. Fold pinned columns and keep a floating scratchpad

Status: accepted

## Context

Pinned columns keep a window such as chat or notes beside the scrolling layout,
but they always take their share of the screen and can only be unpinned to make
room. Niri has no scratchpad, so there was no way to call one window, like a
terminal or a music player, onto whatever workspace is in use and send it away
again.

## Decision

The compositor fork adds both, so they work with keyboard, titlebar menu and IPC
alike.

A pinned column can fold past its edge (`toggle-pinned-column-folded`,
Super+Ctrl+P, or **Fold** in the titlebar layout menu). It keeps its pin and
gives its space to the scrolling columns, which keep their widths. A slim tab in
the gap at the middle of its edge marks it; pointing at the tab names the window
and clicking it unfolds the column. Focusing a folded column any other way
unfolds it too, so focus is never on a column out of view.

The scratchpad holds floating windows hidden on their workspace (`toggle-window-
in-scratchpad`, Super+Shift+`, or **Scratchpad** in the menu). A tiled window
becomes floating at a comfortable size first. Like a special workspace laid over
the others, `toggle-scratchpad` (Super+`) brings all of them to the focused
workspace, each at the position and size it was left with, and hides them all
again when one has focus. Hidden windows stay in their workspace and in the
window list, which reports them as scratchpad windows, so the taskbar still
finds them, labels them **Scratchpad**, and focusing one shows it. The window
bar always carries a scratchpad button while the scratchpad holds windows, with
their count when there are several, filled while one is shown; clicking it is
`toggle-scratchpad`. The `open-in-scratchpad` window rule sends a window there
when it opens.

## Consequences

Folding is per workspace, like pinning; the scratchpad is the way to reach a set
of windows from every workspace. Hidden scratchpad windows keep running and keep
their size and position. Each titlebar's layout button shows the window's
arrangement (pinned, floating or in the scratchpad), so it stays visible without
opening the menu. Shown floating windows cover the edge arrows and folded tabs,
as they cover the tiled windows. The tab, the menu labels and the diagrams are
drawn by the compositor in English, like its other captions.
