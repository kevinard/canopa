# ScreenCast implementation and acceptance

Status: Rust owned capture, v6 sessions and Iced consent are implemented but
UNADVERTISED. Real restricted-frontend-FD monitor/window video and static resume
pass in dark100% and light125% on the pinned ADR0043 candidate. Ongoing visible
sharing/stop controls, Embedded/Metadata cursor acceptance, active caller/backend
loss media checks and compatible distribution selection remain before advertising.
Eleven other interfaces are advertised; full18-interface goal remains active.

## Latest validation

The final Portal binary remains bin-03 SHA
82f5194e5e66d8a3f426431241c4f3eca7e12cd75646c9860bb7a96ba0f85a54 (105unit/21integration
and scoped Clippy pass from prior turn; no Portal Rust changes this turn).
Niri resume candidate SHA
24ab3878f5706cbd4f0a69ed1d58404307fc49c25247956852b0c71b0baf0f39,
canonical overlay/provenance under vendor, original lock unchanged, matched
nightly1.97 compiler. Final media combined7276 terminal0,6groups per appearance,
actual1280x800monitor,400x300dark/500x375lightwindow; window resume sequence1→2
with the same actual pixels. Both original producer removals and foreign FD denial
pass. Patched green-light83692 also terminal0. Default installed-NoGBM66764
terminal0,6groups. Temporary tracing removed, debug prototype archived under
/tmp/canopa-screencast-debug/. Vendor checks,3AST/4docs59links/whitespace and
15affected tooling tests pass; prepare-only/refusal74534 pass.

REQUIRED shared gate running session26599:
/tmp/canopa-screencast-resume-workspace-01.log. Recover actual terminal output;
the previous v6 gate101 on ten Files integration cases is historical. This new
gate follows final canonical vendor/tooling edits. Detailed chronological evidence
and exact baseline/patch/archive/source hashes follow below. No live installation,
routing, user policy, commits or subagents. Goal remains active.

## Session and consent validation before the keyboard correction

The session actor binds caller/app/path, admits16 sessions, handles request and
session cancellation independently of presenter cooperation, retains recordings
after Start, and closes on source/owner loss and shutdown. Armed response guards
retire accepted results discarded by cancellation. Exact v6 results include
64-bit pipewire-serial, distinct nodes/source identities, actual source type and
monitor-only logical geometry. Always fresh consent and persist_mode0.

The Iced helper offers authenticated Niri output/window identities, no initial
selection, explicit Share, search, single/multiple selection, live refresh with
closed/hidden selection removal and keyboard navigation. It imports existing
parent/modal/activation context and shared appearance. Revalidate sources after
review. Rust PipeWire lookup has8 retained job permits,4096 globals and5s timeout.

Final all01/session49213 terminal0:104unit pass/1native ignore,21integration pass/
2native ignores,3m12s build,3.58s/1.93s execution. Four new wire cases cover lifecycle,
exact result signatures and serial width, identity denial, invalid grants/options,
non-cooperative presenter cancellation, admission, source closure and shutdown.
Clippy02/session67043 terminal0,44.39s; headless01/session97725 terminal0,33.75s.
Normal graph excludes GTK/GDK/GIO/GLib, headless graph additionally excludes
Iced/Wayland/PipeWire/libspa. Python AST, whitespace, protocol byte comparison and
5docs/92local links pass. Dependency attribution regeneration/check records649.

Final fixed binary /tmp/canopa-screencast-workflow-bin-02/xdg-desktop-portal-canopa
SHA256 068761fdf72a99b4743f1152ff92964d3451a9adfaa65c3bfab6a65585858863.
Fixed unit harness portal-tests SHA256
8bf881298d88419bdd156c407ae4e21d7401f3d4e833b12a1396b63025ab60a6.
Native dark-final01/session99875 and light-final01/session91626 terminal0,6groups
each on actual frontend/Iced/private Niri. Initial Enter/Cancel, Session.Close,
Request.Close and application disconnect expose no streams; explicit monitor and
window selection reach actual RecordMonitor/RecordWindow and NoGBM failure cleanup.
Both result.json files explicitly report media_flow_verified=false and
shipped_descriptor=false. Inspected dark initial and light selected-window images
show visible footer/actions with no clipping.

Native identities-final01/session10006 terminal0,3groups on real private PipeWire
video producers: exact64-bit serial, removed node unavailable, replacement has a
fresh serial; numeric_id_reused=true. This verifies metadata, not screen frames.

Early full native dark01/02 and minimized pair01 failed after the first monitor
failure; isolated window01 passed. Temporary action-only tracing proved that the
next chooser received Escape when ephemeral wtype sent Down. A persistent test
keyboard with one full US XKB map fixes the fixture; all6groups pass traced dark03
and both final non-tracing appearances. The pixel assertion now also rejects a
vanished chooser. All temporary production traces were removed before final tests
and fixed artifacts. No production cancellation behavior was weakened.

Optional desktop PipeWire0.9.2/v0_3_64 adds16locked packages with no existing
version update reported. Full workspace gate was started as session34639,
/tmp/canopa-screencast-workflow-workspace-01.log; its terminal101 outcome and exact
comparison are recorded below. No live routing, installation,
user policy, compositor upgrade or session takeover occurred. Next implement
ongoing visible sharing/stop controls and validate actual consumer frames on a
disposable supported compositor before advertising ScreenCast.

## Pinned next media fixture investigation

Official releases still list26.04 as latest. Read-only upstream inspection pins
Niri main b22eb842ca9506d4e2ff997b116031cdb3f650f8 (2026-10-30T06:16:36Z).
Its [nested backend](https://github.com/niri-wm/niri/blob/b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/backend/winit.rs)
queries the EGL render node and creates a GBM device for xdp-gnome-screencast,
unlike installed26.04. This is a candidate for disposable media validation, not
proof it works on this host. Sources/revision/cache inventory are retained in
/tmp/canopa-screencast-upstream-inspect-01. A clean isolated checkout is prepared at /tmp/canopa-screencast-niri-native-source-01;
locked dependency fetch01/session11776 terminates0 with its original lock hash
unchanged. Receipt is /tmp/canopa-screencast-niri-native-source-01-receipt.json.
No upstream build was started and no compositor was installed or upgraded. Native pkg-config preflight finds libinput1.32.0,
xkbcommon1.13.2, Pango1.58.2, Cairo1.18.6, GBM26.2.5, libdisplay-info0.3.0,
PipeWire1.6.10, libudev262, libseat0.9.3 and Wayland1.26.0. Defer the independent
compositor build until the Canopa workspace gate retires to avoid contention.

That revision pins Smithay8a270232f1bfc416c781273c98a3f52a812a0bba, uses PipeWire0.10
and has480 registry lock entries,128 absent from the local source cache. Build
the independent upstream workspace in disposable sources with outputs beneath
root target/, preserving source/lock provenance and the license. Do not alter
Canopa's PipeWire0.9.2 binding merely to match the compositor's internal version.
The existing software-rendered acceptance fixture cannot establish GBM media;
use the earlier private hardware-GL Weston probe as a starting point and check
actual frontend/PipeWire frames, monitor/window/cursor content and teardown.
Consume the FD returned by public OpenPipeWireRemote, targeting the returned
pipewire-serial. Installed GStreamer pipewiresrc exposes fd and target-object;
its numeric path property is deprecated. Verify frame content from a controlled
asymmetric parent and retirement after Close. An unrestricted daemon connection
or metadata-only node visibility does not establish the frontend consumer path.

Ongoing controls must be owned by the active session, independent of the expired
Start request. Use a visible Iced surface identifying requester/source with Stop;
closing or losing that surface must stop capture. Keep it non-modal so the
requesting application remains usable during sharing. Any new helper protocol
needs readiness/lifetime/cancellation coverage before a stream grant can depend
on it. Keep Shell optional and raw worker crash cleanup from ADR0041 intact.

## Initial investigation before ADR0042

The following proposal and owned-worker evidence is retained chronologically.
Statements below that public sessions/review are unimplemented describe that
earlier phase, not the current implementation above.

## Contract and proposed boundary

The installed backend XML and current [official ScreenCast contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.ScreenCast.html)
describe version6: CreateSession, SelectSources and Start return numeric response
and typed results. Source/cursor capabilities are bitmasks. Start returns PipeWire
streams with logical geometry/source type; v6 adds a64-bit pipewire-serial because
numeric node IDs can be reused. Persistence defaults to none, and the backend can
reduce a requested persistence mode. Unsupported restore data should lead to
ordinary selection, not implicit permission. The installed XML additionally
documents a session_id result on CreateSession; verify actual frontend use.

Proposed public Canopa policy (not implemented): explicit monitor/window selection,
fresh consent for every session and persist_mode0, no restore grants. Reuse Iced
appearance/parent handling; a stream needs a bounded session owner beyond the
short-lived Requests host. Bind that session to caller identity, app_id and object
path. Serialize Created→Selected→Starting→Active→Closed transitions. Revalidate
chosen output before starting; never substitute a different source. Pin the
unique compositor owner, subscribe before Start, and stop all partial streams on
failure/cancel. Close, frontend/compositor loss and shutdown must retire resources.
Do not hold a global mutable D-Bus method lock across review/start waits.

The [backend Session contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Session.html)
has lowercase version, Close and Closed. Verify frontend/backend close signaling
against actual frontend code before implementing the lifecycle adapter. Test
foreign identity, repeated calls, concurrent Close/Start, caller loss, replacement,
timeout, Settings responsiveness and actual media retirement.

## Niri source and native evidence

Earlier NEXT.md records installed Niri26.04(8ed0da4) Mutter ScreenCastv4
introspection/Create/Stop on a private bus. RecordMonitor/RecordWindow and stream
PipeWireStreamAdded are compositor APIs; they do not require the GNOME portal.
The fetched [current upstream service](https://github.com/niri-wm/niri/blob/main/src/dbus/mutter_screen_cast.rs)
maps monitor names/window IDs, uses cursor values0/1/2 rather than portal flags
1/2/4, and rejects remote-desktop session coupling. Its Stop removes stream and
session objects. These main-branch sources are design references, not installed
binary provenance; pinned full-commit URLs failed to fetch in this turn.

The confined exec environment hides /dev/dri. An escalated read-only inventory
finds writable renderD128 (i915), installed PipeWire1.6.10 and GStreamer tools.
Do not conclude native validation is unavailable from confined device inventory.

`/tmp/canopa-probe-screencast-hardware.py` starts a private bus, PipeWire daemon,
hardware-GL headless Weston and nested Niri, with a disposable asymmetric pattern
window. It removes software rendering only in its own environment and requests
RecordMonitor(cursor Hidden) then Start. It never connects to the live compositor,
user media daemon or system bus. All private processes are retired on failure.

Run01/session67289 exits1. Weston confirms Intel HD520 GL. Niri records the right
monitor, then logs no GBM device and stops the session; no PipeWireStreamAdded or
media sample appears. The script times out20s; this is a failed media probe, not
a portal/capture success. Logs/result in /tmp/canopa-screencast-hardware-01.

The version-matched [v26.04 backend source](https://github.com/niri-wm/niri/blob/v26.04/src/backend/mod.rs)
explains this: gbm_device returns a device for Tty, None for Winit and Headless.
The [current upstream nested backend](https://github.com/niri-wm/niri/blob/main/src/backend/winit.rs)
adds render-node discovery and GBM creation, so latest-source assumptions must not
be applied to installed26.04. A newer disposable compositor with this support or
a separately authorized native Tty environment could validate actual streams.
Do not acquire the live DRM session, change VTs, install/upgrade Niri or advertise
Canopa ScreenCast merely to get past this fixture limitation.

A second private probe, /tmp/canopa-probe-screencast-owner.py, requests no media.
Owner01/session75770 exits0 and observes that an empty session object survives its
creator's D-Bus disconnect; a separate observer explicitly stops it. Do not assume
connection drop releases Niri resources. This proves empty-object retention only,
not active-stream retention. The adapter needs explicit cleanup for backend loss
as well as ordinary Session.Close; investigate an independent owner guard or a
compositor-supported owner lifetime before claiming crash-safe teardown.

Installed XML SHA256:

- ScreenCast:1282ef81555cbad9b1d4eddef3049c61b3197f02b001a963f1c822013db3b7ec
- Session:121183b55dfc4c4615831daf86081cce8289a49698d1ff3c89afd1c6b34ac5e2

The owned compositor adapter is now implemented below. Next implement public
portal Session ownership and real Iced selection/frontend flow. Native successful
media remains a separate acceptance requirement. RemoteDesktop/InputCapture and
Clipboard need their own supported compositor transports; Mutter ScreenCast alone
does not establish those capabilities. Other remaining interfaces retain their
places in ROADMAP rather than being removed from scope.


## Owned capture implementation and terminal validation

ADR0041 implements screencast_capture.rs with private transport/worker modules.
Recorder resolves/pins a compositor owner and starts actual monitor/window sources
through a separate Rust worker. A private pipe lease and backend/caller/compositor
owner watches retire it, including when the backend is SIGKILLed. Finish in-flight
mutations before Stop, retain a late Create reply for cleanup, subscribe before
Start, preserve source order and monitor logical geometry. Window placeholder
geometry is omitted. Cursor0/1/2 mapped explicitly. No Iced or public portal methods
are in this phase; private --screencast-worker mode only. Descriptor unchanged.

Admission is eight unretired workers and16 total sources shared by Recorder
clones. Source list distinct and bounded; private JSON16KiB. Detached reaper holds
both permits until actual worker exit. A completed Start request's disappearing
watch sender does not revoke the recording. Every known partial session attempts
Stop on its original unique owner. Mutating call timeout/transport failures can
still have ambiguous outcomes; no rollback of an unacknowledged Create is claimed.

All05/session98152 passes99units13.07s and17integration1.47s,2native entry points
ignored. Six new contract tests and four real-process integration tests cover
late Create/Record cancellation, reverse/early node signals, monitor/window/cursor
shape, partial failure, caller/compositor replacement, explicit Stop/Drop, actual
backend SIGKILL,8worker admission and aggregate16source budget. Clippy02/session
45535 all Portal targets-Dwarnings passes29.59s. Headless01/session81373 passes
28.01s; core dependency guard and normal no-GTK/GDK/GIO/GLib graph pass. Portal fmt,
whitespace, native PythonAST and docs01(34files/71links) pass. No new dependency,
shared crate, vendor or cross-application source changes; no additional full
workspace gate in this Portal-only phase. Last broad gate remains Screenshot's
ten Files failures, not a green current workspace claim.

Native02/session18334 passes4 failure/cleanup groups with actual installed Niri:
missing monitor cleanup, real monitor NoGBM startup, owned window RecordWindow
NoGBM startup and stale pinned owner rejection. No streams exposed; created
sessions retire. media_flow_verified=false. It uses fixed
/tmp/canopa-screencast-capture-bin-02/xdg-desktop-portal-canopa SHA256
 ea47be8d2d698f20f19aea33580fd51066985321f6bbdcbbd3d88bec94f59a31.
Logs/result /tmp/canopa-screencast-capture-native-02. Native01 initially used
subprocess.run(input=...), closing the lease before Create; it correctly cancelled
and the fixture wrongly waited for Closed. Corrected Popen keeps stdin open.
No production change was needed for that fixture failure. Earlier check01 caught
an async borrow conflict, unit01 a fake D-Bus error type; fixed. Unit02 first5pass,
focused03 six units/two integration pass, all0499units/16integration pass before
the aggregate budget addition. Clippy01 rejected redundant test conversions;
removed, then02 passes. All tool jobs terminal.

### Shared gate outcome, before the keyboard-only correction

workspace01/session34639 terminal101. Tooling, global fmt, workspace Clippy3m40s
and attribution649 pass. All-target test compilation13m43s succeeds. Early
Apps/Assets/CLI/Config/File suites pass; Files library372pass/34ignore,219.12s.
Files integration596pass/10fail/41ignore,590.14s. The normalized ten terminal
failure names exactly match the prior Screenshot/capture gate: no added/resolved
names. Comparison /tmp/canopa-screencast-workflow-workspace-failures-01.json.
This establishes no cause or general flake claim. Later Cargo packages and
post-test scripts are not reached. The full workspace remains red.

After that gate's test build, the Portal-only keyboard correction adds Ctrl+Enter
and a visible hint, plus Up/Down leaving search focus for source selection. Plain
Enter still grants nothing. A native scenario now uses Tab, typed search, Down,
Space and Ctrl+Enter; window confirmation retains the pointer path. One added
behavioral unit case covers empty/unavailable/valid choices. No dependency or
shared crate/vendor change follows the gate. Final all02/session24225 currently
running; final scoped Clippy/build and both native appearances remain before the
correction is validated. Bin-02 remains the pre-correction fixed artifact.

### Final keyboard correction and next native build

Final all02/session24225 terminal0:105unit pass/1native ignore,21integration pass/
2native ignores; compile3m40s, execution3.53s/1.84s. Clippy03/session8996 terminal0,
39.14s. Build02/session33481 terminal0,9.87s. Final fixed bin-03 is
/tmp/canopa-screencast-workflow-bin-03/xdg-desktop-portal-canopa, SHA256
82f5194e5e66d8a3f426431241c4f3eca7e12cd75646c9860bb7a96ba0f85a54;
unit harness SHA256
96b05d3a5f6b2cdadb7615012621e97c55a4c7584ea3f00e2f1c1ec740c4239a.

Keyboard dark01/session90379 and light01/session26713 terminal0,6groups each with
bin-03. Actual Tab→typed search→Down (blur search)→Space→Ctrl+Enter reaches monitor
startup; initial plain/Control Enter without selection cannot grant. Window Share
still tests the pointer. Inspected both selected-display screenshots: hint/footer
fit at100% and125%, no clipping. Wire/cancellation/native NoGBM limits remain as
above, media_flow_verified=false, descriptor unchanged. Serial resolver code did
not change after its3-group real PipeWire metadata proof on fixed bin-02.

Check/release workflows now install libpipewire-0.3-dev and libclang-dev for the new
binding. README records build/runtime prerequisites. Official Ubuntu noble package
metadata confirms the packages (PipeWire dev depends on libspa dev). YAML parses
and dependency declarations pass; affected tooling02/session75358 terminal0,
15tests15.755s. Final scoped fmt, whitespace, Python AST,5docs/92links and absence
of the temporary trace gate in bin-03 pass. The shared gate above predates this
Portal-only UI correction and CI dependency declarations; its red outcome stands.
No shared Rust/vendor/dependency change followed that gate.

Next: isolated upstream Niri dev build01 now RUNNING session95554, log
/tmp/canopa-screencast-niri-native-build-01.log, jobs1, target root
 target/niri-screencast-native. Clean source and original lock from pinned revision
b22eb842ca9506d4e2ff997b116031cdb3f650f8, independent workspace in
/tmp/canopa-screencast-niri-native-source-01. NIRI_BUILD_COMMIT set to the full pin;
no release flags, cache cleanup, Canopa profile change or installation. Receipt
/tmp/canopa-screencast-niri-native-source-01-receipt.json marks build_status=running.
Resume its actual session before restarting. Then test real restricted frontend
consumer frames with the private hardware-GL fixture and implement ongoing Iced
sharing/stop controls. ScreenCast stays unadvertised; full18-interface goal active.

## Real ScreenCast media and static resume — 2026-10-30

Previous goal turn classified PROGRESS: v6/consent/keyboard source code and scoped
checks plus exact shared gate evidence. This continuation finishes the pinned
compositor build and adds actual frontend video acceptance. Full18-interface goal
remains active; eleven interfaces advertised, ScreenCast still unadvertised.

Unmodified upstream main b22eb842ca9506d4e2ff997b116031cdb3f650f8 build01/session95554
terminal0,18m34s, original lock/source clean. Compiler rustc1.97.0-nightly14196dbfa.
Binary SHA2d0bfbefbb0e8985661beccb33aef54e5213d68827513748d9e7073afc1f1a9c,
153140544bytes. Original immutable snapshot now
 target/screencast-media-dark-01/compositor/bin/niri; /tmp original receipt updated
with original_build_output and that snapshot path before rebuilding candidate.

scripts/test-screencast-portal.py gains optional --media, explicit --niri and exact
--niri-sha256. New screencast_media.py snapshots/checks the executable, starts
private hardware-GL Weston/Niri, PipeWire and WirePlumber --profile policy (no
hardware discovery), then consumes public OpenPipeWireRemote FD targeted by the
returned64-bit serial. Actual asymmetric C parent pixels, physical monitor vs
logical metadata, actual window bounds, foreign FD denial and producer retirement
are checked. Default installed26.04 NoGBM mode remains. No Portal Rust source,
root Cargo graph, user installation/routing or live compositor changed this turn.

Unmodified dark01/session57568 terminal0,6groups:1280x800monitor and400x300window.
Light01/session75812 terminal1:1280x800monitor correct500x375source bounds; window
negotiatesBGRA500x375SHM and Streaming but delivers no sample20s. Freshwindow
fractional probe19945 terminal0; full light trace02/session52500 terminal0.
Repeats01/session86775 terminal1 only bootstrap10s timeout, not capture failure;
fixture StartServiceByName now allows30s cold debug startup. Repeats02/session49835
terminal1: run2passes, run3reproduces exact lost-window-frame symptom. Resume-red01/
8597 and resume-red02/62944 terminal1 even at100%, on INITIAL lost frame before
controlled pause hook; do not claim controlled hook reached in red.

Diagnosis skill applied. Ranked hypotheses were initial damage, FD/core reuse,
fractional stride. Trace proves cause: Niri queues seq1 while consumer transiently
Paused, resumes, then skips forever for no damage. GStreamer explicitly corks at
READY→PAUSED before PLAYING. Distinct created/closed Gst cores in passing trace;
fresh500x375windows and failure400x300 rule out a simple fractional stride limit.

Canonical niri-screencast-resume overlay and ADR0043 clear Ready content/cursor
trackers and last cursor position on every Streaming transition before redraw.
Niri only; original GPL retained, original dependency lock unchanged, existing
source-cancellation overlay untouched. Official Niri archive3.4MB SHA
865b04ff9f45a9d1285f7370eb20874f3d79fad56b0c3698e443a6a1aac77e60.
Patch SHAb03b123e00d7496679122d436a8894d7a7cd0445e2678ef35885d9262aebef78;
source SHA923964bb601a70491d0c26adb06919e8de4e71f2bcf77386d41a779d3eb2d35c→
2c042b8e640d13a0849e8de92a77ea44e4f1de674da8bc24f02f22c1b914fdea.
Archive materialization and check-vendor01/80362+02/57010 pass. New
build-niri-screencast.py records source/recipe/lock/compiler/binary, uses ordinary
incremental dev/jobs1 and optional --toolchain. Prepare-only/refusal74534 pass.

Build patch01/session25477 intentionally SIGINTed, tool254/Cargo-2, because its
prepared root directory inherited Root stable1.95 unlike baseline nightly1.97.
Receipt marks cancelled/why. Build02/session23177 terminal0,4m54s, explicit
nightly-x86_64-unknown-linux-gnu matches baseline compiler and reuses its cache.
Receipt target/niri-screencast-resume-source-02/canopa-build.json. Candidate SHA
24ab3878f5706cbd4f0a69ed1d58404307fc49c25247956852b0c71b0baf0f39.

Patched nontracing green-light01/session83692 terminal0,6groups. Final combined
7276 terminal0: dark100% and light125%6groups each with frozen Portal bin-03 SHA
82f5194e5e66d8a3f426431241c4f3eca7e12cd75646c9860bb7a96ba0f85a54 and exact candidate.
Original loop now passes. Added controlled static-window pause/resume: observe
original producer idle, resume, require larger SPA/Gst sequence and same actual
pattern pixels within5s with no source damage. Both final runs advance1→2 and save
media-2-resumed.png. Window400x300dark/500x375light, monitor1280x800both. Inspected
original dark window and final light resumed image: expected asymmetric pattern.
Final artifacts target/screencast-media-resume-final-{dark,light}-01/result.json.
No successful media claim is made for installed26.04nested Winit.

All temporary CANOPA_CAST_MEDIA_TRACE/[DEBUG-cast-media] fixture instrumentation
removed; AST confirms. Throwaway fresh-window prototype moved to clearly marked
/tmp/canopa-screencast-debug/. Default NoGBM regression66764 terminal0,6groups;
tooling35233 terminal0,15tests11.103s;3AST/4docs59links/whitespace pass. New helper
--help and prepare-only execute. Source needed a fresh producer snapshot on
resume; no portal-side cached substitute/changed source grant was introduced.
This real consumer seam now verifies initial/resumed pixels instead of only nodes.

Required full workspace gate started after final vendor/tooling edits; running
/tmp/canopa-screencast-resume-workspace-01.log, handle from latest tool response.
Record actual terminal outcome, not the prior gate. Ongoing Iced sharing/stop
controls, Embedded/Metadata cursor native checks, active caller/backend death
media checks and distribution selection remain before capability advertisement.
No subagents/commits/PRs/live changes. Next continue those after this gate.

Shared gate26599 live at checkpoint: tooling/fmt/workspace Clippy1m06s and
attribution649 pass; full test compilation currently running. Terminal outcome
still required. All scoped native/build/tooling jobs this turn are terminal.

## Session-owned controls and quota recovery — 2026-10-30

Shared gate26599 is terminal101. Tooling/fmt/Clippy/attribution pass, all-target
test compilation completes in6m08s, and Files library reports372pass/34ignored.
Files integration then encounters QuotaExceeded122 throughout temporary-file
creation; the log truncates at110592bytes without a trustworthy final summary.
This is a distinct failed run, not the prior ten-case baseline. Log:
/tmp/canopa-screencast-resume-workspace-01.log.

To preserve evidence and recover temporary space, five inactive goal-created
native binary snapshot directories were copied from /tmp into
target/native-test-snapshots, verified by size and SHA256, and replaced at their
original locations by symlinks. Receipt target/native-test-snapshots/relocation.json.
Relocation37315 terminal0; no Cargo cache deletion. /tmp availability recovers to
about1.9GiB. The user's build-output cleanup is finished and rebuilding is authorized.

ADR0044 adds an independent Iced control helper owned by the active session.
Readiness is bounded; UI loss revokes before cleanup; capture Stop and UI retirement
run concurrently. Completed Start cancellation-sender drop does not revoke the
active control lease. Six real helper-process tests pass in3.32s, including
noncooperative helper kill/reap. Portal check52326 terminal0. Full Portal tests,
native control acceptance, headless and fresh shared validation remain in progress.

Control acceptance now passes. Portal test74194 terminal0: 111 unit tests pass,
one native-only test ignored; 21 integration tests pass, two native-only ignored.
The first native controls run17078 terminal0 passes nine groups, but records
undesirable initial tiling at628×784. The helper now requests a fixed480×280
ordinary window; Niri's fixed-height policy opens it floating without importing
the old consent parent or adding compositor-specific IPC to production UI.

Build63499 terminal0,1m24s. Frozen bin-02:
target/native-test-snapshots/screencast-status-bin-02/xdg-desktop-portal-canopa,
SHA04241bea35f6a87c641dde09f83ccc0279baa6a3a89d54b97e50dcb9b5a0eded.
Dark native58819 and light125%89088 both terminal0, eleven groups each. They
verify initial floating480×280, Stop/Escape/window-close → public Closed and
original producer removal, active caller loss, source-loss/public-Close control
retirement, real static-window resume, and backend SIGKILL → worker Stop/producer
removal/control retirement. Backend-death final monitor frames have no pattern
parent, because the preceding source-loss case destroys it; that case verifies
actual video/dimensions and cleanup, not asymmetric pattern identity. Other six
streams per appearance verify the pattern. Screenshots inspected in both themes.
Artifacts target/screencast-status-media-{dark-02,light-01}/result.json.
Headless check46319 completes normally in38.41s; formatting passes23259.

Fresh shared gate12148 is running at /tmp/canopa-screencast-status-workspace-01.log.
Tooling, Clippy1m47s and649-dependency inventory pass; all-target tests compiling.
No Cargo cache removed. This gate includes the final compact-control Rust change.

Cursor fixture adds a deterministic24×24 cyan/magenta client cursor and a bounded
C PipeWire consumer using the actual frontend FD and returned object serial. It
reads actual frame bytes and SPA cursor position/hotspot/bitmap, without relying
on the GStreamer adapter's position-only ROI. Both C variants and the consumer
compile with -Wall -Wextra -Werror. Dark cursor01/46495 terminal1: display Hidden
passes, then the next case times out before Embedded consumption while waiting
for another pointer-enter event with the pointer still in the source. Fixture
now moves out/back to synchronize entry; corrected run2980 remains in progress.
No producer change or capability advertisement follows that fixture failure.

Corrected dark cursor02/session2980 and light125%cursor01/session81469 are both
terminal0,17groups each. Same frozen bin-02 and compositor candidate. Actual
restricted-FD display and window pixels pass for Hidden, Embedded and Metadata.
Hidden/Metadata contain no checkerboard in the frame; Embedded contains its
correct24×24dark/30×30fractional image at the expected physical position. Metadata
provides the actual separate bitmap and position/hotspot; dark window(60,50),
hotspot(3,5), bitmap24×24; fractional window(75,62), hotspot(3,6), bitmap30×30.
All original producer serials retire. Six cursor captures plus seven existing
media/control/crash streams per appearance pass. Static window resume1→2 remains
verified. Inspected both Embedded window frames and separate Metadata bitmaps.
Artifacts target/screencast-status-cursors-{dark-02,light-01}/result.json.

Both pkg-config headers and pw_get_library_version report PipeWire1.6.10 here.
The pinned Niri source explicitly falls back from Metadata to Embedded below
PipeWire1.4.8. Therefore the remaining advertisement work includes runtime
capability gating/rejection on older libraries, not just selecting/distributing
the verified compositor. Current unadvertised backend still reports7 cursor bits
on desktop; do not advertise it before addressing that mismatch. The long-lived
consumer connection used for each fixture comes exclusively from the public FD.

Shared gate12148 completes test compilation6m32s and is running Files library
tests. Continue its actual session; do not restart for log silence. No Rust source
changed after its launch. Later cursor fixture/docs edits do not change the tested
Rust executable or canonical vendor patch.

Shared gate12148 is terminal143 (SIGTERM), not101 or a completed test summary.
Actual host ps confirms no surviving Cargo/Files integration process from that
gate; termination cause is not established. Tooling/fmt/Clippy1m47s/649inventory,
all-target test build6m32s, and Fileslib372pass/34ignore226.36s pass. Integration
reports the same ten non-helper failure names as the prior completed baseline,
but dies before its final summary. No QuotaExceeded appears in this run. Exact
observation/comparison: /tmp/canopa-screencast-status-workspace-observation-01.json.
Later gate stages are not reached; full workspace acceptance remains incomplete.

Version-policy correction follows that gate's compiled Rust snapshot. Private
shim red8411 terminal1 reproduces ('0.3.64',actual7,expected3) with bin-02, without
claiming actual older-library ABI/media acceptance. New capabilities.rs reads
pw_get_library_version (available without raising the existing0.3.64 ABI minimum),
reports3 below1.4.8 or for malformed/unknown versions, and7 for supported versions.
SelectSources rejects unavailable Metadata with InvalidArgs and closes its session
through the existing malformed-selection path. Headless properties remain0.
Pure tests cover the boundary, suffixes and invalid strings. Focused Portal17105
terminal0:112unitpass/1nativeignore,21integrationpass/2nativeignore, compile4m32s,
execution5.03s/2.00s. Clippy22052 and final executable build11626 still running.
All previous seventeen-group native acceptance uses pre-guard bin-02; final
version-policy and native checks must use the new frozen executable. The canonical
vendor overlay has not changed since the shared gate began. No broad rerun is
needed solely for this Portal-only policy correction; report its scoped checks.

REMOTE-DESKTOP.md records next-slice contract/source investigation only. Existing
private keyboard/pointer delivery is a transport seam, not EIS/portal acceptance.

## Final controls/cursor checkpoint — 2026-10-30

Final runtime-gated executable build11626 terminal0,1m38s; Clippy22052 terminal0,
1m11s; headless62304 terminal0,22.65s; fmt43616 terminal0. Frozen bin-03 is
target/native-test-snapshots/screencast-status-bin-03/xdg-desktop-portal-canopa,
SHA dd6f875e93fdb1981ebbce64307f41c6dbf94d1f0e327420c7fec03255977f8d,
176703896bytes. Root debug binary still matches. No new dependency/version or
vendor-source change accompanies the control helper or version guard.

Version-policy green53470 terminal0: simulated0.3.64/1.4.7/unknown advertise3 and
reject Metadata;1.4.8/1.6.10+fixture advertise7 and accept selection. This is a
version-policy shim, not actual older-library ABI/media acceptance. readelf shows
pw_get_library_version and no pw_check_library_version import. Actual runtime here
is1.6.10. Backend/compositor runtimes still need a compatible distribution pairing.

Final native dark80481 and light125%45183 both terminal0,17groups each on exact
bin-03 and candidate24ab3878f5706cbd4f0a69ed1d58404307fc49c25247956852b0c71b0baf0f39.
Each appearance verifies seven actual media/control/crash streams plus six direct
SPA cursor captures. Source identity/pixels, fractional geometry, Hidden/Embedded/
Metadata bytes, static resume1→2, initial floating controls, Stop/Escape/window
close, active caller loss, source loss, public Close and backend SIGKILL cleanup
pass. Artifacts target/screencast-status-final-{dark,light}-01/result.json.
Installed26.04 nested NoGBM regression62272 terminal0,6groups, no media grant;
target/screencast-status-final-nogbm-01/result.json. All remain private descriptors;
packaging/portal/canopa.portal still omits ScreenCast.

After interrupted shared gate12148, the standard remaining stages were continued
independently. Settings public activation/read/reload passes (separate final log
/tmp/canopa-screencast-settings-final-01.log); CLI build0/13.70s and configuration
checks0/3.20s. First vendor continuation failed only because my workspace-local
TMPDIR made the standalone pipe fixture inherit the root workspace. Normal /tmp
rerun55139 terminal0: fork4tests, event-batch3tests and native-pipe6tests pass.
/tmp/canopa-screencast-vendor-final-01.log. The first wrapper reused its Settings
log filename for aggregate output; the separate successful Settings rerun retains
the actual result. Original continuation receipt remains
/tmp/canopa-screencast-post-gate-01.json; it does not supersede the failed shared gate.

Final3PythonAST/7docs108local-links/whitespace pass. Desktop dependency tree25022
and headless85936 terminal0: no GTK/GDK/GIO/GLib production dependencies; headless
also excludes Iced/Wayland/PipeWire/libspa. Native/compiler/fixture processes all
retired; no ongoing build to resume. No subagents, commits, PRs, installs, live
routing/compositor changes or Cargo cache cleanup. Full18-interface goal active.

Next: resolve compatible compositor distribution selection before advertising
ScreenCast. The resume candidate and ADR0024 source-cancellation companion are
independent pins; do not silently substitute or claim both fixes are integrated.
Investigate the newer pin's Smithay grab/source behavior and run the strict
source-cancellation/retained-offer controls against this capture candidate before
choosing a distribution build. Preserve existing executable/source receipts and
reuse ordinary incremental debug caches. RemoteDesktop preparation remains a
parallel future investigation, not implemented or advertised. Shared workspace
acceptance remains incomplete with the exact SIGTERM/observed-failure evidence
above; do not describe the gate as green or as a completed ten-failure summary.

## Combined compositor continuation

ADR0045 adds the explicit niri-desktop-compat recipe on the capture pin, preserving
the older independent recipes. It ports the six-file Smithay source-lifetime fix
and shares a strict two-package manifest/lock binding transform between builders.
Candidate73b993cbc3c5036701bf612ecb8e260152cce79f408be61b7d2d4cf1bc923026 builds
with the same normalized393-package feature/dependency graph. Original GTK
cancellation, Iced/GTK retained offers, actual Files exchange in both appearances
and dark gestures/Undo pass individually. ScreenCast17groups pass per appearance
on the same compositor and frozen Portal bin-03. Initial startup timeouts remain
unexplained and retained; the original aggregate stays failed.

The generic session integration test now uses mandatory Embedded cursor support;
normal and older-version policy-shim runs each pass21tests/2native ignored.
Source binding4groups, check-script15groups, vendor provenance, formatting and
Clippy pass. A fresh tracked full workspace gate is still running; Files library
372pass/34ignored, integration not yet complete. Its final outcome and all exact
artifacts belong in [the distribution ledger](SCREENCAST-DISTRIBUTION.md).
Full matrix/release/compatible provisioning remains before advertisement.

RemoteDesktop investigation additionally proves Rust reis0.7.1 and actual
libei1.6.0 interoperability over an anonymous socket: exact pointer/button device,
framed input and client-observed removal while held. No Wayland input is injected;
there is no production RemoteDesktop implementation or new dependency. Required
bounded processing, virtual-input/keymap lifetime and combined-session proofs are
recorded in [RemoteDesktop preparation](REMOTE-DESKTOP.md). Full goal stays active.

The fresh tracked workspace gate now finishes101 in1659.48s, with a complete
Files integration summary596pass/10fail/41ignored in847.98s. The ten names exactly
match the prior baseline; no quota error observed. Shared gate remains failed.
The distribution ledger retains the comparison and the separate continuation/
full native matrix preparation. This final result supersedes the running-gate
checkpoint above, not the historical evidence of the earlier SIGTERM.

The separate continuation subsequently exhausts disk during remaining-package
compilation; vendor native-pipe also reports ENOSPC. Settings/CLI/configuration
and vendor fork/event-batch leaves pass. The first full matrix cannot create its
output directory and runs no scenarios. Actual disk recovery to24.5GB permits
tracked retry1017717, status /tmp/canopa-niri-desktop-matrix-worker-02.json, preserving
the same app/compositor snapshots. See the distribution ledger for exact failures,
passed leaves and retry scope. ScreenCast stays unadvertised and the main shared
gate stays red; this is no longer a wait on the retired first continuation.
