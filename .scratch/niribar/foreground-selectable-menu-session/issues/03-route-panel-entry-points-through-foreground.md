Status: ready-for-human

# Route Panel Entry Points Through Foreground

## Parent

.scratch/niribar/foreground-selectable-menu-session/PRD.md

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

- .scratch/niribar/foreground-selectable-menu-session/issues/01-add-foreground-panel-lifecycle-tracer.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/app/foreground.rs; crates/niribar/src/app/selectable_menu_session.rs; src/app/panel_runtime.rs. Evidence: Foreground, menu-session and panel tests; repeated D-Bus menu signal regression.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
