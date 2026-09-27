# Oversized compact-card implementation review

Fixed point: `df88ecda9f2170ff9a1d1e3f62531882a786761e` (user-selected `df88ecd`). Starting tranche HEAD: `8b752f3`. Reviewed `git diff df88ecd...HEAD`, the pending working-tree diff and new ADR/evidence. Commit list: `8be0e99 9289e34 4eaa158 cf449ee 9d62fcf 622201a c170b6d a572593 2b8a21b 24f66bb 9f9f129 6260b01 aae365b 77d3aaf 60d733f 6397b9e 1bf5698 7ce2a0f c681dfe 559cdfa 2394849 8b752f3`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewers inspected the current oversized-card tranche and relevant cumulative layout/acknowledgement interactions without edits or builds. Sources: `AGENTS.md`, tracker/domain instructions, `CONTEXT.md`, ADRs 0028/0033/0044/0045/0046 and completion-plan W10/W11. The Standards review also used the skill's complete Fowler smell baseline, with repo rules taking precedence. This does not recertify the entire cumulative plan.

## Standards

Initial low-severity finding: the new issue comment was prepended rather than appended as required by `docs/notify/agents/issue-tracker.md`. Moved it to the end of the existing history. Re-review found no outstanding documented-standard violations or actionable baseline smells; compact geometry remains local to presentation, with no speculative interface or runtime policy leak.

Standards: **0 outstanding findings; worst: none.**

## Spec

Initial P2 finding: capping every card at exactly viewport height left an oversized middle card only a one-pixel full-containment interval. Ordinary 60px wheel steps skipped it, leaving its timeout pending. The actual renderer/input regression reproduced acknowledgement of targets 1 and 3 but not 2. Multi-item cards now leave a 60px reveal interval on viewports at least 120px high; the new three-card test passes at 150/216/426px. ADR 0046 explicitly records the budget, tiny-output and large-jump limits. Re-review cleared the finding without weakening full-card containment or target ownership.

Spec: **0 outstanding scoped findings; worst: none.**

Both final evidence/document rechecks found zero new findings after inspecting all six captures and the relevant private signal/pointer logs. All 107 focused tests and **395 final private-bus workspace tests** pass, with zero failures or ignored tests. Workspace/all-targets typecheck and Clippy, formatting, whitespace and protocol-snapshot checks pass with existing warnings. The initial sandbox D-Bus bind restriction and successful authorized run are recorded in the [execution log](IMPLEMENTATION.md); see also [graphical evidence](OVERSIZED-CARD-EVIDENCE.md).

Summary: Standards — 0 outstanding findings, worst none; Spec — 0 outstanding scoped findings, worst none. W10/W11 remain incomplete.
