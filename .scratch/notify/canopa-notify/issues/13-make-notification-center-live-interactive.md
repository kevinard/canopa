# Make Notification Center live and interactive

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Turn the read-only Notification Center into the first interactive center. While open, it receives live notification state updates through Notification State Subscription, exposes App Permission Controls backed by the User Rule Store, supports Clear Active Notifications and Delete Notification History, and invokes Notification Actions for silenced or shown Active Notifications.

## Acceptance criteria

- [ ] Panel Provider Mode uses Notification State Subscription while the Notification Center is open.
- [ ] The open Notification Center updates when notifications arrive, are replaced, expire, are dismissed, are closed, or change through DND/rule updates.
- [ ] App Permission Controls show current policy for each App Identity group.
- [ ] Changing an App Permission Control writes to the User Rule Store without rewriting Notification Configuration.
- [ ] Historical records can show their original Delivery Outcome separately from current policy.
- [ ] Clear Active Notifications dismisses active notifications without deleting history.
- [ ] Delete Notification History removes retained inactive history.
- [ ] Notification Actions can be invoked from active notifications shown in the Notification Center, including silenced Active Notifications.
- [ ] Tests cover live updates, app policy changes, clear/delete actions, action invocation, current-policy versus historical-outcome display, and protocol event handling.

## Blocked by

- [10-add-user-rule-store-app-policy.md](10-add-user-rule-store-app-policy.md)
- [12-render-read-only-notification-center-provider.md](12-render-read-only-notification-center-provider.md)
