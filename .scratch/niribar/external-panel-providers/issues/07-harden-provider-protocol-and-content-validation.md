Status: ready-for-human

# Harden Provider Protocol And Content Validation

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Harden the provider platform against invalid provider behavior. Malformed frames, incompatible versions, invalid content models, duplicate ambiguous element IDs, oversized frames, and unexpected provider exits should produce visible provider failure content and tracing diagnostics.

This slice makes the platform reliable enough for external domain providers.

## User stories covered

17, 19, 27, 28

## Acceptance criteria

- [ ] Invalid JSON frames are rejected and surfaced as provider failure content.
- [ ] Oversized frames are rejected before content is applied.
- [ ] Incompatible protocol versions fail during handshake with visible provider failure content.
- [ ] Content models are validated before rendering.
- [ ] Duplicate **External Panel Element IDs** are rejected when they would make events ambiguous.
- [ ] Invalid primitive combinations and invalid list/input/button definitions are rejected.
- [ ] Unexpected provider exit while the panel is active renders provider failure content or preserves an already-visible failure state.
- [ ] Failures include tracing diagnostics that identify the provider/session without leaking secret input values.
- [ ] Tests cover invalid frames, invalid versions, malformed content, duplicate IDs, provider exit, and diagnostic-safe failure behavior.

## Blocked by

- [02 - Render Static External Panel Content After Handshake](02-render-static-external-panel-content-after-handshake.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/external_panel.rs; crates/niribar/src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
