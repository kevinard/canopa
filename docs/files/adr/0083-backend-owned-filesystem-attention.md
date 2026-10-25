# 0083: Backend-owned filesystem attention and acknowledged delivery

Status: scoped verification, 2026-10-19. Continues ADR0074/0076/0081/0082.

OperationJobs owns semantic attention records for new operations: exact-generation
decisions, save failures and stopped outcomes. A unique observer claims the queue's
coalesced notices before the host exposes it. Historical results loaded from disk
do not emit new completion notices or acquire execution authority. The existing
typed answer, source/media checks and review-only restart rules remain in force.

Snapshots alone cannot guarantee this workflow: a short-lived state can be
coalesced, and finished rows are trimmed to one hundred. The attention inbox
instead retains the latest unread state per operation independently of those rows.
A resolved question or successful save retry retires its obsolete unread notice;
already handed deliveries retain their independent responsibility.

Admission refuses at 1024 outstanding operation identities, counting active work
and unread or handed attention without double-counting an identity. At most
seventeen receipts can be handed concurrently. The inbox has at most 1024 unread
records and seventeen handed records; additional generations for one identity
replace its unread record. Delivery uses the existing bounded standard desktop
notification transport. Notification receipt, policy and presentation stay with
Notify.

Taking a receipt keeps the queue's hold. Only explicit acknowledgement of that
opaque receipt releases it; a dropped observer or lost frontend never silently
acknowledges delivery. Phase/result publication and attention registration share
the snapshot lock. Even direct OperationJobs shutdown therefore refuses while
attention delivery is unacknowledged. This replaces the initial tracer's weaker
take-and-release handoff, whose real shutdown gap was reproduced by a test.

OperationAttention owns the observer and transport replies without a renderer.
It records actual success or a visible refusal before acknowledging. It retains
at most one hundred per-job errors and one error outside those rows. Its event
loop waits on queue notices, replies and final-owner drop; bounded backlog draining
also observes owner drop. The background-operations feature builds it without
Iced. LocalOperations creates it alongside volume observation and OperationJobs
before Iced initialization.

Iced observes host snapshots and notices. It retains invocation drafts, source
browser refreshes, clipboard presentation and generation-bound question dialogs.
It no longer detects attention transitions, schedules notification sends or owns
delivery holds. Application exit consults the live backend hold rather than a
potentially stale view snapshot.

The public core/platform seams delegated by ADR0001 use real private files,
directories, Undo and queue shutdown. Four attention tests and the existing queue,
startup, storage, transfer, retained-source and clipboard workflows pass 81 parent
tests, with additional isolated subprocess checks. They cover the handoff gap,
exact decision generations, acknowledgement independence, row trimming,
1024-operation backpressure, seventeen concurrent receipts and no history replay.
The real save-failure test acknowledges all attention while only unsaved results
remain; shutdown still refuses. Restoring the history path and retrying its saves
never executes the cancelled request or changes later file bytes.

Private-bus acceptance passes eight checks across actions and filesystem attention,
without a display or GUI initialization. The operation receiver withholds a real
Notify reply after a directory has been created; direct queue shutdown refuses
until release. A later unanswered Undo reports attention, preserves later work
and blocks queued creation until its exact answer. All four actual notifications
settle before orderly shutdown. Each workflow builds only its background feature,
with no Iced artifacts. The separate action case retains its twenty-command burst.

The default build, formatting, all-target Clippy (three existing warning groups)
and 47 unfiltered Iced library tests pass. An earlier directory-name filter ran
zero tests; that result is explicitly excluded. Binary88a116cd passes 39 graphical
checks across operations in both themes and light-theme actions. Both operation
logs are empty; the action log has only four expected unavailable GIO remote-monitor
messages on the isolated bus. Representative source-closure, question and running
command captures were inspected. Standards and Spec reviews each report zero
scoped findings. Final source12 changes documentation only; evidence is recorded in
`.scratch/file-manager/operation-attention-verification.json`.

## Atomic bounded completion publication

The Trash/Restore work in ADR0084 exposed a completion-publication gap in the
existing attention stress test: a concurrent reader could briefly observe 101
Finished rows. Phase, semantic attention and trimming to the newest 100 Finished
rows now share one snapshot lock. Internal job cleanup follows that publication;
the attention inbox still retains its independent delivery holds. The actual
stress failure, minimized concurrent-reader reproduction and clean regression
results are recorded in `.scratch/file-manager/trash-queue-verification.json`.

The original scoped extraction above does not implement resident launch, primary-instance activation, frontend
process isolation, durable continuation, safe Retry/Continue or full workspace,
packaging/release acceptance. The independent first-window timeout remains open.
