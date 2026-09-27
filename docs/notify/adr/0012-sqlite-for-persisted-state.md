# SQLite for persisted state

Notify stores notification history and UI-managed rules in one per-user SQLite database accessed through `sqlx`. SQLite provides durable local queries, replacement updates, retention pruning, and atomic rule changes without inventing multiple custom file formats. Database migrations exist from the start because history and UI-managed rules are user data.
