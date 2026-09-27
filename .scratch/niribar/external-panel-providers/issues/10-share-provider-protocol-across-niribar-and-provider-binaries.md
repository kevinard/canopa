Status: ready-for-human

# Share Provider Protocol Across Niribar And Provider Binaries

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Make the provider protocol and **External Panel Content** model available to both Niribar and shipped provider binaries from shared library code. A provider binary should be able to encode/decode the same framed JSON messages and validate the same content model as the host without duplicating protocol types.

This slice proves that future shipped providers can use the real external-provider contract instead of carrying a second, drifting implementation.

## User stories covered

12, 16, 17, 19, 28

## Acceptance criteria

- [ ] Niribar and a provider-side test binary can use the same provider protocol message definitions.
- [ ] Frame encoding and decoding behavior is shared between host and provider-side code.
- [ ] **External Panel Content** definitions and validation behavior are shared rather than duplicated.
- [ ] Existing host-side provider protocol tests still pass through the shared code path.
- [ ] A provider-side test can perform a compatible handshake using the shared protocol code.
- [ ] The shared protocol surface does not expose app-only **Panel Runtime** or **Foreground** internals to provider binaries.

## Blocked by

- [02 - Render Static External Panel Content After Handshake](02-render-static-external-panel-content-after-handshake.md)
- [07 - Harden Provider Protocol And Content Validation](07-harden-provider-protocol-and-content-validation.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/external_panel.rs; crates/niribar/src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
