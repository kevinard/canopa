# Route new notifications through Notification Lifecycle

Status: implemented
Type: AFK

## Parent

[Notification Lifecycle Refactor](../PRD.md)

## What to build

Introduce the first **Notification Lifecycle** module as a generic async struct over a **Notification History** trait. Route valid new notification receipt through lifecycle so policy decisions become lifecycle inputs, lifecycle writes retained Notification Threads, and the Notification Daemon derives the sender receipt from fine-grained Lifecycle Effects. No-op Notification Intake rejection should continue returning Dropped directly without entering policy or lifecycle.

## Acceptance criteria

- [ ] Valid shown and silenced notifications are created through Notification Lifecycle and persisted through the Notification History trait.
- [ ] Valid dropped notifications enter Notification Lifecycle and produce a receipt effect without retaining history.
- [ ] No-op Notification Intake rejection still returns Dropped directly before Delivery Policy or Notification Lifecycle.
- [ ] Lifecycle returns fine-grained Lifecycle Effects, including a sender receipt effect.
- [ ] Lifecycle tests use an in-memory Notification History adapter.
- [ ] Existing daemon receipt tests still pass through the public daemon interface.

## Blocked by

None - can start immediately

