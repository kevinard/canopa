# Opened transfer folder facts, in progress

2026-10-22. The full Files goal remains active. ADR0114 records this work; the
required application restart Retry/Continue gate is still open.

The new public regression in job_checkpoints emulates recycled inode tuples
without relying on allocator timing. The original implementation copies a
replacement child (restart-directory-red-01.log). The current implementation
refuses replaced source/destination folders and either parent while retaining
completed-prefix bytes and identity. The old raw tuples are deliberately made
to match the replacements; durable original tokens remain unchanged.

transfer/directory.rs owns opened source enumeration, Merge's pre-question
folder observation, created-folder payload/parent descriptors, and original
folder/parent facts. Created stages are allocated under the actual opened
parent; publication uses descriptor-relative no-replacement rename. Only stable
namespace labels enter the journal. Source names retain Unix bytes. Folder
validation uses O_PATH; opening readable directory streams is limited to actual
enumeration so mode-000/read-only created folders keep the existing access flow.
Checkpoint v3 and schema 30 carry optional four-object traversal facts. Older
formats remain readable but cannot smuggle those new facts. Legacy/unavailable
facts do not become restart execution ownership.

Core01 passes the five initial checkpoint parents. Core02 passes 158 parents
across eight targets, including 41 permissions/cleanup parents and new opened
folder/payload/parent replacement units. Core03 passes the six application queue,
clipboard, stopped-result and source-retention targets (88 parents). The required
restart test is explicitly excluded from the application regression command;
it remains unignored. All 246 parents together cover 14 targets.

Clippy01 found only the version match style (1|2|3 -> 1..=3). That correction
changes no accepted version. Source manifest restart-directory-inputs-02.json
records the final 835 Cargo/crate/vendor inputs. Core04 reruns all 14 targets on
that exact source; its process is still running as of this note. Initial passing
checks belong to source01; source02 current validation is not yet claimed.
Formatting and fixture Python compilation pass; the claim-review schema literal
is now 30. No new native application build has been made.

The separately invoked required gate fails in restart-continuation-red-04.log
on source01/schema30: the application still reconstructs no transfer execution
owner. Its actual private data is retained by the test. A fresh required-gate run
and read-only proof inspection are still needed for the final source02 checkpoint.

Remaining work is actual execution, not more descriptive compatibility:

- Bind child-file reads and symlink reads to opened leaves and validated original
  parents. Root destination installs in the actual held destination parent.
- Capture/persist stage, parent and payload ownership at creation, before data
  writes; secure interrupted cleanup against replacement payloads/stages.
- Claim same-filesystem Move and cross-filesystem source cleanup through opened
  parents. Empty-folder cleanup also needs a verified claim before deletion.
- Validate the exact stopped receipt, saved revision/body and part hashes,
  original storage and journal/step ownership. Add typed handoff/abandon lineage
  before minting a restored execution owner. New-process clipboard stays fresh.
- Pass the required killed-process Copy/Move test in both part orders, then the
  complete crash matrix and the remaining operation families and goal phases.

The existing fingerprint copy, generic stage cleanup and source cleanup still
have pathname-based mutation work to finish. Folder facts alone do not make
those boundaries safe. The private-folder publication helpers do not establish
persistent generic-stage cleanup authority. No full restart, backend, native,
workspace or release acceptance is claimed by this unfinished slice.

## Current state after source07

Source02 Core04 completed: 246 selected parents passed across 14 targets. Later
review added two real Forward-journal boundary cases and the held created-folder
FD for mode restoration. Source05 Core05 then passed 160 folder-engine parents;
its 835 inputs are protected by decoded-verified restart-directory-source-05.tar.gz.
The first boundary03 build hit ENOSPC; its failed log remains. Audited test-cache
reclamation03/04 retained sources, app/Niri and all proof archives. One attempted
cache audit stopped on an unexpected failed-object header before deleting any
file. Subsequent scoped reclamation succeeded; no unowned cache was removed.

The schema29/v2 stopped-receipt regression then reproduced a real upgrade bug:
re-encoding old plans as v3 invalidated their stored hashes. Receipts now verify
original JSON fragments with RawValue; the verified metadata transaction binds
current part hashes while preserving the original whole-end hash. Failed upgrade
writes roll back those bindings. The legacy killed-process test covers save
failure/retry, two starts, newer bytes/inodes and Undo, with no invented traversal
facts. restart-directory-legacy-01.log passes 19 request-execution parents.
Source06 Core06 passes all 160 folder-engine parents on the updated reader.

Source06 Core07 passed the queue/clipboard/continuation targets but exposed an
intermittent immediate history-reopen EAGAIN in request_source_retention. The
diagnose skill was applied. Ranked causes: inherited fork descriptor, premature
worker completion, surviving owner. The first controlled fixture hit a sandbox
socket-timeout restriction; anonymous CLOEXEC pipes remove that dependency. The
real controlled fork failure reproduces three out of three times before any
lease worker exists (restart-history-lock-reproduction-01.json). Linux flock
open-file-description semantics explain the result (ADR0115 cites flock(2)).

Source07 adds a shared HistoryLock guard for the engine and release worker;
the final logical owner explicitly unlocks before close, so an unrelated fork
copy cannot prolong ownership. Active leases keep the same guard. The exact
controlled test passes for both no-lease and active-lease cases. No debug logging
or production test hooks were added for this diagnosis.

Current frozen build inputs: restart-directory-inputs-07.json, 836 entries.
The controlled-fork x20/full-original-retention-suite x3 stress passes; its
original process and all final checks reached terminal exit0. Default all-target
Clippy02 passes. Core08 passes160 parents across8 targets; Core09 passes90 across6
targets, explicitly excluding the required restart gate. Combined250 parents /14
targets have11 ignored inventory entries; invoked subprocess helper results are
not counted as additional parents. The isolated desktop library passes121 with2
ignored on the same836 unchanged inputs (private-bus receipt1792628033).

Required red05 remains an unignored failure: Files restores no Retry/Continue
execution owner. Actual fixture data remains under
`/tmp/canopa-transfer-restart-uZy1HZ` and workspace
`.canopa-transfer-restart-output-YGI43p`. Read-only inspection
confirms schema30/v3, the exact original report/part/saved-body/intent hashes,
sealed originals, created target parent, and newer completed bytes. The boot and
kernel object hashes match all four recorded directory/parent facts. Separate
sandbox command mount namespaces expose different unique mount identities;
those observations do not establish original storage or execution authority.
The first inspector incorrectly expected equal mounts; its failed artifact is
retained as restart-directory-red-05-inspection-attempt-01.json. The corrected
collector reports that distinction explicitly without weakening Original.

Formatting and the schema30 fixture's Python syntax pass. Final evidence is
restart-directory-verification-01.json; source07 is preserved with a decoded
member/hash audit. The source07 application binary has not been built; protected
b379372fb3f1 remains. No complete backend/native/workspace/release acceptance is
claimed. Actual resumed mutation/cleanup and all five goal areas remain active.
