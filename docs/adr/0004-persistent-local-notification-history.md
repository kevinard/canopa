# Persistent local notification history

Notify persists notification history locally by default. This favors a reliable notification center and auditable rule behavior over restart-scoped privacy, with retention policy expected to bound how much history remains available.

The freedesktop `transient` hint is the explicit exception: a thread created as transient remains runtime-only across replacements and leaves no notification row after termination or daemon restart. A transient replacement hint cannot remove an already-retained thread from history. The initial Notification Center exclusion is superseded by [ADR 0041](0041-runtime-only-transient-center-actions.md): live transient threads have an explicitly runtime-only action surface without entering durable history.
