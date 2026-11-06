# 0040: Screenshot review and publication

Status: implemented and advertised after scoped native acceptance,2026-10-30.

Screenshot and PickColor belong to canopa-portal. Reuse one ADR0039 capture
adapter for two retained snapshots across capture, review and publication. The
standard frontend owns sandbox identity, document export and permission storage.
The backend exposes Screenshot version3 with AvailableTargets5: Screen and Area.
It rejects Window and ActiveWindow because an output crop cannot provide an
unobscured window source.

Capture immutable pixels before opening the Iced helper. Interactive requests,
Area and PickColor require explicit confirmation. Noninteractive Screen may
skip review only when the frontend supplies permission_store_checked=true.
The frontend's Access consent therefore remains effective without adding a
second prompt to every permitted capture. An explicit target cannot change in
review; an interactive request with no target can switch between Screen and Area.

Store upright RGBA frames in a single sealed memfd. Pass only that descriptor
through the existing controlled helper exec, plus bounded frame metadata over
the existing JSON pipe. Clear CLOEXEC only in the child for explicitly supplied
descriptors; preserve it in the backend. The helper checks size, seals, frame
offsets and geometry and builds bounded previews (4Mi pixels total,1024px side).
Full pixels stay in the descriptor. Return only a rectangle or frame/pixel
coordinate. The backend validates selection and computes the PNG or normalized
RGB; the helper cannot choose a result pathname or return an unchecked color.

One output preserves its exact physical dimensions, including fractional-scale
logical rounding. Multiple outputs compose at the highest physical pixel density
using their logical positions, including negative origins. Use nearest source
pixels and transparent gaps. Bound the composed raster to64Mi pixels and reject
gap-only areas. PickColor reads the chosen original output pixel directly.
Mouse drag and keyboard Space/arrows select Area; mouse click and arrows select
color. Enter without a focused accepting action does not publish. Escape,
Cancel, window close and request/caller loss retire the helper without acceptance.

The backend owns accepted PNG retention because frontend document export is
asynchronous. Publish random immutable0400 PNGs atomically under the private0700
XDG cache screenshots directory. Anchor filesystem operations to its descriptor,
reject symlinks/unrecognized entries, and serialize quota work with a private
lock. Bound PNGs to128MiB, retained storage to1GiB/128files. Retain published
files for at least24hours; evict older files only under quota pressure. If recent
files fill the cache, fail rather than remove a file a consumer may be opening.
No cleanup on request/helper/backend exit. Bound temporary-file cleanup and scans.

Blocking freeze/composition/encoding workers retain the entire snapshot and its
admission permit until they actually finish, even when the awaiting future is
cancelled. Cooperative row/write checks and a30s deadline bound work; filesystem
calls are not hard real-time. Publication already committed at cancellation may
leave an unused retained PNG; no rollback after publication is promised.

Native acceptance covers actual frontend activation and Access consent, Iced
review in both appearances/fractional scaling, original pixels after desktop
changes, keyboard and pointer area/color selection, caller/owner loss, restart,
publication failure, cache lifetime and unchanged clipboard. A second private
compositor validates production multi-output PNG composition. Wire tests cover
exact signatures/results, foreign Close and concurrent Settings responsiveness.
All tests use private compositor, bus, cache and clipboard state. Full workspace
gate outcomes are recorded separately from scoped portal acceptance.
