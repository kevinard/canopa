# Decide the trustworthy App Identity resolution boundary

Status: ready-for-human
Type: HITL

## Parent

[Canopa Notify](../PRD.md)

## What to build

Define which notification transports can provide a trustworthy App Identity and how Notify treats sender-claimed `desktop-entry` and application-name values. The decision must preserve useful native compatibility while making clear whether App Permission Controls are convenience policy or a security boundary.

## Acceptance criteria

- [ ] An ADR classifies identity evidence from the XDG Notification portal, D-Bus sender credentials, desktop-entry hints, and application names.
- [ ] The ADR defines a canonical App Identity and a separate sender-claimed display identity when trust cannot be established.
- [ ] Fallback behavior is defined for native scripts, applications without desktop files, sandboxed applications, and stale desktop entries.
- [ ] Cross-sender replacement and `CloseNotification` behavior are explicitly included in the trust model.
- [ ] Existing Notification Rules and Notification History records have a migration or compatibility policy.
- [ ] Diagnostics expose enough non-content information to explain identity resolution without leaking notification content or unsafe process details.
- [ ] The follow-up enforcement slice can proceed without additional identity or compatibility decisions.

## Blocked by

None - can start immediately; use evidence from [24-verify-xdg-notification-portal-v2.md](24-verify-xdg-notification-portal-v2.md) when available.
