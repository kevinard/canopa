# Notification Center verification

The compatible host consumes **Canopa protocol v4**. Use the [provider setup](shell-panel-provider.md)
and [recorded desktop acceptance](notification-desktop-acceptance.md). Banners and
Center expose native AT-SPI; the acceptance record includes actual Orca speech,
keyboard reading, links, images/fallbacks and four named actions on a narrow transient.

Run these checks in a disposable compositor with a private session bus and fresh
XDG paths. Build both applications and verify the protocol snapshot before starting.
Native tests need the AT-SPI launcher/registry; see the [Orca fixture](orca-isolated-fixture-research.md)
for real speech and compositor shortcuts. Click the Center header to give its
on-demand surface keyboard focus when another application retains focus.

- Send default/named/resident/transient notifications, including four wrapping
  actions, progress and a rich body. Confirm actual mouse and Enter activation,
  opaque signal keys, and resident/nonresident lifecycle behavior.
- Verify interleaved styled text, PNG/JPEG images, alt fallback for missing/unsafe
  assets and distinguishable HTTP(S) links. A link opens only after deliberate
  activation, without sender action, closure or attention acknowledgement.
- Read with Orca: notification arrival once without focus theft; useful button/link
  names and roles; radio selected state; ordered image alternatives; destination
  details. Updates must not repeat the entire Center.
- Use Tab/arrows, Enter and Escape; PageUp/PageDown/Home/End scroll content. With
  long history, First/Previous/Next navigation reaches every notification. Open two
  provider sessions and confirm their pages are independent. Empty app names must
  not create invalid pages, and category controls cover every included category.
- The reading view starts with messages and compact wrapping actions. Cards have
  a leading app icon and muted source label like banners. Check explicit icons,
  theme/identity fallback and missing-icon initials, including retained cards after
  daemon restart. Settings must show the same app icons beside the app names.
  Interleave notifications from multiple apps and attention states: reading order
  must stay newest first across page boundaries and Settings/Back navigation.
  The icon must remain separate from body images and the source
  must be exposed to native accessibility. Cards have
  no retention, delivery, or lifecycle text. Pending attention has a small dot with
  an `Unread` tooltip and native accessible name; acknowledging removes it.
  Delivery controls and history deletion appear under Settings; Back returns to the same
  notification page. Verify Settings/Back with native accessibility and keyboard
  activation, including narrow panels and updates while Settings is open. The
  private desktop fixture captures `center-reading.png` and `center-settings.png`.
- Automatic Center sizing uses 480 logical pixels of width and the usable screen
  height. Loading, reading and Settings must keep the same geometry. Fill the
  list, verify the cards' right borders remain clear of the scrollbar, then use
  End to reach the last card. The fixture captures `center-overflow.png`; layout
  tests cover scrollbar clearance at narrow widths and sizing on every bar edge.
- Replace while an action is selected/pressed, close/reopen, and restart the daemon.
  Old node/session/revision controls must not activate newer content. Keep one panel
  window during navigation and refresh.
- Fill the banner stack and enable DND. Hidden transient items remain actionable
  in Center without entering durable history or starting their banner timeout.
- Expire retained content and restart with zero archive retention: pending attention
  remains discoverable, old sender actions are absent, and Mark as read/removal works.
- Change app/category rules and DND, close/reopen/restart, and verify durable controls.
  Clear dismisses live notifications; Delete History removes inactive rows explicitly.
- Stop the provider/daemon/compositor. Verify bounded cleanup, visible failure/status,
  safe restart, stale callbacks disabled and transient removal. Existing framed-v1
  providers retain their own behavior and continue passing their host suite.

Historical scale, output-loss, timing, rapid-key and overflow captures remain linked
from [desktop acceptance](notification-desktop-acceptance.md) and the implementation
tracker. No exhaustive hardware/font/locale or native Notification portal-v2 claim
follows from these fixtures.
