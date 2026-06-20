# Niribar Notification Center Provider

Canopa Notify exposes Notification Center content through `panel-provider` mode. The provider talks to the running Notification Daemon through the private runtime socket, streams complete replacement Notification Center content while connected, and returns provider-visible error content if the daemon is unavailable.

Minimal command:

```bash
canopa-notify panel-provider --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock"
```

The provider emits the shared Canopa panel protocol from `canopa-panel-protocol`, including explicit protocol versioning and complete content replacement messages. **Panel Provider Mode** maps **Notification Center Projection** into generic panel sections, items, controls, metadata, and actions; Notify-specific meaning stays in the provider adapter.
