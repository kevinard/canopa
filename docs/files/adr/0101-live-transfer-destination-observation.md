# 0101: Original destination folders through live transfer continuation

Status: scoped backend and native verification passed, 2026-10-20.
Extends ADR0045, ADR0075 and ADR0100. The later application workflow is recorded
in ADR0102; restart authority remains open.

## Evidence and decision

Before exposing application Resume, a public engine regression reproduced a
resumed Move writing into a different destination folder at the same pathname.
A second run, whose callback permits ordinary Copy verification, reproduced
the same error for Copy. The earlier callback cancelled every interaction and
therefore did not establish Copy refusal. Both original attempts are preserved.

The destination identity was local to each execution. Retain an opened root
folder with the original observed transfer and every live Retry/Continue plan.
Capture an already available destination on the admission worker, following
folder aliases. The O_PATH/DIRECTORY/CLOEXEC pin prevents inode reuse while the
plan lives. Recheck that original folder at existing destination checkpoints,
including after interaction. Replacing or retargeting the pathname pauses work;
returning the actual original folder permits continuation. Never observe the
replacement to repair an old plan.

An initially unavailable destination has no opened folder to retain. Preserve
the original storage-placement workflow and bind its first available folder
once, shared by all clones and returned plans. This does not establish that the
returning storage is the originally requested medium: application storage and
request/journal authority remain required.

## Resources and saved data

Destination roots have a separate process allowance of at most 64 pins and at
most one sixteenth of the descriptor soft limit. They do not consume or enlarge
the selected-entry allowance or clipboard replacement credit. At a soft limit
of 128, at most eight destination roots can be held. Clones share their original
reservation; failure and final release return it. The allowance leaves the
existing selected-entry limits intact while bounding added filesystem handles.
Explicit abandonment in ADR0102 retires these owners as well as their
selected-source pins.

Completed results release their observations. Descriptive saved application
results explicitly discard both source and destination observations; ADR0102
adds a separate private owner for live stopped plans. Portable
checkpoints remain version 1 and never serialize or recreate process pins.
A decoded payload remains insufficient application execution authority.

## Verification and remaining work

New public tests cover Copy/Move, a replaced directory and a retargeted alias,
restoration of the original destination, separate Undo of resumed work and its
completed prefix, and destination observation before initial cancellation.
A private subprocess with a descriptor soft limit of 128 exercises bounded
admission, shared clones, final-owner release, failed-capture release and repeated
completion without leaking destination capacity. The frozen source11 full backend
suite passes 556 parent tests across 65 standard result targets, plus its custom
device harness. The 37 ignored entries include parent-invoked helpers and tests
requiring fixtures; they are not all claimed executed. Focused verification
passes 87 parent tests and five invoked helpers.

The default library suite passes 78 tests (one ignored helper); formatting,
all-target Clippy with warnings denied and the default desktop build pass.
The new executable `84168e8d2ee9` is archived and decoded-byte verified. Six
maintained native cases pass 22 grouped checks across dark/100% and light/125%:
transfer progress/result inspection, unchanged descriptor-limited clipboard
capacity, and unavailable-destination Retry. The latter refuses a replacement
folder, preserves the completed prefix, resumes only after the original returns,
and checks shared Undo. Critical screenshots were inspected. No extra GUI fixture
was needed for this engine-owned fix. Primary standards/spec review of the entire
delta from continuation-source-30 has no unresolved scoped findings.

See [the source/binary/evidence chain](../../../.scratch/file-manager/continuation-destination-verification.json).

This closes one prerequisite for live application Retry/Continue. The subsequent
workflow in [ADR0102](0102-live-application-transfer-continuation.md) adds private
pending plans, original live placement, atomic handoff, save-failure ordering,
clipboard settlement and lifetime holds. Pathname checks still do not provide descriptor-relative mutation,
cross-process restart authority or stale-plan replay protection. The full product
brief, native, accessibility, performance, workspace and release gates stay open.
