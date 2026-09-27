# Move runtime policy state into Notification Runtime

Status: implemented
Type: AFK

## What to build

Move loaded policy state ownership into **Notification Runtime**. The runtime should own **Loaded Delivery Policy State**, **User Rule Store** changes, **Do Not Disturb**, and future-request policy evaluation so policy changes stay local to the stateful core (ADR 0027).

## Acceptance criteria

- [x] **Notification Runtime** owns construction of **Delivery Policy** from current runtime policy facts.
- [x] **Notification Runtime** owns **User Rule Store** mutations and applies updated policy to future requests, including replacements.
- [x] **Notification Runtime** owns **Do Not Disturb** mutation and preserves the committed decisions and banner state of existing notifications.
- [x] `Daemon` no longer assembles **Loaded Delivery Policy State** directly.
- [x] Delivery decision logging still records decisions without **Notification Content**.
- [x] Existing policy, Do Not Disturb, replacement, and presentation tests still pass.

## Blocked by

- [02-introduce-notification-runtime-for-lifecycle-operations.md](02-introduce-notification-runtime-for-lifecycle-operations.md)

## Maintenance note

2026-09-21: Acceptance wording reconciled with the selected Notifications 1.3 profile. ADR 0027 supersedes retroactive policy re-evaluation; ADR 0040 defines atomic configuration candidates. Historical filenames and links are retained. This edit does not claim new test or rollout evidence.
