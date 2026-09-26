# 0004: Built-in notification summary and Center

Status: accepted for integration phase 5 (2026-09-26).

Niribar now has a `notifications` Module, placed in the default bar and panel. It
shows Notify's Requires Attention count, DND and explicit unavailability. Click or
keyboard primary activation opens the built-in `notifications` Center provider;
right-click or keyboard secondary activation runs Notify's existing atomic DND
toggle command. Configured actions and providers take precedence over built-ins.

`canopa-notify-client` is the small shared summary and session socket interface.
It depends on Tokio and serialization, with no storage, notification policy or GUI
dependencies. The bar never queries history to compute the count. Notify uses an
indexed count of retained Requires Attention rows and reads DND under its mutation
coordinator. Transient notifications never require attention. Expiry and restart
preserve retained attention; acknowledgment and explicit deletion remove it.

The private Unix socket request `summary-subscribe-v1` returns newline-delimited
complete snapshots containing daemon instance, attention count and DND. Frames
are limited to 512 bytes, with eight summary subscriptions inside the existing
64-connection cap. A watch channel coalesces changes; unchanged summaries are not
sent. Writers have a five-second deadline. Client connection and first snapshot
have a 900 ms deadline; partial frames have a five-second deadline, while a quiet
connected stream may remain idle. Invalid frames and disconnect invalidate the
displayed snapshot. The shell retries after one second and replaces its state
from a fresh snapshot, with one subscription shared by all module instances.

The default endpoint is `$XDG_RUNTIME_DIR/canopa-notify/control.sock`. An absolute
runtime directory is required; there is no shared temporary-directory fallback.
Daemon, CLI and Center retain `--socket` overrides. Built-in companion commands
prefer binaries beside Niribar and fall back to PATH. Center content continues
through Canopa panel protocol v4 with existing revision and daemon-instance guards.
An open Center ends on daemon loss; reopen it after recovery for a new session.
It does not automatically replay queued item actions against a new daemon.

The optional `canopa-session.target` starts both independent services. Target and
services are ordered after `graphical-session-pre.target` and before
`graphical-session.target`, avoiding a cycle through the enabling target. The packaged
user units use `~/.local/bin`; distributors may substitute their install prefix.
Each process can restart independently. Niribar restart rereads configuration and
reconnects to Notify; Notify continues delivering banners while Niribar is stopped.
No DB move, configuration live reload, protocol replacement or portal is introduced.

Validation covers summary meaning across lifecycle changes and restart, bounded
transport, shared shell source state, default actions/provider resolution, and an
isolated desktop session with the built-in module and independent restarts.
