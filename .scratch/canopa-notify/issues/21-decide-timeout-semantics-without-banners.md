# Decide Notification Timeout semantics without Notification Banners

Status: ready-for-human
Type: HITL

## Parent

[Canopa Notify](../PRD.md)

## What to build

Choose and document when a Notification Timeout begins and ends for Active Notifications that are not currently shown in a Notification Banner. The decision must cover banner overflow, Do Not Disturb, Silenced Notifications, Headless Mode, replacement revisions, and the existing rule that hidden banner opportunities are not promoted later. It must reconcile freedesktop sender-visible timeout semantics with Notify's persistent Notification Center behavior.

## Acceptance criteria

- [ ] An ADR defines the timeout origin for visible banners, overflow notifications, Silenced Notifications, and Headless Mode.
- [ ] The decision defines whether overflow notifications are promoted when a slot opens or remain Notification Center-only.
- [ ] The decision defines sender-visible behavior for positive, zero, and server-default timeout values when no banner is displayed.
- [ ] Replacement revisions have an explicit rule for cancelling, retaining, or restarting an existing timeout.
- [ ] Do Not Disturb behavior cannot leave finite-timeout senders waiting indefinitely unless that behavior is explicitly chosen and documented.
- [ ] `CONTEXT.md` and the freedesktop compliance ledger use the agreed terminology and semantics.
- [ ] The follow-up implementation slice has sufficiently precise expected behavior to proceed without further product decisions.

## Blocked by

None - can start immediately.
