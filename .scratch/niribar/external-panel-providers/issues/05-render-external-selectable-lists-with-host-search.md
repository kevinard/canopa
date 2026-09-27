Status: ready-for-human

# Render External Selectable Lists With Host Search

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Add **External Selectable List** as a provider content primitive. Providers supply list entries and metadata; Niribar owns list rendering, focus, scrolling, local **Selectable Search**, disabled-row behavior, and activation events.

This slice validates the reusable list/search behavior needed by the future Wi-Fi provider and likely audio provider.

## User stories covered

8, 23, 24, 40

## Acceptance criteria

- [ ] Provider content can include an **External Selectable List** with provider-supplied entries.
- [ ] Niribar renders list rows using host-owned styling and focus/highlight behavior.
- [ ] Search input is visible while the list view is active.
- [ ] Niribar owns local **Selectable Search** state and matching for the list.
- [ ] Keyboard and pointer activation send typed provider events addressed by opaque element or entry IDs.
- [ ] Disabled entries are visible but cannot be activated.
- [ ] Active, status, or error metadata can be represented in rows without making Niribar interpret provider domain meaning.
- [ ] Tests cover list rendering, host-owned search, focus movement, disabled rows, and activation events.

## Blocked by

- [03 - Send Typed Button Events And Apply Full Content Replacement](03-send-typed-button-events-and-apply-full-content-replacement.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/external_panel.rs; crates/niribar/src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
