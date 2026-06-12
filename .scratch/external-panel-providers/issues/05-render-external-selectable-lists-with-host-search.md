Status: ready-for-agent

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
