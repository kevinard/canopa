# Decide Notification Timeout semantics without Notification Banners

Status: implemented
Type: HITL

## Parent

[Canopa Notify](../PRD.md)

## What to build

Choose and document when a Notification Timeout begins and ends for Active Notifications that are not currently shown in a Notification Banner. The decision must cover banner overflow, Do Not Disturb, Silenced Notifications, Headless Mode, replacement revisions, and the existing rule that hidden banner opportunities are not promoted later. It must reconcile freedesktop sender-visible timeout semantics with Notify's persistent Notification Center behavior.

## Acceptance criteria

- [x] An ADR defines the timeout origin for visible banners, overflow notifications, Silenced Notifications, and Headless Mode.
- [x] The decision defines whether overflow notifications are promoted when a slot opens or remain Notification Center-only.
- [x] The decision defines sender-visible behavior for positive, zero, and server-default timeout values when no banner is displayed.
- [x] Replacement revisions have an explicit rule for cancelling, retaining, or restarting an existing timeout.
- [x] Do Not Disturb behavior cannot leave finite-timeout senders waiting indefinitely unless that behavior is explicitly chosen and documented.
- [x] `CONTEXT.md` and the freedesktop compliance ledger use the agreed terminology and semantics.
- [x] The follow-up implementation slice has sufficiently precise expected behavior to proceed without further product decisions.

## Blocked by

None - can start immediately.

## Comments

- Resolved with the maintainer on 2026-09-03. [ADR 0028](../../../docs/adr/0028-notification-timeouts-begin-with-banner-display.md) records that timeouts begin with actual banner display and deliberately do not begin for notifications that never display a banner.
