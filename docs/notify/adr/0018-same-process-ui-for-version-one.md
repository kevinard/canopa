# Same-process UI for version one

Notify runs D-Bus receipt, storage, policy, private control IPC, and iced/layer-shell banner presentation in the same daemon process for version one. This avoids premature process IPC and lifecycle complexity while leaving crate boundaries available for a future UI helper split if event-loop constraints require it.
