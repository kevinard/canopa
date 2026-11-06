# Phase79: Print resource findings; broader portal delivery takes priority

Full eighteen-interface goal remains ACTIVE. Print remains opt-in. Automatic Qt
integration remains deferred. The user's latest concern is that extended Print
work is stalling the broader goal. Treat that as a priority correction: **park
Print edge-case hardening and resume integration/delivery of the other portals**.
Do not automatically start another Print/FUSE matrix or a large process-launcher
redesign on continuation. These findings remain open before Print promotion.

There is no external blocker to broader work. The last turns overextended the
Print-first feature priority into increasingly deep failure validation. Core
Print features already have scoped native verification. Existing ten Files
workspace failures remain distinct from this investigation.

## Concrete next delivery work

Current shipped descriptors cover eleven interfaces (ten in canopa.portal plus
Notify). Six other implemented backends remain experimental/unadvertised:
ScreenCast, RemoteDesktop, Clipboard, InputCapture, GlobalShortcuts and Usb.
Prioritize desktop sharing: normal ScreenCast/RemoteDesktop integration and
explicit compatible compositor/frontend packaging/routing. Use current sources
and latest companion receipts, not stale historical 'running' notes in older
ledgers. ScreenCast is already served in main.rs; verify the separate
RemoteDesktop wrapper registration, which appears absent from the normal builder.
Do not advertise unsupported capabilities or alter live desktop selection.

## Deferred Print findings

No production Rust/Cargo/vendor changes or Cargo command occurred in this phase.
Normal backend remains47003b28, unit9cce9bee, integration0444f8ea. The new root
source is only scripts/desktop-fixture/print-stalled-fuse.c, a tiny controlled
read-only test filesystem. No system services, queues, routing or physical printer
changed. Every native test is terminal; all80 recorded PID/start identities were
audited absent. No fixture mount or private path remains from final cases.

Native capabilities: /dev/fuse opens; private user/mount namespaces work. Tests
run under unshare --user --map-root-user --mount, without a PID namespace, on a
private bus. A normal snapshot worker remains genuinely D/nonterminal after
SIGKILL until its controlled FUSE flush reply is released. Direct proof01 passes,
unmounts normally and exits cleanly. This is not a ptrace/SIGSTOP/ReapGate model.

The working public baseline uses a host GIO client, standard frontend277568 and
normal470 backend with the same configured state and connection across restarts.
Two blocked snapshot workers become four, then six, each retaining a2435-byte
PDF memfd. Normal backend shutdown stays about15seconds with public response2.
Client semaphores reset in each process; source snapshots have no shared worker
reservation/publication intent. Liveness misclassification is not the cause.
Release of the controlled filesystem replies retires/reaps all workers.

Authoritative receipts under target/portal-print-79:

- native-capabilities-01.json: device/private-namespace probe.
- fuse-counterfactual-01.json: observed-uninterruptible-flush, clean teardown.
- restart-baseline-01/summary.json: FAILED fixture cleanup; original 2/4/6
  observations retained. A final local Gio FD-list reference held the last mount.
  baseline-cleanup-audit-01.json confirms every identity/mount gone and removal
  of only that residual private directory. Do not reclassify as a pass.
- restart-baseline-02/summary.json: clean observed-cross-restart-growth,2/4/6;
  bounded_admission_established=false. All mounts/processes/private data retired.
- bounded-red-03/summary.json: FAILED wrong-symptom timeout with broader child
  close selector, not the resource-budget regression. Clean teardown.
- preexec-04/summary.json: observed-pre-exec-source-close. Held child still has
  the backend argv; syscall59 execve waits in request_wait_answer. Two backend
  runtime threads wait in recvfrom. Settings Read stays pending during the hold,
  succeeds after release, then shutdown exits0 in~37ms. Clean teardown.
- bounded-red-05/summary.json: correct FAILED budget regression: after two normal
  backend epochs four retiring jobs exceed the proposed normal-job limit of two.
  Worker-only selector isolates it from the earlier pre-exec condition. Clean
  teardown, all identities terminal, all mounts removed.

Two distinct unresolved conditions therefore exist: cross-restart retained-job
admission, and arbitrary inherited-FD close during helper exec blocking launch.
The latter means registering a worker only after it has inherited application
FDs is insufficient. A future fix needs to address the process/descriptor seam,
not merely add a fresh semaphore. Preserve both regressions and do not present
the worker-only test as complete filesystem-wait acceptance.

## Replay/source/storage notes

Current public-restart.py supports --source-scope worker|children,
--assert-bounded, --release-each, --rounds2|3 and new --out. The release-each
control and fuse-counterfactual-current.py were prepared but NOT RUN. Each actual
case retains its exact probe-source and pins; do not overwrite old case dirs.

C helper versions are in the checksum store, with build01/02/03 receipts:
BA5(17448B, original all-flush),513(17744B, worker argv filter),D39(18000B,
optional backend-parent filter). The exact older C sources are retained as
fuse-before-worker-filter.c and fuse-before-parent-filter.c. Current D39 supports
both isolated worker and broader child selectors. These tiny raw baselines remain
available for the unresolved findings; no large executable was copied per case.

All fixtures release on stdin/EOF/deadline and server parent death. The corrected
runner clears the final local FD-list alias and registers cleanup before temporary
directory teardown. Native host audit found zero of80 recorded identities live.
About5.5GB remains free. No rebuild, new Cargo graph, cache purge or SDK growth.
