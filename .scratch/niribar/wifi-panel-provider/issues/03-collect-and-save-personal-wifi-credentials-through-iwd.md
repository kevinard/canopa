Status: ready-for-human

# Collect And Save Personal Wi-Fi Credentials Through Iwd

## What to build

Support joining secured **Personal Wi-Fi Networks** that are not yet **Known Wi-Fi Networks**. Selecting an unknown secured network should replace the list with a **Network Credential Prompt**, submit the secret credential to the provider, save the resulting known-network material through iwd, and keep the panel open with clear state updates.

This slice completes the common “join a new secured network” path through the external provider flow.

## User stories covered

2, 5, 9, 10, 25

## Acceptance criteria

- [ ] Selecting a secured **Wi-Fi Network** without saved material shows a **Network Credential Prompt**.
- [ ] The prompt uses secret **External Text Input** and does not render the credential as plain visible text.
- [ ] Submitting the credential starts a connection attempt through iwd.
- [ ] Submitted credentials are saved through iwd's normal known-network mechanism when the join succeeds.
- [ ] A saved network is treated as a **Known Wi-Fi Network** on later provider opens.
- [ ] A rejected credential keeps the prompt visible, shows an inline error, preserves selected network context, and clears the credential field.
- [ ] Repeated submit cannot start duplicate concurrent connection attempts.
- [ ] Provider diagnostics, stderr handling, and panel-visible errors never include the entered **Network Credential**.
- [ ] Tests cover prompt rendering, secret input events, successful saved join, credential rejection retry, and diagnostic-safe failure behavior.

## Blocked by

- [02 - Connect Known And Open Wi-Fi Networks Through Iwd](02-connect-known-and-open-wifi-networks-through-iwd.md)
- [External provider issue 06 - Render External Text Input With Secret Mode](../../external-panel-providers/issues/06-render-external-text-input-with-secret-mode.md)

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/wifi_provider.rs; crates/niribar/src/wifi_provider/backends.rs; src/wifi_provider/session.rs. Evidence: crates/niribar/tests/wifi_provider.rs plus isolated D-Bus adapter and delayed-session tests; hardware validation remains pending.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
