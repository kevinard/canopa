# Load KDL Notification Configuration and config rules

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Add KDL Notification Configuration loading and reload behavior for first-version config-authored rules. The daemon has one active configuration, rejects invalid reloads while keeping the previous valid configuration active, evaluates ordered first-decisive rules by App Identity and optional Notification Urgency, records Rule References and Config Rule IDs where available, re-evaluates Active Notifications on valid reload, and reports configuration errors through logs/control surfaces instead of self-notifications.

## Acceptance criteria

- [ ] KDL configuration is loaded using a parser/diagnostic style aligned with Niribar where practical.
- [ ] The daemon maintains one active Notification Configuration at a time.
- [ ] Invalid reloads are rejected without replacing the previous valid configuration.
- [ ] Config rules use first-decisive delivery semantics.
- [ ] Config rules can match App Identity and optional Notification Urgency.
- [ ] Explicit drop rules apply even to critical notifications.
- [ ] Config Rule IDs are recorded as Rule References, and unnamed rules get generated references.
- [ ] Valid reloads re-evaluate Active Notifications and may close banners without creating late banners.
- [ ] Configuration errors are visible through logs/control surfaces and do not create self-notifications.
- [ ] Tests cover parsing, reload success/failure, rule evaluation, critical drop, Rule References, active re-evaluation, and error reporting.

## Blocked by

- [08-add-do-not-disturb-end-to-end.md](08-add-do-not-disturb-end-to-end.md)
