Status: ready-for-human

# Implement Exact Output Target Lists

## What to build

Implement **Output Target** list semantics for Bars. A Bar may use one exact include list or one exact exclude list, with exact case-sensitive Output name matching. Global **Excluded Outputs** remain a plain exact-name eligibility filter and take precedence over Bar Output Targets.

## Acceptance criteria

- [ ] `output "DP-1" "HDMI-A-1"` renders the Bar only on listed eligible Outputs.
- [ ] `output "!eDP-1" "!HDMI-A-1"` renders the Bar on every eligible Output except listed Outputs.
- [ ] Mixed positive and negative entries in one Output Target are configuration errors.
- [ ] Duplicate entries in one Output Target are configuration errors.
- [ ] Output Targets and global Excluded Outputs match exact names case-sensitively.
- [ ] Output Target names and Excluded Output names that are not currently connected are valid.
- [ ] Global Excluded Outputs take precedence over Output Targets.

## Blocked by

- .scratch/domain-alignment/issues/02-replace-per-output-bar-overrides-with-output-targets.md

## Implementation review — 2026-09-23

Implementation is present in src/config/validate.rs; src/app/surfaces.rs; src/app/niri.rs. Evidence: configuration, output matching, per-surface and Niri tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../docs/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
