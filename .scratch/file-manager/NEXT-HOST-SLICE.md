# Next host implementation slice

Read-only planning during frozen clipboard-capacity validation, 2026-10-19.
Nothing in this note implements or advertises a daemon or activation flag. The
whole PRD and ADR0074/0076 remain authoritative. Prefer advancing actual host
ownership next, rather than treating unrelated safety investigations as a new
prerequisite for every host step.

## Proposed first process layout

One owning local application process initializes backend ownership before Iced,
with lazy GUI initialization on the first Open request. A private Unix socket
activates that owner. Optional residency starts with no Browser and can keep the
owner idle after all windows close. This matches the requested windowless lifetime
without requiring a full remote widget/operation protocol. Core queues remain
usable without desktop features; linking Iced does not justify initializing a
display during headless startup. An independently compiled headless executable is
a separate packaging gate, not implicit evidence from zero windows.

This initial layout does not isolate backend work from a frontend process crash;
the owning process is still interrupted. A later separate frontend remains viable
at the same typed core seams. Record the layout/transport decision in an ADR before
implementation; do not silently reinterpret ADR0076 or claim crash isolation.

## Complete ownership boundary

- Move volume-monitor startup, initial-observation ordering and its lifetime out
  of `Desktop::start_file_operations`/`file_operations::State`. Backend starts
  OperationJobs only after that original observation; monitor loss still refuses
  unsafe operations. Use the existing Storage seam and real private provider.
- Own OperationJobs and ActionJobs before frontend attachment, retaining handles
  across frontend/window closure. A forwarding-only wrapper that still waits on
  Iced Ready/Poll callbacks is insufficient.
- Move configuration worker lifetime out of `iced_desktop/configuration_ui.rs`.
  Its domain ConfigurationRuntime is already reusable; the current worker loop,
  bounded mailbox, ordered writes and acknowledgements still depend on UI-owned
  Worker Drop and subscription. Retain guarded source/version save authority.
- Move native notification delivery/holds and operation attention bookkeeping
  outside Iced. Core action receipts cannot be silently discarded at frontend
  detach; pending notification transport must have a bounded ownership/handoff
  policy. Operation attention can use core job/decision generations, while UI
  refresh and originating Tab callbacks stay presentation concerns.
- Treat native clipboard transport as a platform session owner. Its opaque
  PreparedSelection remains server-owned and tied to its original runtime. Never
  serialize its observations/retention token as restart or replay permission.
- Browser/read/search/preview worker sessions may close with their frontend. They
  must not cancel admitted operations or accepted custom commands.

## Local activation boundary

Owner-only lock/socket under private XDG_RUNTIME_DIR, no-follow lock open and
flock before opening history or mutable state. Follow existing Shell/Notify
ownership patterns; stale socket recovery only for a refused socket owned under
the lock, and cleanup only the original inode. Concurrent cold starts elect one
owner. Bounded, versioned requests preserve Unix path bytes and fail visibly for
incompatible owners. Do not create a second history writer to work around an old
installed process. Explicit configuration/state mismatch needs a clear policy.

Activation only requests UI opening; an in-process Iced adapter can consume the
same backend handles. Do not introduce an enormous serialized Snapshot protocol
just to open a new window. Window-open acknowledgement must reflect actual GUI
startup failure; queueing a request is not proof that a Browser became visible.

Ordinary lifetime and resident idle policy are separate. Orderly shutdown refuses
accepted work/unsaved results and preserves pending decisions. Once Iced has been
initialized, keep its zero-window event loop available for warm residency; do not
attempt to reinitialize the native event loop repeatedly in one process.

## Acceptance sequence

1. A public backend workflow runs real private operations and commands before any
   renderer is attached. Detach/reconnect retains the exact decision generation,
   progress and original observations. Tests stay at existing operation/action/
   configuration/storage seams under ADR0001.
2. Native private session starts the owner without DISPLAY/WAYLAND_DISPLAY,
   verifies no window or GUI initialization, activates into the same owner,
   closes all windows during actual work/question, then reopens that same queue.
3. Simultaneous launches, stale sockets, incompatible versions, idle/active orderly
   shutdown, updater compatibility and explicit configuration identity.
4. Measure idle CPU/memory and warm-open latency; do not promise speed from
   architecture alone. Add a root integration ADR before launcher/service/updater
   packaging changes. Keep restart reconciliation/execution authority separate.

## Constraint found during configuration extraction

This remains a proposal. Local Winit currently chooses and opens the display
from the process environment and exposes no supplied Wayland connection. See
RUNTIME-HOST-INVESTIGATION.md's display-free activation note before choosing
one-process hosting. A no-display owner that already has workers cannot safely
accept a later display by casually changing the process environment. Resolve
that constraint explicitly; no vendor edits or process-layout ADR are accepted
yet. ConfigurationJobs is now extracted independently of that choice (ADR0080).

Next implementation should advance the actual platform/runtime ownership gap
(volume initialization before OperationJobs, retained owner handles, notification
attention/handoff). Do not add unrelated filesystem investigations as implicit
prerequisites for host work. Existing core admission/source/continuation limits
remain authoritative; no new host may enable replay from presentation payloads.
