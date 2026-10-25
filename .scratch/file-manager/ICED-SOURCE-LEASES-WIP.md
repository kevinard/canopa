# Clipboard retained-source leases

## Current verified checkpoint

Full PRD goal remains active. This goal turn made verified progress; no product
checkbox is complete. All compiler, native and archive handles listed below are
terminal. There is no live build/test/desktop process to resume.

Default executable: bf3f9f2f41ad60d3d1c548e9a5021db644b64bed9cc3c870d2c9d989be51f9d4,
118780768 bytes, Iced and canopa-ui, no GTK widget links. Build52422 exit0 from
source09. Source10 freezes533 final inputs; its only delta is ADR0072 and fixture
README documentation. Archive80921 exit0 verifies every source tar member and
the executable gzip's decoded digest. Integrity33905 exit0 verifies final source,
unchanged executable,16 changed inputs' whitespace and322 unchanged vendor files.
See iced-source-leases-verification.json, iced-source-leases-preservation.json and
iced-source-leases-source-integrity.json.

## Implementation

ADR0072/schema22 implements opaque cloneable process-owned retention via hidden
tagged request rows sharing source reservations, budgets and physical migration.
Capture stays non-owning. Opening history under its exclusive lock removes dead
process leases while retaining accepted request reservations. A dedicated worker
holds a duplicated flock descriptor, releases metadata off the UI thread and
retries failed deletes before new acquisition. Ordinary final Drop queues release;
explicit release acknowledges the final deletion. Arc::into_inner coordinates
concurrent explicit owners so one observes a final failure.

OperationJobs lease_sources acquires on its metadata actor during live filesystem
questions. submit_transfer_with_lease owns the clipboard guard in Control through
durable admission, including abandoned replies. Iced Copy/Cut waits for a lease
before publication; Source owns it through exact token, pending read and queued
admission lifetimes. Superseded preparations/publications/owners drop only their
handles. Owned Paste requires its guard. External Paste retains fresh admission
capture. This is retention, not execution or replay authority.

## Validation

- Full core25668 exit0:430 tests. Includes15 source-retention parents and21
  transfer-service tests; crash helpers remain independently scoped.
- Iced library41704 exit0:51 tests. Default check32877 and all-target
  Clippy -D clippy::all28336 exit0, with3 existing unused-helper warnings.
- Native34302 exit0:28 checks across4 cases, dark100/light125. Retained Copy
  survives100 actual UI history operations, pastes exact bytes and releases
  expired data after external supersession. Ordinary clipboard Copy/Cut/Paste,
  conflicts, Undo and closed-Browser lifetime also pass. All4 application logs
  are empty. Dark expiry/light cleanup captures inspected.
- Formatting32809, vendor integrity,67 script ASTs,115-case matrix parsing and
  tracked/changed whitespace checks pass.
- Both independent review axes:0 unresolved scoped findings. Fixed concurrent
  release acknowledgement, private-basename discovery, async cleanup timing and
  stale Immediate state metadata. See iced-source-leases-review.md.

Baseline37554 exit1 on preserved5906ee19 genuinely lost owned copied bytes after
100 native operations. First67612 exit1 was only a capitalization mistake in
its new notification oracle. Core63253 failure was only a foreign-history error
category assertion; refusal already worked. API97782 and29843 are missing-method
compile reds, not runtime regressions. Focused39474 passed36 core/service parents.
Source01 through10 and individual logs retain the progression.

Cache cleanups are recorded in baseline-and-cache and cleanup02/03/04 JSON. They
remove generated inactive application caches, then47 completed Files integration
executables verified through .d source paths under Cargo lock. Source, logs and
preserved baselines remain. Regenerate those test executables with Cargo when
needed; native source-leases uses UI operations and has no helper-binary dependency.

## Next full-goal work

Continue further typed-question native coverage and source/journal/descriptor/
original-media/restart authority before exposing safe Retry/Continue. Native
retained-data acceptance here covers Copy; consuming retained-source Rename/
Trash/Move authority is still open. DnD, activation, Iced Trash/Recovery/devices/
media/Downloads, accessibility/performance/shared-stack/workspace/release and
final full-goal review/commit remain. Reuse this candidate and preserved source;
do not redo passed tests without a new change or unresolved concern. The unrelated
shell search-backends research file and extensive existing goal changes remain
untouched.
