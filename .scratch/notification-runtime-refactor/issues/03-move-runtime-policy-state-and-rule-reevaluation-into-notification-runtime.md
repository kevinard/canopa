# Move runtime policy state and rule re-evaluation into Notification Runtime

Status: ready-for-agent
Type: AFK

## What to build

Move loaded policy state ownership into **Notification Runtime**. The runtime should own **Loaded Delivery Policy State**, **User Rule Store** changes, **Do Not Disturb**, and **Active Notification** re-evaluation so policy changes and lifecycle effects stay local to the stateful core.

## Acceptance criteria

- [x] **Notification Runtime** owns construction of **Delivery Policy** from current runtime policy facts.
- [x] **Notification Runtime** owns **User Rule Store** mutations and re-evaluates **Active Notifications** after rule changes.
- [x] **Notification Runtime** owns **Do Not Disturb** mutation and re-evaluates **Active Notifications** after changes.
- [x] `Daemon` no longer assembles **Loaded Delivery Policy State** directly.
- [x] Delivery decision logging still records decisions without **Notification Content**.
- [x] Existing policy, Do Not Disturb, replacement, and presentation tests still pass.

## Blocked by

- [02-introduce-notification-runtime-for-lifecycle-operations.md](02-introduce-notification-runtime-for-lifecycle-operations.md)
