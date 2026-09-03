# Verify XDG Notification portal v2 delivery end to end

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Establish and test the supported path by which a sandboxed application sends, updates, removes, and activates a notification through XDG Notification portal v2 into Canopa Notify. The slice should identify which portal fields survive the installed portal implementation and provide a repeatable compatibility fixture before Notify adds portal-specific behavior.

## Acceptance criteria

- [ ] A documented test environment routes `AddNotification` through a real XDG Desktop Portal frontend to Canopa Notify.
- [ ] Title, body, application identity, priority, default action, named buttons, and icon behavior are verified end to end.
- [ ] Reusing one application-scoped portal ID updates the same active notification without flicker or duplicate Notification History entries.
- [ ] `RemoveNotification` withdraws the corresponding Active Notification and completes the application-visible lifecycle.
- [ ] Action activation returns the correct application-scoped ID, action target, and Activation Token where the portal supports them.
- [ ] The fixture records current behavior for portal v2 display hints, category, file-descriptor visuals, sound, and inline-reply purposes even when unsupported.
- [ ] Automated coverage handles deterministic protocol behavior, with a clearly documented manual check only for environment-owned presentation behavior.

## Blocked by

None - can start immediately.
