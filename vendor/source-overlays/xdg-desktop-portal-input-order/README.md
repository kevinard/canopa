# Frontend input forwarding, session lifetime, capture phase and USB ownership

This explicit recipe patches the standard XDG frontend 1.22.1. Canopa's backend
and consent/controls remain Rust/Iced. The companion is not installed or selected
by preparing or building it.

The unmodified frontend dispatches authorized methods on independent workers.
Concurrent legacy input can reach a session out of receive order. A session mutex
protects state but does not order those workers. The backend cannot recover the
original order after forwarding has already shuffled events.

The input-order patch changes three upstream files and preserves LGPL-2.1-or-later attribution:

- Receive-ordered admission uses the identity cached when the session was created.
  Original session ownership, grant, option and EIS checks still run in the handler.
- Each authorized session has one FIFO worker. Session setup and authorization
  continue on workers, and the frontend loop never waits for a session lock.
- Admission allows 64 pending messages, at most 4 KiB per body and no input FDs.
  Overflow or rejected descriptors retire the affected session, releasing held
  input even when the rejected message was a release.
- The invocation retains its application identity until dispatch completes.

The separate session-owner patch changes `src/xdp-session.c` and its header.
Implementation session proxies stay bound to their original unique D-Bus owner;
a separate proxy watches the backend's well-known name. Loss or replacement
latches revocation, rejects further session lookup and prevents late creation
success from exporting or reviving a session. Reacquiring the name does not undo
revocation. Backend closure before export produces a failed create response.

For an exported session, cleanup targets the original owner, with no activation
and a five-second asynchronous deadline. A still-live displaced backend must
release its resources; the replacement receives no old-session cleanup. One
worker completes public closure under the session lock, keeping the frontend loop
responsive even while a synchronous ConnectToEIS call holds that lock. Pending
creates finish before their implementation is retired. Ordinary caller closure
and backend Closed retain their existing notification rules.

The separate InputCapture phase patch changes `src/input-capture.c`. Repeated
Enable preserves Active. Release waits for the implementation's phase signals
rather than predicting success: a rejected stale activation ID cannot silently
make an active session fail the frontend's Clipboard access check. Existing owner,
option and phase validation remains in place. The Rust backend independently
enforces actual activation for all Clipboard bytes and metadata.

The separate USB ownership patch changes `src/usb.c`. A successful backend grant
must be a unique subset of the requested devices and may only retain or reduce
write access. The complete result is checked before any permission or pending
acquisition is stored. A reply arriving after Request.Close or caller loss cannot
create a finishable acquisition. Finish rechecks device availability and visibility,
and a cached descriptor is reused only for the exact granted access mode. The
sender map owns one descriptor per device, so repeated acquisition does not leak
references after release. Client FD copies already handed out remain owned by the
client; closing the frontend copy cannot revoke those kernel descriptors.

The archive, licence, four patches and all seven before/after file hashes are pinned in
root provenance. No Cargo dependency or application-local fork is added.

Prepare and build from a previously downloaded official archive:

```sh
python3 scripts/build-xdg-portal-compat.py --archives ARCHIVES \
  --work-dir target/NEW-FRONTEND-SOURCES --prepare-only
python3 scripts/build-xdg-portal-compat.py --archives ARCHIVES \
  --work-dir target/NEW-FRONTEND-SOURCES --reuse
```

Meson/Ninja and GLib development generators must be available. An explicit
`--native-file FILE` can supply build-tool paths. The builder uses one debug
compiler job, disables downloads and installation, verifies all source files
against the archive/patch, and retains the same build cache with `--reuse`.
Upstream tests require additional test dependencies and are run separately.

Scoped keyboard acceptance passes 14 native groups in each appearance, using
concurrent replies and NoReplyExpected input respectively. Fourteen groups also
pass under address/undefined sanitizers, including explicit rejected-FD closure.
These checks include real monitor frames, independent pending consent, caller
guards, budgets, teardown and legacy-to-EIS handoff. Source and binary receipts
retain the failing unmodified build and the instrumented scheduling diagnosis.
See ADR0047 and the portal ledger for current canonical-build and wider-gate results.
Ten private-bus lifetime scenarios pass in normal and address/undefined builds,
covering five session families, backend loss/replacement, late creation, name
reacquisition, ordinary closure, stalled cleanup and busy-worker revocation.
The retained backend-kill native failure and its correction are tracked in
ADR0055 and the frontend-session ledger; current acceptance scope is recorded there.
Pointer/touch, physical input, compatible distribution and broader acceptance
remain. The installed frontend and normal portal routing are unchanged.
