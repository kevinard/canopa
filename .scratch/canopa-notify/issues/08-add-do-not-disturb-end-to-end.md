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
- [ ] Changing DND re-evaluates Active Notifications and closes banners that are no longer allowed.
- [ ] DND changes do not create late banners for older Active Notifications.
- [ ] Tests cover persistence, control commands, non-critical silence, critical exception, active re-evaluation, and no late banners.

## Blocked by

- [06-handle-replacements-banner-visibility-policy.md](06-handle-replacements-banner-visibility-policy.md)
- [07-persist-bounded-notification-history.md](07-persist-bounded-notification-history.md)
