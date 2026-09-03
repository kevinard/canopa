# Apply display-aware Notification Timeouts end to end

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Implement the agreed Notification Timeout semantics across Notification Lifecycle, Notification Banner State, timer effects, sender-visible closure, and Notification Center state. An Active Notification must expire according to the documented presentation state rather than merely because Delivery Policy classified it as shown or silenced.

## Acceptance criteria

- [ ] Notification Timeout scheduling follows the ADR for visible, overflow, silenced, and headless notifications.
- [ ] A finite timeout cannot expire an overflow notification before the ADR-defined display or Notification Center condition occurs.
- [ ] A finite-timeout Silenced Notification completes its Sender Notification Lifecycle according to the documented rule.
- [ ] Replacements cancel or restart timers atomically and stale revision timers cannot close current content.
- [ ] Banner removal, optional promotion, Notification History status, and `NotificationClosed` reason `1` remain consistent.
- [ ] Runtime and presentation tests cover three occupied banner slots, a fourth normal notification, critical displacement, Do Not Disturb, Headless Mode, and replacement races.
- [ ] D-Bus tests verify the sender-visible expiration result for notifications with and without a banner.

## Blocked by

- [21-decide-timeout-semantics-without-banners.md](21-decide-timeout-semantics-without-banners.md)
