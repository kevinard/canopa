Status: ready-for-human

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

## Implementation review — 2026-09-23

Implementation is present in src/app/foreground.rs; src/app/selectable_menu_session.rs; src/app/panel_runtime.rs. Evidence: Foreground, menu-session and panel tests; repeated D-Bus menu signal regression.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
