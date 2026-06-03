Status: ready-for-agent

# Keep Tooltip Passive Outside Foreground

## Parent

.scratch/foreground-selectable-menu-session/PRD.md

## What to build

Reassert **Tooltip** as passive **Bar Surface** affordance outside **Foreground** after interactive overlay ownership has moved. Tooltip hover, dwell, sizing, placement, close-on-origin disappearance, and close-on-interactive-overlay-open behavior should remain intact without making **Tooltip** a **Foreground** variant.

This slice should protect the explicit architecture decision that **Tooltip** is not **Foreground**.

## Acceptance criteria

- [ ] **Tooltip** is not represented as a **Foreground** state.
- [ ] A **Bar Surface** still shows at most one **Tooltip** at a time.
- [ ] Tooltip dwell and hover-exit behavior remain compatible.
- [ ] A **Tooltip** closes when its originating visible target disappears.
- [ ] Opening a **Panel** closes relevant **Tooltips** without making **Tooltip** part of **Foreground**.
- [ ] Opening a **Menu** closes relevant **Tooltips** without making **Tooltip** part of **Foreground**.
- [ ] Tooltip popup sizing and placement behavior remain compatible.
- [ ] Tests cover tooltip passive ownership, close-on-panel-open, close-on-menu-open, origin disappearance, dwell, and sizing/placement behavior.

## Blocked by

- .scratch/foreground-selectable-menu-session/issues/11-narrow-bar-surface-registry-to-bar-surfaces-and-passive-tooltip-metadata.md
