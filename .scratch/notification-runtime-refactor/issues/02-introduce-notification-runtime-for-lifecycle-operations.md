# Introduce Notification Runtime for lifecycle operations

Status: ready-for-agent
Type: AFK

## What to build

Introduce an internal **Notification Runtime** module that owns **Notification Lifecycle**, timer tasks, **Notification Banner State**, sender-visible lifecycle publication, presentation sync, and **Notification State Subscription** publication. Keep `Daemon` public behavior stable while delegating lifecycle operations to the runtime.

## Acceptance criteria

- [x] `Daemon` delegates notification receipt after **Notification Intake**, action invocation, dismissal, expiration, close, and clear-active operations to **Notification Runtime**.
- [x] **Notification Runtime** owns **Lifecycle Effect** application for timer changes, banner changes, sender events, presentation sync, and state-change publication.
- [x] Timer tasks are owned by **Notification Runtime** and call the lifecycle-owned guarded timeout expiration path.
- [x] `Daemon` no longer owns timer task state, **Notification Banner State**, sender event storage, presentation coordinator, or state-change publisher fields directly.
- [x] Public methods for visible banners, sender events, and notification state subscription keep their current observable behavior.
- [x] Existing lifecycle, presentation, replacement, and notification receipt tests still pass.

## Blocked by

- [01-add-lifecycle-owned-guarded-timeout-expiration.md](01-add-lifecycle-owned-guarded-timeout-expiration.md)
