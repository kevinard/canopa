Status: ready-for-agent

# Use Selectable Menu Session For SNI Menus Without Search

## Parent

.scratch/foreground-selectable-menu-session/PRD.md

## What to build

Migrate non-searchable **SNI Menus** to use **Selectable Menu Session** for active menu level, pointer hover, keyboard focus, navigation, and generic selected-entry completion. **Menu Search** must remain disabled by default for **SNI Menus**, and the compact user-facing **SNI Menu** presentation should remain unchanged.

The owning **Menu** foreground adapter should interpret generic selected entries as DBusMenu item activations.

## Acceptance criteria

- [ ] **SNI Menus** open a **Selectable Menu Session** with **Menu Search** disabled by default.
- [ ] Pointer hover for **SNI Menu** entries is owned by the session.
- [ ] Keyboard focus for **SNI Menu** entries is owned by the session.
- [ ] Keyboard navigation still skips separators, disabled entries, and invisible entries.
- [ ] Activating a **SNI Menu** entry returns a generic selected entry from the session.
- [ ] The owning **Menu** foreground adapter interprets the generic selected entry as DBusMenu item activation.
- [ ] Existing compact **SNI Menu** rendering remains unchanged when search is disabled.
- [ ] Existing **SNI Menu** hover, focus, keyboard navigation, and activation tests pass or are migrated to the session seam.

## Blocked by

- .scratch/foreground-selectable-menu-session/issues/02-route-sni-menu-lifecycle-through-foreground.md
- .scratch/foreground-selectable-menu-session/issues/05-use-selectable-menu-session-for-configured-panel-menus.md
