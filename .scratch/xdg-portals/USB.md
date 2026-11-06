# Usb portal: research and proposed Rust/Iced seam

Research only, 2026-11-01. Usb is missing from Canopa's exported interfaces and
descriptor. The smallest faithful replacement is **one consent method in Rust
and one Iced dialog**. Keep the standard XDG frontend responsible for device
discovery, sandbox filtering, permission storage, monitoring, device opens and
FD delivery/release. No backend USB library, hardware service, compositor API,
GTK UI or new device broker is needed for this seam. This is a proposal, not an
implementation or acceptance result. [Canopa registration](/home/kevinard/projects/canopa/crates/canopa-portal/src/main.rs:550),
[descriptor](/home/kevinard/projects/canopa/packaging/portal/canopa.portal:3),
[backend XML](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.Usb.xml:34).

## Sources and version boundary

Implementation target: the supplied, Canopa-patched canonical frontend at
`/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1`.
All detailed C behavior below refers to this tree, including its shared Request
and Session code. Upstream reference is the [official 1.22.1 Usb source](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/src/usb.c).
Read-only SHA-256 receipts:

| Canonical file | SHA-256 |
| --- | --- |
| `src/usb.c` | `7d2d6bd6f9590ba647b0a4a813b2a28d31fc53f6c688127e1d00e1bfe202542f` |
| `data/org.freedesktop.portal.Usb.xml` | `2deed1720b0d8dc53794c1e12ce68d64b627d9f6d6f215de7df6edc00856c047` |
| `data/org.freedesktop.impl.portal.Usb.xml` | `4ead83c5cc9a2c9bd9d82d1091806c2fcff2fca195f0e01ef4f8e4bea8391031` |

Current official [public XML](https://raw.githubusercontent.com/flatpak/xdg-desktop-portal/main/data/org.freedesktop.portal.Usb.xml)
and [backend XML](https://raw.githubusercontent.com/flatpak/xdg-desktop-portal/main/data/org.freedesktop.impl.portal.Usb.xml)
still document v1 and the same wire methods. Current frontend C moved to
[`desktop-portal/usb.c`](https://github.com/flatpak/xdg-desktop-portal/blob/main/desktop-portal/usb.c);
the observed moving-main source retains the consent-only backend call and the
subset/write-escalation TODOs. It is not a byte-verified replacement for the
canonical tree. Current GNOME C was also read as a behavioral reference, not
copied or required: [usb.c](https://raw.githubusercontent.com/GNOME/xdg-desktop-portal-gnome/main/src/usb.c),
[usbdialog.c](https://raw.githubusercontent.com/GNOME/xdg-desktop-portal-gnome/main/src/usbdialog.c).

## Exact wire contract and ownership

All public methods are on `org.freedesktop.portal.Usb`; the backend implements
`org.freedesktop.impl.portal.Usb`, at `/org/freedesktop/portal/desktop`, with
readable `version: u = 1`. Backend AcquireDevices argument order differs from
Access: **handle, parent_window, app_id, devices, options**. No Usb method on
either side takes an input FD. Only public FinishAcquireDevices returns device
FDs. [Pinned public XML](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.portal.Usb.xml:36),
[pinned backend XML](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.Usb.xml:70),
[frontend version](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:1518).

| Interface/method | D-Bus inputs → outputs | Owner and meaning |
| --- | --- | --- |
| Public `CreateSession` | `a{sv}` → `o` | Frontend-only monitoring Session; optional `session_handle_token: s`. No consent or implementation Session. |
| Public `EnumerateDevices` | `a{sv}` → `a(sa{sv})` | Frontend snapshot of visible inventory. No supported options. |
| Public `AcquireDevices` | `s, a(sa{sv}), a{sv}` → `o` | Parent, requested IDs/access options, options (`handle_token: s`); frontend returns a Request handle and later emits Response with empty results. |
| Backend `AcquireDevices` | `o, s, s, a(sa{sv}a{sv}), a{sv}` → `u, a{sv}` | Consent request: frontend adds device information and trusted app ID. No supported top-level options. |
| Public `FinishAcquireDevices` | `o, a{sv}` → `a(sa{sv}), b` | Frontend consumes approved pending IDs for this sender/request; returns per-device success/FD or error and `finished`. |
| Public `ReleaseDevices` | `as, a{sv}` → nothing | Frontend releases sender-owned device records, including when blanket permission was disabled. |
| Public `DeviceEvents` signal | `o, a(ssa{sv})` | Frontend sends session handle plus action (`add`, `change`, `remove`), opaque ID and metadata to the session's sender. |

Backend success returns `response = 0` and `results["devices"]:
a(sa{sv})`, selecting IDs/access options from the input. `writable: b` defaults
to false and means read-write rather than read-only. Cancellation is 1; failure
is 2. Return no fabricated ID or elevated access. A partial subset is supported
by the backend contract; an explicit all-offered grant is sufficient for the
smallest first UI. [Backend result contract](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.Usb.xml:54),
[Canopa response pattern](/home/kevinard/projects/canopa/crates/canopa-portal/src/access.rs:121).

## What the canonical frontend actually does

- **Inventory and identity:** GUdev scans subsystem `usb` and watches uevents;
  only devices with a device file and `DEVTYPE=usb_device` qualify. It assigns
  random opaque IDs, maps sysfs paths internally, and removes mappings on
  unplug. These IDs are neither a path, serial number nor a persistent user
  label. No Canopa re-enumeration is needed.
  [Suitability](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:137),
  [IDs/events](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:574),
  [initial scan](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:1526).
- **Visibility and permissions:** Flatpak app info reads `[USB Devices]`
  `enumerable-devices` and `hidden-devices`; hidden matches win. Matching uses
  vendor/product and class/subclass rules; a stored device `no` also hides it.
  Host app info explicitly supplies `all`. Blanket permission is table `usb`,
  ID `usb`, keyed by app ID; only `no` blocks methods. Device keys are
  `usb:<ID_VENDOR_ID>/<ID_MODEL_ID>/<ID_SERIAL>` with missing components empty.
  Do not equate missing serial with a unique physical unit.
  [Flatpak queries](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-app-info-flatpak.c:376),
  [host default](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-app-info-host.c:101),
  [matching](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:325),
  [permission key](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:158),
  [table names](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-types.h:142).
- **Monitoring:** CreateSession exports a frontend Session, sends initial `add`
  events when nonempty, then filtered events addressed to its unique sender.
  Closing it removes only the monitoring subscription; acquired devices live
  in sender state, not in that Session. Foreign Close is denied and caller
  disconnect closes it. No `org.freedesktop.impl.portal.Session` is constructed
  for Usb, so the implementation-owner companion fix in ADR0055 does not turn
  Usb monitoring into backend-owned sessions.
  [Creation](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:781),
  [monitor close](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:430),
  [sender authorization/loss](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-session.c:355).
- **Acquisition:** The frontend rejects empty/unknown/invisible device requests,
  validates boolean `writable`, and rejects requested write access when its
  `access(W_OK)` check fails. It enriches each ID with metadata, exports the
  public Request and calls the backend. On success it records approved IDs in
  sender-owned pending state and stores per-device `yes`; actual opens happen
  later. This pin always calls the backend even after a previous stored `yes`;
  cancel does not persist `no`. Do not add an independent Canopa permission
  cache or promise a permanent-denial UI.
  [Filtering](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:1031),
  [backend handoff](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:1151),
  [completion/persistence](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:938).
- **FD handoff:** Finish looks up pending work through the calling sender's
  app-info object and request path, rechecks blanket permission, checks device
  existence/visibility, opens `O_RDONLY` or `O_RDWR`, retains a frontend FD and
  transfers a D-Bus FD index. Failures are per device. At most eight successful
  FDs are returned per call; failures do not consume that counter. Continue
  with the same handle until `finished=true`; then the pending entry is removed
  and later Finish fails. The XML's nearby “once” sentence must not be used to
  prohibit the documented batching loop. A previous acquired FD may be reused.
  [Finish implementation](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:1248),
  [batch limit](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:55).
- **Release and sender loss:** Release removes records from that sender's owned
  map and drops frontend FD references. Pending/owned maps are attached to
  app-info; peer disconnect signals Request/Session cleanup and deletes its
  registry entry. In-flight references can delay final destruction. The backend
  never receives ReleaseDevices and never owns these FDs.
  [Owned state/destruction](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:198),
  [release](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:1422),
  [disconnect](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-context.c:325).

**The release boundary must be described honestly.** The pin has no kernel-side
USB revocation; closing the frontend's FD cannot revoke an FD already duplicated
into a client. Device removal has its own kernel consequences. Do not promise
that Deny, ReleaseDevices, monitoring Session.Close, permission changes or
backend death forcibly disables an already delivered device FD. The upstream
design itself records this limitation and describes logind TakeDevice as future
work, not a backend requirement. [Canonical design footnotes](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/doc/usb-portal.md:101).

There are additional **frontend acceptance questions**, not reasons to move
device authority into Rust: subset and write-escalation checks on backend results
are explicit TODOs; completion updates permission/pending state before its final
`request->exported` check, so late-success-after-Close needs a negative test;
reacquiring an owned device reuses the old FD and increments its refcount, so
read-only→write upgrades and repeated acquire/release cleanup need testing.
These are source observations, not reproduced defects or claimed fixes.
[Completion](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:975),
[reuse/refcount](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:1321).

## Consent, labels and lifetime in Canopa

The frontend supplies an opaque ID, optional `parent`, `readable`/`writable`
flags (absence means false), `device-file`, and nested string properties. Its
allowlist is `ID_INPUT_JOYSTICK`, `ID_MODEL_ID`, `ID_MODEL_ENC`,
`ID_MODEL_FROM_DATABASE`, `ID_REVISION`, `ID_SERIAL`, `ID_SERIAL_SHORT`,
`ID_TYPE`, `ID_VENDOR_ENC`, `ID_VENDOR_ID`, `ID_VENDOR_FROM_DATABASE`.
These are display hints, not authenticated hardware identities. Never open the
supplied device path or fetch USB descriptors just to label the dialog.
[Metadata projection](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:501).

GNOME's current implementation echoes offered IDs/access options and uses one
Allow/Deny decision. Its UI resolves the app's desktop name, falls back to app
ID/unknown app, and chooses database model/vendor names before encoded names
and numerical IDs, with optional short serial. It does not require a device
enumerator in the backend. Use this behavior as reference, not GTK/C code to
port. [GNOME method](https://raw.githubusercontent.com/GNOME/xdg-desktop-portal-gnome/main/src/usb.c),
[GNOME labels](https://raw.githubusercontent.com/GNOME/xdg-desktop-portal-gnome/main/src/usbdialog.c).

Proposed seam, following [ADR0026](/home/kevinard/projects/canopa/docs/adr/0026-file-chooser-portal.md:1),
[ADR0027](/home/kevinard/projects/canopa/docs/adr/0027-desktop-application-catalog.md:1)
and [ADR0029](/home/kevinard/projects/canopa/docs/adr/0029-access-portal.md:1):

1. A typed Usb request model parses the wire array, preserves offered IDs and
   requested access, bounds counts/text/aggregate payload, rejects duplicate IDs
   and malformed recognized metadata/options. Ignore unknown extension keys.
   A pure validator accepts only a unique returned subset and permits writable
   only when originally requested. Serialize only the validated result to
   `results["devices"]`; do not trust helper output because the pinned frontend
   does not enforce these checks.
2. Use shared `Requests`, shared admission and owner-authenticated backend
   Request.Close. The backend D-Bus caller is the **frontend's unique sender**,
   not the sandbox app. The frontend authenticates the public sender, supplies
   app ID, and forwards public Request.Close/client loss. Watch frontend loss in
   the backend; settle cancellation without a grant and unexport the Request.
   Public Close forwards synchronously; return its backend Close promptly.
   [Canopa owner/loss checks](/home/kevinard/projects/canopa/crates/canopa-portal/src/requests.rs:96),
   [public authorization](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-request.c:140),
   [Close/loss forwarding](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-request.c:73).
3. An injectable `Presenter` returns an explicit grant or cancellation. Reuse
   `DialogProcess`'s bounded anonymous-pipe JSON exchange and per-request helper;
   add a Usb private mode in a later implementation. No device FD inheritance
   is necessary. Helper failure/invalid response is 2; Deny, Escape, close,
   Request.Close or caller loss is 1. Check the cancellation latch before a
   successful return. Backend termination/bus loss must retire helpers.
   [Presenter pattern](/home/kevinard/projects/canopa/crates/canopa-portal/src/access.rs:63),
   [helper bounds/cancellation](/home/kevinard/projects/canopa/crates/canopa-portal/src/process.rs:95),
   [helper parent-death handling](/home/kevinard/projects/canopa/crates/canopa-portal/src/process.rs:33).
4. The Iced helper displays app label plus app ID, every offered device, and
   requested read-only/read-write access; generic fallbacks keep sparse metadata
   understandable. Apply bounded plain-text rendering, controlled udev escape
   decoding and a serial/detail affordance for otherwise identical units.
   Keep the list scrollable and decision actions visible. Only deliberate Allow
   grants; initial Enter cannot. Use shared appearance and `open_dialog` with
   exported Wayland parent, keeping missing parents usable. Reuse
   `canopa-apps::discover_choice_metadata` off the UI thread for labels/icons;
   metadata never replaces protocol IDs or authenticates a caller.
   [Iced consent](/home/kevinard/projects/canopa/crates/canopa-portal/src/access_ui.rs:120),
   [parenting](/home/kevinard/projects/canopa/crates/canopa-portal/src/access_ui.rs:42),
   [catalog seam](/home/kevinard/projects/canopa/crates/canopa-apps/src/lib.rs:86).

No v1 backend update method carries unplug events into an open dialog; keep its
offered snapshot and let frontend Finish report disappeared devices. Do not add
a second hardware watcher. Public monitoring and already delivered FDs remain
frontend/client concerns after consent completes.
[Sole backend method](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.Usb.xml:70).

Dependency inspection found zbus, Tokio, serde/JSON, Iced, shared UI/apps/assets
already declared; no libusb/udev Rust dependency is needed. Read-only
`pkg-config --modversion` reports GUdev `238` and libusb `1.0.30` here; this is
package metadata, not device or runtime capability evidence. GUdev is a
frontend build condition. Read-only command lookup did not find `umockdev-run`,
and `pkg-config --exists umockdev-1.0` did not find its development metadata;
fixture availability must be established later without assuming it from source.
[Portal dependencies](/home/kevinard/projects/canopa/crates/canopa-portal/Cargo.toml:25),
[frontend build guard](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/meson.build:144).

## Proposed acceptance, with evidence scopes kept separate

No checks below were run in this research. The upstream pin already provides
UMockdev synthetic USB inventory and a mock backend, useful as fixture designs:
[inventory fixture](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/test_usb.py:66),
[public acquisition test](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/test_usb.py:304),
[backend mock](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/templates/usb.py:53).

| Gate | Meaningful evidence to collect | What it does not prove |
| --- | --- | --- |
| Pure model + private D-Bus | Exact introspection/order/version; sparse/wrong-type properties; duplicate IDs; unknown extension keys; writable default; immutable ID/subset/no-escalation validation; cancellation racing grant; malformed/oversized helper replies; duplicate handles, shared 16-slot admission, foreign Close, sender/bus/helper loss. | Public frontend permissions, real device operations, visible consent. |
| Canonical public frontend + synthetic inventory | Use a private session bus/permission store and **UMockdev-isolated inventory**; bus isolation alone still exposes host GUdev inventory. Drive Enumerate/CreateSession/Acquire/Response/Finish/Release against Canopa. Synthetic nodes contain known bytes; validate received FD mode/content without USB ioctls. Cover zero/sparse/identical devices, allowed/hidden/nonmatching queries, initial/add/change/remove events, per-sender isolation, blanket/device `no`, subset/write downgrade, more than eight successful FDs, missing/open-failing nodes, permission change or unplug before Finish, invalid/foreign/consumed handles, monitor close, client/backend loss and late success after cancellation. Assert no open outside fixture nodes. Record canonical frontend hashes and actual routing. | Hardware transactions, kernel revocation, Flatpak's real identity/permission boundary, an installed user's routing. |
| Native Iced consent using synthetic metadata | Render both appearances with a real Wayland parent and missing/expired parent; exercise Allow/Deny, initial Enter, Escape/window close, public Close/client loss, frontend/backend death, concurrent dialogs, long/encoded/missing names, same-model distinct serials, and explicit read/write labels. Bind screenshots and helper retirement to exact executable and request receipts. | USB acquisition unless paired with the public synthetic-inventory gate; sandbox or physical-device success. |
| Real-device/Flatpak/distribution follow-up | Later, explicitly scoped safe device and real consumer using the standard frontend; test Flatpak USB declarations and sender identity, host ACL/read/write failures, libusb FD wrapping, unplug/replug and repeated acquisition/release. Verify frontend GUdev build, compatible frontend/package versions, backend activation/descriptor and per-interface selection. | A synthetic success must never stand in for any of these results. |

Record failed cases as frontend or backend failures according to the ownership
above. In particular, test the pinned FD reuse/refcount and late-completion
questions rather than accepting nominal success alone. Keep existing stable
source/build paths and bounded receipts; do not copy executables into each run.
Usb advertisement and routing should be a later deliberate acceptance step,
following the repository's existing portal policy; this note changes neither.

Research performed: read-only source/docs/package metadata and official web
source inspection; wrote only this note. No build, test, install, USB inventory
probe, real-device open/ioctl, live service call or routing change was performed.
