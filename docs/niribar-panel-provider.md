# Niribar Notification Center Provider

Canopa Notify exposes Notification Center content through `panel-provider` mode. The provider talks to the running Notification Daemon through the private runtime socket, streams complete replacement Notification Center content while connected, and returns provider-visible error content if the daemon is unavailable.

Minimal command:

```bash
canopa-notify panel-provider --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock"
```

The provider emits the shared Canopa panel protocol from `canopa-panel-protocol`, including explicit protocol versioning, complete content replacement messages, grouped Notification History, App Permission Controls, and interaction events for clearing active notifications, deleting history, and invoking active notification actions.
