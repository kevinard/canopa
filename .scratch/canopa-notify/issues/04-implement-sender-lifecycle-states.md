# Implement sender lifecycle states

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Complete the first sender-visible and user-visible notification lifecycle. Active Notifications can become Dismissed, Expired, Closed, or Stale; `CloseNotification` is handled; close signals are emitted consistently; actions are invokable only while active; Resident Notifications remain active after action invocation; and daemon restart converts previously active notifications to Stale Notifications without restoring old banners.

## Acceptance criteria

- [ ] User dismissal from a banner records a Dismissed Notification and emits the expected sender-visible close behavior.
- [ ] Notification Timeout expiration records an Expired Notification without deleting history.
- [ ] Sender `CloseNotification` records a Closed Notification without deleting history.
- [ ] Previously active notifications loaded after daemon restart become Stale Notifications and do not restore banners.
- [ ] Notification Actions are invokable only while the Notification Thread is active.
- [ ] Invoking an action closes the notification unless it is a Resident Notification.
- [ ] Tests cover Dismissed, Expired, Closed, Stale, action lifetime, Resident Notification behavior, and close signal behavior.

## Blocked by

- [02-receive-store-default-allowed-notification.md](02-receive-store-default-allowed-notification.md)
- [03-show-real-minimal-notification-banner.md](03-show-real-minimal-notification-banner.md)
