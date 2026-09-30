Status: ready-for-human

# Use Selectable Menu Session For Configured Panel Menus

## Parent

.scratch/niribar/foreground-selectable-menu-session/PRD.md

## What to build

Introduce **Selectable Menu Session** and migrate configured **Panel Menus** to use it for selectable-list interaction. The session should own active menu level, **Menu Search**, keyboard focus, pointer hover, nested flattening with breadcrumb context, and generic selected-entry completion for configured **Panel Menu** entries.

The owning **Panel** foreground should interpret generic selected entries as configured **Panel Actions**. Existing **Panel Menu** user-facing search and navigation behavior should remain compatible.

## Acceptance criteria

- [ ] **Selectable Menu Session** can open from a configured **Panel Menu** source.
- [ ] The session snapshot includes visible entries, search text, focused entry, hover state, prompt, icon visibility, and breadcrumb context where applicable.
- [ ] Pointer hover and keyboard focus are owned by the session.
- [ ] **Menu Search** preserves current configured **Panel Menu** fuzzy matching behavior.
- [ ] Nested configured **Panel Menu** entries are searchable with breadcrumb context.
- [ ] Activating a configured **Panel Menu** entry returns a generic selected entry from the session.
- [ ] The owning **Panel** foreground interprets the generic selected entry as a configured **Panel Action**.
- [ ] Existing configured **Panel Menu** tests pass or are migrated to assert behavior through **Selectable Menu Session**.
- [ ] New tests cover focus, hover, search, nested entries, and generic selected-entry activation.

## Blocked by

- .scratch/niribar/foreground-selectable-menu-session/issues/01-add-foreground-panel-lifecycle-tracer.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/app/foreground.rs; crates/niribar/src/app/selectable_menu_session.rs; src/app/panel_runtime.rs. Evidence: Foreground, menu-session and panel tests; repeated D-Bus menu signal regression.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
