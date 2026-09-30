Status: ready-for-human

# Replace Per-Output Bar Overrides with Output Targets

## What to build

Remove per-output **Bar** override syntax and behavior. Output-specific behavior should be expressed by defining another named **Bar** with an **Output Target**, matching ADR-0001.

## Acceptance criteria

- [ ] Per-output Bar override config syntax is no longer accepted.
- [ ] Runtime behavior no longer mutates a Bar per Output.
- [ ] Public examples model output-specific behavior as separate named Bars with Output Targets.
- [ ] Configuration errors guide users toward named Bars with Output Targets when old override syntax is encountered.
- [ ] Tests prove one Output can receive multiple Bar Surfaces from multiple matching Bars.

## Blocked by

None - can start immediately

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/config/validate.rs; crates/niribar/src/app/surfaces.rs; src/app/niri.rs. Evidence: configuration, output matching, per-surface and Niri tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
