# Tokio daemon runtime

Notify uses Tokio for daemon plumbing such as D-Bus handling, private Unix socket IPC, provider communication, timers, and shutdown coordination. Pure notification policy and storage logic should remain testable without depending on Tokio where practical.
