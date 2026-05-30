Status: ready-for-agent

# Remove Clock-Specific Panel Runtime Plumbing

## Parent

.scratch/module-detail-session/PRD.md

## What to build

Remove the clock-specific state, events, and shared-interface arguments that became obsolete once the clock calendar is owned by a clock **Module Detail Session**. Generic **Panel Runtime** should no longer interpret clock month browsing or expose clock-specific panel events.

This slice should leave **Panel Runtime** responsible for generic panel lifecycle only, with clock detail behavior tested through the clock-owned **Module Detail Session** seam.

## Acceptance criteria

- [ ] Generic **Panel Runtime** no longer stores clock-specific month offset state.
- [ ] Generic panel messages no longer include clock-specific previous, next, or Today events.
- [ ] The shared **Module** detail rendering interface no longer needs a clock-specific argument.
- [ ] Tests tied to old clock-specific **Panel Runtime** internals are removed or rewritten against the new seam.
- [ ] Existing panel lifecycle tests continue to cover generic open, close, toggle, replacement, focus, and Escape behavior.

## Blocked by

- .scratch/module-detail-session/issues/03-migrate-clock-calendar-to-module-detail-session.md
