# Notification Center reading and settings views

Status: accepted (2026-09-29).

The Center mixed delivery rules, lifecycle diagnostics, destructive history
management and messages in one scrolling list. Permission options appeared ahead
of each app's messages, and stacked full-width buttons consumed the reading area.

The streamed Center now opens in a reading view with messages in date order and
notification actions. Cards omit retention, lifecycle status and
delivery outcome text. Requires Attention is represented by a small dot beside
the title, with “Unread” available through its tooltip and native accessibility.
Acknowledged and transient items have no state label. Opening or reading the
Center never acknowledges a notification. Diagnostic facts remain available
through the control API's domain projection, rather than panel card metadata.

Settings contains the app/category controls for the current bounded page and
Delete Notification History. Their display labels are Show banners, Save quietly,
and Block; the existing allow/silence/drop commands and future-request policy
semantics remain unchanged. Clear Active Notifications stays in the reading view.

The view belongs to the existing paginated session. View changes use its opaque,
generation-bound navigation commands, preserve its position, and never mutate
notification state. Both views retain First/Previous/Next when available. Building
and bounding the complete page before projection preserves category coverage and
all sender actions across view changes, including oversized action lists. The view
split requires no notification store or shared ownership changes.

Niribar lays out generic rich-content actions and control options in wrapping
rows, uses stronger message titles, and gives app groups and cards more space.
Generic metadata with an empty value is treated as a presence indicator in the
heading, using its label for the tooltip and native accessibility. Metadata with a
value keeps its existing text presentation for other providers. This uses the
existing v4/v5 fields and adds no wire schema or notification keys to the host.
The renderer retains revision-bound widget identities, keyboard navigation, the
selected action preview, and native accessibility bounds.

Cards also follow the banner layout with a 34-pixel source icon beside the message,
muted app name below the body, and actions below the content. Missing icons use an
initial badge. Duplicate app headings are omitted in the reading view. Source
identity is carried by the compatible generic attribution extension in
shared ADR 0010 ([panel protocol](../../architecture/panel-protocol.md)); it is distinct
from body images and does not expose notification keys to the host.

The host's `notifications` provider surface prefers 480 logical pixels of width
and fills the usable screen height on every bar edge, including drawer
presentation. Explicit dimensions and caps remain authoritative. Loading, reading
and settings keep the same geometry. Rich-content scrollbars reserve their own
space so card borders and actions cannot be obscured by the scrollbar.

Validation covers v4/v5 provider view round trips over large histories, category
coverage, independent sessions, stale navigation, transient lifecycle, narrow
action wrapping, keyboard reading, and the private desktop fixture's native
Settings/Back controls and sender actions, full-height geometry, and overflowing
card lists.

The reading view is a single chronological list, newest creation date first with
internal ID as tie-breaker, independent of app and attention. Retained and live
transient batches share that cursor order. A v3 store migration replaces only the
Center index; it preserves all notification content and lifecycle/attention data.
The provider retains each bounded row's cursor while assembling the app-grouped
page, then flattens and sorts only the reading presentation. Both views therefore
keep identical page boundaries, category coverage and action continuation.
Settings retains app groups with a decorative source icon beside each app name.

Page construction now opens one Notify-owned `NotificationCenterReader` for each
frame. The reader captures permission maps, icon settings and a sorted lightweight
index of transient positions once, then fetches up to sixteen retained/live rows
per read. Each row retains its own cursor and category controls. Content remains
live at each bounded read; a state subscription requests a new frame after a
concurrent change. No full-history snapshot or long-lived read transaction is held.

The page builder measures each JSON item, action and new control once, including
array separators, before committing it to the page. It reserves byte and interaction
capacity for navigation. Overflow has one admission decision, so a rejected row
cannot leak its controls and action continuation retains the previous cursor plus
the next action offset. Final protocol validation and capped serialization remain
mandatory. Tests compare the estimate to exact escaped JSON at the byte boundary,
exercise action continuation and rejected category controls, and bound read and
serialization work for many small notifications.
