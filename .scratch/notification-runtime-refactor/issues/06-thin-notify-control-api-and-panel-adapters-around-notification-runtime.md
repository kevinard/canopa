# Thin Notify Control API and panel adapters around Notification Runtime

Status: ready-for-agent
Type: AFK

## What to build

Make the Notify Control API, Unix socket framing, CLI formatting, and **Panel Provider Mode** thin adapters over **Notification Runtime**. Stateful command meaning should live in runtime; adapters should own only request parsing, reply formatting, socket framing, and panel protocol mapping.

## Acceptance criteria

- [x] Notify Control API commands route stateful operations through **Notification Runtime**.
- [x] Socket request parsing and reply formatting remain adapter concerns.
- [x] CLI output behavior remains unchanged for existing control commands.
- [x] **Panel Provider Mode** continues mapping **Notification Center Projection** to shared panel protocol content.
- [x] Notify-specific panel action/control meaning stays in `canopa-notify`, not `canopa-panel-protocol`.
- [x] Existing control API, control spine, and panel provider tests still pass.

## Blocked by

- [05-move-notification-center-projection-and-subscriptions-into-notification-runtime.md](05-move-notification-center-projection-and-subscriptions-into-notification-runtime.md)
