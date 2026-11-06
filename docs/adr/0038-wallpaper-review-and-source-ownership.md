# 0038: Wallpaper review and immutable source ownership

Status: implemented and advertised; scoped wire/native acceptance passes,2026-10-30.

The Wallpaper backend connects Iced review to the actual Shell owner in ADR0037.
The public frontend continues to own permissions and sandbox identity. Public
local input uses SetWallpaperFile; its backend call contains a file URI. Public
SetWallpaperURI handles remote pictures. The backend accepts local file and
HTTP(S) sources and freezes bytes before review; it never fetches the caller's
source again after acceptance.

Always show an Iced review, even when show-preview is false, as the contract
permits. Preserve an explicit background, lockscreen or both destination. If
set-on is omitted, Canopa chooses background, matching its desktop-background
purpose without silently changing a separately chosen lock image. This is a
documented Canopa policy, not a default inferred from other backends. The user
must explicitly Apply. Cancellation and caller loss before acceptance cannot
install an image. After acceptance, Shell's admitted operation owns installation;
a later disconnect or ambiguous transport failure does not roll it back or retry.

Keep two admitted wallpaper preparations/reviews. Permits remain with blocking
workers after caller cancellation and with frozen images throughout review.
Read at most32MiB through a regular local descriptor or bounded HTTP streaming.
HTTP uses Rustls, no cookie store or implicit decompression, a20s total request
deadline,5s connection deadline,10s read deadline and at most five redirects.
Reject embedded URL credentials and redirects to other schemes. Report failures
without URLs, paths, response bodies or credentials. TLS verification remains on.
System DNS has two separately admitted blocking jobs, retaining their permits
until lookup actually ends even if the HTTP request is cancelled or times out.

Decode bounded raster content with shared asset mechanisms, applying orientation.
Use the Shell owner's16,384-dimension/64Mi-pixel/512MiB best-effort decoder limits.
Normalize the full image to PNG, capped at128MiB, in a sealed anonymous regular
file. Prepare a separate384px preview as bounded base64 RGBA over the existing
1MiB helper pipe. Review cannot replace the full image or destination. The helper
uses the existing foreign-parent/modal Iced adapter and shared theme/controls.

Resolve the Shell owner's unique bus name before review and retain it for Apply.
An absent/replaced owner, unsupported destination, invalid source, write failure
or failed runtime handoff is an error, never successful preference publication.
Do not start a new Shell instance or rewrite user bindings. Existing custom
locker routes still need to opt into canopa-shell lock as ADR0037 documents.

Native acceptance must use a private bus, real frontend, nested Niri and disposable
HTTP/source/state/locker targets. Verify actual pixels and locker images, frozen
source after mutation/deletion, request/caller cancellation and parent lifetime, both and single
destinations, source limits, owner absence/loss, errors, restart and scaling before
descriptor publication. Add no GTK dependency or live user configuration change.

Final shipped metadata and activation pass ten native groups in dark/100% and
light/125%. They cover real public FD/URI requests, frozen input, independent
destinations, both without locker capability, request/caller cancellation,
private trusted/untrusted HTTPS, owner loss, restart and scaling. The separate
wire tests prove the exact URI method name and replacement-owner identity check.
The [user guide](../wallpaper-portal.md) describes source limits and custom locker
routes. Scoped success is not a green workspace gate or real Flatpak acceptance;
observed outcomes remain in `.scratch/xdg-portals/IMPLEMENTATION.md`.
