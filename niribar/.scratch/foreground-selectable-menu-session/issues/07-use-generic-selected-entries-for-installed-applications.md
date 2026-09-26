Status: ready-for-human

# Use Generic Selected Entries For Installed Applications

## Parent

.scratch/foreground-selectable-menu-session/PRD.md

## What to build

Migrate installed-application **Panel Menu Sources** to use **Selectable Menu Session** generic selected entries. The session should continue to provide searchable installed application entries, and the owning foreground adapter should interpret selected application identifiers as installed application launch requests.

Existing installed-application menu behavior, icon behavior, and search behavior should remain compatible.

## Acceptance criteria

- [ ] Installed-application **Panel Menu Sources** open a **Selectable Menu Session** with **Menu Search** enabled.
- [ ] Installed applications remain searchable by name, description, generic name, and keywords.
- [ ] Installed application icon visibility and missing-icon behavior remain compatible.
- [ ] Activating an installed application returns a generic selected entry from the session.
- [ ] The owning foreground adapter interprets the generic selected entry as an installed application launch request.
- [ ] Activation closes the active **Panel** as before.
- [ ] Tests cover search fields, icon visibility, generic selected-entry interpretation, launch request behavior, and close-on-activation.

## Blocked by

- .scratch/foreground-selectable-menu-session/issues/05-use-selectable-menu-session-for-configured-panel-menus.md

## Implementation review — 2026-09-23

Implementation is present in src/app/foreground.rs; src/app/selectable_menu_session.rs; src/app/panel_runtime.rs. Evidence: Foreground, menu-session and panel tests; repeated D-Bus menu signal regression.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
