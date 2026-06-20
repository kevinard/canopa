# Render a read-only Notification Center through Panel Provider Mode

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Implement the first Niribar-hosted Notification Center path. `canopa-notify panel-provider` connects to the running Notification Daemon, reports provider-visible error content when unavailable, renders a keyboard-navigable read-only snapshot of Active Notifications and recent Notification History grouped by App Identity, and documents the Niribar configuration needed to open it.

## Acceptance criteria

- [ ] `canopa-notify panel-provider` speaks the shared external panel protocol.
- [ ] If the Notification Daemon is unavailable, Panel Provider Mode returns provider-visible error content instead of starting the daemon.
- [ ] When the daemon is available, the provider renders Active Notifications and recent Notification History grouped by App Identity.
- [ ] Opening the Notification Center does not change notification state.
- [ ] Inactive Notification History is rendered read-only except for deletion controls that are intentionally deferred to a later slice.
- [ ] The first panel view is keyboard navigable within the available protocol primitives.
- [ ] Documentation includes a minimal Niribar configuration example for opening the Notification Center.
- [ ] Tests cover provider unavailable behavior, snapshot rendering, grouping, no state change on open, and protocol mapping.

## Blocked by

- [07-persist-bounded-notification-history.md](07-persist-bounded-notification-history.md)
- [11-extract-shared-canopa-panel-protocol-crate.md](11-extract-shared-canopa-panel-protocol-crate.md)
