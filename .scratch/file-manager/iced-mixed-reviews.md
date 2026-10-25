# Mixed batch choices and native Move review

Baseline: iced-question-focus-source-08.json. Final: iced-mixed-source-05.json
(530 inputs). Final reviews include applied engine code, public/native oracles,
Move fixture extension and recorded evidence. No full-PRD acceptance is inferred.

## Standards

No concrete scoped standards or maintainability findings.

The Move fixtures verify fresh Cut capture, atomic Keep both identities,
skipped-source retention, completed-source cleanup, restored payloads and leaf
identities after Undo. Copy checks remain intact. Documentation matches the
receipts: 44 focused tests and 36 declared native checks across eight cases.
Earlier 420-core/51-Iced results remain explicitly separate; broader PRD and
release gates stay open.

## Spec

No actionable scoped Spec findings.

The Move fixture captures fresh Cut intent and checks Keep both identities,
Cancel/Skip preservation, Merge cleanup, skipped leaves, payloads and shared Undo.
These assertions verify filesystem results independently of notifications.
Saved reports support 44 focused tests and 36 declared native checks across eight
passing cases with empty application logs. Documentation correctly assigns the
earlier 420-core/51-Iced results to the prior binary and keeps broader PRD and
authority gates open.

Standards: 0 unresolved findings. Spec: 0 unresolved findings.
