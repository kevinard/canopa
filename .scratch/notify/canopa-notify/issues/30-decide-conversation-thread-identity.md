# Decide a conversation Notification Thread identity contract

Status: ready-for-human
Type: HITL

## Parent

[Canopa Notify](../PRD.md)

## What to build

Choose how a sender identifies a conversation or related activity independently from App Identity and freedesktop replacement IDs. The contract must work for native and portal-delivered notifications, avoid treating sender metadata as trusted user identity, and preserve the existing meaning of a replacement-aware Notification Thread.

## Acceptance criteria

- [ ] An ADR distinguishes a sender conversation key from Notification Thread ID, replacement ID, Notification Category, and App Identity.
- [ ] The ADR selects standard portal data, an existing compatibility extension, or a namespaced Canopa hint for each supported transport.
- [ ] Key scope, normalization, maximum size, persistence, replacement behavior, and collision handling are defined.
- [ ] The model supports direct messages, group conversations, non-message activity groups, and notifications with no conversation key.
- [ ] Notification Rules and App Permission Controls either deliberately ignore or deliberately match conversation identity.
- [ ] Migration and protocol-version consequences for Notification History and the shared panel protocol are documented.
- [ ] The implementation slice has complete display and interaction semantics for grouped active and historical items.

## Blocked by

None - can start immediately; use evidence from [24-verify-xdg-notification-portal-v2.md](24-verify-xdg-notification-portal-v2.md) when available.
