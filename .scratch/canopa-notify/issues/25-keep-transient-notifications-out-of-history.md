# Keep Transient Notifications out of persisted Notification History

Status: implemented
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Honor the freedesktop `transient` hint by allowing a Transient Notification to participate in runtime presentation and Sender Notification Lifecycle without becoming persisted Notification History. Transient behavior must remain replacement-safe and must not weaken normal retained notifications.

## Acceptance criteria

- [ ] A transient notification can produce a Notification Banner, sound, actions, timeout, and sender-visible lifecycle events.
- [ ] No transient notification content or lifecycle row remains in persisted Notification History after receipt or daemon restart.
- [ ] The Notification Center either exposes transient runtime state or omits it according to one documented rule, without implying persistence.
- [ ] A transient replacement updates the correct runtime notification and does not create a persisted duplicate.
- [ ] Replacing a retained Notification Thread with a transient request, and the reverse direction, have explicit tested behavior.
- [ ] Closing, dismissing, invoking, and expiring a transient notification release all runtime images, timers, sender-ID mappings, and presentation state.
- [ ] D-Bus, lifecycle, persistence, and restart tests cover transient new requests and replacements.

## Blocked by

None - can start immediately.
