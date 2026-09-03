# Enforce resolved App Identity across policy, history, and lifecycle

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Implement the agreed App Identity resolution model from notification receipt through Delivery Policy, Notification Lifecycle, Notification History grouping, App Permission Controls, replacements, and diagnostics. Sender-claimed metadata must no longer silently acquire more trust than its transport provides.

## Acceptance criteria

- [ ] Notification Intake receives the identity evidence required by the ADR and produces the canonical App Identity plus any separate display metadata.
- [ ] Delivery Policy and User Rule Store matching use only the canonical App Identity.
- [ ] Notification History and Notification Center grouping remain stable for verified applications and have documented fallback grouping for unverified senders.
- [ ] A sender cannot replace or close another sender's Active Notification when the chosen identity model forbids it.
- [ ] Existing stored rules and history are migrated or read compatibly according to the ADR.
- [ ] Delivery Decision Logs explain the identity source and trust class without including Notification Content.
- [ ] D-Bus and portal tests cover verified, claimed, missing, conflicting, and cross-sender identities.

## Blocked by

- [26-decide-trustworthy-app-identity-boundary.md](26-decide-trustworthy-app-identity-boundary.md)
