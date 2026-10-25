# Phase2 execution authority work

Owner: chat01a1587c-5f38-7f83-9113-c6ac3fcb8815, primary only. Original chat idle;
host audit found no competing writer or running build/test/native/archive job.
Historical private Files PID1804484 untouched. All715 finalsource56 hashes matched
before new work. Preserve every prior archive and unrelated work.

## Current terminal checkpoint

Application code is frozen at execution-authority-source-18.json (721 inputs).
Full backend core21/receipt60 passes577 parent tests across67 standard targets,
plus the custom device harness;39 ignored entries are not all executed. Desktop23/
receipt61 passes83 tests, one ignored. Formatting24, all-target Clippy25 and default
build26 pass. Every frozen input and the application hash matched through native.

ELF:613871c857577a78023afb4ee2627752efe2b165d16f29938b5e9c577339701a,
134416840 bytes. Preserved gzip execution-authority-canopa-files-613871c85757.gz:
31610318 bytes, SHA25609a7fff06935ecf372ad31854a6a8ebf166f277d373e06185941f17a408586dc.
Decoded bytes equal the ELF; build28 receipt binds it to source18. Previous ELF
51f509f7f26f and its gzip were reverified before rebuilding. All238 inherited proof
archives remain unchanged; all nine intermediate source archives passed exact
member/hash verification. Final source36 reconciles documentation/evidence only,
with the additional standalone diagnostic source clearly outside the app build.

Native29 passes Create/Rename/Undo: two cases,24 grouped checks. Native30 passes
live Copy Retry/Continue: two cases,six grouped checks, including opposite orders,
new external clipboard preservation, all-window closure/Quit holds, reactivation,
exact prefix/Keep both files and independent Undo. Native33 passes interrupted
Recovery: two cases,ten grouped checks. Total: six cases/40 groups, dark100% and
light125%, pinned Niri63e39bae3473. Five critical captures were inspected.

Native31 is an interrupted attempt, not a complete pass: its dark case wrote five
passing checks, then the parent exited143 without a root summary or light run.
No private children remained. Cause is unknown. Ranked hypotheses were external
termination, incorrect fixture process-group cleanup, then helper cleanup. The
standalone probe32 installs a sender-recording SIGTERM handler only in the parent
runner and preserves default termination; its library is inherited by private
children but installs no handler there. Native33 retained the same assertions,
timeouts and ELF. Its trace records only arming: the signal did not recur, so no
cause/fix is claimed. Self-test34 exits143 as expected and records its own sender,
verifying capture without hiding termination. Every attempt/probe is retained.

All owned handles are terminal: red/focused02/55452,03/62231,05/18091,07/19003,
09/61752,11/59493,13/82378,15/87218; interrupted17/13901; prebuild19/47488;
full21/38319; desktop23/69205; fmt24/94412; Clippy25/68583; old-ELFcheck26/43358;
defaultbuild26/23026; archive28/49841; native29/80759,30/2909,31/71332,33/60211.
Inventory94201 and source-archive45555 checks are terminal0. Self-test34's143 is
expected. Checkpoint22/27 JSON describes earlier observations and is superseded.
Host audit found no live owned compiler/test/native/archive process. Historical
private Files PID1804484 remains untouched. Original chat was idle earlier and
notLoaded on final recheck; no competing implementation writer was observed.

## Queued Rename diagnosis

The public queue test uses a real Undo conflict as a deterministic blocker.
Source01/core02/receipt51 reproduced replacement-file adoption: Rename reported
Finished and moved the wrong file. Core03/receipt52 reproduced it again unchanged.
Ranked causes: missing admission-time leaf observation; parent-only browsing
binding; execution-time capture of current identity. Inspection confirms Rename
used Mutation directly, while Transfer/Trash/Restore captured original selections.

Source04 adds the original entry guard and positive/negative variants. Core05,
receipt53 passes operation_jobs, operation_transfers, operation_trash and
request_source_retention (inspect exact summaries rather than count helper output
twice). This proves files/folders/final symlinks/missing-then-created refusal,
original return/content edits/raw names, explicit reselection and ordinary Undo.

Source06/core07/receipt54 adds the parent replacement regression. It fails with
the original source inode hard-linked into a replacement parent while the source
browsing binding retains the same canonical path text. The source-parent case is
the first failing iteration; destination-parent failure is not separately claimed
on the old source. Current code adds pinned parent identities, following parent
aliases and sharing ordinary descriptor limits. Tests add original-parent return
and an isolated RLIMIT_NOFILE128 child for admission refusal, partial-capture
release, queued cancellation and repeated completed-row release.

An unsealed intermediate edit briefly added Rename source reservations, then was
removed before any run. Existing retained mutation checks intentionally refuse
all request owners; adding self-retention without an execution-owner seam would
deadlock this workflow. This is documented, not treated as solved retention.

## Request/journal association

Source08/core09/receipt55 failed compilation before tests because the parent-path
iterator supplied &&Path to rustix::open. The corrected iterator is source10;
core11/receipt56 passes all8 affected targets, including the real low-limit child.
No failed attempt was overwritten. Parent and selected-entry guards remain
process-local check/use protection, without a claim of atomic mutation.

Source12/core13/receipt57 introduces actual result-save interruption: an isolated
child reaches SaveFailed after filesystem commit, writes its job/Undo marker and
SIGKILLs itself. Restored Rename lacks its operation association (first failing
iteration). Source14 associates exact Running revision/payload with operation ID
atomically before forward work. Core15/receipt58 passes5 focused targets; the crash
parent now completes both Rename and partial-Copy iterations. Output bytes edited
after the crash remain intact; exact current operation Undo is visible.

Current work adds one-execution-per-request protection even across a newer
Running revision or expired Undo, evidence footprint in existing storage budgets,
rollback/cleanup, foreign/stale refusal, exact digest and schema22 migration tests.
See ADR0104 for scope and why evidence never supplies restored execution authority.

## Open Phase2 gates (continued)

Request/execution revision association with journals and confirmed steps;
filesystem-success/result-save reconciliation; durable proven identities and
explicit new-authority review; retained-source ownership; opened-directory and
entry-relative mutations; actual subprocess interruption fixtures. The current
Rename observation remains process-local and does not close these gates. Phases3–6
and the complete product brief remain active after Phase2.

## Primary review before final verification

Full source16 run17/receipt59 was stopped deliberately with SIGTERM during
compilation after188.876s to apply review corrections. Handle13901 was polled
terminal1; receipt is interrupted, with no completed full-suite claim. The same
runner was not duplicated. Review found unbounded path decoding in request_effects
and character-based label accounting. Current code samples at most128 confirmed
change paths, reports truncation and the total count, and counts UTF8 label bytes.
New public/core tests cover both limits. Final full-suite compilation is separated
from execution so the private execution timeout does not also cover all linking.
Prior inventory verification checked every238 inherited proof archives unchanged.

## Next implementation priorities, not completed features

1. Reproduce queued Rename's retained-source lifetime through history expiry.
   Current admission has object pins but no retained-source reservation. The new
   execution association provides a possible seam for narrowly recognizing the
   matching Forward execution's own reservation while preserving every other
   request/clipboard owner, Undo and expiry protection. Do not simply release its
   only reservation before mutation. This public-queue hypothesis is unverified;
   add the failing behavior test before changing ownership. Ordinary failed/
   restored request review and explicit dismissal also remain necessary.
2. Reproduce the remaining Rename check/use race and audit the prompt's selection
   lifetime before admission. The current fix starts at metadata admission. A
   prepared Rename capability, similar to clipboard/drop preparation, could retain
   backend authority while the name draft stays in Iced. Opened-parent/entry-relative
   mutation needs safe post-claim verification, rollback/quarantine and matching
   journal recovery, not just another pathname check. These are design candidates.
3. Extend request association to other operation families and persist exact engine
   end/step receipts. Undo works on existing operations and needs its own execution/
   reverse-step meaning. Reconcile from receipts against exact scheduling revisions;
   neither the confirmed count nor an absent journal establishes completion.
4. Complete durable original-object/media proof, exact traversal/stage/cleanup
   authority and explicit review/new-authority paths. Loading saved plans or
   pinning current paths again supplies no original authority.
5. Complete actual interruption coverage at preparation, partial copy, quarantine,
   filesystem commit/result save and handoff; then continue phases3–6 from the
   original handoff and requirement ledger. Native Move/Cut continuation, complete
   accessibility/hardware/performance and release acceptance remain open.

A primary-source investigation points to Linux opaque file handles as one candidate
identity primitive: they can compare objects across paths/times; ordinary mount IDs
are reusable, so filesystem identity needs separate proof. Final symlinks are handled
without following them by default, and opening by handle requires privilege. Support
varies by filesystem. See the [Linux manual](https://man7.org/linux/man-pages/man2/open_by_handle_at.2.html).
No production file-handle implementation, supported-restart guarantee or capability
probe is claimed here. Investigate against private files before committing a design.
