# Apply display-aware Notification Timeouts end to end

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Implement the agreed Notification Timeout semantics across Notification Lifecycle, Notification Banner State, timer effects, sender-visible closure, and Notification Center state. An Active Notification must expire according to the documented presentation state rather than merely because Delivery Policy classified it as shown or silenced.

## Acceptance criteria

- [ ] Positive and server-default Notification Timeouts begin only when the current revision first becomes visible in a Notification Banner; a zero timeout never schedules expiration.
- [ ] Overflow notifications, **Silenced Notifications**, requests accepted under **Do Not Disturb**, and **Headless Mode** requests that never display a banner remain active without a timer or sender-visible close event.
- [ ] A timer that began on banner display continues to its original deadline if a critical notification displaces that banner, and hidden notifications are not promoted when a slot opens.
- [ ] Replacements that revise a **Notification Thread** atomically cancel the prior revision's timer and start a fresh timer only when the replacement revision becomes visible; dropped replacements preserve the existing timer, and stale revision timers cannot close current content.
- [ ] An elapsed timer removes the banner, retains an Expired Notification in Notification History, invalidates the active sender ID, and emits `NotificationClosed` reason `1`.
- [ ] Runtime and presentation tests cover three occupied banner slots, a fourth normal notification, critical displacement, Do Not Disturb, Headless Mode, and replacement races.
- [ ] D-Bus tests verify expiration after actual banner display and the absence of expiration when no banner is displayed.

## Blocked by

- [21-decide-timeout-semantics-without-banners.md](21-decide-timeout-semantics-without-banners.md)
