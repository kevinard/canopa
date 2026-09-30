Status: ready-for-human

# Remove Clock-Specific Panel Runtime Plumbing

## Parent

.scratch/niribar/module-detail-session/PRD.md

## What to build

Remove the clock-specific state, events, and shared-interface arguments that became obsolete once the clock calendar is owned by a clock **Module Detail Session**. Generic **Panel Runtime** should no longer interpret clock month browsing or expose clock-specific panel events.

This slice should leave **Panel Runtime** responsible for generic panel lifecycle only, with clock detail behavior tested through the clock-owned **Module Detail Session** seam.

## Acceptance criteria

- [x] Generic **Panel Runtime** no longer stores clock-specific month offset state.
- [x] Generic panel messages no longer include clock-specific previous, next, or Today events.
- [x] The shared **Module** detail rendering interface no longer needs a clock-specific argument.
- [x] Tests tied to old clock-specific **Panel Runtime** internals are removed or rewritten against the new seam.
- [x] Existing panel lifecycle tests continue to cover generic open, close, toggle, replacement, focus, and Escape behavior.

## Blocked by

- .scratch/niribar/module-detail-session/issues/03-migrate-clock-calendar-to-module-detail-session.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/modules.rs; crates/niribar/src/app/module_runtime.rs; src/modules/clock.rs. Evidence: module detail and clock/calendar tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
