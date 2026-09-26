# UI-managed rules use a user rule store

Notify stores rules changed through its UI or control commands in a daemon-owned user rule store instead of rewriting the main KDL configuration. This keeps hand-authored configuration stable while still supporting mobile-style app permission changes from the notification center.

The store supports app-wide rules and rules scoped to one valid Notification Category, each optionally scoped by Notification Urgency. Notification Center category controls write category-wide rules and continue to expose an app-wide fallback.
