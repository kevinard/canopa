# Snooze Active Notifications and restore them later

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Add a user-owned snooze lifecycle that temporarily removes an Active Notification from visible attention and restores it at a chosen time. Snooze state and wake time must survive daemon restart, remain distinct from sender close, dismissal, expiration, and staleness, and produce a fresh presentation opportunity when restored.

## Acceptance criteria

- [ ] `CONTEXT.md` defines Snoozed Notification terminology and its relationship to Active, Dismissed, Expired, Closed, and Stale Notifications.
- [ ] Notification Center offers at least short, medium, and next-day snooze choices for eligible Active Notifications.
- [ ] Snoozing removes any Notification Banner, cancels the current Notification Timeout, and does not emit `NotificationClosed` to the sender.
- [ ] Snooze state and wake time persist in Notification History and are rehydrated after daemon restart.
- [ ] Waking returns the Notification Thread to active state, creates one new banner opportunity, and applies the documented timeout and sound policy.
- [ ] A sender replacement or `CloseNotification` while snoozed has explicit, tested behavior and cannot resurrect obsolete content.
- [ ] Lifecycle, persistence, timer, control, panel subscription, restart, replacement, and sender-event tests cover the complete snooze cycle.

## Blocked by

- [22-apply-display-aware-notification-timeouts.md](22-apply-display-aware-notification-timeouts.md)
