# Open safe Notification Body hyperlinks with Activation Tokens

Status: done
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Complete freedesktop Notification Body hyperlink support by retaining safe hyperlink targets in interpreted Notification Body content and making link text actionable in Notification Banners and Notification Center. Link activation must use an appropriate user-interaction Activation Token and must never turn malformed or unsafe markup into an executable target.

## Acceptance criteria

- [x] Notification Intake retains ordered hyperlink text and targets separately from the plain Notification Body fallback.
- [x] Only an explicit documented set of URI schemes is actionable; malformed, credential-bearing, control-character, oversized, and unsupported targets degrade to plain text.
- [x] Notification Banners and Notification Center visually distinguish link text and expose an accessible action label containing the visible text rather than the raw target.
- [x] Link activation goes through the desktop's URI-opening mechanism with a best-effort Activation Token derived from the user interaction.
- [x] Activating a hyperlink does not implicitly invoke a Default Notification Action or dismiss a Resident Notification unless documented policy says so.
- [x] Raw targets are excluded from normal diagnostics and Delivery Decision Logs while remaining available in the active presentation model.
- [x] `body-hyperlinks` is advertised only after D-Bus, parsing, banner, Notification Center, security-degradation, and activation tests pass end to end.

## Blocked by

None - can start immediately.

## Comments

2026-09-16: Implemented in the paired Notify/Niribar completion changes. [Desktop acceptance](../../../docs/notification-desktop-acceptance.md) records real pixels, input, portal opening and Orca speech; [release verification](../../../docs/notification-release-verification.md) records the full/core/host suites and exact capability checks. Current host transport is Canopa v4. The compatible host source is applied; the live desktop configuration/service was not changed.
