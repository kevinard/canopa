Status: ready-for-agent

# Route Panel Entry Points Through Foreground

## Parent

.scratch/foreground-selectable-menu-session/PRD.md

## What to build

Route all **Panel** open, toggle, and close entry points through **Foreground**. Configured **Panel Actions**, focused key handling, Escape handling, explicit close actions, and **Global Panel Toggle** should all use the same foreground lifecycle instead of reaching panel lifecycle state directly.

This slice should not change user-facing **Panel** behavior; it should eliminate alternate panel lifecycle paths that bypass **Foreground**.

## Acceptance criteria

- [ ] Configured `open-panel` actions route through **Foreground**.
- [ ] Configured `toggle-panel` actions route through **Foreground**.
- [ ] Configured `close-panel` actions route through **Foreground**.
- [ ] Keyboard Escape for an active **Panel** routes through **Foreground**.
- [ ] **Global Panel Toggle** opens or closes through **Foreground**.
- [ ] Toggling the active **Panel** closed clears **Foreground** and removes the physical window.
- [ ] Opening a **Panel** while a **Menu** is active replaces the **Menu** through **Foreground**.
- [ ] Tests cover configured action, Escape, toggle, and IPC/global toggle paths through **Foreground**.

## Blocked by

- .scratch/foreground-selectable-menu-session/issues/01-add-foreground-panel-lifecycle-tracer.md
