# Close Dropped Notifications in Sender Notification Lifecycle

Status: implemented
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Complete the sender-visible lifecycle for new notification requests that Notification Intake or Delivery Policy drops. The Notification Server API must still return a valid nonzero notification ID, then publish a terminal `NotificationClosed` signal with the reserved reason for a request that was never retained. A dropped replacement must leave the existing Notification Thread and its still-active sender ID intact rather than falsely closing the notification it attempted to replace.

## Acceptance criteria

- [ ] A no-op notification request returns a nonzero sender-visible ID and emits `NotificationClosed` with reason `4` for that ID.
- [ ] A new notification dropped by a Notification Rule returns a nonzero sender-visible ID and emits `NotificationClosed` with reason `4` without entering Notification History.
- [ ] The sender-visible ID is registered before the terminal event can be relayed, so the close signal cannot be lost during request handling.
- [ ] A dropped replacement does not revise, close, or invalidate the existing active Notification Thread or its sender-visible ID.
- [ ] The domain rules and freedesktop compliance ledger describe the distinct new-request and replacement behavior.
- [ ] D-Bus integration tests cover no-op drops, policy drops, dropped replacements, signal IDs, signal reasons, and subsequent `CloseNotification` behavior.

## Blocked by

None - can start immediately.
