# Bounded retention of undisplayed and pending notifications

Status: accepted (2026-11). Amends ADR 0028, 0032 and 0049.

## Context

The 2026-11 core-experience audit found that a long session can stop accepting
notifications from every application. Under ADR 0028, overflow notifications,
**Silenced Notifications** and requests accepted under **Do Not Disturb** never
began a **Notification Timeout**, so they stayed live indefinitely and counted
against the 1,024 live-thread limit of ADR 0032. Under ADR 0049, expired but
unacknowledged content stayed pending indefinitely and counted against the
4,096-item and 64 MiB pending budget. Past either limit, every application got
`LimitsExceeded`, not only the noisy one. A day of DND, or one "Save quietly"
application, was enough.

## Decision

Undisplayed notifications expire the way their displayed counterparts would:

- A silenced revision (DND or a Silenced rule) begins its timeout when it is
  accepted, because it never gets a banner.
- An overflowed revision begins its timeout when it overflows. Notify still does
  not promote overflow notifications when a slot frees.
- The timeout value is unchanged: the sender's positive value, or the configured
  urgency default for the server default. Zero and critical defaults never expire.
- Expiry has the usual meaning: `NotificationClosed` reason 1, sender callbacks
  end, and retained content keeps its attention state (ADR 0049). Center actions
  for such a notification are available until it expires, as for a banner.
- **Headless Mode** displays nothing and therefore still expires nothing.

Full limits retire old work instead of refusing new work:

- When the live-thread count is full, admitting a new thread first expires the
  oldest non-critical live thread, by the admission order of its current
  revision. The text and image budgets are checked as if that thread were already
  gone, so a request that would still fail retires nothing. Replacements and
  policy drops need no slot and retire nothing. Only a live set of critical
  threads refuses with `LimitsExceeded`.
- When a pending write exceeds the 4,096-item or 64 MiB pending budget, the oldest
  inactive non-critical pending rows (by last update) are archived in the same
  transaction: their attention is acknowledged, and ordinary archive pruning
  (30 days, 1,000 rows by default) then applies to them. Active and critical
  pending rows are never archived this way. Only a budget they fill refuses the
  write, atomically.
- Both retirements log a content-free warning
  (`event=live_limit_retired_oldest`, `event=pending_limit_archived count=N`).

The live text (16 MiB) and raw-image (64 MiB) budgets keep refusing, because they
bound memory rather than history length.

## Consequences

This supersedes ADR 0028's rule that no-display cases remain active
indefinitely, ADR 0032's "not a successful drop or eviction of existing work"
for the live count, and ADR 0049's "No existing pending item is silently evicted
to admit another". Pending items still stay discoverable until acknowledgement,
explicit removal or this bounded archiving of the oldest non-critical ones.
Critical content is never retired to make room. The Center does not yet warn
that unread history was archived.
