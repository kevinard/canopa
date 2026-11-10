# Frontend session lifetime follows the original backend owner

Status: scoped canonical companion acceptance passes, 2026-10-31. The required
shared gate stops at disk-full Clippy before Rust tests; broader acceptance remains
open. All owned validation/native processes are terminal.
The installed frontend and normal RemoteDesktop advertisement remain unchanged.

Update, 2026-11-10: commit 8ede03f packages the patched frontend at
`/usr/lib/canopa/xdg-desktop-portal` and selects it in the dedicated Canopa
session through `packaging/system/canopa-portal-frontend`. Statements below
that the installed frontend is unchanged describe the state when this was
decided; other sessions still use the distribution frontend.

The mixed-source backend-SIGKILL test in ADR0054 releases held input but never
receives public Session.Closed. The standard frontend only observes the backend's
Closed signal. A killed process cannot send it. A proxy targeting a well-known
name can also move to a replacement process, so merely adding owner-loss closure
would risk sending cleanup to the replacement while the displaced owner remains
alive with input or capture resources.

Pin each implementation Session proxy to the unique owner observed during
construction. Keep a separate proxy for the well-known name and observe its
owner. Loss or replacement atomically latches revocation; session lookups reject
further use immediately. Reacquisition does not reset that state. Creation checks
the latch and original owner before export. Backend Closed before export also
prevents a late successful create reply from reviving the session, without a
public Closed signal for a session that was never exported.

For an exported session, send Close to the original unique owner with no service
activation and a five-second asynchronous deadline. The displaced owner may
still be alive and must retire its resources. The new owner must receive no old
session cleanup. Finish public closure on one worker under the session lock,
retaining the session until that worker finishes. The frontend loop never waits
for that lock: synchronous calls such as ConnectToEIS may hold it. A pending
create instead completes before its implementation is retired. Ordinary caller
Close/disconnect and backend Closed preserve their notification rules.

The canonical overlay keeps the original input-order patch and adds a separate
two-file session-owner patch. Both replay against the official 1.22.1 archive;
all 499 source files match the prepared candidate. LGPL attribution and the
original archive/license hashes remain. The old canonical source and frozen
executables are retained unchanged. This modifies the shared protocol frontend,
while Canopa's backend and presentation remain Rust/Iced.

The isolated-bus fixture uses real frontend sessions for RemoteDesktop,
ScreenCast, GlobalShortcuts v1, InputCapture v1 and Inhibit monitors. It verifies
death, replacement while the original connection remains alive, delayed create,
same-owner reacquisition, early backend closure, unresponsive cleanup, a busy
ConnectToEIS worker, ordinary client Close/disconnect and backend Closed. Foreign
callers are denied; retired paths are unusable; replacements accept fresh
sessions only. All ten scenarios pass on the canonical build and the identical
prepared source under address/undefined sanitizers. The original frontend's reds
and the first draft's busy-worker red remain as controls.

Native acceptance retains the existing assertion for public Closed after
SIGKILL, plus actual held-button/keyboard release, EI exit, both capture producers,
controls and worker retirement. Exact binary, fixture and archive bindings and
remaining checks are in the [frontend-session ledger](../../.scratch/xdg-portals/frontend-session/README.md).
Broader shared failures, physical/touch input and compatible companion
distribution remain separate gates before normal advertisement.

The canonical executable passes 148 native groups: mixed-source projection and
SIGKILL/UI/client loss, existing pointer consent/handoff and ordered keyboard
regressions, capability notifications, standalone ScreenCast and FileChooser in
both appearances. Nine mixed-source groups also pass under address/undefined
sanitizers. Settings activation/read/reload passes. Standalone ScreenCast now
requires public Closed after backend death; the original frontend fails that
assertion and the canonical build passes all seventeen groups per appearance.

The required workspace run exits 101 after 426.07 s. Tooling and formatting pass;
Clippy cannot write the Notify integration incremental query-cache because the
filesystem is full. All 1,501 source inputs remain unchanged. Rust test and later
shared stages are not reached. This run does not clear earlier broad failures.
Resume the failed check and remaining stages after space is available, retaining
the existing incremental root target and the original failure evidence.

API references: [GDBusProxy owner notifications](https://docs.gtk.org/gio/property.DBusProxy.g-name-owner.html)
and [bounded asynchronous calls](https://docs.gtk.org/gio/method.DBusConnection.call.html).

Update, 2026-11-10: the `xdg-desktop-portal-input-order` overlay recipe was removed with `vendor/`. The session-owner patch is now the `canopa/session-owner-lifetime` branch of [kevinard/xdg-desktop-portal](https://github.com/kevinard/xdg-desktop-portal), with a tracking PR in the fork, and is part of the pinned `canopa/main` commit that `scripts/build-xdg-portal-compat.py` builds (with the release tarball's Meson subprojects). See ../vendor-maintenance.md#portal-frontend.
