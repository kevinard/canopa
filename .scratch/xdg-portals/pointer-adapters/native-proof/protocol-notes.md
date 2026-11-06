# Public pointer integration contract

Checked 2026-10-31 against current primary documentation and the pinned local
reis implementation. This is part of the full portal implementation goal.

The [ScreenCast public contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
and [backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.ScreenCast.html)
define optional mapping_id strings to pair video streams with EI regions. A
mapping may match at most one region per device. The size property uses compositor
logical dimensions; only monitor streams carry position. Private compositor
session and stream object paths are not public stream properties.

The [RemoteDesktop backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.RemoteDesktop.html)
defines relative/absolute motion, evdev buttons, smooth axis input with an optional
finish boolean, and vertical/horizontal discrete steps. Absolute input names a
PipeWire node and uses that stream's logical coordinates. Pointer and keyboard
grants are separate device bits.

The [EI device protocol](https://libinput.pages.freedesktop.org/libei/interfaces/ei_device/index.html)
requires regions for virtual absolute devices. Region configuration is immutable
after device.done. Version2 region_mapping_id precedes its matching region event.
Geometry replacement therefore needs a new device, not an in-place region edit.
The [server API](https://libinput.pages.freedesktop.org/libei/api/group__libeis-device.html)
also describes pause as neutralizing held state while input may still be in transit.

Implementation decisions derived from those contracts:

- Public StreamMapping contains only a pairing ID and logical size. Validate
  nonempty bounded IDs, dimensions, uniqueness and agreement with monitor geometry
  before publishing Start. Preserve legacy streams that lack mapping metadata.
- Decode all legacy input synchronously in the existing object-server receive
  loop, then enqueue one typed Input. The one session actor checks device grant,
  selected stream ownership and the EIS one-way gate before dispatch. Shared
  overflow revokes the grant so a lost key/button release cannot leave it active.
- Keep typed public smooth-axis finish semantics until the transport can forward
  both a nonzero final delta and its stop without silently losing either.
- Keep runtime pointer grants disabled until production transport, explicit Iced
  consent and EI support are integrated. Internal command sockets are not EI FDs.

Next EI work must settle region layout and physical scale, mixed capabilities,
mapping-ID negotiation, device replacement/tombstones, per-device pending frames
and held state, and legacy-to-EI retirement. The local reis Seat::add_device
before_done callback can emit raw device configuration; its high-level region TODO
does not by itself establish that a vendor change is required. Verify that seam
against the actual protocol and native client before choosing the integration.

The compositor extensions remain unadopted drafts. Canonical source/provenance,
required shared gates and physical/native/distribution acceptance remain open.

## EI follow-up evidence

[Current Mutter source](https://raw.githubusercontent.com/GNOME/mutter/main/src/backends/meta-eis-client.c)
sets each region's physical scale from its viewport and separates standalone
viewports from a shared absolute device. This is evidence about that provider,
not a reason to copy its implementation or assume Canopa must use the same layout.
The [EI region contract](https://libinput.pages.freedesktop.org/libei/interfaces/ei_device/index.html#ei_deviceregion)
defines scale as the relative-motion factor across regions. Canopa's current
OpenMappedPointer descriptors contain only stream ID, mapping ID and logical size.
The compositor's window mapping reads current output scale when projecting points,
but that value is not part of the descriptor or its invalidation comparison.

Inference to investigate before enabling EI: a single-window recording can keep
the same logical size while output scale changes. Correct immutable EI region
replacement may therefore need scale in the compositor's atomic binding snapshot
and invalidation key; size-only renewal is not proof of that case. Reproduce it
with real EI region/pixel input before deciding the protocol extension. Do not
assume physical scale1.0 or a monitor-only recording as the final supported scope.
