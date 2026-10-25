# Iced operation interface review

Scope: worker-to-Iced name/Undo/Operations integration and private GIO volume
observations, against `iced-operations-worker-source-02.json`. The full PRD and
continuation/descriptor/release gates remain open. Both axes reviewed read-only;
neither reviewer compiled code or ran native applications.

## Standards

No hard documented-standard violations. Two initial P3 judgments were resolved:
replace the observed-mount tuple with named `ObservedMount` fields, and share
the phase protection/save predicates. Follow-up found overlapping notification
holds and checking a helper PID before opening its pidfd. Per-job notification
counts and opening/retaining the pidfd before parent/argv/executable validation
resolve those concerns. Final read-only verification reports no remaining
findings in these fixes.

## Spec

Two initial P2 findings were resolved: operation completion cancelled a newer
navigation, and a failed start checkpoint discarded source tracking before its
request executed. Loading tabs now retain their navigation and refresh the
eventual snapshot; resolved bindings also cover alias views. Start-save failure
is distinct from an unsaved stopped result and retains completion tracking.
The reviewer verified the navigation, alias and actual database write-failure
tests. Follow-up found overlapping notification holds; independent pending
counts resolve it. Final read-only verification reports no remaining findings
in these fixes, forced-close handling or Cancel-only observer-loss questions.

Final findings within this scope: Standards 0 unresolved; Spec 0 unresolved.
This review does not establish acceptance of unported PRD features.
