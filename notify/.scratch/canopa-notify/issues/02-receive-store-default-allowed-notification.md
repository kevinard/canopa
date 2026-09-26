# Receive and store a default-allowed freedesktop notification

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Add the first end-to-end notification receipt path. In development mode, the Notification Daemon owns a dev/test freedesktop-compatible D-Bus service name, accepts a `Notify` call, derives App Identity from desktop entry with app-name fallback, applies default-allow policy, persists an Active Notification Thread in SQLite, and exposes it through a Control Command.

## Acceptance criteria

- [ ] The daemon can own a dev/test notification D-Bus service name without taking over the user's real desktop notification service.
- [ ] A freedesktop `Notify` call creates an Active Notification Thread with App Identity, Notification Summary, Notification Body, Notification Urgency, and timestamps.
- [ ] SQLite migrations create the initial persisted state needed for Notification History.
- [ ] Unknown App Identities are allowed by default.
- [ ] A diagnostic Control Command lists active notifications, with optional unstable JSON output.
- [ ] Tests cover App Identity extraction, default-allow policy, D-Bus receipt through the dev/test service name, persistence, and control listing.

## Blocked by

- [01-bootstrap-daemon-control-spine.md](01-bootstrap-daemon-control-spine.md)
