# Support portal inline replies through Notification Center

Status: ready-for-human
Type: HITL

## Parent

[Canopa Notify](../PRD.md)

## What to build

Support the XDG Notification portal v2 `im.reply-with-text` button purpose from receipt through a keyboard-interactive Notification Center reply control to application activation. Notification Banners must remain non-focus-stealing; the external Notification Center host must be verified to collect and return reply text safely.

## Acceptance criteria

- [ ] Portal notifications with the `im.received` category and `im.reply-with-text` purpose retain the reply action's identity, label, target, and response contract.
- [ ] Notification Center presents a text-entry reply control only for active notifications that requested inline reply.
- [ ] Submitting a reply invokes the correct portal action with target data, platform activation data, and user response in the specified order.
- [ ] Cancelling or submitting a reply has explicit Resident Notification and Sender Notification Lifecycle behavior.
- [ ] Reply text is never written to Notification History, Delivery Decision Logs, or normal diagnostics.
- [ ] Notification Banners remain non-focus-stealing and offer a route to the Notification Center reply UI rather than acquiring keyboard focus.
- [ ] Human verification confirms the active Notification Center host supports the required panel protocol and text-entry interaction; protocol tests cover all daemon-owned behavior.

## Blocked by

- [24-verify-xdg-notification-portal-v2.md](24-verify-xdg-notification-portal-v2.md)
- External Niribar or Notification Center host support for keyboard text entry.
