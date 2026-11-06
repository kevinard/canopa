# Pointer consent and production input

Status: experimental production integration with scoped acceptance,
2026-10-31. Normal RemoteDesktop registration and shipped routing remain disabled.

Connect the ordered public Notify domain to the authenticated Rust pointer worker.
One approved session owns its lazy keyboard and pointer transports, optional
recording, input endpoint and Iced stop controls. Pointer transport failure revokes
the common lifetime and retires the other inputs, capture and controls. Invalid
transitions remain request errors. A changed projection can reject the interrupted
event without replaying it; a verified replacement does not restore held input.

Smooth scrolling retains which axes are active. A finish event sends its nonzero
final delta before stopping all active axes. Zero deltas do not synthesize stops,
and zero discrete steps are no-ops. Discrete vertical/horizontal input maps to the
worker's v120 units. The session's existing receive-ordered bounded queue keeps
keyboard and pointer notifications in one order.

Before handing out an EI FD, detach both intentional-transition monitors and wait
for both legacy transports to retire. Failure revokes the session. Only then open
the EI worker, passing the recording's private bindings when pointer access was
approved. Retain only its worker lease. Later Notify calls are denied, and the
same recording/video pairing continues through the handoff.

Negotiate pointer support through the original input owner without activating a
replacement or opening an input lease. Input-only control needs PointerVersion 1;
captured input also needs MappedPointerVersion 2. Missing properties mean the
feature is unavailable; other probe errors fail closed. Recheck the endpoint and
capabilities after consent. Before publishing capture-bound pointer approval,
require a complete valid binding with known scale. Stop the recording if that
invariant fails. Runtime AvailableDeviceTypes reflects the live endpoint's
relative capabilities; combined consent additionally requires capture mappings.

Iced presents separate unchecked keyboard and pointer permissions. Choosing a
source grants no control. Pointer-only, keyboard-only, combined and view-only
selection remain distinct. Input-only approval needs an explicitly checked device.
The pointer label describes desktop control, including motion, clicking, dragging
and scrolling. Ongoing controls display the exact approved devices and expose one
session-wide stop action. Unrequested device toggles cannot grant access.

The public fixture uses the pinned standard frontend on a private bus/nested Niri.
It checks actual captured colors, logical/physical dimensions and mapping IDs;
fresh Iced consent in both appearances; actual Notify motion, scrolling and held
input; legacy worker loss and session closure; EI handoff with no inherited held
state; and relative-only input without capture. Existing keyboard owner guards
and legacy-to-EI behavior remain separate regressions. No GTK backend is added.

This is scoped experimental acceptance, not distribution or full-matrix approval.
Public monitor/multiple-source projection changes, additional loss/race cases,
physical input coexistence, touch, canonical companion adoption and broader
workspace/native failures remain before advertisement. The installed frontend,
compositor and portal routing are unchanged.

Acceptance is 160 library and 43 integration tests, Clippy and formatting,
17 public pointer/media groups per appearance and two existing keyboard groups
per appearance. Fourteen actual public video frames match their paired input
regions. Fresh/selected Iced consent and controls are visually inspected in both
appearances. All 1,322 respective source inputs stay unchanged, the 11-file delta
replays against copied baselines, and owned native workers terminate. Frontend
availability notifications on compositor changes remain part of the wider matrix.

The [backend RemoteDesktop contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.RemoteDesktop.html)
defines device grants and Notify semantics. The
[public contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html)
requires Notify to stop after EI connects. See
[EI device ownership](0052-session-owned-ei-pointer-devices.md) and
[the production pointer ledger](../../.scratch/xdg-portals/pointer-production/README.md).
