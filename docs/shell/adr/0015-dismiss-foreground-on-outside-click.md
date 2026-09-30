# Dismiss Foreground on Outside Click

The first click outside an interactive overlay dismisses it without activating
the application or Bar underneath. This includes unused desktop space, space below
a capped drawer, and outputs without a Bar. Clicks inside the overlay keep their
normal behavior. Escape and explicit close controls retain their existing behavior.

SNI Menus use the Wayland popup grab supplied by the layer-shell adapter. Panels,
including Panel Menus and drawers, retain their sized Overlay layer windows and
use transparent Top layer dismissal surfaces on each observed output. These
surfaces cover reserved bar space without reserving any space themselves, take no
keyboard focus, and leave application geometry unchanged. Bars configured on the
Overlay layer consume outside clicks in their view because they sit above the
dismissal surfaces. Other applications' Overlay surfaces remain above them.

Foreground owns dismissal surface identities and ties them to the current Panel
window. Replacement rotates those identities; stale input cannot close a newer
Panel. Logical close removes dismissal surfaces immediately, independently of the
outgoing Panel animation, and uses the existing provider and dmenu cleanup path.
Output snapshots reconcile dismissal surfaces even on outputs without a Bar.

Logical removal can precede the final redraw of a physical window. The view
renders Bar content only for registered Bar Surfaces; unregistered windows render
transparent content until Wayland removal completes. Falling back to a default
Bar for a retired dismissal window painted an opaque frame across the whole
output. A raster regression renders the dismissal window before and after logical
close and requires every pixel to remain transparent.

This changes the old click-through behavior below capped drawers described in ADR
0014's fixture coverage. `scripts/verify-drawer-modes.py` verifies dismissal,
consumed clicks, restored application input, empty desktop and Bar clicks on every
edge, and dmenu cancellation in an isolated compositor. The native SNI popup grab
and dismissal on a newly connected output without a Bar are covered by
`scripts/test-fork-desktop.py`.
