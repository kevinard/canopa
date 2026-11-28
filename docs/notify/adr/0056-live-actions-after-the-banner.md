# Live actions after the banner

Status: accepted (2026-11-27). Amends the timeout boundary of ADR 0031 for
freedesktop notifications, as ADR 0055 did for portal notifications.

## Context

A freedesktop notification that asks for the server's default timeout
(`expire_timeout` −1) expired when its 5-second banner closed. Expiry ends the
sender lifecycle (`NotificationClosed`, reason 1), so a calendar reminder's
**Join call** and **Snooze** vanished from the Notification Center a few seconds
after they appeared, leaving only Mark as read. Applications that offer actions
usually keep waiting for one, and other desktops keep such notifications live in
their notification list after the banner leaves.

## Decision

The server-default display timeout of a freedesktop notification with at least
one named action (any action other than `default`) is a **Banner-Hide Timer**: it
hides the banner and leaves the notification active, with its actions, in the
Center. It ends when the user invokes an action, opens or dismisses it, or the
application closes or replaces it, and the live-thread bound of ADR 0032 and
ADR 0054 still evicts the oldest.

Notifications without named actions, transient notifications and notifications
whose application set its own timeout keep expiring when the timeout passes.

## Consequences

- Applications receive `NotificationClosed` for these notifications only when
  the user or the application ends them, not when the banner hides.
- They count as active notifications (the bell count and **Clear active
  notifications**) until then.
