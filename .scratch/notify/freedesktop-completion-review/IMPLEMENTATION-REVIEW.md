# Implementation tranche review

Baseline: Notify `d3f12df1888acbeb1cf05fd81baa00fb500e6a89`; Niribar `d7870c2d20cff5d0f84da03296e15bf65115cc90`.

The implementation skill required independent Standards and Spec reviews. These findings concern the current tranche, not completion of all packages in PLAN.md.

## Standards

- Successful host images also displayed their alternative text, contrary to CONTEXT.md. Corrected: alternative text remains only when an image cannot render.
- Token acquisition preceded action validation. Corrected with non-mutating lifecycle validation, bounded token acquisition, then revision revalidation before commitment. The rejected-action regression failed before the correction and now passes.
- Socket controls discarded the committed-clear count promised by ADR 0030. Corrected with `ClearPartiallyApplied { dismissed }`; CLI exits unsuccessfully with the count and legacy panel commands receive a sanitized failure. The actual socket regression failed before the correction and now passes.
- Heuristic: host action registration and rendering both construct control-option actions. This is a maintainability observation, not a documented-rule violation; further normalization remains useful.

## Spec

- W11 explicitly requires interleaved images without duplicated alternative text. Corrected as above.
- W06 requires defined partial success. The operation's committed count now survives the control adapter, verified through the control socket with an injected storage failure.

Summary: Standards identified three documented mismatches and one heuristic; Spec identified two implementation mismatches. The most consequential findings in each axis concern validation/publication semantics and loss of partial results. No unrequested scope expansion was identified. Graphical verification and the remaining completion packages are not claimed complete.

Follow-up: the Standards reviewer independently verified all three documented corrections in the actual Notify and Niribar worktrees and found no remaining defect in those fixes. Final suites passed 236 Notify tests and 1,141 Niribar tests. The duplication heuristic remains tracked.
