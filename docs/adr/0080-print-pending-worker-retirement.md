# Print replies while worker retirement is pending

Status: accepted; scoped retirement/ownership verification; Print unregistered

## Context

Moving file I/O into workers keeps it off backend request threads. Sending a
signal still does not establish that a worker has exited. Previously the task
runner awaited `Child::kill`, which also waits for exit, and shutdown waited for
all operation permits without a deadline. An unconfirmed exit could therefore
hold a request or shutdown indefinitely.

The [Tokio Child contract](https://docs.rs/tokio/latest/tokio/process/struct.Child.html#method.start_kill)
separates starting termination from waiting for it. A bounded caller response
must not release the reservation or permit another publisher/settler to act
while the original worker might still execute.

## Decision

Retain the independently owned operation task. It continues to own its child,
operation permit, frozen storage, file consent and any staged O_PATH bundle.
After the existing cooperative deadline/grace, start SIGKILL and wait two seconds
for exit confirmation. If confirmation remains pending, report that fact to the
caller through a separate watch channel and continue waiting for the same child.
Observation errors are retried; they never establish death or release admission.
Normal final replies take priority if they race with a conservative pending notice.

Only positive receipts can establish success. An accepted CUPS job remains
accepted, and a published file (or positively reconciled settlement) remains
published. Otherwise the caller receives an unconfirmed-result error and must
not retry automatically. File notices carry an original-location recovery hint
without consulting the filesystem. The owned task does not start file settlement
or partial-job cleanup until the publisher's exit is confirmed. Eventual cleanup
retains its original operation permit and is allowed after shutdown cancellation.

The two operation slots and document/derived-body budgets stay reserved until
retirement and cleanup finish, even though the original request already returned.
Additional requests cannot create replacement helpers outside those budgets.
Shutdown closes admission, signals active stages and waits at most fifteen
seconds, then returns and logs the pending-operation count. Pending tasks continue
under their original ownership while the runtime lives. Printer metadata refresh
also has a fifteen-second readiness wait, so the Iced dialog can report temporary
unavailability instead of waiting forever for a previous operation.

This bounds caller waits for direct-worker retirement; it does not force Linux
to complete uninterruptible I/O. Across backend process exit/restart, named journal
and recovery material remain evidence, not authority for replay. Runtime disposal,
converter descendant supervision, cross-restart resource accounting and actual
remote/kernel-wait acceptance remain part of the broader recovery/distribution
work. The [Tokio runtime shutdown contract](https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html#shutdown)
does not promise completion of ordinary spawned tasks when the runtime is dropped.

## Validation scope

Formatting, strict workspace/all-target Clippy and original-graph all-target
compilation pass. The harness passes 99 Print and 12 Lockdown tests; full Portal
regressions pass 317 library and 55 integration tests (17/2 ignored). New tests
check terminal reply priority, both occupied slots, no third helper, bounded
shutdown and eventual reservation release.

A test-only per-client barrier delays confirmation of
real worker exits; it is compiled out of production and has no environment or CLI
switch. The OS child can already be terminal. This seam checks bounded replies,
resource reservations, no settlement before confirmation and eventual cleanup
after bounded shutdown. It is not presented as a real kernel D-state test.

Three native confirmation-barrier groups pass with the actual normal worker.
Two pending snapshots return in about 4.43 seconds while retaining both document
and operation budgets. Known publication returns committed output in about
4.62 seconds; an actual commit with discarded reply returns an uncertain error
in about 2.40 seconds. Shutdown returns after about 15 seconds with reservations
still held. Operation traces show no file settler before confirmation is released
and exactly one settler afterward, even after shutdown. Staged document and
previous-file bytes remain intact before settlement; eventual cleanup completes.
All ten observed fixture processes are terminal.

The existing six real FIFO/publication groups (15 observed processes), 24 native
production renderer/storage/cancellation groups (115 processes), and 36 public
canonical frontend/Production/Iced/private-CUPS groups (172 processes) pass on
the changed inputs. Both appearances, 22 actual printer deliveries, live GTK and
explicit Qt DBus/PDF clients remain correct. All 312 observed processes are
terminal, with no emergency fixture-cleanup kills. Guarded inputs and binary
pins match; startup linkage has no GTK, CUPS or qpdf dependency.

The authoritative receipt is `target/portal-print-39/acceptance-01.json`, indexed
by `.scratch/xdg-portals/print/retirement-evidence.json`. The original incremental
graph/cache is reused, with measured phase net growth about 173 MB. No new Rust
native driver, dependency graph, package dependency or copied executable was added.
No new full workspace test gate ran; ten recorded Files integration failures
remain. Print registration, startup recovery/index/UI, ambiguous CUPS delivery
and the other remaining eighteen-interface gates are unchanged.
