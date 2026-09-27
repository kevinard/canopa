# Normalize freedesktop Notification Actions and Resident Notifications

Status: implemented

## Parent

.scratch/notify/canopa-notify/PRD.md

## What to build

Normalize freedesktop **Notification Action** input end to end so Canopa Notify stores only valid sender action facts, treats action keys as opaque sender-defined values, and honors boolean true resident hints as **Resident Notification** behavior without advertising freedesktop notification persistence.

This slice should make action receipt behavior observable through normal notification receipt, history, and diagnostic output. It should not add banner buttons, Notification Center interaction handling, action icons, activation tokens, inline replies, or the freedesktop `"persistence"` capability.

## Acceptance criteria

- [ ] Freedesktop action pairs with empty keys, empty labels after trimming, malformed tails, or duplicate keys are normalized so only valid first-seen action keys remain.
- [ ] **Notification Action** labels are stored after trimming leading and trailing whitespace while preserving interior whitespace.
- [ ] **Notification Action** keys remain opaque and case-sensitive through storage and invocation matching.
- [ ] A boolean true freedesktop resident hint produces **Resident Notification** behavior; missing, false, or wrong-typed resident hints do not.
- [ ] `GetCapabilities` still advertises `"actions"` but does not advertise `"persistence"`, `action-icons`, activation tokens, or inline replies from this slice.
- [ ] Tests cover action normalization, resident hint decoding, persistence capability exclusion, and history/diagnostic visibility of normalized action facts.

## Blocked by

None - can start immediately
