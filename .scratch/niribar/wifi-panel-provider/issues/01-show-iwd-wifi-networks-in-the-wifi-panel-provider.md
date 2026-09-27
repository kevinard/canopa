Status: ready-for-human

# Show Iwd Wi-Fi Networks In The Wi-Fi Panel Provider

## What to build

Create the shipped `niribar-wifi-provider` binary and use it to show iwd-backed **Wi-Fi Networks** as semantic **External Panel Content**. Opening the built-in `wifi` provider should complete the handshake promptly, render initial scanning or active-network content, scan through iwd, and stream full content replacements as visible Wi-Fi state changes.

This slice proves the first real **Wi-Fi Panel Provider** end to end without implementing connection attempts yet.

## User stories covered

1, 2, 8, 10, 12, 20, 42

## Acceptance criteria

- [ ] A shipped `niribar-wifi-provider` binary starts as an **External Panel Provider** using the shared protocol code.
- [ ] Opening built-in provider name `wifi` can launch the shipped binary through the provider platform.
- [ ] The provider completes handshake promptly without waiting for scan completion.
- [ ] Initial content can show scanning status and the **Active Wi-Fi Network** when iwd reports one.
- [ ] The provider selects one Wi-Fi device automatically, preferring the active connected device and then an available managed Wi-Fi device.
- [ ] The provider starts one **Wi-Fi Scan** when opened and exposes a manual refresh action.
- [ ] Scan results render as one row per **Wi-Fi Network**, collapsing multiple physical access points into a single user-facing choice.
- [ ] Wi-Fi rows expose semantic metadata for active, known, secured/open, signal strength, connecting, and failed state without provider-owned styling.
- [ ] Rows are ordered with active first, known next, stronger signal next, and stable label ordering as the final tie-breaker.
- [ ] Provider tests use backend trait fakes and do not require a real iwd service.

## Blocked by

- [External provider issue 10 - Share Provider Protocol Across Niribar And Provider Binaries](../../external-panel-providers/issues/10-share-provider-protocol-across-niribar-and-provider-binaries.md)
- [External provider issue 11 - Stream Provider Content Without Request Response Coupling](../../external-panel-providers/issues/11-stream-provider-content-without-request-response-coupling.md)
- [External provider issue 12 - Support External Custom List Values](../../external-panel-providers/issues/12-support-external-custom-list-values.md)
- [External provider issue 13 - Resolve Built-In Panel Provider Names](../../external-panel-providers/issues/13-resolve-built-in-panel-provider-names.md)
- [External provider issue 14 - Close Provider Sessions Gracefully And Capture Diagnostics](../../external-panel-providers/issues/14-close-provider-sessions-gracefully-and-capture-diagnostics.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/wifi_provider.rs; crates/niribar/src/wifi_provider/backends.rs; src/wifi_provider/session.rs. Evidence: crates/niribar/tests/wifi_provider.rs plus isolated D-Bus adapter and delayed-session tests; hardware validation remains pending.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
