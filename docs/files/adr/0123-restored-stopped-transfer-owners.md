# Restored stopped-transfer execution owners

Status: scoped backend and desktop-library verification. The full restart and
product work in the implementation ledger remains active.

## Problem and boundary

The scheduler restored the exact stopped Copy/Move report but left both private
execution-owner fields empty. The required real-SIGKILL test therefore exposed
neither Retry nor Continue. Restoring a report or decoding its checkpoint does
not establish an execution owner.

This change handles engine-returned, durably stopped transfers with no pending
filesystem journal. It does not restart an arbitrary Running or Queued row.

## Decision

The engine first verifies the independently sealed stopped result against the
current request revision, phase and complete body, immutable report facts, and
each still-available part hash. Its accepted transfer intent must also verify the
original physical history file. Schema38 adds that optional original to new
accepted intents; older completions remain readable, while older intents without
this proof cannot acquire restart ownership. A copied history cannot mint another
owner for the same part.

Each part reopens the original selected roots and destination using their recorded
boot/mount/file-handle generations, with selected parents and original traversed
source/destination pairs checked separately. It retains the resulting descriptors
under the existing process budgets. Unselected children may be repaired; a
replacement selected root, containing directory, destination or device cannot be
adopted. Completed paths are not walked or copied again. Original raw Unix paths
are retained throughout, including the history filename read from SQLite.

A failed proof blocks that part independently. Other proved parts can continue.
A sealed but unavailable part still permits metadata-only Abandon. Saved-container
movement and older traversals without original-folder proofs remain unavailable.

A handoff checks that exactly one part becomes HandedOff and the successor has
that exact engine checkpoint. It advances the stopped result's revision seal in
the same transaction as the original result and successor insertion. Abandon
advances the seal with only Abandoned dispositions. Generic scheduling writes
cannot advance this seal. A second restart therefore retains the other part and
cannot hand off the consumed part again. These writes leave filesystem work and
Undo entries intact.

## Verification and limits

The required unignored test covers Copy and actual cross-filesystem Move in both
part orders, preserves an edited completed prefix and its inode, retains chosen
Keep both names, and undoes each successor independently. Additional process tests
kill a second worker after consuming one part and then finish the other. Refusal
cases replace a selected entry, its actual parent while retaining entries, the
destination root, or a traversed output folder; alter the plan/revision or remove
its seal; and copy the history database. A typed engine test rejects a changed
successor and proves that the original sealed part remains available.

The final frozen source passes 381 selected backend tests across 19 targets
(including 83 library parents), and 164 isolated default desktop-library tests.
Default all-target Clippy with warnings denied, formatting and the schema fixture
syntax pass. The required gate remains unignored. Protected app/Niri binaries and
the earlier local-runtime gate are unchanged. Receipts and the source archive are
recorded in [the WIP ledger](../../../.scratch/file-manager/RESTART-OWNER-WIP.md).
No native, full workspace/release or physical hardware acceptance is implied. Mid-execution continuation, pre-dispatch queued
handoffs, metadata-save retry restoration, saved-container movement, complete
Preparing/Cleanup/migration binding, and Undo/relocation/expiry use of newer
original facts still require work. Trash/Restore and the other four goal areas
also remain open.
