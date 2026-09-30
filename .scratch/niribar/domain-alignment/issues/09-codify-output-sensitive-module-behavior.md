Status: ready-for-human

# Codify Output-Sensitive Module Behavior

## What to build

Align output-sensitive Modules with the accepted Bar Surface context model. Global data sources may share runtime state by Module Name, while output-sensitive rendering and Module Actions use the originating Bar Surface context.

## Acceptance criteria

- [ ] A Module reused across Bars shares runtime state by Module Name when its data source is global.
- [ ] Separately named Modules of the same Module Kind do not share runtime state.
- [ ] Output-sensitive Module rendering uses the target Bar Surface Output.
- [ ] Output-sensitive Module Actions use the originating Bar Surface context.
- [ ] Workspace-capable Modules render Workspaces relevant to the Bar Surface Output by default.
- [ ] Focused-window Module shows the Focused Window on the focused Output and Active Window for non-focused Outputs.
- [ ] Tests cover reused global modules, separately named same-kind modules, workspace output filtering, and focused-window focused/non-focused Output behavior.

## Blocked by

- .scratch/niribar/domain-alignment/issues/04-make-bar-and-module-identity-strict.md

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/config/validate.rs; crates/niribar/src/app/surfaces.rs; src/app/niri.rs. Evidence: configuration, output matching, per-surface and Niri tests.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
