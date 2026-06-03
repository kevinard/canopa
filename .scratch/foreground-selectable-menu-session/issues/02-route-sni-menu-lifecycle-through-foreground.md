Status: ready-for-agent

# Route SNI Menu Lifecycle Through Foreground

## Parent

.scratch/foreground-selectable-menu-session/PRD.md

## What to build

Route the basic **SNI Menu** open, replace, close, and stale-close lifecycle through **Foreground**. Opening a **SNI Menu** should make it the active interactive overlay, close the current **Panel** if one is active, close relevant **Tooltips**, and own the physical **Menu** window identity through **Foreground**.

This slice should preserve current compact **SNI Menu** behavior. It should not add **Menu Search** yet.

## Acceptance criteria

- [ ] **Foreground** can represent an active **SNI Menu**.
- [ ] Opening a **SNI Menu** through **Foreground** records the physical **Menu** window identity.
- [ ] Opening a **SNI Menu** replaces an active **Panel**.
- [ ] Opening a **SNI Menu** closes open **Tooltips** on the originating **Bar Surface**.
- [ ] Replacing one **SNI Menu** with another makes the old **Menu** logically gone immediately.
- [ ] A stale close event from the replaced **SNI Menu** window does not clear the current **Foreground**.
- [ ] A close event from the current **SNI Menu** window clears the current **Foreground**.
- [ ] Existing **SNI Menu** rendering and item activation behavior remain user-facing compatible.
- [ ] Tests cover open, replacement, current close, stale close, and replacement of an active **Panel**.

## Blocked by

- .scratch/foreground-selectable-menu-session/issues/01-add-foreground-panel-lifecycle-tracer.md
