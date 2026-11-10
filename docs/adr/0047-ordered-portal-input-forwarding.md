# Ordered input forwarding in the standard portal frontend

Status: scoped native compatibility candidate; distribution and broader acceptance
remain open, 2026-10-31.

Update, 2026-11-10: commit 8ede03f packages the patched frontend at
`/usr/lib/canopa/xdg-desktop-portal` and selects it in the dedicated Canopa
session through `packaging/system/canopa-portal-frontend`. Statements below
that the installed frontend is unchanged describe the state when this was
decided; other sessions still use the distribution frontend.

## Context

Canopa keeps the standard XDG frontend's caller, sandbox and document policy and
implements desktop backends and UI in Rust/Iced. RemoteDesktop input order must
survive that frontend before the backend's session actor can preserve it.

The installed frontend 1.22.1-2 and a source-built, unmodified 1.22.1 both shuffle
concurrent legacy Notify calls. The public bus trace preserves the caller's order;
forwarded calls and received key transitions do not. Instrumentation locates the
first swaps at worker entry and further swaps before handler entry. Proxy sends
preserve handler order. A mutex around the handler does not restore receive order.
The original 32-event burst fails on the baseline and passes on the candidate.

Serializing the entire interface would put synchronous session setup and EIS
operations on the frontend loop. Dispatch must preserve input order while keeping
those operations asynchronous and separating independent sessions.

## Decision

Maintain the explicit `xdg-desktop-portal-input-order` source overlay, pinned to
the official 1.22.1 release archive. Only `remote-desktop.c`, `xdp-context.c` and
`xdp-context.h` change; original attribution and the LGPL licence remain recorded.

The RemoteDesktop vtable admits Notify calls in receive order. A valid session
already initialized its caller's identity, so this path uses a cached identity
lookup and retains it for the invocation lifetime. Session ownership is checked
before admission. A short, separate mutex protects the admission grant and queue;
the frontend loop does not acquire the session mutex or perform metadata I/O.

Each session has one FIFO worker. It invokes the original generated handler,
which rechecks current ownership, grants, options and input mode. Successful Start
enables admission; closure and EIS handoff disable it. Queued pre-consent input is
denied. Other RemoteDesktop methods retain worker-based authorization and request
initialization. Pending consent in one session does not stall another's input.

There are at most 64 pending input messages per session, each with a body of at
most 4 KiB and no attached FDs. Exceeding these limits retires that owned session
before draining queued work. This prevents a discarded release from leaving a
held key active. Rejected FDs are closed with their invocation. The original
handlers remain responsible for method-specific validation and forwarding.

Use the root source-overlay verifier and `build-xdg-portal-compat.py` to prepare
and build the companion without installation. The builder verifies every source
input, records recipe/tool bindings and binary hashes, and reuses its Meson/Ninja
cache explicitly. An empty generated Meson subproject lock is accounted for apart
from source inputs. No Cargo profile, application-local fork or live routing
changes are part of this recipe.

## Evidence and remaining acceptance

The source baseline's concurrent 32-event burst fails in 28.29 seconds. The
instrumented baseline also fails and identifies worker scheduling as the cause.
The candidate passes the original burst in 17.98 seconds, then 14 native groups
in dark/100% and 14 in light/125%. The latter uses NoReplyExpected input. Both
include 12 real monitor consumers and 14 terminal input workers. Foreign callers
are denied; automatic identity initialization, independent pending consent,
body/FD retirement, caller/worker/backend loss and legacy-to-EIS handoff pass.

The same 14 groups pass under address and undefined-behavior sanitizers with no
memory-error report. Explicit inspection confirms the rejected pipe FD closes in
the frontend. Leak scanning of global GLib caches is excluded from that check.
The original oversized-body and attached-FD behavior is retained as verified
failing controls. The canonical recipe verifies 499 source files and reproduces
the exact three patched file hashes. Integrity probes reject altered source,
undeclared files, a symlinked source root and a nonempty generated lock file.
Native screenshots and exact fixture sources are archived with the binary pins.

Canonical source replay/build checks, the required workspace gate and current
receipts are recorded in the portal ledger and recipe candidate metadata. The
upstream Python suite has not run: pytest, dbusmock and UMockdev are unavailable.
The installed frontend remains unchanged. Pointer/touch and physical-input
acceptance, distribution provisioning and the broader eighteen-interface goal
remain open; RemoteDesktop stays unadvertised.

The canonical frontend also passes private Settings activation/read/reload and
12 FileChooser native groups per appearance. A diagnosed fixture correction keeps
its Shift device alive until selection and waits for the Wayland roundtrip; the
old fixture fails identically on the original and candidate frontends. The
post-adoption workspace gate remains red on the same ten Files integration
failures, with 2,024 inputs unchanged; tooling/fmt/Clippy/all-target compilation pass.
The subsequent native fixture correction changes test tooling only. Receipts
retain both controls, the modifier-state probe and final portable fixture runs.

All unreached shared stages were continued: remaining Rust tests, Settings, CLI
and vendor checks pass. Configuration passes after rebuilding its missing normal
Files executable, with source inputs unchanged; the initial prerequisite failure
is retained. The full workspace gate remains red on the same ten Files tests.
Pointer continuation has a separate unadopted failing control and liveness probe;
it adds no advertised capability. The portal ledger retains those scoped results.
