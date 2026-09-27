Status: ready-for-human

# Reset SNI Menu Session On Layout Update

## Parent

.scratch/niribar/foreground-selectable-menu-session/PRD.md

## What to build

When a **SNI Menu** layout updates while it is open, reset its **Selectable Menu Session** to the top level with empty **Menu Search** text and initial focus. This should apply to searchable and non-searchable **SNI Menus**, preserving the decision that DBusMenu layout replacement should not try to preserve stale item focus or search state.

## Acceptance criteria

- [ ] A **SNI Menu** layout update replaces the session source entries.
- [ ] The active menu level resets to the top level after a layout update.
- [ ] The focused entry resets to the initial valid entry after a layout update.
- [ ] Pointer hover state is cleared or reset consistently after a layout update.
- [ ] **Menu Search** text is cleared after a layout update when search is enabled.
- [ ] Non-searchable **SNI Menus** continue to render compactly after a layout update.
- [ ] Tests cover searchable and non-searchable **SNI Menu** layout updates.

## Blocked by

- .scratch/niribar/foreground-selectable-menu-session/issues/09-add-searchable-sni-menus.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/app/foreground.rs; crates/niribar/src/app/selectable_menu_session.rs; src/app/panel_runtime.rs. Evidence: Foreground, menu-session and panel tests; repeated D-Bus menu signal regression.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
