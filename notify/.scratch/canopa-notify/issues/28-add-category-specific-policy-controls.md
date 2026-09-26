# Add Notification Category-specific policy controls

Status: implemented
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Let users control notification delivery for a specific Notification Category within one App Identity. Category-aware Notification Rules must work through configuration, the User Rule Store, Delivery Policy, Notification Center controls, persistence, and live policy replacement while preserving the existing app-wide fallback.

## Acceptance criteria

- [ ] A Notification Rule can match App Identity plus an optional Notification Category and Notification Urgency.
- [ ] Category-specific rules take precedence according to one documented deterministic ordering that preserves existing rule behavior.
- [ ] Notification Configuration can declare category-specific rules and invalid category rule shapes fail atomically on reload.
- [ ] The User Rule Store persists category-specific allow, silence, and drop outcomes.
- [ ] Notification Center groups expose category controls only when category information is available and retain an app-wide fallback control.
- [ ] Policy changes apply to future requests and replacements without retroactively rewriting historical Delivery Outcomes.
- [ ] Tests cover standard, vendor-prefixed, missing, malformed, and changing categories across configuration, control, policy, and presentation seams.

## Blocked by

None - can start immediately.
