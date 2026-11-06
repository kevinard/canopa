# USB consent stays in Rust/Iced; device access stays with the standard frontend

Status: experimental backend and canonical frontend candidate, 2026-11-01.
The normal portal descriptor and installed routing remain unchanged. Compatible
frontend distribution, real-device/Flatpak acceptance and the broader release
gate remain open. Exact results are recorded in the [USB ledger](../../.scratch/xdg-portals/usb/README.md).

The backend implements `org.freedesktop.impl.portal.Usb` v1 at the existing
portal object. AcquireDevices takes a request handle, parent window, application
ID, enriched devices and options, returning two real output arguments: response
and results. Success returns a unique subset of offered IDs with boolean writable
options. Access can be reduced to read-only, never increased. A denied or cancelled
request returns no grant. The frontend retains discovery, application filtering,
permission storage, monitoring sessions, device opens and Unix FD transfer.
The backend adds no USB hardware library, device-opening worker or GTK dependency.

The shared request owner bounds concurrent dialogs and binds Close and caller
loss to the original unique caller. A one-shot child presents the shared Iced
appearance and exported Wayland parent. Devices start unselected; Allow is disabled
with an empty selection. Users may choose a subset and downgrade requested write
access before explicitly allowing it. Initial Enter cannot grant. Deny, Escape,
window closure, request cancellation and caller loss retire the helper. Device
metadata is bounded, decoded and reduced to plain labels. A displayed device path
is text, never authority to open it. The backend rejects input FDs and oversized
requests and validates the helper's complete result independently.

A private synthetic-device probe uncovered existing frontend failures. Reopening
a cached device ignored the newly granted FD mode; repeated acquisition added an
unbalanced owned-device reference; and successful consent after public Request.Close
could still populate FinishAcquireDevices. The frontend also accepted unrequested,
duplicate, stronger or mistyped grants and reused cached FDs after unplug or
permission revocation. These are frontend responsibilities; adding a second broker
to the Rust backend would preserve the faulty public access path.

The canonical companion therefore gains a separate one-file USB patch. It saves
the original filtered request and atomically validates the full backend grant
before recording any permission or pending acquisition. A closed request cannot
create new state. Finish rechecks current availability and visibility, reuses a
cached FD only for the exact granted mode, and replaces the cached FD only after
successful duplication into the outgoing FD list. The sender map owns one record
per device; reacquisition adds no unbalanced reference. Existing eight-FD batching
and client ownership are retained. Closing a frontend FD cannot revoke a kernel
FD already handed to a client; release evidence makes no such claim.

The original archive, LGPL attribution and previous patches remain pinned.
The complete 499-file old and new source inventories and a fresh archive
materialization prove that only `src/usb.c` changes. The existing source directory,
one-job debug Meson/Ninja cache and checksum-addressed frontend/compositor stores
are reused. The old frontend remains available as a failing control.

Two reusable fixtures exercise the actual boundaries. `scripts/test-usb-frontend.py`
uses a controllable backend to test malicious and late replies, exact FD modes,
release, batching, permission changes and unplug. `scripts/test-usb-portal.py` uses
the actual Rust main executable, Iced windows and nested Niri to test explicit
consent, subset/read-only selection, cancellation, concurrent callers, helper
retirement and directed device monitoring. Both start an empty UMockdev inventory,
add only private regular fixture nodes and assert that transferred FDs target those
nodes. No hardware USB ioctl, host bus, system installation or desktop routing is
part of these checks. Synthetic acceptance does not establish hardware or sandbox
packaging acceptance.

The final normal executable from the complete original-graph workspace run passes
28 native groups in dark/100% and light/125%, with all 22 helpers terminal. Portal
179 library/55 integration tests pass both focused and workspace checks. The full
gate remains red solely on the same ten Files integration failures; other Rust
and all four post-Rust stages pass, with 1,388 guarded inputs unchanged. The final
application and actual workspace harness have verified lossless archives. This
checkpoint adds about 0.38 GB of observed filesystem use and leaves about 14.47 GB
free; synthetic USB acceptance does not clear the broader release requirements.
