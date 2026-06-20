# Move active transitions to Lifecycle Effects

Status: ready-for-agent
Type: AFK

## Parent

[Notification Lifecycle Refactor](../PRD.md)

## What to build

Move active notification transitions into **Notification Lifecycle**. Dismiss, expire, sender close, action invocation, resident action behavior, clear active notifications, and stale startup conversion should all produce fine-grained Lifecycle Effects. The Notification Daemon should publish sender-visible events from those effects instead of embedding lifecycle meaning directly.

## Acceptance criteria

- [ ] Dismiss, expire, sender close, and stale startup conversion are applied through Notification Lifecycle.
- [ ] Notification Action invocation validates active state through lifecycle and closes non-resident notifications.
- [ ] Resident Notification action invocation remains active while still producing the action effect.
- [ ] Clear Active Notifications uses lifecycle transitions rather than direct status updates.
- [ ] The Notification Daemon derives sender-visible close/action events from Lifecycle Effects.
- [ ] Tests cover active-state validation, dismissed, expired, closed, stale, resident behavior, and sender event publication.

## Blocked by

- [01-route-new-notifications-through-lifecycle.md](01-route-new-notifications-through-lifecycle.md)

