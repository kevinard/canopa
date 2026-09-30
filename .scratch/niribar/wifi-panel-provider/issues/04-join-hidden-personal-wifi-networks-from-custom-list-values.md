Status: ready-for-human

# Join Hidden Personal Wi-Fi Networks From Custom List Values

## What to build

Use **External Custom List Values** in the Wi-Fi network list to start the **Hidden Wi-Fi Network** join flow. A typed non-blank network name that does not exactly match a discovered row label should lead to a **Network Credential Prompt** for a personal secured hidden network and then connect/save through iwd.

This slice covers hidden personal networks while keeping enterprise/EAP setup out of V1.

## User stories covered

8, 9, 10, 25

## Acceptance criteria

- [ ] The Wi-Fi network list allows **External Custom List Values**.
- [ ] Non-blank typed values without a case-sensitive exact row-label match expose a hidden-network join affordance.
- [ ] Blank or whitespace-only values do not expose or submit a hidden-network join affordance.
- [ ] Submitted hidden-network names are trimmed before the join flow starts.
- [ ] A typed value that exactly matches a discovered row label activates the discovered **Wi-Fi Network** path instead of the hidden-network path.
- [ ] The hidden-network flow supports personal secured networks through a **Network Credential Prompt**.
- [ ] Enterprise/EAP hidden-network setup is shown as unsupported or remains unavailable in V1.
- [ ] Successful hidden-network joins save known-network material through iwd when supported.
- [ ] Tests cover exact matching, blank values, trimmed values, prompt flow, unsupported enterprise/EAP handling, and saved hidden joins.

## Blocked by

- [03 - Collect And Save Personal Wi-Fi Credentials Through Iwd](03-collect-and-save-personal-wifi-credentials-through-iwd.md)
- [External provider issue 12 - Support External Custom List Values](../../external-panel-providers/issues/12-support-external-custom-list-values.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/wifi_provider.rs; crates/niribar/src/wifi_provider/backends.rs; src/wifi_provider/session.rs. Evidence: crates/niribar/tests/wifi_provider.rs plus isolated D-Bus adapter and delayed-session tests; hardware validation remains pending.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
