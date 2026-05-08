Status: ready-for-agent

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

- .scratch/domain-alignment/issues/04-make-bar-and-module-identity-strict.md
