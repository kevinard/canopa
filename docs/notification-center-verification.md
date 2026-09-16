# Notification Center verification

The compatible host consumes **Canopa protocol v4**. Use the [provider setup](niribar-panel-provider.md)
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
