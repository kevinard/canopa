Status: ready-for-agent

# Add Foreground Panel Lifecycle Tracer

## Parent

.scratch/foreground-selectable-menu-session/PRD.md

## What to build

Introduce **Foreground** as the owner of one active interactive overlay and prove the shape with a narrow **Panel** lifecycle path. The slice should let a **Panel** open, replace an existing **Panel**, close, and ignore stale physical-window close events through **Foreground** while preserving current user-facing **Panel** behavior.

This is a tracer bullet: it should establish the **Foreground** intent/effect seam and physical overlay window identity for **Panels** without migrating every **Panel** entry point or every **Menu** path yet.

## Acceptance criteria

- [ ] **Foreground** can represent no active interactive overlay and an active **Panel**.
- [ ] Opening a **Panel** through **Foreground** records the physical overlay window identity for that **Panel**.
- [ ] Replacing one **Panel** with another makes the old **Panel** logically gone immediately.
- [ ] A stale close event from the replaced **Panel** window does not clear the current **Foreground**.
- [ ] A close event from the current **Panel** window clears the current **Foreground**.
- [ ] Existing **Panel** rendering and basic open/close behavior remain user-facing compatible.
- [ ] Tests cover opening, replacing, current close, and stale close through the **Foreground** seam.

## Blocked by

None - can start immediately
