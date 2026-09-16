# Retained attention and sender lifecycle

The selected Desktop Notifications 1.3 profile includes persistence. A banner's
expiry closes its sender lifecycle with reason 1; it does not acknowledge the
content. Retained notifications therefore have a separate `requires_attention`
state. This supersedes ADR 0004's use of all inactive rows as a prunable archive.

New retained content requires attention, including notifications silenced by DND
or a user rule. Replacement resets attention for the new content. Banner expiry
and restart preserve attention. Restart still marks active sender lifecycles stale
and disables their callbacks. Transient notifications retain no durable attention
state, including across replacement, and explicit policy drops create no history.

Deliberate dismissal, sender recall, and invoking a sender action acknowledge
content. Resident actions acknowledge without closing their sender lifecycle.
The Center offers acknowledgement of expired or stale content without issuing a
second closure or a stale sender action. Revision and daemon-instance guards also
apply to acknowledgement. Merely opening the Center or following a body link is
not acknowledgement. The existing explicit delete-history command removes all
inactive rows, including pending ones; it is a deliberate bulk removal operation.

Only inactive, acknowledged rows are subject to archive age/count pruning.
Pending rows stay discoverable until acknowledgement or explicit removal. Admission
must bound pending count and serialized bytes separately from live notification
resources; exceeding that budget rejects the new receipt atomically, preserving
previously accepted content and replacements. No existing pending item is silently
evicted to admit another.

Schema migration treats existing active rows as requiring attention and preserves
existing inactive rows as archive. The earlier version promised historical storage,
not a pending-attention queue, so migration must not fabricate unread state for
previously closed history. Capability advertisement remains gated on executable,
storage, and Center evidence.
