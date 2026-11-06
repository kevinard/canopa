# Reviewed Move Undo projections retain their original history binding

Status: functional regressions and workspace history boundaries verified;
the separate 500-item Undo timing gate remains failing.

Portal delivery's existing workspace gate exposed two Files regressions. A live
same-filesystem Move review may restore only its saved previous destination after
the incoming item disappears. It may instead return only the incoming item while
an unavailable saved version remains pending. Both workflows already exist in the
public Undo model; private task admission rejected their reviewed representations.

Saved-return admission already projected a stored Copied or paired SourceRemoved
step to RetainedOnly. Add the corresponding Rename projection, using its original
current path, saved stage and incoming identity. Existing reviewed-version and
relocation checks continue to constrain the selected task. The independent
canonical hash still records the complete stored history, not the projected form.

Return-work admission must likewise recognize a live source-only Rename with no
selected retained version. Its missing saved version stays in original history
and is not erased or adopted by another operation. Normalize only this field in
the expected execution form, alongside the existing reviewed-version handling.
Continue hashing the complete stored step and exact selected pending task.

No schema or public API changes. Generic journal writes still cannot create a
binding. Changed history, pending data, roots, sources and target names retain
their existing checks. The existing public unavailable-move/saved-version tests
are the regression seams; the saved-return and return-work boundary tests protect
the refusal/restart invariants. Do not weaken those tests to accept the regression.

Candidate2's original-ten run passes all eight unavailable-move/saved-version
regressions, including the real 500-item killed-worker continuation. The other
two failures remain separately reproduced: initial queued Copy exceeds its
completion encoding budget, and 500 Keep Both restorations exceed their timing
budget. Sixteen selected library boundary parents pass (one helper is ignored),
including changed-history refusal, source reservations and consumed-name restart.
Evidence is under `target/portal-audit-83/`; this does not establish workspace or
native acceptance.

Candidate06's full workspace run passes all eight functional regressions and all
375 Files library cases (34 ignored helpers). Files integration finishes605passed,
onefailed and41ignored. Its only failure is the separate500-item Keep Both
20-second assertion, measured22.417508467s. Portal439library/55integration cases
and strict workspace Clippy pass. This verifies the functional admission fixes;
it does not make the full workspace gate green. The receipt is
`target/portal-audit-83/workspace-final-results-06.json`.

See [saved-return restart](0150-selected-saved-return-restart.md),
[proved refusal](0152-proved-saved-return-refusal.md) and
[return-work retention](0142-undo-return-work-retention.md).
