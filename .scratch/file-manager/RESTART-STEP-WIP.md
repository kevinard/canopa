# Journal step ownership before recovery mutation

2026-10-22. The full Files goal remains active. The preceding schema34
replacement checkpoint is decoded-verified in restart-replacement-source-01.tar.gz
(SHA ee02679a0f76579649f466a2ce698a925fa3e7e79a984c1bbefae47fbc979574;
1164 members). Backend360 parents pass across19 no-default-feature targets;
the required unignored restarted Retry/Continue owner gate remains the sole
failure. Default desktop rerun147 passes,6 ignored, after an undiagnosed video
timeout; isolated video also passes. Clippy/format pass. All evidence and both
protected app/Niri binaries are retained. Do not mistake this for completed
restart execution or native/full workspace/release acceptance.

The next reproduction holds actual original generations constant while changing
only the pending operation, ordinal, target, existing slot, or pending context.
The worker is genuinely SIGKILLed at replacement Publishing. Each test must
preserve the actual payload and previous across two opens, without publication
or rollback under a different history step. Tests live in
src/operations/replacement_boundary/journal.rs (under the Files crate). They
exercise Copy and whole-entry Move. Inputs01 captures855 Rust/Cargo/vendor and
fixture paths; production code is unchanged from the verified replacement
checkpoint. Red01 session28671 is terminal101: all five probes failed on Copy.
Operation/ordinal/occupied-slot recovery moved the payload before rejecting the
record; target/context changes completed outright. Preserve the five private
source/output pairs named in restart-step-red-01.log. No production changes
preceded this reproduction.

Ranked predictions: (1) recover_pending checks operation/slot/context only during
append, after moving the payload; (2) pending_bindings contains older ownership
references but does not bind the exact current claim path/role/body; (3) generic
append_forward Cleanup retains prepared payload deletion rights after publication.
Reproduce the first two before choosing the smallest complete proof seam. The
cleanup boundary still needs its own reproduction, including an already
published object moved back into its former private work name.

An initially considered seam was an independent durable journal envelope scoped to this
history, lane, operation/ordinal and exact intent, written atomically with each
intent transition. Validate it and the current history slot/context before any
filesystem work, including directory-access preparation. Cleanup writes inside
append_with_cleanup bypass Store.intent and need the same transaction. Clear
the envelope with its intent; do not bless a decoded legacy/missing envelope or
silently renew evidence from current paths. This is a candidate, not an
implemented general policy. Consider existing upgrade/recovery behavior and preserve
unknown legacy data. Source/parent/object generations remain independently
required. Complete typed handoff/abandon lineage and actual stopped execution
owners remain later work; keep the public restart gate unignored.

The implemented narrower seam reuses independently persisted pending_bindings
for Forward only. Schema35 adds a forward proof with history identity and a
SHA-256 over a domain, history, lane and the canonical full Pending::Forward.
Store.intent writes it with the intent transaction. Current recovery validates
that proof, context, operation existence and next unoccupied ordinal BEFORE
with_directory_access/prepare_directory_access or any claimed-object movement.
Proofless older opened-object transfer claims preserve their data for Recovery;
this does not validate or modernize every legacy pathname-only journal.
StageMigration pseudo-forward context gets a new proof when prepared, but its
own recovery and Preparing/Cleanup are not yet covered by this guard. General
Undo and saved-container ownership remain open.

Core01 session66212 is terminal0/acknowledged:5 regression parents pass for both
Copy and whole-entry Move (10 actual killed workers, two opens each). Inputs02
captures that source with855 paths. Three further parents cover deletion of a
still-bound operation, removal of just the newer proof from otherwise valid
context, and a VACUUM INTO history clone referring to the original staged data.
The full no-default-feature library Core02 session95348 finished0/acknowledged:
74 parents pass,5 ignored. Its changes after inputs02 are the three tests and
clearer refusal messages, captured by inputs03. The earlier Context proof
prototype had5 passing parents, so both kinds actually ran all10 crash cases.

Cleanup Red02 session38013 finished101/acknowledged on the first ordinary-lane,
unedited case. After a real post-append SIGKILL, moving completed output back to
stage/folder causes cleanup to DELETE that same original object. Preserve
/tmp/canopa-cleanup-returned-NhQxyV. Inputs04 captures that red source. The test
also covers edited/unmodified output in both journal lanes, two opens each.

Store.append_with_cleanup now derives empty-only cleanup rights and returns
that limited Stage. append_forward uses that returned value for immediate
cleanup; restarted Cleanup also limits older payload rights. Unfinished claim
roles and sibling payloads are rejected instead of normalized away. Preparing
keeps its separate partial-work policy. A briefly mis-targeted patch to Preparing
was corrected BEFORE compiling/running this fix. Schema35 gates both changes.
Inputs05 captures855 paths for this combined implementation. Full library
Core03 session84002 finished0/acknowledged:75 parents pass,5 ignored, including
all four returned-output cases in both lanes. Formatting06 finished0/acknowledged.
run-step-checks.py/session11295 is running bounded Core04..12 integration batches
with unchanged inputs05. Core05 has only the required restarted-owner gate
failure (Copy/Failed-first); request_execution passes19. Gate receipt/SHA and
retained paths are in restart-step-required-gate-01.json. Preserve
/tmp/canopa-transfer-restart-0hgFSZ and
crates/canopa-files/.canopa-transfer-restart-output-2NqLRf. The five journal reds
and cleanup red are decoded-verified in restart-step-red-fixtures-01.tar.gz
(SHA38bad981d32d5aecbc87f61b56d3edb2444cdce4f7ddf600101fc9e2ba8200bb,
11 roots/55 members). The driver finished0/acknowledged:369 parents pass across19
selected no-default-feature targets, one required restart gate failure and22
ignored helper inventory entries. No child summaries are counted twice.
All-target default Clippy01/session37541 finished0/acknowledged, warnings denied.
The required fixture is separately decoded-verified in
restart-step-required-fixture-01.tar.gz (SHA
c260577efd6ba3b9f74f879305befcd8744acf39aa3ddc288ba91a63043910ea,
14 members). Default desktop01/private9507/session43904 finished0/acknowledged
with RUST_BACKTRACE=1:156 parents pass,6 ignored. Its receipt proves855 captured
paths unchanged before/after. Current schema35 verification/source archive is
being recorded next; check those artifacts before treating that final proof as
complete. The required restart owner gate remains open. No native app rebuild,
complete backend/workspace/release or physical-device acceptance is claimed.

Next audit after this checkpoint: ordinary non-replacing Copy still constructs
Change::Copied.publication=None in transfer.rs and uses generic commit_forward.
Generic startup uses change.committed() for published output. Verify whether it
consumes the Stage ownership's original payload at the published name before
accepting completion; do not assume the new replacement-specific publication
proof already covers plain Copy. This is source inspection, not a reproduced
failure. Other immediate gaps remain Preparing/Cleanup/migration binding,
decomposed empty-folder removal, saved-container claims, unsupported/legacy safety
and typed handoff/abandon lineage. Actual restored owners must wait for those
proofs, then pass all Copy/real cross-filesystem Move part orders of the required
gate. Broader restart families and all later goal areas remain active.
