# Device availability and mixed-source input

Status: availability fix verified; mixed-source projection and UI/client-loss
checks pass; the original backend-death closure red is retained, 2026-10-31.
Its canonical frontend correction passes scoped acceptance in ADR0055.
Normal RemoteDesktop registration and installed routing remain unchanged.

The standard frontend binds its exported AvailableDeviceTypes to the backend
proxy's cached property. A fresh backend getter does not update that cache. The
identical private fixture reproduces a backend mask change from 3 to 1 while the
real frontend stays at 3. Add a registration-owned availability watcher after the
backend interface is served, and retain its guard until shutdown.

Subscribe before the initial read. Observe the input service's owner changes and
PropertiesChanged notifications with a bounded stream. Read authoritative state
after each notification; never authorize a device from signal contents. Owner
changes first invalidate the old mask, and a replacement must pass the existing
bus/IPC process checks. Verify the well-known owner before and after capability
queries so a still-live old unique connection cannot publish stale availability.
Emit backend PropertiesChanged only for changed masks. Explicit guard shutdown
aborts the task; no idle polling, input leases or consent are needed for probing.

A real private frontend fixture now passes six groups: initial capabilities,
pointer capability removal/restoration, owner loss, validated replacement, and no
input methods opened. The registration-lifetime test also verifies that stopping
the watcher prevents later publications. The experimental production fixture
retains and stops this guard; normal registration remains disabled.

Extend the public pointer fixture to select a monitor and window together and
pair both with distinct EI regions. Actual client coordinates and restricted
PipeWire frames pass through two window-size and two output-scale changes.
Pointer devices are replaced after held-button cleanup, while keyboard input and
mapping identities continue. UI-stop and EI-client death retire both producers,
controls and held input. Existing single-window/input-only consent/handoff checks
still pass in both appearances.

The backend-SIGKILL case is a real remaining failure. Held pointer/keyboard release
reaches the receiver, but the frontend never emits Session.Closed. Later producer,
control and worker assertions are not reached. The pinned frontend's
src/xdp-session.c attaches only to the backend session's Closed signal, and lacks
a proxy name-owner-change handler. The red remains evidence, not a relaxed
expectation or accepted backend-death result. Prepare and validate a frontend
companion correction before any canonical adoption or normal advertisement.

Scoped Rust validation passes 161 library and 43 integration tests, Clippy and
formatting. The final availability fixture passes six groups; two final
mixed-source runs pass nine groups each, plus an earlier nine-group explicit-close
run. Existing consent/handoff regressions pass seventeen groups per appearance.
All eight changed files replay against copied baselines, and owned native workers
terminate. Physical/touch, additional cancellation/source-loss cases, canonical
companion verification/distribution and existing broad gate reds remain open.

Evidence: [availability and matrix ledger](../../.scratch/xdg-portals/pointer-matrix/README.md),
[production pointer ownership](0053-pointer-consent-and-production-input.md).

Follow-up: [ADR0055](0055-frontend-session-owner-lifetime.md) corrects the retained
backend-death gap, verifies original-owner replacement and pending-create races,
and preserves this slice's original failure. Shared validation remains open.
