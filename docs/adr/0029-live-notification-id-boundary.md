# Live notification IDs are not history IDs

The D-Bus adapter owns the session-scoped mapping from nonzero, unsigned sender IDs to internal Notification Thread IDs. No unrecognized wire value may reach the lifecycle as an internal replacement target. Internal IDs may refer to retained SQLite rows or negative runtime-only threads; their numerical values have no wire meaning.

Replacement operates only on live threads. Closed, dismissed, expired and stale history is immutable through `Notify`. Fixed caller-supplied replacement tokens remain a transport compatibility behavior: a previously unknown token creates a new thread and subsequent live uses replace it. It never searches history for an application/token alias. This removes the former lifecycle fallback; callers of the library must use a receipt's internal ID to replace a live thread.

Native desktop-entry remains an untrusted claimed application identity for grouping and rules, not a security principal (ADR 0002). Live IDs are session-global. This preserves freedesktop interoperability, including portal adapters that multiplex multiple application identities over one connection, without claiming sender authentication that the protocol does not provide.

The specification requires returning a nonzero replacement ID unchanged but does not fully specify unknown-token behavior. The compatibility policy above must remain separate from durable history addressing. Closed-ID retirement and reliable signal delivery are coordinated at the transport boundary, not by scanning persisted rows.

Reference: [Desktop Notifications 1.3, protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html).
