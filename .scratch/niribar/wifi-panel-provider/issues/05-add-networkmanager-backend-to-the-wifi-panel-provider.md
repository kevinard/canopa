Status: ready-for-human

# Add NetworkManager Backend To The Wi-Fi Panel Provider

## What to build

Add NetworkManager support behind the same backend-neutral **Wi-Fi Panel Provider** state machine used by the iwd path. At runtime, the provider should use NetworkManager when available, fall back to iwd when NetworkManager is unavailable, and render no-control status content when neither supported control backend is available.

This slice completes the documented backend policy while preserving the provider's semantic content model and backend-neutral user flow.

## User stories covered

2, 10, 42

## Acceptance criteria

- [ ] The provider detects NetworkManager availability when it starts.
- [ ] When NetworkManager is available, the provider uses NetworkManager for Wi-Fi scans, active network state, known/open joins, and personal secured joins.
- [ ] When NetworkManager is unavailable and iwd is available, the provider uses the existing iwd fallback path.
- [ ] When neither backend is available, the provider renders no-control status content instead of failing silently.
- [ ] The provider does not mix NetworkManager status/control with iwd control in one active session.
- [ ] The NetworkManager path uses the same provider-owned state machine and renders the same semantic **External Panel Content** concepts as the iwd path.
- [ ] Backend-specific errors are mapped into the same user-facing categories as the iwd path.
- [ ] Normal tests use backend trait fakes and do not require a real NetworkManager service.

## Blocked by

- [01 - Show Iwd Wi-Fi Networks In The Wi-Fi Panel Provider](01-show-iwd-wifi-networks-in-the-wifi-panel-provider.md)
- [02 - Connect Known And Open Wi-Fi Networks Through Iwd](02-connect-known-and-open-wifi-networks-through-iwd.md)
- [03 - Collect And Save Personal Wi-Fi Credentials Through Iwd](03-collect-and-save-personal-wifi-credentials-through-iwd.md)
- [04 - Join Hidden Personal Wi-Fi Networks From Custom List Values](04-join-hidden-personal-wifi-networks-from-custom-list-values.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/wifi_provider.rs; crates/niribar/src/wifi_provider/backends.rs; src/wifi_provider/session.rs. Evidence: crates/niribar/tests/wifi_provider.rs plus isolated D-Bus adapter and delayed-session tests; hardware validation remains pending.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
