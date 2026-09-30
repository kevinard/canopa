Status: ready-for-human

# Align Empty and Unavailable Module Semantics

## What to build

Align module rendering and interaction behavior with the accepted distinction between **Empty Module**, **Unavailable Module**, and **Configuration Error**. Empty and unavailable modules render no visible content, leave no gap, have no Tooltip, and are skipped by Keyboard Focus navigation.

## Acceptance criteria

- [ ] A valid Module with a missing runtime dependency becomes an Unavailable Module, not a Configuration Error.
- [ ] A Module with an available data source but no current content becomes an Empty Module.
- [ ] Empty and unavailable Modules render no visible content and leave no gap.
- [ ] Empty and unavailable Modules have no Tooltip and are skipped by Keyboard Focus navigation.
- [ ] Script command failure makes the script unavailable for that refresh rather than keeping stale output.
- [ ] Script command success with empty rendered output becomes an Empty Module.
- [ ] A valid button remains visible; command failure after activation is a failed Module Action, not an Unavailable Module.
- [ ] Tests cover focused-window no-window, tray with zero icons, script empty output, script failure, and missing runtime dependency cases.

## Blocked by

- .scratch/niribar/domain-alignment/issues/04-make-bar-and-module-identity-strict.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/config/validate.rs; crates/niribar/src/app/surfaces.rs; src/app/niri.rs. Evidence: configuration, output matching, per-surface and Niri tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.

### Desktop verification follow-up — 2026-09-23

The private compositor check exposed blank panel frames and invisible navigation
targets despite the existing state transitions. A shared module content policy
now filters bar/panel frames, navigation and tooltips, including output-specific
workspace children and empty script output. Data-source loss retires stale focus
and tooltip state; recovery restores the visible target. App-level regressions
cover omission and loss/recovery. See the
[desktop record](../../../../docs/shell/audits/2026-09-23-desktop-validation.md).
Operator acceptance remains open.
