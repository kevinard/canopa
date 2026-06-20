# Remove banner runtime state from Notification History

Status: ready-for-agent
Type: AFK

## Parent

[Notification Lifecycle Refactor](../PRD.md)

## What to build

Remove banner runtime visibility from **Notification History** domain objects and persistence. History should store durable Notification Thread facts only; banner visibility and prior presentation state should live entirely in runtime Notification Banner State and reset across Notification Daemon restart.

## Acceptance criteria

- [ ] Notification History domain objects no longer expose banner visibility or presentation runtime fields.
- [ ] SQLite history storage no longer relies on banner runtime fields to answer daemon, control, or presentation behavior.
- [ ] Restart behavior marks previously active notifications stale without restoring old Notification Banners.
- [ ] Existing history, lifecycle, presentation, and policy tests are updated to assert durable state separately from runtime banner state.
- [ ] Tests prove Notification History persists threads while Notification Banner State resets across daemon restart.

## Blocked by

- [04-drive-runtime-banner-state-from-lifecycle-effects.md](04-drive-runtime-banner-state-from-lifecycle-effects.md)

