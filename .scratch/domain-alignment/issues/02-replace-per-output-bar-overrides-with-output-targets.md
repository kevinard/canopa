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
