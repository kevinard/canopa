# Ship service-mode startup and packaging artifacts

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Make Canopa Notify usable as the real notification daemon for a Graphical User Session. Service mode owns the real freedesktop Notification Server API name, fails clearly if that name or the daemon instance is already owned, installs/runs through a systemd user unit, and documents startup and diagnostic workflows.

## Acceptance criteria

- [ ] Service mode uses the real freedesktop notification service name.
- [ ] The daemon fails fast with a clear diagnostic if another notification server already owns the Notification Server API.
- [ ] The daemon enforces a single active instance per Graphical User Session.
- [ ] Install artifacts include a systemd user unit and do not include an autostart desktop file.
- [ ] `GetServerInformation` reports product name Canopa Notify and vendor Canopa.
- [ ] Documentation explains service startup, development service-name mode, control diagnostics, and common failure cases.
- [ ] Tests or documented manual checks cover service-name ownership failure, single-instance enforcement, and systemd user service behavior.

## Blocked by

- [03-show-real-minimal-notification-banner.md](03-show-real-minimal-notification-banner.md)
- [09-load-kdl-notification-configuration-rules.md](09-load-kdl-notification-configuration-rules.md)
