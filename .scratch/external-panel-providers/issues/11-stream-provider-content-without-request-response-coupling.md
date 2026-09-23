Status: ready-for-human

# Stream Provider Content Without Request Response Coupling

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Allow an active **External Panel Provider** to send full **External Panel Content** replacements whenever provider-owned state changes, not only as an immediate response to a host event. Host interaction events become fire-and-forget; transport or protocol failure still fails the active **External Panel Session**.

This slice supports providers that subscribe to backend state, such as the future **Wi-Fi Panel Provider** updating scan results or connection state while the **Panel** remains open.

## User stories covered

18, 22, 27, 29, 30

## Acceptance criteria

- [ ] After a successful handshake, Niribar continuously accepts provider `content` messages for the active session.
- [ ] A valid unsolicited content replacement updates the visible provider panel.
- [ ] Host interaction events are written to the provider without requiring an immediate content response.
- [ ] Invalid post-handshake provider output fails the active session and renders provider failure content.
- [ ] Stale provider output from an ended or replaced session is ignored.
- [ ] Tests cover unsolicited content updates, fire-and-forget host events, invalid streamed content, and stale streamed content.

## Blocked by

- [10 - Share Provider Protocol Across Niribar And Provider Binaries](10-share-provider-protocol-across-niribar-and-provider-binaries.md)
- [03 - Send Typed Button Events And Apply Full Content Replacement](03-send-typed-button-events-and-apply-full-content-replacement.md)
- [04 - Own External Panel Session Lifecycle In Foreground](04-own-external-panel-session-lifecycle-in-foreground.md)
- [07 - Harden Provider Protocol And Content Validation](07-harden-provider-protocol-and-content-validation.md)

## Implementation review — 2026-09-23

Implementation is present in src/external_panel.rs; src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
