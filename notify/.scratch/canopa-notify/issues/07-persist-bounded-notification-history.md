# Persist bounded Notification History

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Make Notification History durable and bounded. The daemon persists retained Notification Threads across restarts, prunes inactive history after writes and startup using the default 30 day / 1,000 thread limits, exempts Active Notifications from pruning, exposes history through diagnostics, and implements separate Clear Active Notifications and Delete Notification History behaviors.

## Acceptance criteria

- [ ] Retained Notification Threads survive daemon restart.
- [ ] Inactive history is pruned after writes and at startup using the default retention limits.
- [ ] Active Notifications are exempt from retention pruning.
- [ ] Clear Active Notifications dismisses active notifications without deleting history.
- [ ] Delete Notification History removes retained inactive history records.
- [ ] Inactive Notification History is read-only except for deletion, with stale action labels retained as metadata only.
- [ ] Diagnostic Control Commands can inspect active/recent history, including optional unstable JSON output.
- [ ] Tests cover persistence, pruning, active exemption, clear active, delete history, inactive read-only behavior, and diagnostics.

## Blocked by

- [04-implement-sender-lifecycle-states.md](04-implement-sender-lifecycle-states.md)
- [05-normalize-notification-content-metadata.md](05-normalize-notification-content-metadata.md)
