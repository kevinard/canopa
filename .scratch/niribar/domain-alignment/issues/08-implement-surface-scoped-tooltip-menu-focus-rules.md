Status: ready-for-human

# Implement Surface-Scoped Tooltip Menu Focus Rules

## What to build

Implement the accepted popup and keyboard focus model for visible Module targets and Dynamic Module children. Tooltips are scoped to a Bar Surface, Menus are single globally, and Keyboard Focus moves into and out of Menus predictably.

## Acceptance criteria

- [ ] A Bar Surface shows at most one Tooltip at a time.
- [ ] Niribar shows at most one Menu at a time globally.
- [ ] Opening a Menu closes open Tooltips on the same Bar Surface.
- [ ] Tooltips and Menus close when their originating visible target disappears.
- [ ] Tooltips and Menus close when their owning Bar Surface changes Expansion state.
- [ ] Keyboard Focus enters a Menu while it is open and returns to the originating target when the Menu closes if that target still exists.
- [ ] Targets exposing a Tooltip or Menu participate in Keyboard Focus navigation.
- [ ] A Bar Surface with no visible actionable or inspectable targets has no Keyboard Focus target.

## Blocked by

- .scratch/niribar/domain-alignment/issues/07-align-empty-and-unavailable-module-semantics.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/config/validate.rs; crates/niribar/src/app/surfaces.rs; src/app/niri.rs. Evidence: configuration, output matching, per-surface and Niri tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/niribar/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
