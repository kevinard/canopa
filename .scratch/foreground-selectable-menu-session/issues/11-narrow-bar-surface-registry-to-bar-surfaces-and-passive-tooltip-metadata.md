Status: ready-for-agent

# Narrow Bar Surface Registry To Bar Surfaces And Passive Tooltip Metadata

## Parent

.scratch/foreground-selectable-menu-session/PRD.md

## What to build

Remove interactive **Panel** and **Menu** lifecycle ownership from the **Bar Surface Registry** after those lifecycles have moved to **Foreground**. The registry should remain the source of truth for live **Bar Surface** records and passive **Tooltip** metadata, while interactive overlay replacement, physical interactive window identity, and stale-close behavior are owned by **Foreground**.

This slice should update tests so they no longer preserve the old interactive overlay ownership model.

## Acceptance criteria

- [ ] **Bar Surface Registry** no longer owns active interactive **Panel** metadata.
- [ ] **Bar Surface Registry** no longer owns active interactive **Menu** metadata.
- [ ] **Bar Surface Registry** still owns live **Bar Surface** records.
- [ ] **Bar Surface Registry** still supports passive **Tooltip** metadata needed by current tooltip behavior.
- [ ] View classification and input routing use **Foreground** for interactive **Panel** and **Menu** windows.
- [ ] Removing a **Bar Surface** still closes or invalidates associated interactive **Foreground** when needed.
- [ ] Tests that previously asserted registry ownership of **Panel** and **Menu** metadata are migrated to **Foreground** tests.
- [ ] Registry tests continue to cover live **Bar Surface** reconciliation and passive **Tooltip** behavior.

## Blocked by

- .scratch/foreground-selectable-menu-session/issues/03-route-panel-entry-points-through-foreground.md
- .scratch/foreground-selectable-menu-session/issues/04-route-menu-keyboard-and-focus-close-paths-through-foreground.md
- .scratch/foreground-selectable-menu-session/issues/08-use-selectable-menu-session-for-sni-menus-without-search.md
