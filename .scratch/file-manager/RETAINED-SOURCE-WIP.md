# Retained source reservation checkpoint

Status: verified scoped progress, 2026-10-18. The full PRD goal remains
active; no requirement or completion boundary is removed. Iced and `canopa-ui`
remain mandatory. ADR0068 records the new decisions.

## Implemented scope

Opaque, history-bound captures identify known retained containers used by a
selection. Initial transfer admission in the shared Iced operation service saves
the request and reservations together. Reservations survive unfinished records,
reopening and failed saves; stale/foreign admission and destructive pending work
are refused. Consuming Undo and automatic expiry share the transaction interlock.

Finish/dismiss release reservations. Handoff protects the successor before the
old row releases anything, preserving an unselected plan's protection. Physical
saved-container migration moves reservations with catalog cutover. Reservation
storage counts toward existing logical quotas. Capture includes owned data in
selected directories, resolves ancestor aliases, preserves final links and
does not adopt unowned lookalikes.

## Evidence

Baseline: `iced-transfer-source-02.json` (500 inputs). Initial reviewed source is
`retained-source-source-01.json` (503); changed paths are in
`retained-source-changes-01.json`. Final review refactoring is captured in
`retained-source-source-02.json`. The two reviewers found no correctness defects;
the Standards P3 about repeated journal classification is resolved. Both final
axes report zero remaining scoped findings; see `retained-source-review.md`.

Actual red/green evidence:

- `retained-source-red-01.log` / `retained-source-green-01.log`: public capture
  and admission preserve retained bytes through expiry, restart and dismissal.
- `retained-source-migration-red-01.log` / migration-green01: migrated bytes were
  lost during expiry until reservations followed physical catalog cutover.
- `retained-source-service-red-01.log` / service-green01: a stopped transfer lost
  its source after other jobs expired history; shared admission now protects it.

Core01 (session2206) explicitly terminated0:413 tests across44 binaries, with22
helper definitions exercised by their parents, plus doc tests. This verifies
source01, before the behavior-preserving shared-classifier/test-slice refactor.
Final focused02 (35047) terminated0:50 tests across5 binaries and1 helper
definition. It verifies final source02 for retention, transfers, ordinary
operations, scheduling records and handoff. All-target Clippy01 (52801) terminated0
with `-D clippy::all`; the three existing unused-helper warning groups remain.
Format02 (82973) terminated0, and all10 changed source/document paths have clean
whitespace. The earlier tracked whitespace check also terminated0.

Iced library/controller tests (73002) terminated0 with51 passed. The default
build (91876) terminated0. Candidate `eafd77bb…` is115276304 bytes, its libraries
resolve without GTK, and `--version` exits0. All503 final source inputs remain
unchanged through verification. See `retained-source-verification.json` for the
full hash, input and command receipts. Every compiler, test and metadata session
in this checkpoint is terminal; both reviewers are complete with no unresolved
findings. No native fixture ran in this slice.
The prior native candidate and its archived executable remain historical proof
in `ICED-TRANSFER-WIP.md`, not acceptance of this source snapshot.

## Next work for the full goal

Complete ownership for ordinary mutations/Trash and other consuming transfers,
stages created after capture or still in forward journals, exact saved-plan path
rebinding after migration, original media and directory-descriptor authority.
Verify the converse acquisition/destructive-intent ordering at an actual journal
boundary. Finish exact journal association and crash reconciliation before
restored Retry/Continue executes; current restored plans stay review-only.

Continue Iced transfer invocation/questions/progress/controls, runtime exact
Retry/Continue and real clipboard/DnD; Trash/Recovery, devices, media, FileManager1,
the Downloads walkthrough, accessibility/performance, workspace and release
gates, independent final review and commit. Preserve the ledger's entire Next
work list. The unrelated shell research document stays untouched; Git is read-only.
