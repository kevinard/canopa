# Add Do Not Disturb end to end

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Implement persistent Do Not Disturb as a global presentation mode. DND silences non-critical notifications while retaining them, allows critical Notification Banners by default, persists across daemon restarts, takes precedence over app allow rules for non-critical notifications, and is controllable through `canopa-notify ctl`.

## Acceptance criteria

- [ ] DND state can be queried and changed through Control Commands.
- [ ] DND state persists across daemon restarts.
- [ ] Non-critical notifications received while DND is enabled are retained without visible banners.
- [ ] Critical notifications still show banners by default while DND is enabled.
- [ ] Changing DND affects future requests, including replacements; existing Active Notifications keep their committed Delivery Outcome and banner state (ADR 0027).
- [ ] DND changes do not create late banners for older Active Notifications.
- [ ] Tests cover persistence, control commands, non-critical silence, critical exception, future-request-only changes, and no late banners.

## Blocked by

- [06-handle-replacements-banner-visibility-policy.md](06-handle-replacements-banner-visibility-policy.md)
- [07-persist-bounded-notification-history.md](07-persist-bounded-notification-history.md)

## Maintenance note

2026-09-21: Acceptance wording reconciled with the selected Notifications 1.3 profile. ADR 0027 supersedes retroactive policy re-evaluation; ADR 0040 defines atomic configuration candidates. Historical filenames and links are retained. This edit does not claim new test or rollout evidence.
