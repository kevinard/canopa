# ScreenCast portal development

The Rust backend implements ScreenCast version 6 sessions and Iced display/window
consent. It uses Niri's compositor capture API and Rust PipeWire metadata lookup.
The [Arch packages](installation.md#desktop-portals) ship compatible companions
and select Canopa ScreenCast in the dedicated session. The
[portable desktop-sharing component](desktop-sharing.md) ships the companions
and extended descriptor for explicit selection. Real
display/window video and static-window resume pass on the pinned compatibility
candidate. Session-owned Iced stop controls and active caller/backend-death cleanup
also pass private native checks, as do all cursor modes for both source types at
100%/125%. Runtime cursor capability gating is implemented.

Capture requires the compositor's creator-cleanup contract: an independently
killed worker must retire its recording even when it cannot send Stop. The
pinned kevinard/niri fork supplies that contract alongside capture-bound pointer
mappings (since its `canopa/remote-input-pointer` topic, formerly the
`niri-desktop-pointer` recipe). The backend and worker reject a provider without
the verified contract before consent or recording starts. Missing/unknown versions
are rejected, and the worker independently rechecks before creating its session.
Scoped production admission/media acceptance passes. Distribution and broader
acceptance remain pending; see [the creator lifetime decision](adr/0056-capture-creator-lifetime.md).

Every session requests fresh consent. Nothing is selected initially; choose a
source, then Share or press Ctrl+Enter. Tab focuses search; Up/Down moves into the
source list and Space toggles a source. Search and live catalog updates remove
hidden or closed selections. A caller requesting multiple sources can choose up to 16. Cancel,
Escape, window closure, frontend request/session cancellation and application
disconnect retire pending review. A completed Start retains its recording until
session closure, source loss or shutdown. A compact floating Iced window shows the
application and shared sources. Stop sharing, Escape or closing that window ends
its session. Losing the control helper also stops sharing. No persisted or
restored grants exist.

On a provider with CanopaSourceVersion=1, the chooser carries the live monitor
identity through review and recording. Replacing a monitor under the same
connector clears its selection and requires fresh approval. The worker rejects a
stale identity and reads monitor geometry after its producer starts. Providers
without the extension retain the existing view-only connector path; that path
does not authorize absolute input. See
[the source identity decision](adr/0048-live-capture-source-consent.md).

Recordings on the mapping-capable provider also retain a complete private binding
group for session-owned input workers. A separate Rust pointer worker checks the
same live sources, translates approved PipeWire node selectors to compositor
streams, and releases held input before renewing changed regions. Interrupted
commands are never replayed. Production Notify and explicit Iced pointer approval
now use this path in the unadvertised RemoteDesktop implementation. See
[the pointer worker decision](adr/0049-capture-bound-pointer-workers.md).

Mapped recordings publish an optional `mapping_id` and logical `size` so an
EI region can be paired with its video stream. Mapped windows publish size without
monitor position. The backend validates this metadata before Start succeeds; the
compositor object paths remain private. The receive-ordered pointer Notify
foundation is implemented and production pointer grants require verified bindings
and fresh Iced approval. See
[the public pairing decision](adr/0050-pointer-notify-and-stream-pairing.md).

The prepared mapped-pointer version 2 snapshots physical scale with logical size
and retires the old input source when either changes. This covers a window-only
recording whose logical size stays fixed while its video resolution changes.
The Rust worker negotiates versions 1 and 2 and renews verified projection changes
after cleanup. Version-1 scale remains unknown. See
[the atomic scale decision](adr/0051-atomic-input-region-scale.md).

The private Rust EI worker now provides paired absolute regions, relative motion,
buttons and scrolling alongside an independently owned keyboard. Size/scale
changes replace the pointer device after held-input cleanup, preserving keyboard
input. Real libei/media and cleanup checks pass. See
[the EI pointer decision](adr/0052-session-owned-ei-pointer-devices.md).

Experimental production RemoteDesktop sessions now offer separate unchecked
keyboard and pointer controls, with runtime capability checks before and after
consent. Public Notify input, held-input cleanup, legacy-to-EI handoff on the same
recording and relative-only input without capture pass scoped native checks in
both appearances. The normal backend registers RemoteDesktop alongside ScreenCast
and Clipboard, sharing their session controller and publishing input availability
changes. Broader projection, loss/race and physical-input acceptance remain open. See
[the production pointer decision](adr/0053-pointer-consent-and-production-input.md).

The private RemoteDesktop fixture also supports Clipboard v1. Requesting it adds
an independently unchecked Iced choice, with Alt+C as its explicit shortcut.
Clipboard-only grants need no input or recording permission. Approval opens one
session-owned transport; ongoing controls describe clipboard access and stop it
with the session. Initial metadata is delivered after the successful Start reply,
and retained application pipes are revoked on closure. Broader clipboard
compatibility and distribution acceptance remain pending. See
[clipboard consent and ordering](adr/0058-clipboard-session-consent-and-ordering.md)
and [the acceptance ledger](../.scratch/xdg-portals/clipboard/README.md).

The backend also publishes device availability changes for the frontend's cache.
Mixed monitor/window sessions pass size/scale replacement and UI/client-loss
checks while preserving keyboard input and video pairing. The original backend
death case exposed a missing public Session.Closed in the standard frontend.
The canonical frontend companion now tracks the original unique backend owner,
closes public sessions on its loss/replacement and prevents late-create revival.
The rebuilt portal passes backend-death and capture-worker-death cases with real
producer retirement on that frontend and the creator-cleanup compositor. The
installed frontend remains unchanged. See
[the availability decision](adr/0054-device-availability-and-mixed-source-input.md),
[frontend ownership](adr/0055-frontend-session-owner-lifetime.md) and
[creator cleanup](adr/0056-capture-creator-lifetime.md).

The development fixture adds ScreenCast only to its private descriptor. It starts
a private bus and nested desktop and does not change installed routing. Positive
consent/media acceptance requires a compositor with the creator-cleanup contract;
the installed older Niri is now rejected before consent. Historical NoGBM runs
that showed consent on that provider do not describe current admission behavior.

The activation smoke test checks ScreenCast, RemoteDesktop and Clipboard on the
actual executable. To also check the normal backend's production input presenter
and availability notifications through a real frontend, without opening input or
starting a desktop, run:

```sh
python3 scripts/test-remote-desktop-availability.py --normal-backend \
  --portal target/debug/xdg-desktop-portal-canopa --portal-sha256 PORTAL_SHA256 \
  --frontend FRONTEND_BINARY --frontend-sha256 FRONTEND_SHA256 \
  --output target/desktop-sharing-availability
```

This uses a fake input provider on a private bus and verifies capability changes,
owner loss and replacement. It does not establish capture or physical-input
acceptance. Omit `--normal-backend` and supply `--harness`/`--harness-sha256` to
retain the isolated unit-fixture mode.

Build the Niri and frontend companions from their pinned forks
([ADR0056](adr/0056-capture-creator-lifetime.md),
[ADR0055](adr/0055-frontend-session-owner-lifetime.md)), using stable work directories
and the same incremental root build caches. These helpers prepare/build without
installing or selecting either companion:

```sh
python3 scripts/build-niri.py --archives target/fork-archives --download \
  --work-dir target/niri-fork-sources --fetch
python3 scripts/build-xdg-portal-compat.py --archives target/fork-archives --download \
  --work-dir target/portal-frontend-sources
```

Add `--ref canopa/TOPIC` to the Niri builder to compare an earlier fork topic.
For existing prepared sources use the builder's `--reuse` option; `--prepare-only`
verifies the fork archive and source inventory without compiling. Read each binary path and checksum
from its build receipt. Run the following on the already-built Rust portal:

```sh
python3 scripts/test-screencast-portal.py --media \
  --niri NIRI_BINARY --niri-sha256 NIRI_SHA256 \
  --frontend FRONTEND_BINARY --frontend-sha256 FRONTEND_SHA256 \
  --binary target/debug/xdg-desktop-portal-canopa --scale 1.25 \
  --output target/cast-media-dark
```

Add `--color-scheme light` with a fresh evidence directory for the other theme.
The paired frontend arguments pin and verify the exact companion before/after
execution. No binary is copied into each evidence directory; native compositor
staging refers to the shared checksum-addressed store.

To verify refusal on a preserved older compositor that lacks creator cleanup,
run the maintained admission fixture with explicit binary checksums:

```sh
python3 scripts/test-capture-owner-admission.py \
  --portal target/debug/xdg-desktop-portal-canopa --portal-sha256 PORTAL_SHA256 \
  --niri UNSUPPORTED_NIRI_BINARY --niri-sha256 UNSUPPORTED_NIRI_SHA256 \
  --output target/cast-owner-refusal
```

It requires an otherwise supported capture API, verifies missing/unknown cleanup
support, then asserts a failed Start before any Iced window or recording object.
The same fixture fails on the retained pre-admission portal by exposing consent.
This negative test establishes refusal; the positive fixture above establishes
actual consent, frame delivery and cleanup on the compatible stack.

The media mode starts private hardware-GL Weston/Niri, PipeWire and WirePlumber's
policy-only profile. It snapshots/checks the chosen compositor, consumes the FD
returned by OpenPipeWireRemote using the returned serial, checks actual source
pixels and logical/physical geometry, denies a foreign caller, and verifies
producer retirement. A static window must send a newer, correct frame after
consumer pause/resume. Add `--color-scheme light --scale 1.25` for fractional
acceptance. Stop, Escape, window close, active caller disconnect and backend
SIGKILL must retire real producers and their controls. Cursor acceptance also
requests Hidden/Embedded/Metadata for both source types and reads the actual
frame plus SPA cursor bitmap through the restricted FD. These tests require hardware
render-node access and GStreamer GI/video/PipeWire plugins. Installed Niri 26.04
does not support this nested GBM path.

The current candidate's Metadata mode requires PipeWire1.4.8 or newer; older
libraries cause it to embed the cursor instead. Acceptance uses1.6.10. The backend
omits Metadata and rejects its selection when its linked library is older or has
an unknown version. Distribution must provide a compatible compositor runtime as
well. The candidate is built for private verification and is not selected or
installed by the Canopa application archive.

The combined candidate also passes source-cancellation/retained-offer controls and
scoped actual Files exchange/gesture checks. The first aggregate regression run
still contains startup failures; successful targeted reruns do not turn it into a
green full matrix. Distribution and release acceptance remain separate work.

Check the version policy independently, using a private library-version shim:

```sh
python3 scripts/test-screencast-capabilities.py \
  --binary target/debug/xdg-desktop-portal-canopa --output target/cast-capabilities
```

This checks advertised masks and Metadata selection across old, supported and
unknown versions. It does not replace actual older-library ABI/media acceptance.

Run library and wire tests with:

```sh
dbus-run-session -- cargo test --locked -p canopa-portal --lib --test integration
```

## Real Flatpak client

`scripts/prepare-screencast-flatpak.py` adds a tiny GIO/PipeWire application to an
existing fixture-owned Flatpak installation. It reuses an installed runtime and
copies six installed PipeWire client libraries into the application. It downloads
no runtime or SDK. The application has no filesystem, display or host PipeWire
socket permission. The backend, Iced UI and client use no GTK implementation.

```sh
python3 scripts/prepare-screencast-flatpak.py \
  --store target/EXISTING_PRIVATE_STORE --runtime EXISTING_RUNTIME_REF \
  --output target/sharing-flatpak/prepare.json
python3 scripts/test-screencast-flatpak.py \
  --binary target/debug/xdg-desktop-portal-canopa --binary-sha256 PORTAL_SHA256 \
  --niri FROZEN_NIRI_BINARY --niri-sha256 NIRI_SHA256 \
  --frontend FRONTEND_BINARY --frontend-sha256 FRONTEND_SHA256 \
  --flatpak-store target/EXISTING_PRIVATE_STORE \
  --client-build target/sharing-flatpak/client-build.json \
  --output target/sharing-flatpak/native
lua scripts/test-screencast-policy.lua
```

The compositor must already be in the shared checksum-addressed snapshot store.
The test binds a temporary runtime directory only inside its own user/mount
namespace so Flatpak sees standard runtime paths. The sandbox calls the real
public portal, receives the approved window through its PipeWire FD and checks
every pixel in a 400×300 pattern. Direct host socket/backend calls fail and an
unrelated host node stays hidden, including while the consumer creates its node.
Cancel returns no stream; Iced Stop closes the session and denies another remote;
app exit without Session.Close retires the producer and controls.

The optional Canopa WirePlumber policy is loaded from the same packaged sources.
`--stock-wireplumber --modes stop` retains the upstream permission-widening
counterfactual on WirePlumber 0.5.18. `--permission-diagnostics` records targeted
access decisions. This is a window-only, dark/100% GIO sandbox check on PipeWire
1.6.10 and WirePlumber 0.5.18; it does not establish every toolkit, runtime or
distribution. The fixture and shared store have a combined 136 MiB allocation
limit and preserve a 3 GiB root free-space floor.

`scripts/test-screencast-identities.py` exercises the actual serial resolver with a
private PipeWire daemon and GStreamer producers; supply its `--harness` unit
harness. It verifies producer disappearance and numeric node reuse with a new
64-bit serial. It does not capture a screen. See
[ADR0041](adr/0041-owned-screencast-recordings.md) and
[ADR0042](adr/0042-screencast-sessions-and-consent.md) for ownership and bounds;
[ADR0044](adr/0044-session-owned-sharing-controls.md) specifies ongoing controls.
