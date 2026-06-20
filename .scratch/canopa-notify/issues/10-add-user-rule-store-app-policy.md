# Add User Rule Store and control-managed app policy

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Implement UI/control-managed app policy in the User Rule Store. User-managed Notification Rules are persisted in SQLite, keyed by App Identity and optional Notification Urgency, take precedence over Notification Configuration rules, support allow/silence/drop outcomes, re-evaluate Active Notifications without rewriting inactive history, and preserve historical Delivery Outcomes.

## Acceptance criteria

- [ ] User Rule Store rules can be created, updated, listed, and removed through the Notify Control API.
- [ ] User Rule Store rules are keyed by App Identity and optional Notification Urgency.
- [ ] User Rule Store rules take precedence over Notification Configuration rules.
- [ ] Silenced Notifications are retained and may remain active without visible banners.
- [ ] Dropped Notifications are not retained and are immediately closed from the Sender Notification Lifecycle perspective.
- [ ] Silenced replacements update history and close visible banners.
- [ ] Dropped replacements do not update Notification History.
- [ ] Rule changes affect future notifications and re-evaluate Active Notifications without reclassifying inactive history.
- [ ] Tests cover precedence, allow/silence/drop behavior, replacements, active re-evaluation, non-retroactive history, and persisted rule storage.

## Blocked by

- [09-load-kdl-notification-configuration-rules.md](09-load-kdl-notification-configuration-rules.md)
