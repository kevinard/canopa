Status: ready-for-human

# Close Provider Sessions Gracefully And Capture Diagnostics

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Make **External Panel Session** shutdown observable and diagnosable. When a provider-backed **Panel** closes, Niribar should send a best-effort close event, give the provider a short bounded window to exit, then terminate it if needed. Niribar should also capture a bounded tail of provider stderr for diagnostics without displaying raw stderr in panel content by default.

This slice improves shipped provider operability without changing Niribar's ownership of **Panel** lifecycle.

## User stories covered

6, 27, 28, 29, 30

## Acceptance criteria

- [ ] Closing a provider-backed **Panel** sends a best-effort close event to the active provider.
- [ ] Niribar gives the provider a bounded grace period to exit after close.
- [ ] A provider that does not exit after the grace period is terminated and cleaned up.
- [ ] Closing the panel ends the **External Panel Session** but does not imply rollback of provider-owned backend work.
- [ ] Niribar captures a bounded tail of provider stderr for tracing diagnostics.
- [ ] Raw provider stderr is not shown verbatim in panel content by default.
- [ ] Diagnostic handling avoids logging secret **External Text Input** values or **Network Credentials**.
- [ ] Tests cover graceful close, forced termination, stderr capture bounds, and diagnostic-safe failure behavior.

## Blocked by

- [11 - Stream Provider Content Without Request Response Coupling](11-stream-provider-content-without-request-response-coupling.md)
- [04 - Own External Panel Session Lifecycle In Foreground](04-own-external-panel-session-lifecycle-in-foreground.md)

## Implementation review — 2026-09-23

Implementation is present in src/external_panel.rs; src/external_panel/protocol.rs; src/app/external_panel_session.rs. Evidence: provider protocol, transport and lifecycle tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
