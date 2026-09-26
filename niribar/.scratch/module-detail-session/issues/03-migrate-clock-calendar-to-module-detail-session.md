Status: ready-for-human

# Migrate Clock Calendar to a Module Detail Session

## Parent

.scratch/module-detail-session/PRD.md

## What to build

Move clock calendar detail state and interactions into a clock-owned **Module Detail Session**. The clock **Module** should own month browsing state, interpret previous/next/today detail messages, and render the calendar from its own detail session.

The user-facing clock calendar behavior should remain unchanged: opening clock **Module Detail Content** shows the current local month, previous and next controls browse months while the **Panel** remains open, Today returns directly to the current month, today is marked when visible, weeks start on Monday, and cells outside the displayed month remain blank.

## Acceptance criteria

- [x] Clock calendar month browsing state is owned by the clock **Module Detail Session**.
- [x] Opening clock **Module Detail Content** always starts from the current local month.
- [x] Previous, next, and Today interactions update the clock-owned detail session through opaque detail messages.
- [x] Existing clock calendar rendering behavior is preserved.
- [x] Tests cover session initialization, previous-month browsing, next-month browsing, Today reset, and reset-on-reopen behavior.

## Blocked by

- .scratch/module-detail-session/issues/01-add-module-detail-session-seam.md
- .scratch/module-detail-session/issues/02-route-opaque-module-detail-messages.md

## Implementation review — 2026-09-23

Implementation is present in src/modules.rs; src/app/module_runtime.rs; src/modules/clock.rs. Evidence: module detail and clock/calendar tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
