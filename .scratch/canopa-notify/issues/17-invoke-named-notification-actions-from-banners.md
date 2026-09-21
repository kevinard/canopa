# Invoke named Notification Actions from Notification Banners

Status: implemented

## Parent

.scratch/canopa-notify/PRD.md

## What to build

Expose named **Notification Actions** on **Notification Banners** without changing the banner activation contract. Activating the notification itself should continue to invoke the **Default Notification Action** when present and otherwise dismiss the notification. Separate visible banner controls should show all accepted non-default named actions in sender order and invoke those actions through the daemon lifecycle.

Banners remain non-focus-stealing. All accepted named actions are reachable through bounded scrolling (ADRs 0046 and 0048). Inline replies remain separate scope; activation-context behavior is covered by the selected Notifications 1.3 profile.

## Acceptance criteria

- [ ] Banners render no separate control for the `"default"` action.
- [ ] Banners render all accepted non-default named **Notification Action** controls in sender order.
- [ ] Invoking a visible banner action uses the same action invocation path as the private control command and publishes the same sender-visible lifecycle events.
- [ ] Activating banner content still invokes `"default"` when present and otherwise dismisses without emitting `ActionInvoked`.
- [ ] Banner layout remains stable for notifications with zero, one, two, and more than two non-default actions.
- [ ] Tests cover banner action projection, default action hiding, action invocation, and scrolling to actions beyond the second control.

## Blocked by

- .scratch/canopa-notify/issues/16-publish-correct-sender-lifecycle-for-invoked-actions.md

## Maintenance note

2026-09-21: Acceptance wording reconciled with the selected Notifications 1.3 profile. ADRs 0046/0048 supersede the original two-action limit. Historical filenames and links are retained. This edit does not claim new test or rollout evidence.
