Status: ready-for-human

# Document Provider Configuration And Protocol Contract

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Document the v1 provider platform for users and provider authors. The docs should explain configured providers, provider-opening actions, optional module defaults, stdio framed JSON, the handshake, full content replacement, supported primitives, typed events, and failure behavior.

This slice makes the platform understandable before domain-specific providers such as Wi-Fi and audio are built against it.

## User stories covered

13, 14, 16, 17, 20, 46

## Acceptance criteria

- [ ] User-facing configuration docs show how to declare a named **Configured Panel Provider**.
- [ ] User-facing configuration docs show how to open a provider from a **Panel Action**.
- [ ] Module docs explain default provider behavior, override behavior, and disable behavior.
- [ ] Protocol docs describe stdio framed JSON transport and the required handshake.
- [ ] Protocol docs describe full content replacement and the v1 primitive set.
- [ ] Protocol docs describe typed events and opaque **External Panel Element IDs**.
- [ ] Docs explain that v1 is panel-only and does not support provider-owned **Menus** or **Tooltips**.
- [ ] Docs explain provider failure behavior and diagnostic expectations.
- [ ] Existing default configuration comments remain accurate.

## Blocked by

- [02 - Render Static External Panel Content After Handshake](02-render-static-external-panel-content-after-handshake.md)
- [05 - Render External Selectable Lists With Host Search](05-render-external-selectable-lists-with-host-search.md)
- [06 - Render External Text Input With Secret Mode](06-render-external-text-input-with-secret-mode.md)
- [07 - Harden Provider Protocol And Content Validation](07-harden-provider-protocol-and-content-validation.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/external_panel.rs; crates/niribar/src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
