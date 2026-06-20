# Handle replacements in Notification Lifecycle

Status: ready-for-agent
Type: AFK

## Parent

[Notification Lifecycle Refactor](../PRD.md)

## What to build

Move replacement handling into **Notification Lifecycle**. Delivery Policy should still run before lifecycle, but lifecycle should own replacement target lookup, revision behavior, and the effect semantics for shown, silenced, and dropped replacements.

## Acceptance criteria

- [ ] Replacement target lookup happens inside Notification Lifecycle through the Notification History trait.
- [ ] A shown replacement revises the existing Notification Thread and produces lifecycle effects for the new revision.
- [ ] A silenced replacement revises history while producing effects that allow runtime banner state to hide any visible banner.
- [ ] A dropped replacement produces a dropped receipt effect without revising the existing Notification Thread.
- [ ] Replacement tests cover shown, silenced, dropped, missing target, and revision behavior through lifecycle and daemon seams.

## Blocked by

- [01-route-new-notifications-through-lifecycle.md](01-route-new-notifications-through-lifecycle.md)

