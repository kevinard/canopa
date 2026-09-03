# Open safe Notification Body hyperlinks with Activation Tokens

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Complete freedesktop Notification Body hyperlink support by retaining safe hyperlink targets in interpreted Notification Body content and making link text actionable in Notification Banners and Notification Center. Link activation must use an appropriate user-interaction Activation Token and must never turn malformed or unsafe markup into an executable target.

## Acceptance criteria

- [ ] Notification Intake retains ordered hyperlink text and targets separately from the plain Notification Body fallback.
- [ ] Only an explicit documented set of URI schemes is actionable; malformed, credential-bearing, control-character, oversized, and unsupported targets degrade to plain text.
- [ ] Notification Banners and Notification Center visually distinguish link text and expose an accessible action label containing the visible text rather than the raw target.
- [ ] Link activation goes through the desktop's URI-opening mechanism with a best-effort Activation Token derived from the user interaction.
- [ ] Activating a hyperlink does not implicitly invoke a Default Notification Action or dismiss a Resident Notification unless documented policy says so.
- [ ] Raw targets are excluded from normal diagnostics and Delivery Decision Logs while remaining available in the active presentation model.
- [ ] `body-hyperlinks` is advertised only after D-Bus, parsing, banner, Notification Center, security-degradation, and activation tests pass end to end.

## Blocked by

None - can start immediately.
