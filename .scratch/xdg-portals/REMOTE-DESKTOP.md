# RemoteDesktop preparation

Status: shared session core, production Iced consent/controls and scoped native
monitor/keyboard sessions verified, 2026-10-31. Legacy Notify/keysyms are now wired
through the shared actor and production control; eleven Cargo private-bus contracts
and nine native public reply-barrier groups per appearance pass. A pinned frontend
companion fixes concurrent forwarding and passes14native groups per appearance
and under sanitizers; the installed frontend is unchanged. Pointer/touch,
physical-input and compositor distribution acceptance remain; there is no main
registration or descriptor advertisement. The eighteen-interface roadmap remains
active. Historical preparation and failed proofs below retain their original scope.

The [official backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.RemoteDesktop.html)
and installed org.freedesktop.impl.portal.RemoteDesktop.xml describe version2.
CreateSession/SelectDevices/Start establish consent and granted device types.
Keyboard, pointer and touchscreen are separate mask bits. Start can return
ScreenCast streams and clipboard_enabled. A combined capture/control session is
started and stopped through RemoteDesktop. ConnectToEIS returns a session-bound
Unix FD; legacy input notification methods also remain part of the contract.
Restore data must not bypass fresh consent under Canopa's current policy.

## Legacy keyboard continuation

[Legacy keyboard preparation](LEGACY-KEYBOARD.md) records the ten-test planner
prototype, the reproduced Shift ownership bug and its fix, the adopted and native
verified compositor-owned typed symbol lease, and current Notify verification.

## Verified source seams

Pinned Niri b22eb842ca9506d4e2ff997b116031cdb3f650f8 creates Smithay
VirtualKeyboardManagerState and VirtualPointerManagerState with
client_is_unrestricted in src/niri.rs:2603. Its handlers/virtual_keyboard.rs
implements keymap switching and virtual device removal; handlers/mod.rs:650
forwards virtual pointer events. The existing private portal fixture already
delivers real keyboard and pointer input through these protocols. That establishes
a possible transport, not a RemoteDesktop authorization boundary.

The pinned src/dbus directory has no RemoteDesktop implementation; source search
finds no EIS/libei server. Do not infer an EIS FD from Mutter ScreenCast or from
the ability to inject fixture input. A Rust EIS bridge and its lifecycle would
need separate investigation and native proof. Touchscreen is not established by
the keyboard/pointer evidence and must not be advertised from it.

## Proposed ownership and required proofs

RemoteDesktop-created paths must be understood by ScreenCast.SelectSources when
the frontend requests a combined session. The existing ScreenCast host binds a
path to an app/caller and owns source consent, capture and ongoing controls. Extend
that lifetime coherently; do not create unrelated grants sharing only a string
path. Keep request completion independent of active session resources.

Authenticate the actual Wayland peer against the uniquely pinned compositor
before creating privileged virtual devices. Revoke on compositor/backend/caller
loss. Device selection is explicit and fresh; ongoing controls must state that
input control is active and stop every resource together.

Bound and validate input, reject ungranted device classes, and never translate
absolute coordinates through a disappeared or substituted stream/output.
Disconnected remote clients, Stop and backend death must release held keys/buttons
and retire virtual devices. Keymap/modifier behavior needs explicit testing with
more than the fixture's US layout. A real EIS client must receive the intended
devices/keymap and lose them on revocation; returning an arbitrary socket is not
acceptance. Clipboard authorization remains an additional session-owned grant.

Native work must use the private desktop and harmless receiver. No live input
injection, compositor replacement or user-session takeover is needed for these
proofs. Keep existing routing until actual Rust input/media/FD acceptance passes.

## Rust EIS investigation, 2026-10-30

[reis0.7.1](https://docs.rs/reis/0.7.1/reis/) provides Rust EI and EIS protocol
implementations. Its [EIS Context](https://docs.rs/reis/0.7.1/reis/eis/struct.Context.html)
accepts a UnixStream, including an anonymous socket pair suitable for a future
ConnectToEIS FD. Tokio support wraps the client side; the server requires its own
readiness/handshake/request loop. High-level server conversion buffers input until
the device's frame; this is not an input authorization or resource-lifetime owner.

Official registry archive verified against the crates.io checksum:
4b967ec6489a42067a20724f11987bb96178ea796873cbe1afd25fb93ee6f85f.
Receipt /tmp/canopa-remotedesktop-reis-investigation-01/receipt.json, repository
https://github.com/ids1024/reis. Inspection copy stays outside the workspace;
no supported Cargo dependency or descriptor changes at this checkpoint.

The [COSMIC portal implementation](https://github.com/pop-os/xdg-desktop-portal-cosmic/blob/master/src/remote_desktop.rs)
obtains its EIS FD from a compositor D-Bus method. Its
[legacy input adapter](https://github.com/pop-os/xdg-desktop-portal-cosmic/blob/master/src/remote_desktop_ei.rs)
is an EI client. That route depends on COSMIC's compositor implementation; it does
not establish an EIS server in Niri or a directly reusable transport for Canopa.

The inspected reis server read loop drains into an internal buffer until
WouldBlock. Its converter also queues unframed events. A production adapter must
bound that work before accepting untrusted input, enforce the granted capability
set and sender role, and retire held input on every exit. Do not put an unbounded
parser/queue directly in the portal's shared process. Existing separate-worker
leases provide a lifetime pattern; EIS client disconnect/device removal, virtual
input teardown and modifier/keymap handling still require their own proof.

A debug-only anonymous-socket interop probe at
/tmp/canopa-remotedesktop-reis-investigation-01/probe. It uses the registry Rust
server and installed libei1.6.0 client, negotiates exact pointer/button classes,
sends framed motion/button input and removes that device while held. It opens no
Wayland connection and injects no desktop input. Run40360 terminal0,47.76s build,
proves actual libei negotiation, exact capability receipt, framed motion/button
delivery and client-observed removal. Log /tmp/canopa-remotedesktop-reis-interop-01.log.
This is protocol interoperability, not RemoteDesktop support or proof of held
input release in a compositor. Its one unused-must-use warning was subsequently
corrected; retain the original log and observe the separate warning-free rerun.
Warning-free rerun4367 terminal0,1.47s build, repeats the same actual libei checks;
/tmp/canopa-remotedesktop-reis-interop-02.log. No production dependency or routing
was added. Next implement a bounded worker with the real virtual-input adapter,
then integrate shared session consent and revocation before advertising anything.

The exact combined companion source also establishes an important keyboard
distinction. Smithay virtual_keyboard::destroyed releases its tracked held keys
in reverse order before DeviceRemoved. Niri restores a distinct virtual keymap on
removal, but explicitly cannot attribute absolute `modifiers` masks to individual
devices sharing the seat keymap. Do not assume zeroing those masks on teardown
preserves local input. Prefer forwarding validated actual key transitions through
the normal keyboard path; prove modifier behavior and non-US layout handling on
the private desktop before choosing a production keymap strategy. This is source
evidence, not native held-key/modifier acceptance. The inspected paths are
src/handlers/virtual_keyboard.rs in Niri b22eb842 and
src/wayland/virtual_keyboard/mod.rs in Smithay8a270232, as materialized by ADR0045.

## Verified staged bounded reader

Original registry library regression: direct rustc reuses the actual already-built
reis0.7.1 dependency, writes only /tmp, and runs one exact socket test. One call
consumes8192bytes; the2048-byte invariant fails101. Log
/tmp/canopa-remotedesktop-reis-bounds-red-03.log. Earlier queued Cargo attempts
37771/8908 are deliberately cancelled143, first for an exact-filter correction
and then to avoid the shared artifact lock/disk writes; neither is a test result.

Staged source /tmp/canopa-remotedesktop-bounded-reis-stage-01/reis adds read_chunk
only in src/eis.rs and src/wire/backend.rs. Existing read behavior is unchanged.
One call receives at most2048bytes, pending bytes are capped at the1MiB protocol
message limit, incoming descriptors are rejected and EOF remains distinct from
WouldBlock. Drain pending requests between chunks and use level-triggered readiness;
the converter's unframed queues still require a separate application work budget.

The actual modified library passes four socket regressions (including partial
message preservation) and the original real libei client with identical capability,
framed-input and device-removal assertions. No Wayland connection or input injection.
Warning-free formatting/library/test/client checks pass via one command:
python3 /tmp/canopa-remotedesktop-bounded-reis-stage-01/check.py.
Receipt verification-02.json in that directory; final log
/tmp/canopa-remotedesktop-bounded-check-02.log. The staged patch SHA is
d6d96cf4f22e8fc5312513366fe095e9f10e1e7c51e6ce87798e02dc0935d223.
The inspection and registry source match all38 official archive files. Staging
uses cached Rustix1.1.5; adoption must compile against the real workspace graph
(currently Rustix1.1.4) and preserve normal Cargo/provenance/attribution checks.
No root dependency, backend interface or descriptor has been added yet.

The combined Smithay virtual_pointer::destroyed also releases tracked held
buttons before DeviceRemoved. This supports an independent cleanup design for a
worker crash, but source inspection is not native held-input acceptance. Still
prove actual normal Stop, client/owner loss and SIGKILL on the private desktop,
including modifier/non-US layout behavior and shared capture/control retirement.

The root disk outage is resolved by actual free-space inspection after the user's
cleanup reply. Shared continuation retry02 runs before production adoption; its
full desktop matrix uses frozen apps, allowing later Portal work without changing
their bytes. Preserve existing native fixture sources throughout that matrix.

## Workspace adoption and pending-request guard

Retry02 remaining-workspace passes0/1434.18s; native-pipe passes0/4.27s. The actual
188-case native matrix starts on frozen matrix-apps-01 and combined73b. Canonical
adoption then proceeds without changing existing fixture sources. Current status
/tmp/canopa-niri-desktop-matrix-worker-02.json.

The reader is now under vendor/reis with its original MIT licence, commit
02bc1b52aca10231d380c0cf236d69ec3c56cae4 and exact two-file d6d96c patch. The normal
checker verifies37 local files,38 archive files, retained licence and exact patch;
full offline vendor integrity also passes. Only Cargo.lock is omitted locally.
The desktop-only Portal dependency adds exactly reis0.7.1 and adds only reis to
Portal's existing dependency list. Every other locked package is unchanged;
actual Rustix remains1.1.4. Workspace-graph Portal library check68535 passes0/1m02s.
Four reader cases are registered in the consolidated integration executable;
Portal library/integration tests are running. ADR0046 records ownership requirements.

Separately staged pending.rs caps raw input before conversion at512 per device
and two device identities. Pointer interfaces share their budget; frames/Stop/
release clear only their own device. Retired interfaces lose their identities.
Six tests pass53605, then real libei interoperability60207 passes through both
bounded reads and that guard. Logs /tmp/canopa-remotedesktop-eis-pending-02.log and
/tmp/canopa-remotedesktop-eis-pending-interop-02.log. Initial staging compile errors
(protocol field/enum names and comment placement) are retained, not runtime reds.
Guard SHA eaea43a4e53b9cd0f7fa8af6d3562d87ad9699393ac7c5d29c0bfab3c3405079.
The guard stays staged until the real worker owns/uses it. These checks establish
neither native input nor role/grant enforcement and session cleanup.

Next complete virtual-input/keymap/lease ownership, shared capture/control sessions
and Iced consent/controls, then prove the real FD/native lifecycle. RemoteDesktop
is not advertised. The last full shared gate still has ten Files baseline failures;
the new fork needs the full shared gate after relevant implementation edits settle.

Workspace adoption tests29289 terminal0:112library pass/1native ignored, then
25integration pass/2native ignored in2.89s, including all four new eis_reader cases.
Focused all-target Clippy0/85.86s and headless all-target check0/179.63s pass20669;
the latter includes artifact-lock wait behind a native fixture build. Normal and
headless dependency trees contain no GTK/GDK/GIO/GLib. reis is present only in the
desktop graph; headless also excludes Iced/Wayland/PipeWire/libspa. Receipt
/tmp/canopa-remotedesktop-eis-dependencies-01.json. Generated attributions now
record650packages; test-layout3groups and whitespace/local links pass. Full shared
validation for the new fork remains before phase acceptance, not implied by the
earlier pre-adoption remaining-workspace success.

Upstream behavior investigation reinforces scope: GNOME's v2 backend registers
both legacy Notify methods and ConnectToEIS; its RemoteDesktop dialog receives the
selected ScreenCast source types. See
[backend](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/remotedesktop.c)
and [dialog](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/remotedesktopdialog.c).
Do not infer that only EI clients or monitor-only combined sessions cover that
contract. No GNOME implementation code was copied. Pinned Niri WindowLayout
provides tile position in workspace view, visual size and offset; KeyboardLayouts
provides names/current index. These facts are not proof of complete absolute
window mapping or keyboard modifier-state observation. Investigate those seams
before claiming a virtual-input bridge or choosing an extra compositor broker.

The same real libei/staged-guard socket probe also passes when relinked against
the actual canonical workspace library/Rustix1.1.4, not only the earlier staging
dependency graph. Receipt
/tmp/canopa-remotedesktop-bounded-reis-stage-01/workspace-interop-result-01.json;
log /tmp/canopa-remotedesktop-eis-workspace-interop-01.log. The initial library
selector expected an absolute depfile source path and selected none; corrected
selection recognizes Cargo's relative vendor/reis path. That first attempt ran
no test. Canonical rlib SHA8f10001e94b6f54bc90b777aa0540a0390b3db01f47c198f8f229e530cfe0aaf.
Final formatting,650-package attribution check and63local documentation links pass.

## Compositor keyboard lease candidate, 2026-10-31

Niri owns the authoritative seat state. A separate candidate adds
org.canopa.Niri.RemoteInput1.OpenKeyboard and KeyboardSnapshot. Open returns a
lease socket, the configured keymap and depressed/latched/locked/layout masks.
The normal compositor keyboard policy processes a separate KeyboardSource for
each lease. Socket EOF or malformed input releases that source directly, clears
its no-longer-held pending bindings and never runs a release-triggered bind.
Another source holding the same key remains held. Portal consent/authorization
and session lifetime are still separate required work.

The candidate permits eight pending/active leases, eight pending commands,
64held keys per lease,32fixed eight-byte frames per dispatch and a keymap under
1MiB. Invalid code/state, duplicate press and unpaired release revoke it.
The main thread owns key transitions; no unbounded input thread/channel is added.
A changed keymap is rejected before the next input. Immediate retirement upon
keymap change and asynchronous modifier updates are still required before EIS
integration; keyboard_snapshot polling alone is not final EIS behavior.

Initial reviewable keyboard-only candidate:
[vendor/source-overlays/niri-remote-input](../../vendor/source-overlays/niri-remote-input/README.md),
patch72039e7fe4a27e734eab981823e075b92994256596b44055de98a963ebd59db6.
Five Niri files, including three explicit additions; original GPL/MIT attribution
is retained. No manifest/dependency/lockfile change. Exact base is the combined
niri-desktop-compat recipe; its capture/drag patches and original recipes remain
unchanged. Candidate is intentionally unregistered until the generic source
verifier supports added files. Existing scripts are preserved while the frozen
full desktop matrix runs. Exact five-file patch materialization passes with
--fuzz=0 and original/added-file/hash assertions:
/tmp/canopa-remote-input-patch-check-01.json. Ordinary registered vendor integrity
passes96659; that check does not establish unregistered candidate acceptance.

Prepared source target/niri-remote-input-source-01, reusing the unchanged accepted
Smithay source and the ordinary incremental dev target niri-screencast-native.
Nightly rustc1.97.0-nightly14196dbfa, one job, locked/offline build14806 passes0 in
3m41s. Receipt target/niri-remote-input-source-01/remote-input-build.json. Initial
compile60244 fails101 because KeyboardSource uses new_auxiliary, not new; corrected
before build02. Both logs remain /tmp/canopa-niri-remote-keyboard-build-{01,02}.log.
Three std-only wire tests pass: partial frames/WouldBlock/no read-ahead, invalid/
truncated frames, and finite paired held-key transitions. The separate in-module
admission unit test has not been run; native admission/recovery is verified below.

Read-only frozen executable:
target/native-test-snapshots/compositor-by-sha256/a4edd3182912045fd8967c77188746682a13f803197b25bf9772fd36f176ee4c/niri,
154067280bytes. Actual private bus owner PID matches that Niri process. Native
harness scripts/test-remote-input-lease.py and harmless no-toolkit receiver
scripts/desktop-fixture/remote-input-receiver.c compile with warnings denied.
Run1388 terminal0, target/remote-input-keyboard-native-04/summary.json:

1. French evdev16 produces a and Shift+A in the real focused Wayland receiver.
2. EOF releases a held Shift and returns authoritative depressed mask to zero.
3. Overlapping leases produce one key-down and one final key-up; half-close/read
   EOF proves the first lease's cleanup completed while the second remains held.
4. An unfocused new lease's initial snapshot includes the other source's Shift.
5. Actual SIGKILL of the process holding the sole lease FD releases its held key.
6. Invalid state revokes and releases.
7. Unpaired release revokes and releases.
8. Duplicate press revokes and releases.
9. Ninth lease is rejected; admission recovers after closing the original eight.

Preserve initial fixture failures: native01 rejects the older non-NUL keymap
buffer before any lease input; native02 misses wl_seat.name after binding a newer
seat and aborts before input. The receiver now uses explicit buffer length and
handles all events for its bound seat version. Native03 passes the first four
checks and records actual SIGKILL cleanup in receiver.log, then fails because the
harness still treats its intentionally killed child as an unexpected process
exit. Native04 removes only that expected terminal child and passes all nine with
the same a4edd executable. Logs /tmp/canopa-remote-input-keyboard-native-{01..04}.log.
No deadline extensions or compositor retries concealed these original outcomes.

This is keyboard lease acceptance only. There is no real EIS-to-lease worker,
pointer/touch transport, physical-keyboard native proof, RemoteDesktop backend,
Iced consent or combined capture/control session yet. Keep the full roadmap active.

### Keyboard state extension, 2026-10-31

The preceding nine-case a4edd candidate and source72039patch are preserved in
native-04/source-niri.patch and source-candidate.json. The current reviewable
candidate adds sixteen-byte asynchronous modifier records and retires changed
keymaps during compositor refresh, without waiting for input from the lease.
Slow/partial output writes retire the source instead of queuing updates. The
registry holds weak references and uses the same finite admission. A small
Smithay keymap_id getter exposes its cached content identity: ordinary refresh
and key input no longer serialize/copy the full keymap.

Current candidate patches after the unchanged combined base:

- Niri6eb982418f9b2e21b93df519f719892ded5e51ddf03a28bce6e8205fc86d0dc8,
  six files including the compositor refresh hook and three additions.
- Smithay21d7f419b4f1c322244bbd29096953a5a3890e7e587e0c7e070270c083ceb2a4,
  only the cached keyboard keymap-ID getter.

Seven-file exact materialization and added-file/absence/hash assertions pass:
/tmp/canopa-remote-input-state-patch-check-01.json. The root candidate.json now
contains both patches. Root registration and generic archive-verifier added-file
support remain pending while existing matrix fixture scripts stay unchanged.
No dependency, feature selection or lockfile changes. The build now uses the new
prepared Smithay path as required for the getter, retaining the same incremental
dev target/jobs1/nightly/locked/offline settings. Build66815 passes0/6m46s; receipt
target/niri-remote-input-source-01/remote-input-state-build.json, unchanged lock
8e4397ae9e95917464b13427278486a0da66c0d109e9eaa0d7840c6cce337d6a.

Frozen executable ecd14ac0d3d591114e5b1d198403bd8d83c9e6f229db045113dcb3ce787b4d0b,
154183184bytes, under the compositor-by-sha256 store. Expanded native run96444
passes0,13checks; target/remote-input-keyboard-state-native-01/summary.json. The
executable remains byte-identical, the unique service owner matches the compositor
PID, and the receipt captures exact harness/receiver/shared-fixture hashes. All
previous nine behaviors pass, plus:

- Caps Lock persists across lease cleanup and appears in a new authoritative
  snapshot; normal key transitions restore the original locked state.
- The compositor switches a single fr,us keymap to US, publishes group1 and still
  accepts evdev16 as q on an existing lease; switching back publishes group0.
- An idle observer receives another source's modifier changes, including the final
  release. It does not need a focused surface or polling to learn those changes.
- Replacing configured fr,us with us retires a lease holding Shift without any
  subsequent remote input. Actual receiver key-up and zero depressed mask verify
  release after the keymap reload.

This closes the keyboard broker's asynchronous-state/keymap-retirement gaps only.
The EIS-to-lease worker, real sender/grant enforcement, legacy keysym semantics,
pointer/touch capabilities, physical-input acceptance, Iced consent and shared
capture/control sessions remain active work. No RemoteDesktop advertisement or
live compositor installation occurred. The staged pending-request guard and
canonical EIS reader still need integration into the actual owned worker.

## Actual Rust keyboard EIS worker, 2026-10-31

The desktop-only remote_input module now owns a real EIS-to-Niri keyboard worker.
The hidden --remote-input-worker entry is not a portal capability or consent flow.
Bootstrap is at most4096bytes on stdin, followed by a parent lease; stdout is an
anonymous Unix socket carrying exactly one ready byte and EI FD via SCM_RIGHTS.
No public listener/socket path or new dependency is introduced. The parent
production adapter and process/connection admission are still required.

Before opening a compositor lease the worker subscribes to backend/caller/service
loss, validates the original unique service owner and bus PID, and checks the
private IPC peer against that PID. It never activates a replacement. Input.grants
currently accepts only the keyboard mask1. EI receiver contexts are rejected
before any seat/device/keymap is offered. The initial handshake is bounded by
5seconds/64requests/256-byte labels; the existing chunk reader rejects incoming
FDs and bounds bytes. The previously staged Pending guard is now actual worker
code:512raw input requests per device before conversion, two maximum identities,
shared interface budgets, own-device frame clearing and retirement of old IDs.
Six guard regressions plus a wrap-aware acknowledged-serial regression pass.

The worker offers one keyboard, a sealed XKB keymap and authoritative modifiers
with the EI depressed/locked/latched/group ordering. It validates serials and all
key transitions before forwarding a bounded frame (at most4096bytes). A write has
a250ms deadline; output flush failure retires the connection. Readiness remains
cached until read_chunk returns WouldBlock, preserving bounded-read semantics.
An explicit yield after each chunk lets owner-loss watches run during flooding.
Normal retirement half-closes the Niri FD and waits at most2seconds for EOF after
source cleanup. Client/worker crash closes FDs directly.

StopEmulating releases the old compositor source, awaits its cleanup, renews a
matching-keymap lease and keeps EIS open for a higher sequence number. Device or
keyboard-interface release destroys the old device and permits RequestDevice to
create a replacement. A finite16-ID tombstone set drains queued messages for old
objects without routing them to a replacement. The raw libei trace establishes
that interface release can precede device release during teardown.

Initial worker snapshot3e540a6a51cc217c732c8fae94f2682fcdf48cde106f46038a7f64364a5fbafa
passes10native groups at target/remote-input-worker-native-01/summary.json.
Its source/harness copies remain in that output's sources/. Repeated emulation
and device-request controls then fail on that same frozen executable: it closes
EIS at Stop or teardown. The actual receiver records initial A/Shift cleanup in
the repeated case; failure is the prematurely exited worker, not missing first
input. Preserve target/remote-input-worker-repeat-red-01,
remote-input-worker-request-red-01 and request-trace-red-01, whose logs identify
the old release sequence. These controls were observed terminal1 before checking
the fixed worker. Initial library check01 fails only on ambiguous BitFlags .into;
check02 passes with one must-use warning, corrected before the first binary.
Initial Clippy01 reports two unnecessary unwraps; the branches were refactored.

Current frozen executable:
target/native-test-snapshots/remote-input-worker-bin-02/xdg-desktop-portal-canopa,
SHA950eeb2c8f3fb8b07e5cb4c3c692e731dd1f4ab4afef2a47658b2dbf4b1b9bc9,
178530600bytes. Receipt and five exact source copies accompany it. Niri remains
ecd14ac0d3d591114e5b1d198403bd8d83c9e6f229db045113dcb3ce787b4d0b.
Expanded actual libei run88585 terminal0, target/remote-input-worker-native-02:

1. Stop/restart preserves EIS, releases Shift and delivers subsequent unshifted a.
2. Device close/request creates a new device and delivers actual input.
3. Parent EOF retires EIS and releases held input.
4. Actual libei-client SIGKILL retires the worker and releases held input.
5. Actual worker SIGKILL disconnects libei and releases its Niri source.
6. Caller bus loss retires the worker, EI device and held input.
7. A real receiver-role client is rejected before native input.
8. A real unframed keyboard flood is rejected before native input.
9. An unsupported grant is rejected before any ready FD.
10. Wrong compositor PID is rejected before any ready FD.
11. A mismatched IPC peer is rejected before any ready FD.
12. Keymap reload retires EIS and releases held input without further EI requests.

Positive cases verify exactly keyboard-only capabilities, a parseable sealed XKB
keymap, actual French Shift+A and authoritative modifier feedback. Both frozen
executables remain byte-identical. The receipt captures all four fixture hashes;
C clients/receiver compile with -Wall -Wextra -Werror. The final valid client uses
separate key-down/up frames; the flood intentionally violates framing. Traces
retain actual serials, repeated sequence2 and newly created object identities.
This is keyboard transport/worker acceptance, not physical input or full portal
acceptance. Pointer/touch capabilities, legacy Notify including keysym semantics,
parent adapter, Iced consent/controls and shared capture/control sessions remain.

Final all-target Clippy6590 stage04 passes0/1m02s. Full Portal tests on a private
host bus pass119library/1native ignored in5.66s, then25integration/2native ignored
in2.38s. Earlier sandbox run stops at98pass/21fail/1ignored because socket/HTTP
binds and private D-Bus startup return EPERM; the host rerun permits those existing
fixtures, without changing deadlines or assertions. Logs
/tmp/canopa-eis-worker-{clippy-04,all-tests-03,build-03}.log. Final binary build
passes0/3.57s. Headless all-target check97841 passes0/26.75s; test-layout3groups,
formatting and whitespace checks pass. This focused suite does not supersede the
red full shared gate or the still-running frozen desktop matrix. Full shared
validation of the canonical EIS fork remains required after this phase settles.


2026-10-31 terminal matrix and parent-adapter checkpoint:
The preserved 188-case desktop matrix02 has finished: 107 pass, 81 fail, aggregate
exit1. Host worker1017717/start_ticks25850704 is gone. Its terminal receipt says
all frozen executables were unchanged and changed_fixture_sources is empty.
Before subsequent tooling edits, all149 recorded fixture files were copied and
hash-verified under target/niri-desktop-compat-full-matrix-02/fixture-sources/.
Do not describe the original aggregate as accepted or classify all81 failures
as baseline. An unchanged isolated files-iced-clipboard-dark control on earlier
capture-only Niri24ab and the same frozen Files48b fails at the same final
process.wait(timeout=10). This establishes that this particular failure is not
unique to combined Niri73b; it does not establish the cause of other failures.
A no-job Copy/close/supersede diagnostic on Files48b/Niri73b passes with the same
10-second wait (target/niri-clipboard-minimal-01). Earlier scenario operations
are being narrowed without changing production code or the accepted fixtures.

The real input parent adapter now validates the unique compositor owner, bus/IPC
PID, protocol version and exact ready FD. Connector clones share eight admission
slots; a slot remains held until the actual child is reaped. Startup cancellation,
active drop and parent EOF close the worker lifetime pipe. The parent retains no
duplicate EI client FD. Five focused unit tests pass on the host; two sandbox
attempts are retained with EPERM creating the anonymous socket from stdout.
Frozen parent harness7022e782e9e6abb52f6a12e8678995be7517dacc2c7b5ccf338534581832513c
is recorded in target/native-test-snapshots/remote-input-parent-harness-01/.
Six real-libei native groups pass in target/remote-input-parent-native-02:
parent EOF, client SIGKILL, worker SIGKILL, actual parent/backend SIGKILL, caller
bus loss and keymap loss. Worker950e/Niriecd14/harness7022 remain byte-identical.
The initial native01 failure was metadata parsing of libtest's inline prefix,
before the EI client started; the parser fix and original artifacts are retained.
This is process/FD/lifetime acceptance, not an integrated RemoteDesktop portal.
Fresh Iced consent, shared ScreenCast/RemoteDesktop sessions, controls, legacy
Notify/keysym, pointer/touch and physical-input acceptance remain required.

The exact-added-file compositor source recipe niri-remote-input is now explicitly
registered, never the default. Four patches/14 file entries preserve the same
upstream commits and lock graph. Null upstream hashes require absence, rather
than masking existing files; three new source-addition tests and four existing
source-binding tests pass. Registered prepare-only materialization verifies all
14 resulting hashes and only the two recorded path bindings change the lockfile.
No compositor install, live replacement or portal advertisement occurred.

A new full bash scripts/check.sh --workspace run covers the adopted Reis fork,
parent adapter and source registration. Driver1107282/start_ticks27031661, receipt
/tmp/canopa-remote-input-workspace-01.json and log of the same stem. It records1462
source-input hashes; those inputs are frozen until the command is terminal.
Earlier pre-Reis full-gate failures remain evidence, not acceptance of this tree.


Clipboard diagnosis narrowed further on unchanged Files48b/Niri73b. The Copy/Undo-only
prefix passes (target/niri-clipboard-prefix-02). Adding the first conflict Cancel
reproduces the same10s process-exit timeout (prefix03), while an independent read
observes the foreign clipboard. Captured private SQLite state records request4,
revision3, phase3 TransferPending with canceled=true and exact remaining
original.txt. The public operation-transfer test explicitly requires unfinished
work to hold the process until resume/Abandon. No Files lifetime code was changed.
The temporary explicit-Abandon diagnostic is not accepted: initial syntax, startup,
immediate persistence assumption and fixture C-compile failures are retained.
An alternative correction is staged under /tmp/canopa-clipboard-fixture-stage-01:
a dedicated no-operation clipboard lifetime case keeps the original10s assertion,
while the original multi-operation workflow checks the independent unfinished-work
owner. Python syntax only; native proof and adoption remain pending. Original
188-case red aggregate and149 exact fixture-source copies remain untouched.

Shared-session draft /tmp/canopa-remote-sharing-stage-01 is unadopted/uncompiled.
It extends the existing ScreenCast registry/actor with immutable session origin,
independent device/source selection and cancelable EI FD connection; five private
bus tests are prepared for shared ownership, startup/connection cancellation,
invalid grants, common16-session admission/shutdown and caller/source/backend loss.
Only rustfmt has run. No RemoteDesktop production presenter, legacy Notify, Iced
remote consent, main registration or advertisement is claimed. Source adoption
waits for the full gate's1462-input receipt to become terminal.


2026-10-31 fixture correction native batch01 is terminal1: lifetime-dark passes
33.87s, lifetime-light passes22.98s, full workflow-dark fails103.21s, full
workflow-light passes127.65s. Artifacts and source/binary receipt are under
target/niri-clipboard-fixture-correction-01/. Frozen Files48b/Niri73b are unchanged;
changed_sources is empty. Both dedicated lifetime cases retain the exact10s exit
assertion. Full light workflow reaches the corrected final check and confirms
unfinished transfers retain Files after clipboard supersession. Dark fails at
fixtureline238 while wtype enters the first background-conflict destination; its
command times out10s before reaching the changed final check. Cause remains
unestablished. Preserve the red3/4 batch; do not report four passes or extend any
deadline. The correction is staged, not yet adopted. The broad gate also passes
the explicit unfinished-work lifetime/Abandon policy integration tests.


2026-10-31 shared session core and fixture correction adopted after full gate:
Driver1107282/start27031661 is terminal101/3803.34s and verified gone. All1462
recorded inputs were unchanged. Clippy passed7m20, all-target test compilation
36m45, Files library372pass34ignored276.42s and integration596pass10fail41ignored
646.23s. Exact ten failure names match the prior baseline; no added/removed names.
Comparison /tmp/canopa-remote-input-workspace-comparison-01.json. This remains a
red full gate; later packages, including Portal, were not executed by that run.

Guarded adoption adds a shared ScreenCast/RemoteDesktop host, immutable session
origin, independent sources/devices, approved active FD connection, common16-slot
admission and one retirement actor. Five real private-bus contracts now cover the
new paths. Standalone headless library/test metadata first passed before adoption;
actual Cargo tests caught ConnectToEis versus the required ConnectToEIS. An explicit
wire name fixes it. Portal library124pass/2native ignored4.85s; initial integration
28pass2fail/2ignored is retained; corrected full integration30pass/2ignored2.03s.
Logs /tmp/canopa-sharing-session-tests-{01,02}.log. Clippy passes1m15s, layout3groups
and formatting pass. Headless all-target Cargo check is still running53336.
The real production presenter, combined Iced approval/ongoing controls, legacy
Notify/keysym and pointer/touch remain; no main registration or advertisement.

Clipboard fixture correction is also adopted. The original full workflow retains
its file operations and now expects canceled unfinished work to hold the process
after clipboard supersession. Two new clipboard-lifetime cases isolate the no-job
owner and retain the exact10s natural-exit assertion. The manifest now has190cases;
the original188-case aggregate and149 source copies remain historical evidence.
The3/4 native correction batch remains failed. An unchanged isolated dark follow-up
passes after the filesystem stress ends (native dark-control-02); the earlier
wtype timeout did not reproduce and its cause remains unestablished. Four exact
verified source files for the original batch are archived alongside its receipt.
Both lifetime themes and both full workflows have now passed at least once.
Direct actual runner/CLI registration also passes at
target/niri-clipboard-lifetime-selector-01/summary.json. Python syntax, runner8tests,
preflight and whitespace checks pass. No Files lifetime implementation was changed.

Final session-core verification: headless all-target Cargo check passes35.88s; formatting/whitespace pass. All owned validation processes are terminal. Real production/Iced/legacy integration remains next, not advertised.


2026-10-31 production RemoteDesktop presenter and direct native keyboard proof:
The real presenter now reuses the capture factory/recorder budget, pins compositor
identity across fresh Iced consent, observes caller/compositor loss before the
first EIS connection, and owns capture, input leases and one ongoing scope-aware
Iced control surface together. Keyboard control starts unchecked; input-only
requires an explicit grant and a combined request permits view-only reduction.
The normal main service/descriptor still do not expose RemoteDesktop.

Portal library 130 passed/2 ignored and integration 30 passed/2 ignored; the added
native harness raises library ignored count to 3. All-target check, Clippy with
warnings denied, headless check and native build pass. Two production lifetime
regressions pass again after fixing fixture marker ordering. Logs are under
/tmp/canopa-remote-production-{tests,check,clippy,headless,lifetime-tests}-*.log.
Six actual Iced/libei native groups pass in
 target/remote-desktop-production-native-01/summary.json:
initially unchecked fresh approval and Request.Close, UI Stop, Session.Close,
client loss, caller loss and backend SIGKILL. Five owned input workers are terminal;
held Shift is released and modifiers clear in every accepted active case. Frozen
portal 9014f526 and harness c94bb45c, compositor ecd14ac0, are unchanged. The four
exact fixture sources are archived beside the native summary. Screenshots were
inspected; the input-only disabled button wording was subsequently corrected from
Share to Allow control. Rebuild and theme/combined/frontend proof are in progress.
This does not establish legacy Notify/keysyms, pointer/touch, shared media/frontend,
physical input, compatible distribution or the still-red full workspace/matrix.


2026-10-31 production theme and combined public-frontend acceptance:
The wording/config rebuild is terminal0: library test compilation53.35s, binary
build2m30s (/tmp/canopa-remote-production-native-build-02.log). Snapshot03 freezes
portal d4f278dbd3afcfabf523c76763128867153a9d69d901a86de60e3f7fd2141dc2 and
harness4896723068ed558d4a1d0498e72a2295c8feecb1ac52f89dd039b059cb020ee9 after
terminal build. Snapshot02 is explicitly unused: its portal was copied before the
rebuild completed; it cannot substantiate the wording change. Snapshot01 and its
six accepted dark input-only groups remain unchanged historical evidence.

Input-only light/125% passes all6groups at
 target/remote-desktop-production-light-03/summary.json.
The preceding light02 attempt failed before UI because the sandbox denied the
private D-Bus socket; no product result was obtained. Exact accepted fixture
sources are archived and unchanged. Fresh consent and ongoing controls were
visually inspected; the disabled action now says Allow control.

New scripts/test-remote-desktop-media.py serves the real production adapters with
one capture factory through the standard XDG frontend and a private opt-in
ScreenCast/RemoteDesktop descriptor. Six groups each pass at dark100/light125:
 target/remote-desktop-media-dark-04/summary.json and
 target/remote-desktop-media-light-01/summary.json.
Each run verifies six actual asymmetric monitor frames via restricted
OpenPipeWireRemote FD + exact producer serial, both source/device selection orders,
view-only reduction with exact AccessDenied/No input device was granted, and real
French shifted A/modifier feedback in five granted input sessions. UI Stop,
Session.Close, client/caller loss and backend SIGKILL remove the selected producer,
retire controls and release held Shift/modifiers. All ten owned input workers are
terminal. The same portal/harness/compositor hashes remain unchanged; exact7fixture
sources are archived before final runs, with no changed sources at completion.
Screenshots in both appearances were inspected. Aggregate verification receipt:
 target/remote-desktop-production-acceptance-03.json.

Keep prior failures: combined dark01 frontend SIGABRT due missing fixture GSettings
schema path; dark02 fixture KeyError persist_mode after actual approval. The public
contract reports restore_token rather than the backend-only persist_mode result.
Corrected dark03 passes6; final dark04 also tightens exact denial and positions
controls clear of the reference source at fractional scales. No product change or
deadline relaxation was needed for these fixture corrections. All old sources are
archived separately. Current formatter/test-layout3groups and Python syntax pass;
final Portal regression run is pending. The red full workspace gate and historical
107/81 native matrix remain red, with their causes/scopes recorded above. This
acceptance does not cover legacy Notify, pointer/touch, physical input, shipped
metadata or compatible compositor distribution; RemoteDesktop stays unadvertised.

Final production regressions are terminal0: Portal library130pass3ignored6.39s, integration30pass2ignored1.97s (tests03 log, compilation41.39s); Clippy03 passes71s. Formatter, test-layout3groups, Python syntax and whitespace pass. All owned validation/native jobs are terminal. The broader goal remains active.


2026-10-31 typed compositor keyboard owner and portal transport continuation:
LEGACY-KEYBOARD.md records the adopted SymbolVersion1/OpenKeyboardSymbols protocol,
borrowed authoritative-map planner, sequence/status acknowledgements, exact refusal
and owned-chord cleanup. The registered explicit recipe changes only the Niri patch
(e1bd2127); Smithay/pins/lock graph remain unchanged. Archive replay15entries passes,
as do vendor integrity and source policy3+binding4tests. Frozen NiriE5e792 passes
12new symbol groups, all13v1 groups and6combined frontend/media/EIS groups with the
existing PortalD4/harness489. All native candidate jobs are terminal0. No install
or default compositor selection; prior accepted NiriECD bytes remain unchanged.

Portal LegacyInput/worker bridge is implemented but Notify remains unwired. It
uses one pending command, fixed partial-read buffers, finite acknowledged status
and a shared authenticated/reaped8-worker pool. Initial library check passes33.41s;
bridge3 and input-suite18pass1ignored pass. The added coverage retains a red
19pass1fail2ignored test03 caused by an immediate Tokio try_read readiness assertion
on a closed peer. Tests04 replace that assertion with bounded async EOF and are
running. The guarded real parent harness/private driver are ready but not run.
Full source details, receipts and logs are linked from LEGACY-KEYBOARD.md. Required
full vendor/shared gate remains pending for this slice; prior full gate/matrix reds
are still red. Legacy Notify/input mode, pointer/touch, physical input and compatible
distribution remain before advertisement; the18-interface goal remains active.

Final legacy transport evidence: new frozen worker477fc400/harnessae400644 pass5
real legacy parent/worker groups and6existing EIS groups on compositorE5, with
all11owned workers terminal and binaries unchanged. Exact fixture sources are
archived. Scope excludes public Notify and legacy consent. Full Portal scoped
validation passes138library/4ignored and30integration/2ignored; Clippy104.28s,
headless36.55s and format/layout/syntax/whitespace pass. Receipt
/tmp/canopa-legacy-keyboard-validation-01.json. Required full workspace gate is
next. Notify/actor draft is unadopted and passes headless metadata only.

2026-10-31 typed transport full gate is terminal101/1547.65s. All1483recorded
inputs were unchanged. Files integration596pass10fail41ignored606.54s stops the
gate before Portal/later stages. Failure names exactly match the prior ten; no
added/removed names. Comparison /tmp/canopa-legacy-keyboard-workspace-comparison-01.json.
Dedicated Portal138/30, Clippy and headless passes remain scoped evidence.
The historical matrix and full gate remain red. Notify/production draft stays
unadopted while its mock-bus contracts are completed; see staging checkpoint.


## Notify adoption and focused contracts, 2026-10-31

After the previous full workspace gate became terminal, four guarded source
files were adopted from /tmp/canopa-legacy-notify-stage-01. The actual Cargo
library check passes25.70s, and the eight RemoteDesktop integration contracts
pass0.99s at /tmp/canopa-legacy-notify-contracts-01.log. They exercise original
caller/session/grant checks, recoverable domain refusal, the one-way EIS guard,
and pending acknowledgement cancellation on session/caller/source/backend loss.
A blocked input command cannot expand the actor's bounded queue; overflow revokes
the common session so a discarded release cannot leave held keys active. That
case was reproduced red against the earlier draft before the retirement fix.

Production starts one authenticated legacy worker lazily on the first accepted
Notify. ConnectToEIS stops and reaps that worker before creating EIS, deliberately
removing its lifetime observer so this transition does not close consent. The
same capture and ongoing controls remain owned by the shared session. These are
implementation details verified by focused contracts and compilation, not yet
public frontend/native acceptance. The new nine-case native fixture includes an
eight-event shortcut burst, real monitor frames and legacy-to-EIS continuity.
The main backend and shipped descriptor still do not register RemoteDesktop.


### Shortcut burst diagnosis

The first new native run passes input-only and view-only, then fails the ordinary
eight-event Ctrl/Alt/Shift/F13 burst. The isolated replay also fails, with empty
public replies, partial receiver events, Session.Closed and retired controls:
 /tmp/canopa-remote-desktop-notify-dark-01/summary.json and
 /tmp/canopa-remote-desktop-notify-burst-red-01/summary.json.
All owned fixture executables are confirmed terminal after both runs. These are
retained failures, not native acceptance. The new delayed-ack private-bus
regression reproduces explicit LimitsExceeded in0.27s at
 /tmp/canopa-legacy-notify-burst-contract-red-02.log.
The cause is the original four-command shared queue, appropriate for startup
commands but too small for asynchronously forwarded input. The fix permits64
pending commands per session (still16sessions) and retains session retirement on
overflow. A65-call blocked-input regression checks that retirement. Post-fix
Cargo and native validation are pending; no pointer/touch or distribution claim.


### Ordered backend admission and frontend forwarding, 2026-10-31

After the capacity fix, native burst-green03 delivers all events but swaps two
presses while the session and controls remain active. A32-event trace05 then
localizes one native ordering failure to the installed xdg-desktop-portal1.22.1-2:
public messages match the issued order; backend forwards do not; the actual
receiver matches those forwards after refusing duplicate/unpaired transitions.
 /tmp/canopa-legacy-notify-frontend-order-01.json pins the frontend executable and
comparison; trace05 archives its exact fixture sources. Trace04 passes once;
that does not disprove the retained races. The incomplete burst-green02 copy
failed filesystem quota before any native case; it is an environment failure.

Independently, the new direct four-thread regression fails in round0/0.36s before
an ordering fix, at /tmp/canopa-legacy-notify-order-contract-red-01.log. The new
RemoteDesktop Interface adapter admits keyboard commands in receive order before
spawning acknowledgement tasks, preserving asynchronous typed setup handlers.
Its library check passes33.21s. Eleven actual contracts pass1.55s at
 /tmp/canopa-legacy-notify-order-contracts-03.log, including1,024 ordered direct
transitions and another session's keyboard while consent is pending. An earlier
contracts02 compile failed only a new test's shadowed helper name; corrected in03.
Final library/full integration/Clippy/headless/native rebuild is running at
 /tmp/canopa-legacy-notify-order-validation-03.json.

The full public reply-barrier dark01 attempt returned Start2 in input-only;
 target/remote-desktop-notify-reply-barriers-dark-01/summary.json retains that
separate unresolved failure. The selected Iced consent screenshot was reviewed:
keyboard is checked and the action enabled. No cause is assigned yet. No native
acceptance or normal RemoteDesktop registration is claimed. Concurrent public
forwarding remains a frontend compatibility gate even after backend FIFO is fixed.


## Current ordered Notify acceptance, 2026-10-31

The final dispatcher passes library138/4ignored5.42s and integration36/2ignored2.88s,
including11RemoteDesktop contracts and1,024 ordered transitions across four threads.
Clippy all-targets with warnings denied passes88.64s; headless all-target check
passes49.39s; normal incremental default binary build passes4.09s. No new vendor,
profile, dependency graph or routing change accompanies this Portal-only slice.

Frozen ordered snapshot03 passes the isolated input-only native case, then all9
public reply-barrier groups in dark100 and light125 (58.10s and56.66s). Each full
run consumes8real monitor streams and records9terminal input workers. Legacy-to-EIS
releases the old held chord/worker, transfers the EI FD and keeps the original
approved stream usable. View-only denial, common Stop, explicit session close,
worker/caller/backend loss and Notify-after-EIS denial pass. Native screenshots
were reviewed in both appearances, including checked input-only consent and
unchecked view-only consent with one selected source. Exact7fixture sources per
run and99Portal/build inputs are archived unchanged; frozen binary hashes remain
unchanged. No live routing or RemoteDesktop main/descriptor registration.

Evidence: target/remote-desktop-notify-acceptance-03/receipt.json;
 target/native-test-snapshots/legacy-notify-ordered-bin-03/receipt.json;
 target/remote-desktop-notify-ordered-{dark,light}-03/summary.json.
The postbuild copies remove debug sections to fit storage; original Cargo debug
outputs/profile remain, and matching ELF build IDs/executable segment hashes are
recorded. Exact immutable test compositor duplicates are stored by reference to
the unchanged frozen compositor after each terminal run. Original tmp evidence
paths remain aliases to the stored successful artifacts.

This scope uses public reply barriers. The installed frontend1.22.1-2 concurrent
forwarding race remains independently reproduced and open, with persistent trace
and comparison at target/remote-desktop-notify-acceptance-03/frontend-ordering.json.
The older Start2 failure remains unexplained and retained; the current isolated
and full cases pass, without attributing its cause. The broader workspace and
historical full matrix remain red; focused Portal success does not clear them.
Pointer/touch, physical input, compatible distribution and the remaining interfaces
on the eighteen-interface roadmap remain active work.


## Canonical frontend companion acceptance, 2026-10-31

ADR0047 and the registered xdg-desktop-portal-input-order recipe address the
upstream worker-scheduling race. An exact official 1.22.1 build fails the same
concurrent burst; instrumentation shows reordering at worker/handler entry, while
proxy sends preserve handler order. Only three frontend files change. Cached
caller ownership and approved input are checked before a bounded per-session
queue; original handlers retain grant/option/EIS checks. Setup stays asynchronous.
The 64-pending/4-KiB/no-FD limits retire the owned session on overflow.

Canonical archive replay verifies 499 inputs and all three patched hashes.
Same-cache debug build/reuse succeeds; tampered/extra source, a symlinked root
and a nonempty generated lock are rejected. Frozen frontend b77492eb passes all
14 native groups in dark100 (86.95s, concurrent32events) and light125 (87.70s,
NoReplyExpected32events). Each has12real monitor consumers,14terminal workers,
unchanged7fixture sources and4binary pins. Explicit pre-consent denial and
rejected-FD closure pass. The separately frozen candidate also passes14groups
under ASan/UBSan with no memory-error report; global-cache leak scanning is
excluded. Original ordering/body/FD failing controls and interrupted-build logs
are retained, without assigning unrelated earlier failures a cause.

Persistent receipt: target/xdg-portal-input-order-acceptance-01/receipt.json.
The required post-adoption workspace gate finishes101in1667.42s at
 /tmp/canopa-frontend-order-workspace-01.json. Tooling/fmt/Clippy/all-target
compilation pass;2024inputs remain unchanged. Files integration finishes
596pass/10fail/41ignored648.07s, with exactly the same ten failure names and no
added/removed names. Remaining gate stages are tracked separately. Focused native
success does not clear this red gate or the historical107pass/81fail matrix. The installed frontend and live
routing remain unchanged. Pointer/touch, physical-input, compatible distribution
and the remaining eighteen-interface roadmap stay active; no RemoteDesktop
main registration or advertisement is added.


Canonical frontend Settings activation/read/reload also passes3.00s. FileChooser
passes12native groups per appearance (dark39.37s/light12541.66s) on the
portable root fixture after a diagnosed test-device lifetime correction. Both
original and candidate frontends fail the old one-shot Shift fixture identically;
a separate native probe observes mask0after return,1while the device lives and
0after retirement. The fixture now keeps that device alive across selection and
waits for its Wayland roundtrip. No application behavior changes. Root native
fixture sources and runtime binary pins remain unchanged during both corrected
runs. The full workspace gate above preceded this tooling-only correction.
Original failures and diagnostic/final fixture versions remain archived.


### Remaining shared checks and pointer continuation

The unreached stages finish with all remaining Rust tests passing1723.24s,
Settings activation1.10s, CLI build11.27s and vendor checks9.44s. Configuration
initially fails because the normal canopa-files executable is missing; the failed
control is retained. Its ordinary incremental debug build passes216.54s and the
configuration suite passes32.80s. The2024recorded source inputs remain unchanged.
This does not clear the required full gate's same ten Files integration failures.
Persistent continuation and recovery receipts are under
 target/xdg-portal-input-order-acceptance-01/.

Pointer continuation remains unadopted. Frozen E5 reproduces two virtual pointers
releasing each other's button state in8.13s, while its single-source control passes.
Five ownership tests and seven cycle/origin/routing model tests pass, each including
576four-source orders. A two-file getter/trace probe builds in345.28s in the same
nightly1.97 incremental dev cache. Its21.34s native diagnosis confirms teardown
releases already have a dead Wayland resource before Niri binding dispatch, while
ordinary releases have a live resource. The ownership contract remains red.
All owned probe processes are terminal; accepted E5 and source/runtime pins stay
unchanged. At that checkpoint the temporary trace is removed afterward; only a
read-only getter remains in the unadopted prepared tree. Evidence and binding/grab/mapping
continuation notes are at target/remote-input-pointer-investigation-01/.
This does not establish pointer/touch or physical-input support.

The follow-up prepared Rust pointer foundation now passes nine ownership model
tests and 38 native checks on one frozen compositor, including overlapping sources,
release bindings, consumed pickers, DnD, synthetic host focus, gestures and global
resets. The clean source build and native verification preserve all 5,954 inputs.
An unfenced drag-setup timeout is retained and explained by a controlled server
trace; a same-client Wayland sync callback fixes that fixture's ordering without
a compositor change. Reviewable seven-file delta patches and fixture sources are
preserved outside rebuildable outputs. See [pointer evidence](POINTER-OWNERSHIP.md).
The canonical recipe and portal capabilities remain unchanged. Pointer leases,
approved coordinate mapping, Iced pointer consent and broader acceptance remain.

The prepared pointer lease now implements bounded relative/button/scroll commands
and acknowledged source cleanup. Six wire tests and 94 native checks pass on one
frozen binary, including direct lease binding/DnD, all 38 cleanup checks and both
keyboard suites. Its five-file delta composes with the ownership foundation in an
exact eleven-file replay. Absolute approved mapping, EI/legacy pointer transport
and Iced pointer approval remain. See [the lease draft](pointer-lease-draft/README.md).
The canonical recipe, worker grants, normal registration and advertisement remain
unchanged; the full workspace/matrix/distribution gaps are not cleared.

Live capture identity and capture-bound absolute pointer drafts now have scoped
native evidence. Metadata/geometry checks and actual video pass; absolute input
matches captured regions for both ordinary and offset window geometry. The mapped
candidate also passes all 94 existing input regressions. See
[mapping evidence and remaining integration](POINTER-MAPPING.md). Portal catalog
negotiation, worker renewal, EI/Notify pointer adapters and Iced pointer approval
remain; neither draft changes public grants or canonical recipes.

The portal source-ID path is now implemented: live monitor identities survive
catalog refresh, Iced consent, revalidation and the separate worker's recording
precondition/post-start metadata check. Forty native groups pass across the new
provider, legacy view-only capture and combined keyboard EIS/media lifetime.
All 15 new-provider monitor recordings carry their identity precondition.
The final focused portal checks pass 145 library and 37 integration tests plus
Clippy; an earlier parallel run's two cancellation timeouts remain unexplained,
with direct controls and the full normal rerun passing. See
[source identity evidence](portal-source-identities/README.md) and ADR0048.
Capture-bound pointer ownership/renewal, EI/Notify pointer adapters, Iced pointer
permission and canonical/full-matrix/distribution acceptance still remain.

Completed inactive native snapshots were preserved on disk after process checks
and per-file hashes:11directories/1770596661bytes, with original/tmp paths retained
as aliases. The relocation receipt is
 target/native-test-snapshots/tmp-preserved-01/relocation.json. No active executable,
Cargo cache or root debug output was deleted. The full eighteen-interface goal
and distribution/full-matrix acceptance remain active; no live portal routing
or RemoteDesktop registration changed.
