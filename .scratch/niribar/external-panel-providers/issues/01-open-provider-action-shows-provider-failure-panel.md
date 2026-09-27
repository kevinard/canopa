Status: ready-for-human

# Open Provider Action Shows Provider Failure Panel

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Add the first vertical path for **Configured Panel Providers**: configuration can declare a named provider, **Panel Actions** can open that provider, and opening a provider that cannot launch renders provider failure content in a **Panel** instead of silently doing nothing.

This slice proves the host-side lifecycle without needing a working provider protocol yet. A missing or unlaunchable explicitly configured provider should be visible when the user opens it.

## User stories covered

5, 27, 33, 34, 39

## Acceptance criteria

- [ ] Configuration supports globally named **Configured Panel Providers** with provider commands using the existing **Configured Command** grammar.
- [ ] Configuration supports a provider-opening **Panel Action** that references a **Panel Provider Name**.
- [ ] Invalid provider references are reported as configuration errors.
- [ ] Opening a configured provider routes through the normal **Panel Action** and **Foreground** path.
- [ ] When the provider command cannot launch, Niribar opens a **Panel** containing visible provider failure content.
- [ ] Provider launch failure is logged through tracing with enough context to diagnose the configured provider.
- [ ] Existing `open-panel`, `toggle-panel`, `open-menu`, `toggle-menu`, and `command` actions keep their current behavior.

## Blocked by

None - can start immediately

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/external_panel.rs; crates/niribar/src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
