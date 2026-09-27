# Add User Rule Store and control-managed app policy

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Implement UI/control-managed app policy in the User Rule Store. User-managed Notification Rules are persisted in SQLite, keyed by App Identity and optional Notification Urgency, take precedence over Notification Configuration rules, support allow/silence/drop outcomes, apply to future requests, including replacements, without reclassifying existing notifications (ADR 0027), and preserve historical Delivery Outcomes.

## Acceptance criteria

- [ ] User Rule Store rules can be created, updated, listed, and removed through the Notify Control API.
- [ ] User Rule Store rules are keyed by App Identity and optional Notification Urgency.
- [ ] User Rule Store rules take precedence over Notification Configuration rules.
- [ ] Silenced Notifications are retained and may remain active without visible banners.
- [ ] Dropped Notifications are not retained and are immediately closed from the Sender Notification Lifecycle perspective.
- [ ] Silenced replacements update history and close visible banners.
- [ ] Dropped replacements do not update Notification History.
- [ ] Rule changes affect future requests, including replacements, without changing existing Delivery Outcomes or banner state (ADR 0027).
- [ ] Tests cover precedence, allow/silence/drop behavior, replacements, future-request-only changes, non-retroactive history, and persisted rule storage.

## Blocked by

- [09-load-kdl-notification-configuration-rules.md](09-load-kdl-notification-configuration-rules.md)

## Maintenance note

2026-09-21: Acceptance wording reconciled with the selected Notifications 1.3 profile. ADR 0027 supersedes retroactive policy re-evaluation; ADR 0040 defines atomic configuration candidates. Historical filenames and links are retained. This edit does not claim new test or rollout evidence.
