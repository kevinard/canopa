Status: ready-for-human

# Support External Custom List Values

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Extend **External Selectable Lists** so a provider can explicitly allow an **External Custom List Value**. When enabled, Niribar renders a distinct custom-value affordance for non-blank search text that has no exact entry-label match, and activation sends a typed custom-value event to the provider.

This slice enables provider-owned flows such as joining a **Hidden Wi-Fi Network** without treating typed text as if it were a provider-supplied entry.

## User stories covered

8, 22, 23, 24, 40

## Acceptance criteria

- [ ] Provider content can mark an **External Selectable List** as allowing custom values.
- [ ] Lists that do not allow custom values keep existing search and activation behavior.
- [ ] When custom values are allowed, non-blank search text without an exact entry-label match renders a distinct custom-value affordance.
- [ ] Exact entry-label matching for custom-value suppression is case-sensitive.
- [ ] Blank or whitespace-only search text never produces a custom-value affordance.
- [ ] Activating the custom-value affordance sends a typed provider event with the list element ID and submitted value.
- [ ] Normal entry activation still sends provider-supplied entry IDs and does not use the custom-value event.
- [ ] Tests cover allowed and disallowed lists, exact matches, blank values, value trimming, and event serialization.

## Blocked by

- [10 - Share Provider Protocol Across Niribar And Provider Binaries](10-share-provider-protocol-across-niribar-and-provider-binaries.md)
- [05 - Render External Selectable Lists With Host Search](05-render-external-selectable-lists-with-host-search.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/external_panel.rs; crates/niribar/src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
