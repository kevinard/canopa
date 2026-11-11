# Live notification IDs are not history IDs

The D-Bus adapter owns the session-scoped mapping from nonzero, unsigned Sender Notification IDs to Internal Notification IDs. No unrecognized wire value may reach the lifecycle as an internal replacement target. Internal IDs may refer to retained SQLite rows, negative runtime-only threads, or immediately dropped receipts; their numerical values have no wire meaning. The wire type stays adapter-local; the nonzero internal type is shared through receipt, intake, lifecycle, history, runtime and presentation/control targets.

Request decoding only decodes content: it must never cast a wire replacement ID into an internal identity. The adapter supplies a typed replacement target resolved under the receipt guard. This deliberately changes Rust library signatures to require Internal Notification IDs, while preserving signed numeric JSON and SQLite representations. Legacy stored replacement metadata of zero is read as no replacement; zero cannot identify an actual thread. Numeric conversion belongs at storage/control parsing, not between core modules. Runtime-only allocation uses checked arithmetic and reports exhaustion before producing a receipt or thread.

Replacement operates only on live threads. Closed, dismissed, expired and stale history is immutable through `Notify`. Fixed caller-supplied replacement tokens remain a transport compatibility behavior: a previously unknown token creates a new thread and subsequent live uses replace it. It never searches history for an application/token alias. This removes the former lifecycle fallback; callers of the library must use a receipt's internal ID to replace a live thread.

Replacement resolves one active ID through a fallible point lookup. Runtime-only
threads use their keyed store; retained threads use an ID/status query. Missing or
inactive IDs remain distinct from unreadable targets: a storage error fails the
request, while corruption in an unrelated active row cannot block replacement.
Bulk active reads remain reserved for operations such as clear-active.

Native desktop-entry remains an untrusted claimed application identity for grouping and rules, not a security principal (ADR 0002). Live IDs are session-global. This preserves freedesktop interoperability, including portal adapters that multiplex multiple application identities over one connection, without claiming sender authentication that the protocol does not provide.

The specification requires returning a nonzero replacement ID unchanged on success but does not fully specify unknown-token behavior. A live token replaces its current thread. A token that is not live, whether never issued or already closed, starts a new thread and is returned unchanged; later live uses replace that thread. Closed history is never revived. This follows dunst and keeps `notify-send -r` OSD scripts working after their notification expires. An earlier revision of this ADR rejected closed tokens with `InvalidArgs`; that broke those scripts on the second call, so it was reverted after the 2026-11 core-experience audit.

Automatic allocation never reuses an ID during one daemon lifetime, including after an immediate drop; only a caller that names a closed token reuses it. Exhaustion returns `LimitsExceeded` without mutating notification state instead of wrapping or waiting indefinitely. Committed IDs are remembered as a contiguous prefix plus merged sparse ranges; ordinary generated-ID churn needs no per-closure tombstones. At most 4,096 sparse ranges are admitted. Reusing a remembered token needs no new range. Caller tokens that would add another range fail before receipt, while automatic IDs can always extend the prefix until the numeric space is exhausted. Failed receipts consume no token. Preparation and registration stay inside the runtime mutation boundary; retirement happens synchronously at committed closure, before signal delivery can block or fail.

Client disconnection does not close notifications: short-lived senders and portal multiplexers retain the same session-global behavior. A daemon restart starts a fresh wire namespace and does not register retained history as live. Clients must treat a service-owner change as a new namespace, not carry old IDs across it. Trustworthy application identity and sender authentication remain separate, undecided work.

Reference: [Desktop Notifications 1.3, protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html).
