# Add notification burst cooldown without losing Notification History

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Reduce interruption when one App Identity produces a burst of repetitive notifications while retaining each accepted Notification Thread in Notification History. Cooldown must be deterministic, configurable, visible in delivery diagnostics, and independent from explicit user Notification Rules.

## Acceptance criteria

- [ ] Notification Configuration defines a cooldown window, threshold, and enabled state with safe defaults.
- [ ] Repeated notifications from one App Identity within the window suppress later sounds and banner opportunities according to a documented rule while remaining available in Notification Center.
- [ ] Critical notifications bypass cooldown unless an explicit Notification Rule says otherwise.
- [ ] Replacement updates for one Notification Thread do not consume the same burst budget as unrelated new notifications.
- [ ] Cooldown state is runtime-only, bounded in memory, and cleared predictably across daemon restart and configuration reload.
- [ ] Delivery Decision Logs identify cooldown without exposing Notification Content or pretending it was a user-authored rule.
- [ ] Tests cover thresholds, window expiry, multiple applications, Do Not Disturb, category rules, replacements, sound, banner overflow, and Sender Notification Lifecycle timeouts.

## Blocked by

- [22-apply-display-aware-notification-timeouts.md](22-apply-display-aware-notification-timeouts.md)
