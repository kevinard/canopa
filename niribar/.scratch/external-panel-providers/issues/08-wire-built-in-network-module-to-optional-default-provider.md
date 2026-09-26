Status: ready-for-human

# Wire Built-In Network Module To Optional Default Provider

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Connect the existing network **Module** to the provider platform without moving network status out of core. The network **Module** remains passive status by default, but its built-in primary activation opens the default or overridden Wi-Fi provider when available and when no configured click action overrides it.

This slice proves how built-in modules can launch optional external providers while preserving user configuration.

## User stories covered

2, 3, 4, 35, 36, 37, 38, 42

## Acceptance criteria

- [ ] The network **Module** continues to render passive status from the existing status integration.
- [ ] The network **Module** can advertise built-in primary activation for opening its default provider.
- [ ] A configured click **Module Action** takes precedence over built-in provider activation.
- [ ] User configuration can override the network module's default provider.
- [ ] User configuration can disable the network module's default provider.
- [ ] A missing optional built-in provider does not make the network **Module** unavailable at startup or in normal status rendering.
- [ ] Explicitly opening a missing configured provider surfaces provider failure content.
- [ ] Tests cover default activation, configured-action override, provider override, provider disable, missing optional provider behavior, and unchanged status rendering.

## Blocked by

- [01 - Open Provider Action Shows Provider Failure Panel](01-open-provider-action-shows-provider-failure-panel.md)
- [04 - Own External Panel Session Lifecycle In Foreground](04-own-external-panel-session-lifecycle-in-foreground.md)

## Implementation review — 2026-09-23

Implementation is present in src/external_panel.rs; src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
