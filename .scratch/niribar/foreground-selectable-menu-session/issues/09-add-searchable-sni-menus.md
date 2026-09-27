Status: ready-for-human

# Add Searchable SNI Menus

## Parent

.scratch/niribar/foreground-selectable-menu-session/PRD.md

## What to build

Add tray **Module** configuration that enables **Menu Search** for **SNI Menus**. When enabled, an open **SNI Menu** should render a visible search input and use the same **Menu Search** matching, ranking, nested-entry, breadcrumb, focus, hover, and activation behavior as other searchable selectable menus.

Search must remain disabled by default for **SNI Menus**.

## Acceptance criteria

- [ ] Tray **Module** configuration can enable **Menu Search** for **SNI Menus**.
- [ ] Search is disabled by default for **SNI Menus**.
- [ ] A searchable **SNI Menu** renders a visible search input while open.
- [ ] Searchable **SNI Menus** use the shared **Menu Search** fuzzy matching behavior.
- [ ] Searchable **SNI Menus** search nested entries and display breadcrumb context.
- [ ] Searchable **SNI Menus** preserve source order when search text is blank.
- [ ] Activating a searched **SNI Menu** result still produces a generic selected entry interpreted as DBusMenu item activation.
- [ ] Tests cover config parsing/validation, default disabled behavior, visible search input, search results, nested breadcrumb results, and activation.

## Blocked by

- .scratch/niribar/foreground-selectable-menu-session/issues/08-use-selectable-menu-session-for-sni-menus-without-search.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/app/foreground.rs; crates/niribar/src/app/selectable_menu_session.rs; src/app/panel_runtime.rs. Evidence: Foreground, menu-session and panel tests; repeated D-Bus menu signal regression.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
