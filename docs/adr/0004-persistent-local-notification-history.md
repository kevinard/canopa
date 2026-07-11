# Persistent local notification history

Notify persists notification history locally by default. This favors a reliable notification center and auditable rule behavior over restart-scoped privacy, with retention policy expected to bound how much history remains available.

The freedesktop `transient` hint is the explicit exception: a thread created as transient remains runtime-only across replacements, does not appear in the first Notification Center, and leaves no notification row after termination or daemon restart. A transient replacement hint cannot remove an already-retained thread from history.
