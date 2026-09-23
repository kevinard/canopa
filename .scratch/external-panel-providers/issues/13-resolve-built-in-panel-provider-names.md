Status: ready-for-human

# Resolve Built-In Panel Provider Names

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Add global built-in **Panel Provider Name** resolution for shipped providers, starting with `wifi`. Explicit `open-provider "wifi"` and the network module's default provider activation should resolve to the built-in provider when no user-declared provider overrides that name.

This slice makes built-in providers zero-config while preserving user configuration precedence and visible failure behavior when a shipped provider binary is missing.

## User stories covered

2, 3, 4, 35, 36, 37, 38, 39

## Acceptance criteria

- [ ] Configuration validation accepts provider references that resolve to either user-declared or built-in provider names.
- [ ] Unknown provider names that are neither declared nor built-in remain configuration errors.
- [ ] User-declared providers take precedence over built-in providers with the same **Panel Provider Name**.
- [ ] `open-provider "wifi"` can resolve to the built-in Wi-Fi provider from any **Panel Action**.
- [ ] The network module's omitted provider and `provider "wifi"` default to the built-in Wi-Fi provider unless overridden by a user declaration.
- [ ] `provider #false` still disables the network module's default provider activation.
- [ ] Launching a missing or unlaunchable built-in provider renders provider failure content in the hosting **Panel**.
- [ ] Built-in provider launch looks for a sibling provider binary first, then falls back to resolving it from `PATH`.

## Blocked by

- [10 - Share Provider Protocol Across Niribar And Provider Binaries](10-share-provider-protocol-across-niribar-and-provider-binaries.md)
- [01 - Open Provider Action Shows Provider Failure Panel](01-open-provider-action-shows-provider-failure-panel.md)
- [08 - Wire Built-In Network Module To Optional Default Provider](08-wire-built-in-network-module-to-optional-default-provider.md)

## Implementation review — 2026-09-23

Implementation is present in src/external_panel.rs; src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
