# Move Notification Center Projection and subscriptions into Notification Runtime

Status: implemented
Type: AFK

## What to build

Move **Notification Center Projection** derivation and **Notification State Subscription** publication into **Notification Runtime**. The projection remains derived current state, rebuilt from runtime-owned state when requested or emitted, with no separate projection store.

## Acceptance criteria

- [x] **Notification Runtime** derives **Notification Center Projection** from current **Notification History** and app permission policy facts.
- [x] **Notification Runtime** owns **Notification State Subscription** publication for projection-relevant changes.
- [x] Subscriptions still emit an initial complete projection.
- [x] Subscriptions still emit complete replacement projections after notification, lifecycle, rule, and Do Not Disturb changes.
- [x] No separate projection store is introduced.
- [x] Existing Notify Control API and Panel Provider Mode tests still pass.

## Blocked by

- [03-move-runtime-policy-state-and-rule-reevaluation-into-notification-runtime.md](03-move-runtime-policy-state-and-rule-reevaluation-into-notification-runtime.md)
