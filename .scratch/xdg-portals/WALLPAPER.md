# Wallpaper preparation

Status: implemented and advertised, with scoped wire/frontend/native validation.
See docs/adr/0038-wallpaper-review-and-source-ownership.md and the evidence ledger.
Final shipped dark01 and light01 (125%) each pass10 native groups, with backend
SHA256 6f8c87e725400eee3d53397d68909e028a5a133045b911340559c82cd6fa61cd.
Sources, bytes, identities, errors, source/worker limits, codec formats, immutable
review, unique-owner pinning, cancellation, actual desktop/locker behavior,
trusted/untrusted HTTPS and persistence are covered. No user config was changed.
The current workspace gate stops at a Files directory-worker timeout (371pass/
1fail/34ignore); the same case passes alone7.69s. Tooling, Clippy and all-target
compilation pass; the parallel gate remains red and later stages are not reached.
Do not infer broad acceptance from scoped results. Real Flatpak remains separate.

## Shell owner predecessor evidence

See docs/adr/0037-shell-wallpaper-owner.md. Shell has a
private PNG-descriptor owner, durable selections, separate Iced background
surfaces and a `canopa-shell lock` Swaylock descriptor handoff. Owner validation:
12 focused tests, all1275 Shell units serially, all30 integration cases, and
seven native groups each for top/background bars. Final native artifacts
/tmp/canopa-wallpaper-owner-native-top-04 and ...-background-03 share SHA256
dcbd94ac5fb05f8f3899df49f51dc8423191aff5a421de1d5a3b0866ea0f69be.
The workspace gate exits101 at a Files playback timeout (371pass/1fail/34ignore);
that exact case passes alone0.96s. Clippy, tooling and all-target compilation pass,
but the parallel workspace gate is not green. Later gate stages were not reached.
Portal review,
request cancellation/parenting, public frontend flows and output hotplug remain
subsequent acceptance work. Owner success does not establish portal acceptance.

Installed `org.freedesktop.impl.portal.Wallpaper.xml` defines SetWallpaperURI with
request handle, app ID, parent, URI and options, returning one numeric response.
It has no properties/version. `show-preview` is boolean; `set-on` is a string
with background, lockscreen or both values. The backend may show a preview even
when not requested. Do not infer an omitted-option default from this XML.

Further source inspection for the next slice:
- The installed public Wallpaper XML explicitly rejects file: URIs for public
  SetWallpaperURI; local clients must use SetWallpaperFile. Its backend method
  still receives a file URI after frontend descriptor/path resolution. Native
  tests must exercise public FD input, rather than passing file: to the public URI
  method. Public version1 and backend absence of a version property are distinct.
- [GTK backend](https://raw.githubusercontent.com/flatpak/xdg-desktop-portal-gtk/main/src/wallpaper.c)
  also ignores set-on and loads the URI with GFile; that is not a Rust network
  implementation to reuse.
- [KDE backend](https://raw.githubusercontent.com/KDE/xdg-desktop-portal-kde/master/src/wallpaper.cpp)
  distinguishes the three explicit set-on values, rejects other/omitted values,
  and defaults its preview behavior to true. These implementations do not establish
  a universal omitted-option default. Choose/document Canopa's policy explicitly.
- No direct HTTP client is declared in current workspace manifests. The URI path
  needs a deliberate bounded Rust remote-source adapter, separate from the owned
  regular-descriptor path; no network dependency or implementation was added in
  the owner slice. Freeze any downloaded/local image before review, enforce byte,
  redirect, decoding, cancellation and worker limits, and pass the reviewed owned
  canonical PNG to Shell. Do not fetch caller data again after acceptance.

The official frontend1.22.1 source retained under
`/tmp/canopa-xdp-lockdown-source-01/xdg-desktop-portal-1.22.1/src/wallpaper.c`
handles permission storage and Access consent when preview is not requested and
the app has no prior grant. A denied permission stops the call. Its public URI
and FD methods converge on backend SetWallpaperURI: it resolves the FD to a path
and constructs a file URI rather than copying immutable image contents. The
backend must own the accepted image's lifetime rather than retaining a mutable
caller path. The existing Rust Access backend can provide frontend consent.

Initial repository search found no supported wallpaper renderer or wpaperd/swww/swaybg
integration. Shell's documented Lock action launches `swaylock` through the generic
session-owned Launch action, with no image argument. Sources:
`crates/canopa-shell/README.md`, bundled default.kdl session-lock action and root
ADR0014. Historical audit prose and mock gradients are not a wallpaper owner.

The first implementation seam is the actual display/locker owner. Read Shell
AGENTS, domain context and relevant ADRs before changing that module. Decide how
an accepted immutable image reaches real Niri output surfaces, persists across
restart and reaches the configured locker. Honor every claimed set-on target;
returning success after writing unused preferences or partially applying both
would not replace the GNOME backend. A Shell background layer/session adapter or
a dedicated owner requires an integration decision and cross-application checks.
Do not silently overwrite unrelated user Niri or locker configuration.

Reuse request lifetime/parenting and Iced review. Keep image preparation off the
event loop, bound file/decoder/worker resources, and keep full wallpaper quality
separate from preview pixels. Copy/read the accepted source through an owned
regular descriptor. Changes to or removal of the caller's original must not
change the reviewed or displayed image. Closing/caller loss before acceptance
must not install it. Runtime owner failures need an explicit response and must
not be presented as successful configuration.

Native acceptance must use private bus, frontend, nested Niri and disposable
source/state, then inspect actual desktop pixels and configured locker handoff,
including both destinations, output/scale changes, restart, cancellation,
unavailable owner and malformed/deleted source. Never launch or configure the
real user's locker during tests. Screenshot preparation remains a separate compositor seam.
