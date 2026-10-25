# 0106: Prepare Rename authority before its name draft

Status: scoped backend, Iced and native verification, 2026-10-20.
Continues ADR0103/0105 and Phase2; durable restart authority remains open.

The real Iced F2 regression in prepared-rename-native-02 reproduces adoption in
both appearances on the previously sealed ELF73cc8dfa2c9d. While the name dialog
is open, park the selected original and install a different file at its pathname.
After normal watcher refresh, submitting the typed name renames that replacement.
The original bytes and inode remain parked. The old UI held only the source path;
backend observations began at later admission. A renderer epoch check cannot
supply original authority when refresh has not arrived.

OperationJobs now prepares an opaque PreparedRename on its metadata worker.
It opens the final selected entry without following its symlink, follows and pins
its original parent, captures original storage requirements, and acquires a
retained-source lease. Use existing source-pin
and request budgets. Iced opens the draft only after preparation succeeds and its
origin is still current. Iced owns name text, focus and window lifetime. It does
not inspect filesystem identity or assemble source reservations. Preparation
establishes its process-local observation boundary, not an earlier browsing or
atomic kernel mutation guarantee.

Clones share one single-use capability and runtime identity. Admission accepts one
Unix filename in the same parent, checks the original observation, and preserves
it in the queued job. Another runtime, consumed capability, changed leaf/parent
or invalid name is refused. A failed admission leaves the original capability
usable after correction or original return. A committed admission consumes every
clone, even if its later filesystem work stops. Cancellation drops an unaccepted
draft's ownership; ordinary UI Drop remains asynchronous.

Move retained-source references from this preparation's lease to the new queued
request in the same immediate SQLite admission transaction. A refused insert,
reference deletion or budget check rolls back both sides. The empty old lease
row retires through its normal release worker. Do not release the only protection
before acceptance. Independent leases and requests keep their own references;
ADR0105's exact Forward Rename exception remains the only execution exception.
No new schema, budget, persisted token or saved-plan replay authority is added.

Public regressions cover source and parent replacement/original return, runtime
and name refusal, single use, queued dispatch, transactional handoff refusal,
retention through100 intervening operations, cloned and abandoned drafts, and
independent prepared owners. The expanded native case also returns the original
to the same refused draft and exercises real shared Undo. Current evidence and
terminal attempts live in PREPARED-RENAME-WIP.md. Atomic check/use mutation,
durable original-object/media proof and the rest of Phase2 remain separate gates.

## Original device requirements through the draft

Core23 and native25 reproduce a second gap after the object fix. Preparing an
intent under one provider UUID, replacing that private provider at the same path
while keeping the actual source/directory inode unchanged, then submitting the
same F2 draft completes the Rename under the new device requirements. Native25
reproduces in both appearances; its exact observation JSON records Finished,
source absent and target present. No actual host mount or hardware changes.

Store a Storage snapshot in the same opaque preparation and use it for admission
instead of taking fresh requirements after the name has been typed. Its live
availability checks still consult current storage, while original requirements
remain fixed. A new deliberate F2 selection captures the reviewed replacement.
The native case additionally inspects the failed result, preserves exact bytes/
inode, selects the replacement afresh and exercises real shared Undo. Current gates
for this extension are recorded below and in the work log.

## Current verification

Source26/739 inputs passes591 backend parent tests across67 standard targets plus
the custom device_runtime harness;39 ignored entries are not all independently
executed. Default Iced84 pass/1 ignored, formatting, all-target Clippy with warnings
denied and the desktop build pass. Final ELF240396644fda (134636048 bytes) is
protected in a decoded-verified gzip. Six current native cases/40 grouped checks
pass on that same ELF in dark100% and light125%, with pinned Niri63e39bae3473.
They cover expanded F2 authority, original return/fresh selection, Create/Rename/
Undo and real SIGKILL/result-save dismissal. Four final captures were inspected.

Earlier ELF6fc9783ac705 separately passes ten native cases/52 groups, including
continuation and interrupted Recovery; those are not counted as final-ELF gates.
All failed and intermediate attempts remain. Source/ELF hashes match throughout
each run. Evidence: prepared-rename-verification.json and primary review40. The
complete interruption matrix, atomic descriptor-relative mutation and durable
restart continuation remain open under the full product brief.
