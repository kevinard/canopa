# Wallpaper portal

Canopa reviews wallpaper requests in Iced and applies accepted images through
Canopa Shell. The shipped descriptor advertises Wallpaper after scoped wire and
native acceptance in both appearances. The standard `xdg-desktop-portal` frontend continues to
own application identity and permission storage.

The review shows the requesting application, image, full image dimensions and
destination. Apply is the only accepting action. Cancel, Escape, closing the
review or losing the caller before acceptance installs nothing. A preview is
always shown, including when an application requests `show-preview=false`.
Foreign Wayland parents and modal hints use the existing Iced adapter; an absent
or expired export leaves a usable independent window.

Explicit `background`, `lockscreen` and `both` destinations are honored. When an
application omits `set-on`, Canopa chooses the desktop background and preserves
the existing lock image. Review cannot silently substitute a different destination.

Shell must be running. Desktop images cover its current named outputs without
taking input or reserving space. In a Swaylock session, use `canopa-shell lock`
to load the lock image selected through the portal. The bundled Lock button uses
`canopa-lock`, whose appearance comes from the [session tools](session-tools.md).
Canopa Settings changes the wallpaper daemon's per-output assignments. The
Wallpaper portal stores its Shell selections separately.

The backend freezes the source before review and gives Shell that same sealed
image after Apply. Editing or deleting the original file or changing the remote
response while the review is open cannot change the accepted pixels. Desktop and
lockscreen selections persist across Shell restart. Selecting both publishes the
two durable references together. Success acknowledges durable state and local
Shell installation; compositor repaint is asynchronous.

An unavailable/replaced owner, unsupported destination, invalid source, failed
storage operation or failed handoff produces an error. After explicit acceptance,
an operation already admitted by Shell can complete despite caller loss or an
ambiguous transport failure. The backend never retries against a replacement
owner and does not promise rollback after acceptance.

## Source and resource policy

Public local input uses `org.freedesktop.portal.Wallpaper.SetWallpaperFile`.
The frontend converts the regular descriptor to a backend file URI. Public
`SetWallpaperURI` rejects file URIs; use it for HTTP(S) pictures instead.
PNG, JPEG, GIF and WebP raster input is supported. Animated files produce a still
image. Orientation is applied before preparing the full PNG and its separate
aspect-preserving preview. Other formats can be rejected explicitly.

Two wallpaper preparations/reviews are admitted at once. Local reads require a
regular descriptor; a FIFO or device cannot become a blocking image input.
The source limit is32MiB. Decoding allows16,384 pixels per dimension and64Mi source
pixels, with a512MiB best-effort decoder allocation limit. Full output PNGs are
capped at128MiB. The384px preview travels as bounded base64 RGBA through the
existing1MiB private helper pipe and does not replace the full-resolution image.
Blocking preparation retains its permit after cancellation until it actually ends.

HTTP uses Rustls certificate verification, a20s request deadline,5s connection
deadline,10s read deadline and at most five redirects. Embedded URL credentials,
other schemes and compressed HTTP bodies are rejected. No cookie store, automatic
decompression, referrer forwarding or retry is enabled. Streaming size checks also
apply when Content-Length is absent. System DNS has two separately admitted
blocking jobs whose permits survive cancellation until lookup ends. These are
logical work limits, not filesystem deadlines or total process/GPU memory caps.
Failures do not expose source paths, URLs, credentials or response bodies.

## Development validation

Run focused tests from the workspace root:

```sh
cargo test --locked -p canopa-portal --lib --test integration wallpaper
```

The maintained native scenario uses private activation, a real frontend, nested
Niri, disposable source/state/HTTP server and a recording locker:

```sh
python3 scripts/test-wallpaper-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa \
  --shell-binary target/debug/canopa-shell \
  --shipped-descriptor \
  --output /tmp/canopa-wallpaper-native
```

Use a fresh output directory for each scenario. Add `--color-scheme light --scale
1.25` for light appearance and scaling. The preparation run injects Wallpaper only
into its disposable descriptor; `--shipped-descriptor` requires actual shipped
metadata. No live portal preferences or user locker are changed. The separate
`scripts/test-wallpaper-owner.py` scenario also verifies actual installed Swaylock
loading an inherited image on its private compositor.

See [the Shell owner decision](adr/0037-shell-wallpaper-owner.md),
[review/source ownership](adr/0038-wallpaper-review-and-source-ownership.md) and
[portal setup](settings-portal.md). Scope and observed failures are recorded in
`.scratch/xdg-portals/IMPLEMENTATION.md`; focused success is not a passing workspace
gate or complete native/Flatpak acceptance.
