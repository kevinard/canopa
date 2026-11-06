# Print worker resources across an unconfirmed exit and backend restart

Status: needs-triage

Defer behind broader portal delivery following the user's priority correction.
Phase79 now reproduces retained-worker growth and a separate pre-exec descriptor
close blockage. See [the checkpoint](../PHASE79-LIVE.md) for exact successful,
failed and wrong-symptom receipts. The correct budget regression is bounded-red-05;
it fails at four retained jobs after two backend epochs. Full eighteen-interface
scope stays active, Print opt-in, automatic Qt integration deferred. Do not repeat
completed Print matrices by default on continuation.

## Current evidence and gap

`print/worker/client.rs` constructs two operation permits and two document permits
per Client. Clones share them; derived storage retains its original body budget.
`snapshot` allocates a sealed-body memfd and an operation before any publication
intent exists. Its worker Operation::Snapshot has no recovery registration.
These are source facts, not proof of a leaked worker or an unbounded process.

`print/worker/client/retirement.rs` retains guards until actual child exit and
releases the caller through a pending notice. `task.rs` bounds stop/reap waits,
then keeps owning the same child. Existing ReapGate tests deliberately delay
confirmation of a child that can already have exited. They prove the software
contract, not a process stuck in uninterruptible kernel I/O.

[ADR0080](../../../docs/adr/0080-print-pending-worker-retirement.md) explicitly
leaves runtime disposal, converter descendants, cross-restart accounting and
actual remote/kernel waits open. Later printer/file journals and pidfd ownership
must be considered before proposing a new mechanism. Current local SIGKILL
publication checks prove ordinary parent-death cleanup, not the pending-exit case.

## Investigation and acceptance

1. Inspect current normal-service, worker, renderer, recovery and packaging
   ownership together. Identify existing process/cgroup/global limits and their
   scope. Do not infer a leak solely from per-Client semaphores.
2. Establish a controlled native pending-exit counterfactual. Check the native
   host's actual FUSE/device capabilities; tool sandbox visibility is not host
   evidence. Use only private mounts/processes. A stopped worker, ptrace hold or
   ReapGate must not be described as an uninterruptible kernel wait.
3. Observe original caller completion and bounded normal shutdown while the
   exact worker's PID/start/pidfd remains nonterminal. Verify no settlement or
   destination mutation before confirmed exit, and eventual retirement after
   the controlled obstruction is released.
4. Restart the actual normal backend with the same private state while the old
   worker remains unconfirmed. Measure aggregate helpers and retained bodies,
   including source snapshot before a publication journal and converter
   descendants. State the intended finite admission policy and verify it across
   repeated restarts. Preserve live/unrelated ownership; never retry documents.
5. If a real gap is reproduced, retain the baseline and diagnose before editing.
   Add the ownership seam needed to enforce the policy, regression-test it, then
   run the appropriate focused checks and one required original workspace gate.
   Keep the existing ten Files failures explicit; do not change the Cargo graph,
   disable incremental compilation or purge caches.
6. Own a release/abort path for every obstruction and private mount. Confirm all
   native processes, requests, descriptors and mount paths retired. Reuse the
   pinned executable/compositor/Flatpak stores and storage/free-space budgets.

If the environment cannot create the required native kernel condition, record the
actual refusal and continue independent Print work. An unavailable physical
counterfactual is not permission to relabel a software barrier as equivalent.
Power-loss, physical printers and release/default routing remain separate gates.
