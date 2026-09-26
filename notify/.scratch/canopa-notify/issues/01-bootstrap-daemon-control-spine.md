# Bootstrap the `canopa-notify` daemon/control spine

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Create the first runnable Canopa Notify spine: a Cargo workspace with a single `canopa-notify` binary that can run in Headless Mode as a Notification Daemon, expose a private runtime socket, answer a minimal `ctl status` Control Command, and produce useful diagnostic logs. This slice should prove process shape, command dispatch, async runtime startup, and basic daemon availability without implementing notification receipt yet.

## Acceptance criteria

- [ ] `canopa-notify daemon` starts in Headless Mode and stays running until interrupted.
- [ ] `canopa-notify ctl status` reports daemon availability through the private runtime socket.
- [ ] `canopa-notify ctl status` reports a clear unavailable error when the daemon is not running.
- [ ] The binary exposes the planned command modes without implicit daemon startup from control commands.
- [ ] Basic structured logging is present for startup, shutdown, and control requests.
- [ ] Tests cover daemon availability and unavailable-daemon control behavior without a Wayland Session.

## Blocked by

None - can start immediately
