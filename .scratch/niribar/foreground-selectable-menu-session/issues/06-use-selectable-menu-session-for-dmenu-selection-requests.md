Status: ready-for-human

# Use Selectable Menu Session For Dmenu Selection Requests

## Parent

.scratch/niribar/foreground-selectable-menu-session/PRD.md

## What to build

Migrate **Dmenu Selection Requests** to use **Selectable Menu Session** for search, focus, activation, and **Custom Menu Value** behavior. The session should return generic selected entries or **Custom Menu Values**, and the owning foreground adapter should translate those into **Dmenu Selection Request** IPC responses.

Existing **Dmenu Selection Mode** behavior should remain compatible, including default custom values and `--no-custom`.

## Acceptance criteria

- [ ] **Dmenu Selection Requests** open a **Selectable Menu Session** with **Menu Search** enabled.
- [ ] Plain and rich dmenu entries appear as selectable session entries in input order when search is blank.
- [ ] Rich dmenu labels, icons, and keywords continue to affect display and search as before.
- [ ] A focused matching dmenu entry activates as a generic selected entry.
- [ ] A **Custom Menu Value** appears after fuzzy matches or first when there are no matches when custom values are enabled.
- [ ] No **Custom Menu Value** appears when custom values are disabled.
- [ ] The owning foreground adapter translates generic selected entries and custom values into the correct IPC response.
- [ ] Cancellation behavior remains distinct from selected values and request failures.
- [ ] Tests cover plain entries, rich entries, search, custom values, `--no-custom`, activation, and cancellation through the new session seam.

## Blocked by

- .scratch/niribar/foreground-selectable-menu-session/issues/05-use-selectable-menu-session-for-configured-panel-menus.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/app/foreground.rs; crates/niribar/src/app/selectable_menu_session.rs; src/app/panel_runtime.rs. Evidence: Foreground, menu-session and panel tests; repeated D-Bus menu signal regression.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
