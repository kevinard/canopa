# Dispatch validated configuration policy slices to Notification Runtime

Status: implemented
Type: AFK

## What to build

Keep full-file **Notification Configuration** parsing and validation in the configuration module, then dispatch the validated policy slice to **Notification Runtime**. Runtime installs config **Notification Rules** for future requests only after a complete valid configuration has been accepted (ADRs 0027 and 0040).

## Acceptance criteria

- [x] The configuration module validates the full **Notification Configuration** before any normalized slices are dispatched.
- [x] Invalid reload keeps the previous valid configuration active and does not update runtime policy facts.
- [x] Valid reload installs the policy slice into **Notification Runtime**.
- [x] Valid reload applies the new rules to future requests, including replacements; existing notifications and banner state remain unchanged.
- [x] `Daemon` coordinates configuration dispatch without owning policy evaluation details.
- [x] Existing configuration reload and policy tests still pass.

## Blocked by

- [03-move-runtime-policy-state-and-rule-reevaluation-into-notification-runtime.md](03-move-runtime-policy-state-and-rule-reevaluation-into-notification-runtime.md)

## Maintenance note

2026-09-21: Acceptance wording reconciled with the selected Notifications 1.3 profile. ADR 0027 supersedes retroactive policy re-evaluation; ADR 0040 defines atomic configuration candidates. Historical filenames and links are retained. This edit does not claim new test or rollout evidence.
