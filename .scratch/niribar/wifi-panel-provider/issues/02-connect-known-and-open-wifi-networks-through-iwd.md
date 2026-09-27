Status: ready-for-human

# Connect Known And Open Wi-Fi Networks Through Iwd

## What to build

Extend the iwd-backed **Wi-Fi Panel Provider** so activating a **Known Wi-Fi Network** or open **Wi-Fi Network** starts a **Wi-Fi Connection Attempt**. The panel should stay open, prevent duplicate attempts, and stream connecting, active, failed, or no-control state through full content replacements.

This slice makes the provider useful for credential-free joins while keeping credential prompts out of scope.

## User stories covered

2, 5, 8, 10

## Acceptance criteria

- [ ] Activating a **Known Wi-Fi Network** row starts a connection attempt through iwd.
- [ ] Activating an open **Wi-Fi Network** row starts a connection attempt through iwd without a **Network Credential Prompt**.
- [ ] The provider keeps the hosting **Panel** open after a connection attempt starts or succeeds.
- [ ] The selected row reports connecting, active, or failed state through full content replacements.
- [ ] Repeated row activation cannot start duplicate concurrent connection attempts for the same target.
- [ ] Manual refresh remains available only when iwd can tolerate concurrent scan and connect operations; otherwise it is disabled during the attempt.
- [ ] Backend errors are mapped into user-facing categories rather than shown as raw D-Bus errors.
- [ ] Provider diagnostics do not include **Network Credentials** and avoid SSIDs at normal log levels.
- [ ] Tests use backend trait fakes for success, failure, duplicate activation, and refresh-during-connect behavior.

## Blocked by

- [01 - Show Iwd Wi-Fi Networks In The Wi-Fi Panel Provider](01-show-iwd-wifi-networks-in-the-wifi-panel-provider.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/wifi_provider.rs; crates/niribar/src/wifi_provider/backends.rs; src/wifi_provider/session.rs. Evidence: crates/niribar/tests/wifi_provider.rs plus isolated D-Bus adapter and delayed-session tests; hardware validation remains pending.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
