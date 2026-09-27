# Publish correct sender lifecycle for invoked Notification Actions

Status: implemented

## Parent

.scratch/notify/canopa-notify/PRD.md

## What to build

Make **Notification Action** invocation publish the correct **Sender Notification Lifecycle** for freedesktop clients. Invoking a valid action on an active non-resident notification should emit the sender-visible action event first, then dismiss the notification and emit the sender-visible dismissal event. Invoking a valid action on a **Resident Notification** should emit only the action event and leave the notification active.

Add a real diagnostic **Control Command** for invoking a notification action so headless end-to-end tests and debugging can drive the same behavior through the running daemon.

## Acceptance criteria

- [ ] Invoking a valid non-resident **Notification Action** emits `ActionInvoked` before `NotificationClosed`.
- [ ] The `NotificationClosed` signal for non-resident action invocation uses freedesktop reason `2`, matching user dismissal rather than sender closure.
- [ ] Non-resident action invocation leaves the retained **Notification Thread** as a **Dismissed Notification** and cancels any pending **Notification Timeout** through the dismissal lifecycle path.
- [ ] Resident action invocation leaves the **Notification Thread** active and preserves its existing timeout behavior.
- [ ] Missing, stale, inactive, or absent action keys fail through the private control path instead of invoking by label or fallback matching.
- [ ] A diagnostic control-socket command can invoke a valid action on a running daemon in headless mode.
- [ ] D-Bus integration tests cover action invocation signal order and close reason through the running daemon.

## Blocked by

- .scratch/notify/canopa-notify/issues/15-normalize-freedesktop-notification-actions.md
