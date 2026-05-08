Status: ready-for-agent

# Migrate Bar Zones to Start/Center/End

## What to build

Update Niribar so public Bar module placement uses the logical **Zone** names `start`, `center`, and `end` throughout configuration, defaults, parsing, diagnostics, tests, and examples. The old physical zone names `top` and `bottom` should not remain accepted aliases because Niribar has not been released yet.

## Acceptance criteria

- [ ] Bar module placement config accepts `start`, `center`, and `end`.
- [ ] Bar module placement config rejects legacy `top` and `bottom` zone names with actionable configuration errors.
- [ ] Public default config and examples use `start`, `center`, and `end`.
- [ ] Duplicate placement of the same **Module** within one **Bar** is a configuration error.
- [ ] Tests cover vertical and horizontal Bars so `start` and `end` map correctly for both orientations.

## Blocked by

None - can start immediately
