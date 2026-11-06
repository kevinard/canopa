# Screenshot portal

Canopa provides Screenshot and PickColor with a Rust capture backend and Iced
review. Screen/Area and PickColor require a compositor exposing wlr-screencopy3
and xdg-output2 or newer. Window capture also requires the compatible
[Niri desktop-sharing companion](desktop-sharing.md) and PipeWire. It does not
invoke Niri's screenshot command or change the clipboard. Unsupported capture sources fail.
The shipped descriptor advertises Screenshot after scoped wire/native acceptance.

Screen captures the complete output layout. Area selects a rectangle. Window
opens the Iced window picker and captures the chosen window, including a window
on another workspace. ActiveWindow resolves the focused window before opening
any helper. Its absence is an error. Window captures contain the window's full
physical pixels, rather than a crop of the desktop.

The backend advertises Window/ActiveWindow only when its capture owner matches
the Niri IPC endpoint and the helpers' Wayland display. AvailableTargets is15
with that provider and5 without it; the frontend is notified when it disappears.
An application requesting an explicit target keeps that target; interactive
requests without one can choose Screen, Area or an available Window (Alt+W).
The standard xdg-desktop-portal frontend retains application permissions and
document export. After frontend consent, permitted noninteractive Screen requests
can complete without another review. Interactive requests and PickColor require
confirmation. Window and ActiveWindow always require final review and Share.

The review uses frozen pixels. Changing the desktop while it is open cannot
change the accepted image or color. Click Share to publish, or Use color to return
the selected original RGB. Cancel, Escape or closing the window publishes
nothing. Initial Enter does not accept an unfocused action. Existing Wayland
parent and modal handling applies; missing parents leave an independent window.

For Area, drag inside the preview, or move the cursor with arrow keys and press
Space to start and finish selection. Shift moves ten pixels. For PickColor,
click a pixel or use arrows; Next display changes the selected output. Returned
color channels are normalized to0..1 from the original output's RGB bytes.

A single output retains its full physical dimensions. Multiple outputs use the
highest physical pixel density and their logical positions, with transparent
gaps and nearest source pixels. A gap-only area cannot be accepted.

Accepted PNGs are private immutable files under
`$XDG_CACHE_HOME/canopa/portal/screenshots` (or `$HOME/.cache/...`). They remain
readable across backend restart. The directory is0700 and files0400. The cache
retains files at least24hours and caps storage at1GiB/128files. A full cache of
recent files returns an error; older files are evicted only to make room.
Consumers needing permanent storage should copy their accepted result.

Two snapshots/reviews can be retained at once. Source and composed images are
bounded to64Mi pixels; PNG files to128MiB. The helper receives a sealed descriptor
and bounded previews instead of copying full images through JSON. See
[the capture decision](adr/0039-request-owned-screenshot-capture.md) and
[review/publication ownership](adr/0040-screenshot-review-and-publication.md).
Window capture briefly records one frame, stops its producer before review and
uses the same immutable snapshot owner; see
[window capture and review](adr/0118-window-screenshot-capture-and-review.md).

Focused checks:

```sh
cargo test --locked -p canopa-portal --lib --test integration screenshot
python3 scripts/test-screenshot-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa --output /tmp/screenshot-dark
python3 scripts/test-screenshot-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa --output /tmp/screenshot-light \
  --color-scheme light --scale 1.25
python3 scripts/test-screenshot-composition.py \
  --binary target/debug/xdg-desktop-portal-canopa --output /tmp/screenshot-outputs
```

The native fixtures isolate buses, desktops, caches and clipboard state. The
first uses the actual frontend and Iced UI; the composition fixture calls the
backend directly on two private headless Sway outputs. Add `--shipped-descriptor`
to the first fixture to require the repository's actual capability metadata.
Routing remains an explicit session choice; see the
[Settings setup guide](settings-portal.md). No fixture rewrites live routing.

`scripts/test-window-screenshot-portal.py` checks Window/ActiveWindow through the
real frontend and Iced. It requires explicit backend, companion and frontend paths
with their SHA-256 values; see `--help`. Use dark/100% or `--scheme light --scale 1.25`.
It checks off-workspace pixels, cancellation, frozen review after source closure,
fixed explicit targets, unchanged clipboard and capability withdrawal. Native
callers and companions are reused from stable caches.
