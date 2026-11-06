# 0042: ScreenCast sessions and source consent

Status: implemented behind capability advertisement, 2026-10-30. Real media passes
on the [ADR0043 compatibility candidate](0043-screencast-frame-resume.md).
[Session-owned controls](0044-session-owned-sharing-controls.md), active caller
loss and backend-death cleanup also pass native checks. All cursor modes pass for
both source types at100%/125%. Runtime cursor capability gating is implemented;
compatible compositor distribution selection precedes advertisement.

Canopa Portal implements the version 6 ScreenCast backend in Rust. The standard
frontend retains application identity and PipeWire consumer access. Each backend
session owns its state and capture independently of the short-lived CreateSession,
SelectSources and Start requests. ADR0041 owns the compositor worker and cleanup.

Bind each session path to its unique D-Bus caller and application ID. Admit at
most 16 sessions, and serialize New → Selected → Starting → Active → Closed in a
session actor. Never await consent or capture startup while holding the shared
session map lock. Repeated selection/start calls cannot replace an active grant.
Validate requested source and cursor masks. Accept persistence requests but always
return persist_mode 0 and request fresh consent; restore data grants no access.

The Iced chooser starts with no source selected. Share requires an explicit,
currently offered selection; Ctrl+Enter explicitly confirms a valid choice, while
Enter alone grants nothing. Escape, Cancel, closing
the window, Request.Close, Session.Close and caller loss retire pending consent.
Selection supports actual displays and windows, one source or up to 16 when
multiple was requested. Search and live updates clear unavailable selections.
Keep the footer visible, use shared appearance and controls, and import the
request's parent, modal and activation context through existing window helpers.

Capture sources come from the Niri IPC socket captured at backend startup. Its
peer PID must match the uniquely pinned compositor's D-Bus PID. Bound catalog
reads, item counts and displayed text. Refresh the catalog during review and
revalidate selected identities after consent; never substitute another source.
The helper returns identities only, not PipeWire nodes or permission decisions.
The [live monitor extension](0048-live-capture-source-consent.md) distinguishes
Output replacement under the same connector on compatible providers. Legacy
view-only capture retains connector semantics; it is insufficient for absolute
pointer authorization.

After the owned worker reports producers, resolve each node's 64-bit object.serial
through the Rust PipeWire binding. Bound resolver jobs, registry entries and wait
time, retaining job admission through cancellation. Results include distinct
nonzero numeric nodes and serials plus actual source type. Publish compositor
logical geometry only for displays; omit Niri's placeholder window geometry.
Never grant a failed, closed, cancelled or malformed capture.

An armed response guard closes its session if a concurrent request cancellation
discards an accepted result. An active session retains the recording after Start
finishes. Request cancellation is watched independently of presenter cooperation.
Session closure drops pending review/startup and explicitly stops active capture;
backend shutdown waits for retirement. Explicit frontend Session.Close is silent
on the backend signal path, while unexpected source loss emits the no-argument
Closed signal. These semantics follow the actual frontend session adapter.

Wire tests cover lifecycle, caller/app isolation, exact version 6 types, malformed
grants, cancellation with a non-cooperative presenter, admission and shutdown.
Private PipeWire tests verify serial lookup, removal and numeric node reuse.
Native frontend/Iced tests cover explicit consent and real compositor failure
cleanup. Real restricted-FD monitor/window pixels and static resume also pass on
the ADR0043 candidate. Installed Niri 26.04's nested backend has no GBM device;
that fixture proves failure cleanup only. Keep ScreenCast out of canopa.portal
until compatible compositor distribution selection is resolved.

Gate Metadata on the backend's linked PipeWire library version. Below1.4.8, or
when its version cannot be interpreted, advertise only Hidden/Embedded and reject
Metadata selection with InvalidArgs. Use the longstanding version getter so the
check itself does not raise the existing ABI requirement. This matches the pinned
compositor's minimum for real Metadata instead of accepting its silent Embedded
fallback. Five private version-policy cases pass; actual media acceptance uses
PipeWire1.6.10. Distribution must keep the compositor runtime compatible too.

References: [ScreenCast backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.ScreenCast.html),
[Session backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Session.html),
[frontend session adapter](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/src/xdp-session.c),
[Niri 26.04 backend device selection](https://github.com/niri-wm/niri/blob/v26.04/src/backend/mod.rs).
