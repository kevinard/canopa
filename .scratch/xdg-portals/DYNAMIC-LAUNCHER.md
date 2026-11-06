# DynamicLauncher preparation

Status: implemented and advertised. Eight native groups pass with the shipped
descriptor and activation service in dark/100% and light/125%.
See the evidence ledger for source changes and actual validation attempts.

The [backend v1 reference](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.DynamicLauncher.html)
defines PrepareInstall with request handle, app ID, parent, proposed name,
serialized GBytesIcon and options. The result is an explicit response plus chosen
name and icon. Application and Webapp launcher types are separate bits; target
URI is shown for a Webapp. Name editing defaults on, icon editing is optional,
and modal defaults on. RequestInstallToken is a separate non-interactive policy
decision, not an instruction to invent or persist a backend token.

The [installed frontend 1.22.1](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/src/dynamic-launcher.c)
owns sender-scoped, expiring install tokens, desktop-entry validation/publication,
icon storage, launching and removal. It validates an input bytes icon and retains
its original format/size. Backend results require nonempty name and a nested
variant icon. Its subsequent writer expects a bytes icon. Returning another
format after icon editing needs careful compatibility investigation because this
version retains the input format/size. Unknown host identity can bypass backend
non-interactive policy, so public denial acceptance must use a resolved app ID.

Locally verified with Gio.BytesIcon serialization: the contained icon has `(sv)`
shape `('bytes', <ay>)`. The PrepareInstall input is a D-Bus variant around that
structure. Output `a{sv}` additionally needs the value type required by the
frontend's `icon` response filter (`v`); wire tests must check the nesting rather
than treating all OwnedValue shapes as interchangeable.

The [current GNOME backend](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/dynamic-launcher.c)
preserves the incoming icon variant and explicitly leaves editable-icon
unimplemented. It allows unattended tokens for GNOME Software identities. Those
desktop-specific trust decisions are not a Canopa policy requirement. A local
GLib shape probe confirms `a{sv}` with `icon: Variant('v', bytes_icon)` returns a
`v` value whose contents have `(sv)` type, as required by the installed frontend.

Implementation direction:

- Reuse Requests and ordinary helper ownership/parenting. Keep frontend tokens,
  file publication, launch and uninstall out of the backend.
- Present an Iced name/icon review with explicit Install and Cancel, no initial
  Enter acceptance and readable target/app identity. Validate returned selections
  against the request, including a non-editable name.
- Own and bound icon bytes before presentation; use Rust raster/SVG preparation
  on a worker. Pass bounded preview pixels through the anonymous pipe and keep
  the authoritative original icon in the backend. Investigate editable-icon
  format/size compatibility before offering an icon picker.
- Define explicit non-interactive installation policy. Default denial is a valid
  decision; do not copy another desktop's implicit application trust list.
- Verify exact properties/types, bad serialized icons, cancellation/caller loss,
  concurrent requests and shared capacity. Native acceptance must use private
  desktop entries/data directories, real frontend token redemption and replay
  rejection, confirmed name/icon bytes, and uninstall cleanup in both appearances.
  Avoid launching real user applications or publishing into the user's data home.

The investigation below preceded implementation; Background validation itself
made no changes for this interface. The current implementation preserves this
frontend/backend boundary.

Existing Rust seams inspected while the workspace test build runs:
`RasterBatch::decode` accepts owned encoded bytes with allocation/dimension/pixel
budgets, and `canopa_assets::svg::rasterize` blocks external image/font/filesystem
resources. `icons.rs` and `account_profile.rs` demonstrate their existing policies.
Use those mechanisms with launcher-specific limits rather than loading a caller
path or sending the original icon through the helper's 1 MiB JSON transport.
Admission should precede expensive decoding/copying. Keep the original icon in
the backend, render a small owned preview on a worker, and revalidate the chosen
name after the helper returns. An optional read-only icon is consistent with the
current upstream implementation and avoids the installed frontend's edited-format
compatibility problem. No shared decoder or vendor change appears necessary.
