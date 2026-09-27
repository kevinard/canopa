# Apply display-aware Notification Timeouts end to end

Status: implemented
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Implement the agreed Notification Timeout semantics across Notification Lifecycle, Notification Banner State, timer effects, sender-visible closure, and Notification Center state. An Active Notification must expire according to the documented presentation state rather than merely because Delivery Policy classified it as shown or silenced.

## Acceptance criteria

- [x] Positive and server-default Notification Timeouts begin only when the current revision first becomes visible in a Notification Banner; a zero timeout never schedules expiration.
- [x] Overflow notifications, **Silenced Notifications**, requests accepted under **Do Not Disturb**, and **Headless Mode** requests that never display a banner remain active without a timer or sender-visible close event.
- [x] A timer that began on banner display continues to its original deadline if a critical notification displaces that banner, and hidden notifications are not promoted when a slot opens.
- [x] Replacements that revise a **Notification Thread** atomically cancel the prior revision's timer and start a fresh timer only when the replacement revision becomes visible; dropped replacements preserve the existing timer, and stale revision timers cannot close current content.
- [x] An elapsed timer removes the banner, retains an Expired Notification in Notification History, invalidates the active sender ID, and emits `NotificationClosed` reason `1`.
- [x] Runtime and presentation tests cover three occupied banner slots, a fourth normal notification, critical displacement, Do Not Disturb, Headless Mode, and replacement races.
- [x] D-Bus tests verify expiration after actual banner display and the absence of expiration when no banner is displayed.

## Blocked by

- [21-decide-timeout-semantics-without-banners.md](21-decide-timeout-semantics-without-banners.md)

## Comments

- Implemented on 2026-09-03. Notification Lifecycle now emits revision-specific start-on-display intents and revision-bounded replacement cancellation. The runtime applies banner placement, waits for graphical presentation acknowledgement, and starts a timer only when that exact revision is visible outside Headless Mode. Server-default duration resolution is deferred until that acknowledgement, and stale timeout changes cannot replace or cancel a newer revision's timer.
- Verified with lifecycle, presentation/runtime, and D-Bus coverage for finite, server-default, zero, presentation acknowledgement, overflow, silenced, Do Not Disturb, critical displacement, delayed-display acknowledgement, coalesced pre-frame updates, carried-forward visible timeout intents, concurrent replacement safety, sender-ID invalidation, and retained expired-history behavior. `cargo test --workspace --all-targets` passes all 214 tests.
