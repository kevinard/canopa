Status: ready-for-human

# Render Static External Panel Content After Handshake

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Launch a provider process, perform a framed JSON protocol-version handshake over standard input and standard output, accept the provider's first complete **External Panel Content** model, and render simple static content in a **Panel**.

This slice should use a fake provider in tests so the provider platform can be verified without a real Wi-Fi or audio provider.

## User stories covered

12, 16, 17, 18, 19, 20, 27, 28

## Acceptance criteria

- [ ] Niribar can launch a provider process and communicate over framed JSON on standard input and standard output.
- [ ] An **External Panel Session** begins with a required protocol-version handshake.
- [ ] A compatible provider handshake can return an initial full **External Panel Content** model.
- [ ] Niribar validates and renders a simple static provider content model using host-owned panel styling.
- [ ] A successful static provider panel is visible through the normal **Panel** rendering path.
- [ ] A handshake timeout or incompatible protocol version renders visible provider failure content.
- [ ] Tests exercise the protocol and provider session through a fake provider or provider process adapter seam.

## Blocked by

- [01 - Open Provider Action Shows Provider Failure Panel](01-open-provider-action-shows-provider-failure-panel.md)

## Implementation review — 2026-09-23

Implementation is present in src/external_panel.rs; src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
