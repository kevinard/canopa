Status: ready-for-agent

# Route Menu Keyboard And Focus Close Paths Through Foreground

## Parent

.scratch/foreground-selectable-menu-session/PRD.md

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

- .scratch/foreground-selectable-menu-session/issues/02-route-sni-menu-lifecycle-through-foreground.md
