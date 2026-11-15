# Desktop sharing architecture

This document covers the ScreenCast, RemoteDesktop, Clipboard, InputCapture,
GlobalShortcuts and Usb backends in `crates/canopa-portal`, and the compositor
and frontend changes they depend on. For the user and packager view, see
[Desktop sharing](../desktop-sharing.md). For the general backend structure
(request admission, dialog helpers, descriptors and routing), see
[Portals architecture](portals.md).

Earlier decision records were consolidated into this document; the originals are
in the git history at commit cb300936 under docs/adr/.

## Components

| Component | Role | Source |
| --- | --- | --- |
| Portal backend | Public `org.freedesktop.impl.portal.*` interfaces, session actors, consent, workers | [`crates/canopa-portal/src/`](../../crates/canopa-portal/src/) |
| Canopa compositor | Niri fork: capture lifetime, source identity, input leases, capture barriers, shortcut leases | `kevinard/niri`, pinned in [`packaging/forks.json`](../../packaging/forks.json) |
| Smithay fork | Input source release used by the compositor | `kevinard/smithay`, pinned through Niri's lockfile |
| Portal frontend | xdg-desktop-portal fork: ordered input, session-owner lifetime, InputCapture phase, USB grant ownership | `kevinard/xdg-desktop-portal`, pinned in `packaging/forks.json` |
| reis fork | Bounded `read_chunk` reads for EI connections | `kevinard/reis`, pinned in the root `Cargo.toml` |
| WirePlumber hook | Keeps portal PipeWire clients restricted to their granted streams | [`packaging/portal/desktop-sharing/`](../../packaging/portal/desktop-sharing/) |
| Packaging | Arch install and portable component | [`scripts/install-system.py`](../../scripts/install-system.py), [`scripts/package-desktop-sharing.py`](../../scripts/package-desktop-sharing.py) |

The interface versions are ScreenCast 6, RemoteDesktop 2, Clipboard 1,
InputCapture 2, GlobalShortcuts 2 and Usb 1. The backend serves all six on the
bus; the descriptor decides which ones the frontend routes to it.
`package-desktop-sharing.py` defines the default set (`INTERFACES`: ScreenCast,
RemoteDesktop, Clipboard) and the experimental set (`EXTRA_INTERFACES`:
InputCapture, GlobalShortcuts, Usb). The Arch install writes `descriptor()` with
the default set; the portable component adds the extra set with
`--extra-portals`. Building the forks is described in
[Upstream dependency forks](../vendor-maintenance.md).

## Process model

The backend process holds D-Bus state and session actors only. Everything that
touches the compositor, PipeWire, EI or a toolkit runs in a child started from
the backend's own executable with a hidden mode flag:

| Mode | Purpose | Module |
| --- | --- | --- |
| `--screencast-worker` | One recording: Mutter ScreenCast session, PipeWire serials, private input bindings | `screencast_capture.rs`, `screencast_capture/` |
| `--screencast-dialog`, `--remote-desktop-dialog` | Iced source and permission consent | `screencast_ui.rs` |
| `--screencast-status` | Iced sharing controls for one session | `screencast_status.rs`, `screencast_status/` |
| `--remote-input-worker` | Compositor input leases and the EI sender | `remote_input.rs`, `remote_input/` |
| `--input-capture-worker` | Capture lease and the EI receiver | `input_capture_transport.rs`, `input_capture_transport/` |
| `--input-capture-dialog` | Iced InputCapture consent | `input_capture_ui.rs` |
| `--global-shortcuts-dialog` | Iced shortcut editor and key recording | `global_shortcuts_ui.rs`, `global_shortcuts/editor/` |
| `--usb-dialog` | Iced device consent | `usb_ui.rs` |

The clipboard transport runs in a dedicated thread of the backend
(`clipboard_transport/worker.rs`). Helpers inherit only the descriptors they are given and exchange bounded frames
with the parent.

## Compositor contracts

The backend uses Niri's Mutter-compatible ScreenCast service plus private Canopa
services from the fork. It reads each version property before consent and fails
closed when one is missing or unknown.

| Service or protocol | Version check | Used for |
| --- | --- | --- |
| `org.gnome.Mutter.ScreenCast` | `Version` ≥ 4, `CanopaCaptureOwnerVersion` = 1 | Recordings, stopped when their creating connection disappears |
| same | `CanopaSourceVersion` = 1 | Live monitor and window identity; without it, view-only legacy capture |
| `org.canopa.Niri.RemoteInput1` | `PointerVersion` = 1, `MappedPointerVersion` = 2, `TouchVersion` = 1 | Keyboard, relative and mapped pointer, and touch leases |
| `org.canopa.Niri.InputCapture1` | `Version` 1 (pointer) or 2 (adds keyboard) | Barriers, zones, activation and capture leases |
| `org.canopa.Niri.GlobalShortcuts1` | `Version` = 1 | Shortcut reservations and activation |
| `ext-data-control-v1` (Wayland) | protocol presence | Clipboard bridge |

Every private service is reached through the unique bus owner resolved before
consent. The backend checks that the Niri IPC peer belongs to that owner and
revalidates it before each mutation, so a replacement compositor never inherits
a grant.

## Sessions and consent

ScreenCast and RemoteDesktop share one session registry and actor
(`screencast/session.rs`, `screencast/remote.rs`); InputCapture
(`input_capture/`) and GlobalShortcuts (`global_shortcuts/session.rs`) have their
own. Each registry admits at most 16 sessions. A session path is bound to the
caller's unique name and app ID, and every method rechecks both. A session moves
through New, Selected, Starting, Active and Closed in its actor; the Start request
owns nothing after it replies. If a request is canceled after the actor accepted
a result, a response guard closes the session instead of leaking it. Any failure
during Start tears down the whole session.

Consent is fresh for every session. Sources, keyboard, pointer, touch, clipboard,
shortcut actions and USB devices all start unchecked; ScreenCast confirms with
Ctrl+Enter, never a plain initial Enter. `persist_mode` is accepted but the reply
always returns 0, so there are no restore tokens.

### Sharing controls

Each active session owns a `--screencast-status` helper: a floating Iced window
that names the requester and the exact granted scope. Streams are returned to
the caller only after the helper reports ready. Stop, Escape, closing the window
or the helper exiting revokes the whole session. The short-lived Start request
holds no lease on this window.

## Screen capture

Each recording has its own worker with its own connection to the compositor.
The backend holds the worker through a lease pipe, and the worker sends Stop when
the lease closes, including after the backend dies. The compositor fork also
stops a recording when the D-Bus connection that created it goes away, so a
killed worker never leaves a live producer (`CanopaCaptureOwnerVersion`). At most
8 recording workers and 16 sources run at once.

With `CanopaSourceVersion=1`, each monitor carries a process-local output
identity and each window its mapped ID through the catalog, consent, revalidation
and the record call. After the stream is added, the worker checks the stream's
`canopa-source-id`. If a monitor is replaced under the same connector, its
selection is cleared. A provider without the property falls back to connector
names; that path is view-only and never authorizes absolute pointer input
(`screencast_capture/monitor.rs`).

The backend resolves each producer's PipeWire `object.serial` through the Rust
PipeWire binding (`screencast/production/serial.rs`) and returns it to the
frontend, which gives the application a restricted PipeWire FD for those nodes
only. Monitors publish logical position and size; windows publish only size.
Each stream also carries an optional private `mapping_id` and logical size so an
EI region can be paired with its video. Compositor object paths never leave the
worker (`screencast_capture/bindings.rs`).

The metadata cursor mode is advertised only when the linked `libpipewire` is
1.4.8 or newer (`screencast/capabilities.rs`); the compositor falls back to an
embedded cursor below that.

### PipeWire access policy

The frontend grants a sandboxed client access to the selected nodes. On
WirePlumber 0.5.18, the stock portal permission manager then widens the client's
fallback permissions and exposes unrelated nodes. Canopa ships a WirePlumber hook
(`canopa-screencast.lua`, loaded by `90-canopa-screencast.conf` in the `main` and
`policy` profiles). For clients with `pipewire.access=portal` and no media role,
it sets fallback permissions to none and grants the core object, leaving the
frontend's per-stream grants in place. Camera clients and administrator
configuration are untouched. WirePlumber versions without the permission-manager
API keep their own policy. The hook must be installed with the backend; the Arch
install does so.

## Remote input

Remote input is owned by the session, not by a request. The compositor owns each
input source and its held keys, buttons and touch contacts; a worker holds the
source's lease FD, and closing it releases only that source.

- **Notify methods.** The backend admits Notify calls synchronously in D-Bus
  receive order into one queue of at most 64 commands per session and awaits
  replies in detached tasks (`screencast/remote/dispatch.rs`). The frontend fork
  forwards them in receive order through one FIFO per session. Overflow revokes
  the session, because a dropped release would leave a key held. Notify uses
  lazily started legacy workers (`remote_input/legacy.rs`).
- **EI.** ConnectToEIS retires the legacy workers, then hands the application one
  anonymous EI socket; later Notify calls are denied. One sender connection
  carries separate keyboard, pointer and touch devices, each with its own budget
  (`remote_input/server/`). Keymaps are sealed memfds and modifier state is
  authoritative. Pending input per device is bounded before the reis buffer.
- **Pointer mapping.** Absolute input needs a shared source and
  `MappedPointerVersion=2`. Opening a mapped pointer returns stream, mapping,
  logical size and physical scale in one reply; EI absolute devices use one
  region per stream. When a projection's size or scale changes, the worker
  releases held input, waits for the compositor to confirm the source closed,
  opens a new one and replaces the EI device. Input is never replayed. Sessions
  without a shared source get relative pointer only.
- **Touch.** `TouchVersion=1` provides mapped touch only, with contact IDs
  namespaced per source. Physical touch takes priority over a remote owner of the
  same slot. Touch needs its own approval and a shared source. EI touchscreen
  version 1 is negotiated.
- **Release.** When a source closes, the Smithay fork sends a final key-up only
  for presses that were forwarded to the client, so bindings never receive a lone
  release.
- **Availability.** `screencast/remote/availability.rs` watches the input
  service's owner and properties and publishes `AvailableDeviceTypes` changes,
  because the frontend caches that property.

## Clipboard

Clipboard extends a RemoteDesktop or InputCapture session; it is never a session
of its own (`clipboard.rs`). RequestClipboard must come before Start and gets its
own unchecked choice. Each grant opens a private `ext-data-control-v1`
connection in the clipboard thread, pinned to the compositor process and limited
to the regular selection. Transfer pipes run through that thread, so revocation
also cuts off pipes the application still holds. Initial selection signals are
sent only after the Start reply, behind a ready barrier matched by message
serial, and only to the original owner (`clipboard/dispatch.rs`). For
InputCapture, the clipboard connection exists only while capture is active; each
activation creates a fresh transport (`input_capture/clipboard.rs`).

## Input capture

The compositor fork owns capture: barriers, zones, their generations, activation
IDs and the enabled or active state, and it decides when a barrier is crossed.
Up to 8 leases may exist with one enabled. Ctrl+Alt+Escape, the lock screen and
the screenshot UI suspend an active capture. The privileged socket never reaches the
application.

A worker per session (`input_capture_transport/worker.rs`) revalidates the
compositor, backend and caller, then gives the application an EI receiver
connection. Each activation ID becomes the EI sequence number. Held input is
released before emulation stops, and the backend never sees input data.
Keyboard capture (service version 2) keeps a separate XKB state for captured
keys and pairs each captured press with its release, so a release that arrives
after deactivation never reaches the local client and a new activation cannot
adopt it. A keymap change pauses capture and replaces the EI device. Topology
changes invalidate zones and barriers until the application fetches new ones.

The frontend fork keeps the public session phase in step with confirmed
compositor transitions: a repeated Enable stays Active, and Release waits for
the Deactivated signal. InputCapture supports the deprecated CreateSession as
well as CreateSession2.

## Global shortcuts

The compositor fork owns admission, resolution against the current keymap,
reservations and press recipients. Each session owns one authenticated private
lease (`global_shortcuts/transport.rs`); the Iced helper never receives a
compositor descriptor. Replacing a set of shortcuts is all-or-nothing against the
observed generation. Releases go to the client that received the press.
Activation signals carry the source event time in u64 milliseconds; no
activation token is fabricated.

Bind is attempted once per session and shows every action unchecked. Configure
reopens the editor and replaces the set atomically on Apply; Cancel keeps the
existing set. An ambiguous compositor result or a persistence failure revokes the
session. Explicit choices are saved in `$XDG_STATE_HOME/canopa/portal-shortcuts.db`
(`global_shortcuts/history.rs`) only as suggestions; they grant nothing.

The editor requests `keyboard-shortcuts-inhibit-v1` for its own window, so while
it is focused the compositor suspends shortcut activation without dropping
registrations. Key recording happens in the helper at the Winit level before
ordinary input handling. It arms only after held keys are released and records
base-layer symbols with the real modifier state.

## Usb

The backend only decides consent (`usb.rs`, `usb_ui.rs`). It returns a subset of
the device IDs the frontend offered, optionally downgraded to read-only. The
frontend discovers and filters devices, stores permissions, delivers device
events and opens, transfers and releases FDs. The frontend fork validates the
whole grant against the original request and owns cached FDs. The backend never
opens a device path.

## Limits

| Area | Limit |
| --- | --- |
| ScreenCast, InputCapture, GlobalShortcuts sessions | 16 each |
| Recording workers / sources | 8 / 16 |
| RemoteDesktop Notify queue | 64 commands per session |
| Remote input worker children | 8 |
| Clipboard | 16 transfers, 128 MIME types of up to 256 bytes |
| InputCapture leases | 8, one enabled |
| GlobalShortcuts | 64 actions per session; history of 128 apps |
| Usb | 64 devices per request |

## Testing

Unit and D-Bus tests run with the crate's library and integration tests (see
[Testing](../testing.md)). Native checks start a private bus, a nested compositor
and the real frontend, and take explicit executable paths and checksums:

| Script | Covers |
| --- | --- |
| `scripts/test-screencast-portal.py --media` | Consent, frames, cursor modes and cleanup on a private desktop |
| `scripts/test-capture-owner-admission.py` | Refusal of a compositor without creator cleanup |
| `scripts/test-screencast-flatpak.py`, `prepare-screencast-flatpak.py`, `test-screencast-policy.lua` | Real Flatpak client and the WirePlumber policy |
| `scripts/test-remote-desktop-portal.py`, `test-remote-desktop-pointer.py`, `test-remote-desktop-availability.py` | Keyboard, pointer, EI handoff and availability |
| `scripts/test-clipboard-portal.py` | Clipboard consent, transfers and revocation |
| `scripts/test-input-capture-portal.py` | Activation, release, topology, keyboard and clipboard |
| `scripts/test-global-shortcuts-portal.py` | Review, conflicts, recording, signals |
| `scripts/test-usb-portal.py`, `test-usb-frontend.py` | Usb consent and frontend ownership with synthetic devices |
| `scripts/test-desktop-sharing-package.py` | Portable component staging and verification |

The capture checks need a hardware render node. Most fixtures accept
`--portal-descriptor PATH` to use a packaged descriptor.

The Usb fixtures use UMockdev synthetic devices and need a private UMockdev
prefix (library, preload interposer, typelib and CLI), passed with
`--umockdev`. Use UMockdev 0.19.9 or newer, which handles libpcap 1.11 USB
headers. Its build always needs `valac`, so the practical route is to extract the
Arch `umockdev` x86_64 package into a private directory instead of installing it.

## Design decisions

**Rust and Iced backends; the standard frontend keeps identity.** Canopa
implements the backends itself and relies on the forked xdg-desktop-portal for
caller and sandbox identity and restricted PipeWire access. This reuses the
frontend's public policy and avoids GNOME or GTK portal processes. The frontend
fork is therefore a hard dependency.

**Sessions own resources; requests do not.** Grants live in one actor per
session and are revoked only by session closure, caller or owner loss, or the
sharing controls. A short-lived request can neither revoke nor leak a grant.

**No persisted grants.** Every session asks again, and choices start unchecked.
Nothing stored can grant access later; the cost is repeated prompts.

**One worker per recording, with compositor-side creator cleanup.** A worker
killed before it sent Stop used to leave a live producer. Requiring
`CanopaCaptureOwnerVersion=1` closes that gap; a compositor without it is refused
before any dialog.

**Live source identity instead of connectors or node IDs.** Connector names and
PipeWire node IDs are reused, so they cannot authorize input. A changed identity
clears the selection, and legacy connector capture stays view-only.

**Static frames replayed on resume.** The compositor fork redraws a stream after
each transition to streaming, so a consumer that pauses during negotiation still
receives a frame of a static window.

**Restricted PipeWire access is enforced in WirePlumber too.** The stock
permission manager widened access for sandboxed clients. The shipped hook keeps
fallback access at none, so the hook is part of the install.

**Input order is preserved end to end.** Both the backend and the frontend fork
admit Notify calls in receive order through bounded queues and revoke on
overflow. Reordered or dropped transitions would leave keys held.

**Compositor-owned input leases with a one-way move to EI.** Held input belongs
to the compositor source, so closing a lease releases exactly its own input.
After ConnectToEIS, Notify is refused, so the two paths never mix.

**Projection changes replace the pointer device.** Size and scale are snapshotted
together and a change renews the source and EI device after held input is
released, never replaying input. Window streams keep a fixed logical size while
their video resolution changes.

**Touch is separate and mapped only.** Touch needs its own approval and a shared
source, and physical touch wins a contested slot. Granting keyboard or pointer
never grants touch.

**Frontend sessions follow the original backend owner.** The frontend fork pins
each session to the backend's unique name at creation and emits Closed when that
owner disappears or is replaced. Close goes only to the original owner, so a
restarted backend never receives an old session.

**Clipboard follows its session.** The clipboard has no session of its own and
for InputCapture exists only while capture is active. Pipes run through the
transport so revocation stops further data, though bytes already read stay with
the application.

**Capture barriers and shortcut reservations live in the compositor.** The
compositor sees input first and resolves keymaps, so it decides barrier crossings
and shortcut matches. The backend and helpers hold only leases.

**Shortcut history is advisory.** Saved triggers fill the editor but never bind
without fresh approval. The editor inhibits shortcuts on its own window so
existing shortcuts can be recorded without firing.

**Usb access stays in the frontend.** The access bugs were in the frontend, and a
second device broker in the backend would have duplicated them. The backend
returns only a subset of offered devices.

## Known limitations

- InputCapture, GlobalShortcuts and Usb are not routed by the Arch packages.
- InputCapture cannot capture touchscreens, and EI touchscreen cancel is not offered.
- Window streams report no position, and occlusion is not handled.
- Revocation cannot recall clipboard bytes or USB FDs already given to the application.
- A compositor call that times out can leave cleanup unconfirmed.
- The ScreenCast, RemoteDesktop, sharing-control and GlobalShortcuts dialogs publish no accessibility tree.
