Status: ready-for-human

# Render External Text Input With Secret Mode

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Add **External Text Input** as a provider content primitive. Providers supply text input state and metadata; Niribar renders the input, supports optional secret mode, and sends value-change and submit events back to the provider.

This slice gives the future Wi-Fi provider the primitive needed for **Network Credential Prompts**.

## User stories covered

9, 25, 40

## Acceptance criteria

- [ ] Provider content can include an **External Text Input** with an opaque **External Panel Element ID**.
- [ ] Niribar renders normal text input and secret text input modes.
- [ ] Secret mode does not render the entered value as plain visible text.
- [ ] Value changes send typed provider events with the element ID and current value.
- [ ] Submit behavior sends a typed provider event to the provider.
- [ ] Provider responses can replace the content model after input changes or submit.
- [ ] Validation or error text supplied by the provider can be rendered near the input.
- [ ] Tests cover normal input, secret input, value-change events, submit events, and replacement after provider response.

## Blocked by

- [03 - Send Typed Button Events And Apply Full Content Replacement](03-send-typed-button-events-and-apply-full-content-replacement.md)

## Implementation review — 2026-09-23

Implementation is present in src/external_panel.rs; src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
