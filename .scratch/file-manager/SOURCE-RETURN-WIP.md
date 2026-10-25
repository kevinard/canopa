# Returned-source restart continuation

The previous turn made verified progress; the full five-area goal remains active.
At this turn's start the schema41 archive was verified:
terminal-leaf-source-01.tar.gz, SHA256
0d438a2bb836db72516e049def09d4934b57c9097c5633f3854373e0dc874493,
1085 decoded members. All 864 captured inputs matched before new test edits.
Previous data, sources, proofs and protected binaries remain. No commits,
worktrees, subagents or vendor changes were made.

source-return-red-01.log and -red-02.log repeat a public-service SIGKILL after the
source-to-slot rename with a Ready journal. Public Reconcile returns the source,
but the cursor refuses continuation: “A completed filesystem step lost its exact
active-task proof.” Durable inspection shows the unchanged returned source, clear
journal, committed Copy0 and uncommitted removal1 still bound. The physical return
and copy were intact; the missing metadata disposition was the cause.

The new Store.returned_source_intent operation accepts only the exact normalized
Ready/Returning Forward-to-Preparing transition and verifies the independently
bound Forward context. In the same transaction it verifies the durable active
head/step, retires only its exact uncommitted removal receipt and saves the
empty-only Preparing intent. It never drops committed Copy proof or recreates
missing receipts. Changed/out-of-order/committed removal evidence refuses the
transition. Shared active-head validation also protects ordinary live binding.
No on-disk row format changed; schema41 and checkpoint3/4 remain.

core-01.log passes the first Ready and Returning public continuation gate. The
expanded fault harness initially had a Rust string quoting compile error in
core-02.log; the corrected core-03.log passes all 20 copied/return boundary parents,
3 ignored helpers. lib-01.log passes 104 library parents, 10 ignored inventory
helpers. Four new unignored public-service parents cover actual Ready/Returning
SIGKILL, public review, two later opens, original-copy inode retention, another
crash after Preparing commits, unexpected private payload refusal, byte-identical
replacement source/output refusal, damaged Forward/head/step proofs and rollback
at receipt deletion or Preparing save. Failed transitions retain Forward and its
receipt; removing the actual SQL refusal lets review safely continue.

source-return-inputs-01.json captures 864 Rust/Cargo/vendor/fixture paths. Only
source_removal.rs, store.rs, store/transfer_cursor.rs and transfer/copy_boundary.rs
changed from the previous captured source. The bounded selected-backend batches
verify inputs before/after every run. The library plus core-10.log through core-19.log pass 417 selected backend
parents across 20 targets, 31 ignored helpers, on unchanged inputs. All-target
Clippy with warnings denied, formatting and the captured schema41 fixture syntax/
hash pass. Final desktop results and exact parent/owner receipts are recorded in
source-return-verification-01.json. The isolated default library passes 185
parents, 11 ignored; all four new public parents and all earlier copied/terminal
parents pass on that source. Receipt continuation-core-1792698001-receipt.json
verifies all 864 inputs before/after the private D-Bus/HOME/XDG run, without a
display or live system bus. Both required stopped/active restart gates remain
unignored and current. Protected app, compatible Niri and earlier local-runtime
gate hashes remain unchanged. Do not infer native,
full workspace/release, Shell/Notify or physical-device acceptance from this scope.
The prior intermittent paused-video/seek issue remains undiagnosed.

The two original failure pairs are preserved in
source-return-red-fixtures-01.tar.gz, SHA256
ff2298ece0c3a0290c6d7b33d74a2c64850c0f79dc92e33ac7cfe6754bb4b703,
16 decoded verified entries. Its verification JSON records actual roots, metadata,
file hashes and durable-state probes. Current source/docs/proofs are preserved in
source-return-source-01.tar.gz; see its verification JSON for actual hash and
decoded member count. Original
failure roots must remain. Preserve source27 app, compatible Niri and the earlier
local-runtime restart gate. The current no-default gate may be rebuilt by Cargo,
but cache reclamation continues to exclude it.

Next: proof-owned in-process Retry Save for failure to record an engine end;
directory publication/child-queue and other task phases; complete Preparing/
Cleanup context binding; legacy loose-binding handling; queued lineage/admission;
large reports/paged owners; and sanctioned Undo/relocation/expiry. Trash/Restore,
renderer separation, product/device/accessibility acceptance, first-window/focus
reliability and performance, native/workspace/release/Niri integration and GTK
reference retirement remain in the full active goal.
