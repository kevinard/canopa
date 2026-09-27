Status: ready-for-human

# Route Menu Keyboard And Focus Close Paths Through Foreground

## Parent

.scratch/niribar/foreground-selectable-menu-session/PRD.md

## What to build

Route **SNI Menu** keyboard and focus-driven close behavior through **Foreground**. Escape, focus loss, keyboard activation, and keyboard navigation should operate against the current foreground **Menu** rather than separate role-specific popup state.

This slice should preserve current non-searchable **SNI Menu** keyboard behavior while making **Foreground** the only owner of menu close state.

## Acceptance criteria

- [ ] Keyboard Escape closes the active **SNI Menu** through **Foreground**.
- [ ] Focus loss closes the active **SNI Menu** through **Foreground** when current behavior requires it.
- [ ] Keyboard activation of the focused **SNI Menu** entry still activates that entry.
- [ ] Keyboard focus movement still moves through visible enabled **SNI Menu** entries.
- [ ] Close events from stale **SNI Menu** windows are ignored after focus or keyboard-triggered replacement.
- [ ] Tests cover Escape, focus loss, keyboard activation, keyboard focus movement, and stale close behavior.

## Blocked by

- .scratch/niribar/foreground-selectable-menu-session/issues/02-route-sni-menu-lifecycle-through-foreground.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/app/foreground.rs; crates/niribar/src/app/selectable_menu_session.rs; src/app/panel_runtime.rs. Evidence: Foreground, menu-session and panel tests; repeated D-Bus menu signal regression.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
