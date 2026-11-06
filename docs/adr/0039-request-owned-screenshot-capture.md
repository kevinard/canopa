# 0039: Request-owned screenshot capture

Status: capture adapter implemented with scoped native validation,2026-10-30.
Screenshot review/publication is implemented under ADR0040.

Screenshot capture belongs to a portal request. Shell continues to own persistent
desktop surfaces, session integration and wallpaper/locker state. A screenshot
request owns a short-lived Wayland client that reads pixels; it neither changes
compositor configuration nor injects input. The adapter lives in canopa-portal,
without a shared service or another application dependency.

Use a fresh connection for each capture. The pinned Niri26.04 exposes
wlr-screencopy3 and xdg-output3, while its newer ext-image-copy transport is absent
in the verified installation. Niri's screenshot IPC always updates the clipboard
and its reply precedes encoding completion. It is not the portal source adapter.
Bind screencopy3 and xdg-output>=2 explicitly; unsupported compositors fail rather
than returning stale pixels or silently invoking another screenshot utility.

Retain two admitted snapshots, including their full pixels through review. A
request has a ten-second connection/capture/conversion deadline. Connect through
Tokio and poll Wayland reads at most50ms at a time on the blocking worker. A
future-drop guard requests cancellation; the worker retains admission until it
actually exits. Do not wrap an uncancellable blocking_dispatch in an async timeout.
An absent display or inherited WAYLAND_SOCKET is unavailable; do not guess another
desktop, consume the inherited connection or mutate the shared environment.

Bound the registry to512 globals and16 outputs. Use compositor-provided logical
geometry, current physical mode and transform, with16,384-pixel dimension limits.
Bound the aggregate source to64Mi pixels and256MiB of padded shared memory. Accept
explicit32-bit RGB/BGR shared-memory formats, including premultiplied alpha;
reject unsupported layouts before allocating or copying. Frame memfds have fixed
size seals but remain writable by the compositor until Ready. Copy into private
upright RGBA pixels, applying stride, native endianness, alpha, y inversion and
inverse output orientation. No filesystem path or compositor-owned buffer escapes capture.
Layout changes/removal during capture fail instead of mixing coordinate spaces.

An immutable snapshot must precede Iced selection/review. No Screenshot interface
is advertised until its source, actual review, cancellation, accepted-file lifetime
and real frontend behavior have native acceptance. Window capture requires an
unobscured window source; cropping an output does not establish that capability.
The frontend retains sandbox/document permissions and exports accepted URIs
asynchronously, so result-file publication needs its own bounded retention owner.

Native adapter verification uses disposable Niri and a second compositor with
multiple outputs. Compare complete physical pixels and logical geometry, all
transforms and fractional scaling, retained-snapshot admission and immutability,
clipboard preservation, socket loss/reconnect and descriptor retirement. This
does not establish Iced review, PickColor or public Screenshot acceptance.

The final private Niri and Sway runs pass three and four groups respectively,
using the same compiled harness. All eight transforms use an asymmetric source.
Niri's fixed nested transform and125%->150% scaling are verified separately.
At rounded logical geometry the reference utility resamples/drops a physical row;
the adapter retains the full physical mode and compares interior source colors.
This distinction is recorded rather than truncating capture to match a reference.
Detailed run outcomes remain in `.scratch/xdg-portals/IMPLEMENTATION.md`.
