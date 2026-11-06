# Canopa polkit agent

An unprivileged, event-driven Wayland authorization agent for the user's graphical
logind session. Reuses Canopa session presentation and erased input; native polkit
owns credentials, its helper/PAM stack and authorization receipts. No window is
mapped while idle. Supports authority-selected identities, interactive prompts,
device factors, cancellation, request details and session-scoped systemd readiness.

See [setup, interaction and validation](../../docs/polkit.md).
