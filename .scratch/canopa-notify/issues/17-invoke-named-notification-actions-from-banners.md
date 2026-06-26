# Invoke named Notification Actions from Notification Banners

Status: implemented

## Parent

.scratch/canopa-notify/PRD.md

## What to build

Expose named **Notification Actions** on **Notification Banners** without changing the banner activation contract. Activating the notification itself should continue to invoke the **Default Notification Action** when present and otherwise dismiss the notification. Separate visible banner controls should show at most two non-default named actions in sender order and invoke those actions through the daemon lifecycle.

This slice should keep banners non-focus-stealing and avoid adding action overflow menus, action icons, activation tokens, or inline replies.

## Acceptance criteria

- [ ] Banners render no separate control for the `"default"` action.
- [ ] Banners render at most two non-default named **Notification Action** controls in sender order.
- [ ] Invoking a visible banner action uses the same action invocation path as the private control command and publishes the same sender-visible lifecycle events.
- [ ] Activating banner content still invokes `"default"` when present and otherwise dismisses without emitting `ActionInvoked`.
- [ ] Banner layout remains stable for notifications with zero, one, two, and more than two non-default actions.
- [ ] Tests cover banner action projection, default action hiding, action invocation, and the two-action cap.

## Blocked by

- .scratch/canopa-notify/issues/16-publish-correct-sender-lifecycle-for-invoked-actions.md
