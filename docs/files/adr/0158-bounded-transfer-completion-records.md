# Transfer completion records use their existing selection-sized budget

Status: completion-budget and pagination regression verified in the workspace;
the separate Undo timing failure prevents full workspace acceptance.

The Recovery pagination integration case failed before opening Recovery. Its
270-item Copy request stopped before the first conflict because transfer intent
serialization used the generic 128 KiB filesystem-step codec. The request's live
journal records `operation step exceeds its storage budget`; the backtrace
places the timeout in `answer_replace`, not the Recovery reader.

The existing `request_transfer_ends` schema bounds both intent and result bodies
at 4 MiB. Use a local encoder and decoder with that bound for these completion
records, which contain the whole original selection. Preserve the shared 128 KiB
step codec, exact request/history/digest checks, and aggregate request budgets.
No schema, wire-format or replay-policy change.

The existing 270-item public queued Copy/Recovery test is the regression seam.
Related request execution, transfer restart and completion-boundary tests must
also pass before claiming the fix verified. Native/workspace/release acceptance
is separate.

The corrected intent now reaches the conflict question. Profiling and private
journal observations then exposed expensive repeated pure metadata work; ADR0159
addresses it. Copy progress improved from11 to121 confirmed items within9s, while
the pagination test's whole270-item setup still outlasted its generic10s wait.
Give only that asynchronous setup a60s completion wait, preserving all catalog
waits, content/identity assertions, the dedicated2s page-latency check and the
independent20s Undo budget. Bulk setup completion is not catalog responsiveness.
Tracked caller locations make any later fixture timeout identify its actual stage.

Candidate06 passes the complete pagination/identity/expiry regression both in the
original-ten check and in the full workspace run. All375 Files library cases
pass (34 ignored helpers), retaining the transfer completion and history boundary
coverage. The workspace still fails its independent500-item Keep Both timing
check (22.417508467s against20s); no full workspace success is claimed. Evidence:
`target/portal-audit-83/workspace-final-results-06.json`.
