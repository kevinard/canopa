# Make notification actions and dismissal accessible

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Make Notification Banner and Notification Center interactions expose meaningful accessible names, roles, states, and keyboard behavior while preserving the rule that Notification Banners do not steal focus. Action icons must retain their localized labels for assistive technology, and dismissal must no longer be represented only by a visual `x` glyph.

## Acceptance criteria

- [ ] Notification Banner content, Default Notification Action, named actions, hyperlinks, progress, and dismissal expose meaningful accessible semantics supported by the UI stack.
- [ ] Every icon-backed Notification Action retains its localized label as an accessible name even if presentation later omits visible text.
- [ ] The dismiss control has a localized accessible label that identifies the affected notification.
- [ ] Notification Banners remain non-focus-stealing and do not introduce an unexpected global keyboard focus target.
- [ ] Notification Center items and controls are reachable in a predictable keyboard order and can be activated, dismissed, and controlled without a pointer.
- [ ] Dynamic replacements, progress changes, and newly arrived items have a documented announcement policy that avoids repeated disruptive announcements.
- [ ] Automated widget/projection tests cover labels and interaction mapping, with a documented manual screen-reader and keyboard verification checklist for the active UI hosts.

## Blocked by

None - can start immediately.
