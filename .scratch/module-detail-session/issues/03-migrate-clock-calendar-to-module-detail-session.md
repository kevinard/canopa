Status: ready-for-agent

# Migrate Clock Calendar to a Module Detail Session

## Parent

.scratch/module-detail-session/PRD.md

## What to build

Move clock calendar detail state and interactions into a clock-owned **Module Detail Session**. The clock **Module** should own month browsing state, interpret previous/next/today detail messages, and render the calendar from its own detail session.

The user-facing clock calendar behavior should remain unchanged: opening clock **Module Detail Content** shows the current local month, previous and next controls browse months while the **Panel** remains open, Today returns directly to the current month, today is marked when visible, weeks start on Monday, and cells outside the displayed month remain blank.

## Acceptance criteria

- [ ] Clock calendar month browsing state is owned by the clock **Module Detail Session**.
- [ ] Opening clock **Module Detail Content** always starts from the current local month.
- [ ] Previous, next, and Today interactions update the clock-owned detail session through opaque detail messages.
- [ ] Existing clock calendar rendering behavior is preserved.
- [ ] Tests cover session initialization, previous-month browsing, next-month browsing, Today reset, and reset-on-reopen behavior.

## Blocked by

- .scratch/module-detail-session/issues/01-add-module-detail-session-seam.md
- .scratch/module-detail-session/issues/02-route-opaque-module-detail-messages.md
