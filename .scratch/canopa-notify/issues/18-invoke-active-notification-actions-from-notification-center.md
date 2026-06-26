# Invoke active Notification Actions from Notification Center

Status: implemented

## Parent

.scratch/canopa-notify/PRD.md

## What to build

Make the **Notification Center** the complete action surface for active named **Notification Actions**. Active notifications should expose all non-default named actions through the panel provider, including silenced active notifications. Notification Center item activation should invoke the **Default Notification Action** when present and otherwise dismiss the notification. Inactive **Notification History** entries should remain read-only and not expose invokable action controls.

Panel Provider Mode should translate panel interaction events into **Notify Control API** commands while keeping Notify-owned controls distinct from sender-provided action keys.

## Acceptance criteria

- [ ] Active Notification Center items expose all non-default named **Notification Actions** in sender order.
- [ ] Silenced **Active Notifications** shown in the Notification Center remain action-invokable.
- [ ] Inactive history entries do not expose invokable **Notification Action** controls.
- [ ] Notification Center item activation invokes `"default"` when present and otherwise dismisses without emitting `ActionInvoked`.
- [ ] Panel provider interaction handling maps item action events to the private action invocation path.
- [ ] Notify-owned panel controls use keys that cannot collide with sender-provided **Notification Action** keys.
- [ ] Tests cover active, silenced active, inactive history, item activation, and provider-owned control collision behavior.

## Blocked by

- .scratch/canopa-notify/issues/16-publish-correct-sender-lifecycle-for-invoked-actions.md
