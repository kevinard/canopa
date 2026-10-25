# 0068: Durable retained-source ownership

Status: implementation in progress, 2026-10-18. Continues ADR0056/0067 and the
entire PRD; this is not acceptance of restored execution or complete ownership.

A saved request must keep the retained versions it still uses. Request payloads
are scheduling/presentation data, not ownership. Add core-owned reservations for
identified retained containers, captured off the UI thread through the public
operation-engine interface. Capturing returns an opaque, history-bound handle;
the renderer cannot invent container identities by decoding a path or payload.
Ordinary files and similarly named unowned folders do not become retained data.

Accept a request and its reservations in one transaction. Refuse a stale capture
when the catalog owner disappeared or destructive journal work already started.
Cleanup and consuming Undo intents check the same reservations before their
journal transaction commits. Thus acquisition and destruction have one ordering,
including while the mutation worker waits for a user decision.

Unfinished requests retain reservations through restart and failed saves.
Finishing or dismissing releases them atomically with the request update.
Retry/Continue handoff copies ownership to its successor before releasing the old
row; an unselected plan keeps its original protection. Conservatively sharing a
reservation between stopped plans is allowed until both finish or are dismissed.
This may retain data longer, but must not erase a still-needed source.

The core's existing physical-stage migration must move reservations together
with catalog ownership. A reservation protects retention; it does not authorize
executing a changed source, adopting another mount, or replaying a Running task.
Original placement, exact plan/journal association, descriptor authority and
explicit reconciliation remain required gates before restored execution.

Include reservation storage in the existing request budgets. Acquisition or
handoff failure leaves previous records and ownership unchanged. Do not infer
ownership from a saved directory's name, and do not silently delete incompatible
records. Older request rows acquire no retrospective source authority.

Verification uses the already established public seams: OperationEngine capture,
request admission/checkpoint/handoff/dismissal, actual transfers/Undo/Recovery,
and private filesystem bytes. Exercise history expiry, reopening, failed/stale
updates, independent owners and the acquisition/destruction ordering. No tests
query private ownership tables to prove retention.

The initial integration is transfer admission in the shared Iced operation
service. It captures selected sources on the metadata worker and reserves known
catalogued containers with the saved request. Directory selections include owned
containers below them; ancestor aliases resolve, while a final symbolic link
retains its link semantics. Reservations are schema 21 metadata and count toward
the existing logical request budgets at 64 bytes per container. Older binaries
refuse this newer schema rather than executing requests without protection.

Public checks cover expiry/restart/dismissal, cross-filesystem container
migration, handoff with and without an unselected plan, stale revisions,
foreign/consumed captures, storage-limit rollback, alias/link/lookalike selection
and admission during a live Undo review. A service regression reproduces expiry
of a stopped transfer's source before integration and verifies its retention
through other jobs and reopening afterward.

This is protection against automatic expiry and consuming Undo of identified
saved versions. It is not complete source ownership: ordinary Rename/Trash and
other consuming transfers, containers created after capture or still in a
forward journal, saved-plan path rebinding after migration, original media and
directory-descriptor authority, and crash reconciliation still need integration
and verification. Keep restored plans review-only until those gates pass.
