# Make notification actions and dismissal accessible

Status: done
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Make Notification Banner and Notification Center interactions expose meaningful accessible names, roles, states, and keyboard behavior while preserving the rule that Notification Banners do not steal focus. Action icons must retain their localized labels for assistive technology, and dismissal must no longer be represented only by a visual `x` glyph.

## Acceptance criteria

- [x] Notification Banner content, Default Notification Action, named actions, hyperlinks, progress, and dismissal expose meaningful accessible semantics supported by the UI stack.
- [x] Every icon-backed Notification Action retains its localized label as an accessible name even if presentation later omits visible text.
- [x] The dismiss control has a localized accessible label that identifies the affected notification.
- [x] Notification Banners remain non-focus-stealing and do not introduce an unexpected global keyboard focus target.
- [x] Notification Center items and controls are reachable in a predictable keyboard order and can be activated, dismissed, and controlled without a pointer.
- [x] Dynamic replacements, progress changes, and newly arrived items have a documented announcement policy that avoids repeated disruptive announcements.
- [x] Automated widget/projection tests cover labels and interaction mapping, with a documented manual screen-reader and keyboard verification checklist for the active UI hosts.

## Blocked by

None - can start immediately.

## Comments

2026-09-14: The [Center keyboard continuation](../../freedesktop-completion-review/CENTER-KEYBOARD-EVIDENCE.md) establishes partial real-host keyboard behavior and records a concrete blocker for assistive technology. The pinned Iced widget and layer-shell stack has no implemented semantic-tree/platform bridge. Visible names and provider labels must not be counted as accessible names/roles/states or an announcement policy. A supported bridge and full-session screen-reader verification remain required; no acceptance boxes are closed by this partial keyboard work.

2026-09-14: The [window-lifetime follow-up](../../freedesktop-completion-review/CENTER-WINDOW-EVIDENCE.md) corrects rapid-key loss and replacement-induced interruption by keeping the Canopa panel window stable. Forty graphical trials and six new application regressions pass; the full host suite passes 1,151 tests. This remains partial keyboard evidence, not semantic-tree, announcement, held-key or full keyboard-reading acceptance.

2026-09-16: Implemented in the paired Notify/Niribar completion changes. [Desktop acceptance](../../../docs/notification-desktop-acceptance.md) records real pixels, input, portal opening and Orca speech; [release verification](../../../docs/notification-release-verification.md) records the full/core/host suites and exact capability checks. Current host transport is Canopa v4. The compatible host source is applied; the live desktop configuration/service was not changed.
