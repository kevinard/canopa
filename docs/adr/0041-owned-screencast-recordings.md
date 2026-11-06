# 0041: Owned ScreenCast recordings

Status: capture worker implemented with scoped contract and native failure-path
validation,2026-10-30. Public sessions and Iced review are implemented under
[ADR0042](0042-screencast-sessions-and-consent.md), still unadvertised. Real media
passes on the [ADR0043 candidate](0043-screencast-frame-resume.md).

Canopa Portal owns capture consent and the lifetime of requested recordings.
Niri owns rendering and PipeWire producers. The standard frontend owns application
identity, document permissions and consumer access. Shell does not need to run
for a portal recording. Use the compositor's Mutter ScreenCast API directly in
Rust; no GNOME portal or GTK process is required.

Resolve and retain the unique compositor owner before consent. Revalidate the
well-known binding immediately before creating a recording, then call only that
unique endpoint. Never transfer a pending or active grant to a replacement owner.
The adapter supports actual monitor and window sources, not output crops presented
as window streams. Convert cursor modes explicitly between portal bit values and
compositor enum values. Subscribe to session closure and each stream's node signal
before Start. Return streams in selection order. Use compositor geometry only for
monitors; Niri26.04's placeholder window geometry must not become a false size.

Niri26.04 retains raw session objects after their creator disconnects. Give each
recording a separate Rust worker that owns its compositor connection and session.
The backend holds a pipe lease; closing it requests Stop. The worker also observes
backend/caller D-Bus loss, compositor owner changes and SIGTERM/SIGINT. Unlike UI
helpers, it must survive backend death long enough to clean up. It does not inherit
the UI helper's parent-death KILL behavior. A current-thread Tokio runtime and one
stdin reader thread keep its process/thread cost bounded.

The backend admits eight unretired workers and16 sources in total. A recording
can select at most16 distinct sources within that shared budget.
The private JSON frames are bounded to16KiB. The backend reaper retains admission
through cancellation until the worker exits. Completion of a short-lived Start
request does not revoke a recording; only its session lease does. Stop waits for
worker retirement. Dropping a recording initiates cleanup without blocking Drop.

Once a session path is known, every failed or cancelled start attempts Stop on
that original owner, including partial RecordMonitor/RecordWindow failure. On
ordinary cancellation, finish an in-flight mutation before Stop; otherwise it
could create a stream after its session was removed. Continue awaiting an initial
CreateSession reply after cancellation so a late returned path can be retired.
Creation allows30s, subsequent startup20s with individual calls at most5s, and
Stop5s. Non-mutating subscriptions are bounded too. A reaper gives workers60s to
retire after closing their pipe, then reports failure and terminates them.

Timeouts and transport failures can leave a compositor mutation's outcome unknown.
No claim of rollback or cleanup of an unacknowledged CreateSession is made. A
worker killed independently before it can send Stop also cannot guarantee cleanup
against this compositor API. Known object disappearance/unique-owner loss is
handled explicitly; other Stop failures are reported as unconfirmed cleanup.

Verification uses private-bus compositor fixtures for exact monitor/window/cursor
calls, signals emitted before Start replies, ordered results, late mutation
replies, partial failure, caller/compositor loss, and actual worker exec/lease and
backend SIGKILL. Installed Niri26.04 nested Winit cannot supply GBM even with a
hardware GL parent. A separate native fixture verifies real failed starts and
session retirement; that is not evidence of successful PipeWire media. Public
source consent and portal Session methods are covered by ADR0042. ADR0043 verifies
real consumer frames on a pinned candidate; ongoing controls, remaining native
coverage and distribution selection remain before capability advertisement.
