# Dispatch validated configuration policy slices to Notification Runtime

Status: implemented
Type: AFK

## What to build

Keep full-file **Notification Configuration** parsing and validation in the configuration module, then dispatch the validated policy slice to **Notification Runtime**. Runtime installs config **Notification Rules** and re-evaluates **Active Notifications** only after a valid configuration has been accepted.

## Acceptance criteria

- [x] The configuration module validates the full **Notification Configuration** before any normalized slices are dispatched.
- [x] Invalid reload keeps the previous valid configuration active and does not update runtime policy facts.
- [x] Valid reload installs the policy slice into **Notification Runtime**.
- [x] Valid reload re-evaluates **Active Notifications** under the new rules.
- [x] `Daemon` coordinates configuration dispatch without owning policy re-evaluation details.
- [x] Existing configuration reload and policy tests still pass.

## Blocked by

- [03-move-runtime-policy-state-and-rule-reevaluation-into-notification-runtime.md](03-move-runtime-policy-state-and-rule-reevaluation-into-notification-runtime.md)
