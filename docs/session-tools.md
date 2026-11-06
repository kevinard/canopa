# Wallpaper and lockscreen

Build from the workspace root with Wayland, libxkbcommon and Linux-PAM development
libraries installed:

```sh
cargo build --locked -p canopa-session -p canopa-cli
```

Install `canopa-wallpaper`, `canopa-lock` and `canopa-idle` beside `canopa` (normally
`~/.local/bin`). The repository updater builds and installs both. Neither tool
uses Iced or creates a GPU device. They target compositors with wlr-layer-shell
(wallpaper) and ext-session-lock-v1 (locker), including Niri.

## Wallpaper

```sh
canopa wallpaper daemon
canopa wallpaper status
canopa wallpaper set ~/Pictures/mountains.jpg --output DP-1
canopa wallpaper set ~/Pictures/forest.png --all --fit fill
canopa wallpaper reload
canopa wallpaper stop
```

The same commands work through `canopa-wallpaper`. `status` returns JSON output
names, logical dimensions and active image/fit assignments. Output names are exact
Wayland connector names. An assignment to an absent output is retained for when it
appears. `--all` replaces the default and clears per-output overrides, so newly
connected monitors receive that image too. Changes do not restart the daemon.
Changes are saved as private assignment overlays in
`$XDG_STATE_HOME/canopa/wallpaper.json` (or `~/.local/state/canopa/wallpaper.json`)
and restored across daemon restarts/logins. `set --temporary` affects only the
running daemon, including when another monitor's choice is saved later. State
contains local image paths and fit modes, is bounded, mode 0600 and atomically
replaced. Concurrent graphical sessions for the same account share these saved
preferences; their IPC endpoints and running assignments remain display-specific.

Image/configuration changes are watched with inotify, including atomic file
replacement and missing-parent creation. Valid configuration updates preserve
manual overlays. Explicit `reload` clears overlays and stops any slideshow,
restoring the selected configuration. Invalid candidates preserve the displayed
wallpaper and saved assignments. Filesystem events are coalesced while admission
is in progress; no periodic filesystem scan is used.

`fill` crops centrally to fill the monitor; `fit` preserves the whole image with
solid-color letterboxing; `stretch` fills without preserving proportions; `center`
uses original pixel dimensions and crops oversized images. PNG/JPEG are supported.
Alpha composites onto the configured background. EXIF orientation is applied.

By default the daemon has no animation, periodic image polling or redraw timer.
It sleeps until IPC, source-file or Wayland output events. Images are decoded/resized only when needed;
after submission the compositor retains the shared-memory backing and the daemon
drops its decoded pixels and mapping. Admission is bounded to 64 MiB encoded,
32 megapixels and a 256 MiB decoder allocation budget per image; output buffers
also have a 32-megapixel limit, with a 256 MiB combined output-surface admission
budget for a candidate. A distinct image is decoded once for all its candidate
outputs. These are admission limits, not an RSS guarantee.

To start with the graphical session, install
`packaging/systemd/canopa-wallpaper.service` under `~/.config/systemd/user/`, then
run `systemctl --user daemon-reload` and
`systemctl --user enable --now canopa-wallpaper.service`. `canopa-session.target`
now includes it. The unit waits for Niri readiness, matching the other companions.

## Wallpaper picker and slideshow

```sh
canopa wallpaper pick
canopa wallpaper pick ~/Pictures/Wallpapers
canopa wallpaper pick ~/Pictures/one.jpg ~/Pictures/two.png
canopa wallpaper slideshow ~/Pictures/Wallpapers --all --interval 300
canopa wallpaper next
canopa wallpaper stop-slideshow
```

The CPU-rendered picker previews selected/all monitors at their actual aspect
ratios and buffer scale, using the daemon's background color. Choose the image,
monitor and fit, then Apply. Previewing and closing the picker do not change the
wallpaper. Left/Right chooses images, Up/Down cycles monitor selection, F cycles
fit, Tab selects a control, Enter activates it and Escape closes. F5 refreshes
files/outputs. Pointer and native accessibility actions use the same controls.
Invalid/unready previews cannot be applied. Default discovery includes active
wallpapers and `~/Pictures/Wallpapers` (or `~/Pictures`); explicit files/folders
replace that input list. Folder discovery is bounded to 2,048 entries and 256
image candidates. All-monitor previews show up to eight outputs at once.

The optional slideshow scans at most 2,048 directory entries and selects up to
256 PNG/JPEG paths in sorted order. Intervals are 10..86,400 seconds (default 300).
It can target one `--output NAME` or `--all`. The timer exists only while the
slideshow is active. Invalid slides retain the last valid frame, and the next
advance continues to another candidate. Slideshow policy is saved at startup;
frame changes do not write state on each timer tick. `stop-slideshow` saves its
current wallpaper, and a manual `set` stops the running slideshow. Source files
and output hotplug continue to use events. No transitions or GPU device are used.

## Lock

Install the PAM service **before** using Canopa's Lock button. Copy the appropriate
example as root to `/etc/pam.d/canopa-lock`: `packaging/pam/canopa-lock.arch` for
Arch's `system-auth`, or `packaging/pam/canopa-lock.debian` for Debian/Ubuntu's
`common-auth` and `common-account`. Release archives ship these examples under `share/canopa/pam/`.
Other distributions must use their standard screen-unlock PAM stack. There is no setuid binary or hard-coded password.

```sh
canopa lock
canopa lock --canopa-config ~/.config/canopa/config.kdl
canopa lock preview /tmp/canopa-lock.png --width 1920 --height 1080
canopa lock --defaults check-config
```

`canopa-lock` is the direct executable. It starts on demand and remains in the
foreground until authenticated unlock. Enter submits the current PAM prompt;
Backspace edits the response. Escape clears nonempty input, or cancels the current PAM
transaction when input is empty. After cancellation/failure, Enter starts a fresh
transaction. Authentication and account checks must both succeed; locked/expired
accounts cannot unlock. Only one PAM transaction runs at a time, in a cancellable
child process. Retry is delayed until at least two seconds after the previous
attempt started, in addition to PAM's own policy. `auth-timeout` bounds each entire
transaction (default 120 seconds, configurable 5..600).

The conversation supports successive hidden password/PIN/OTP prompts, visible
prompts and module-provided information/error messages. Responses are fresh for
each prompt; a password is never reused as an OTP or PIN. Authentication starts
when the compositor confirms locking, so PAM can accept a fingerprint or security
key without typing. Device waits show the module's cue. Cancellation, timeout and
worker death retain the lock and discard the transaction.

Hardware support comes from the installed PAM modules and enrollment, rather than
Canopa-specific device drivers. Use the distribution's existing unlock stack when
it already implements your policy. See [Yubico's pam-u2f documentation](https://developers.yubico.com/pam-u2f/Manuals/pam_u2f.8.html)
and [fprint's device/PAM support](https://fprint.freedesktop.org/).
For example, on an Arch system requiring **both** its system authentication and
an enrolled FIDO token, a local policy can use:

```pam
auth substack system-auth
auth required pam_u2f.so cue
account include system-auth
```

`substack` keeps an inner `sufficient` success from skipping the required token.
Enrollment and module options must match the machine's policy. For fingerprint,
use the distribution's configured `pam_fprintd` stack; sufficient/required controls
determine whether fingerprint replaces or supplements a password. PAM runs modules
serially; this implementation does not race a password stack against a separate
fingerprint stack. When PAM reports an expired password after authentication, the
locker asks for explicit consent before running the configured PAM password stack.
Cancel leaves the session locked. Current/new/repeated passwords use fresh protected
prompts; both the change and a fresh account check must succeed to unlock. Install
the matching `password` include from the supplied PAM example to enable this recovery
path.

## Logind and systemd

Install `packaging/systemd/canopa-lock.service` with the other user units, then:

```sh
systemctl --user daemon-reload
systemctl --user enable --now canopa-lock.service
loginctl lock-session
```

The unit runs `canopa-lock daemon`, a small event-driven listener. It becomes ready
after subscribing to the selected session's `Lock` signal, opening its private
control endpoint and acquiring logind's sleep delay inhibitor. With it running,
`loginctl lock-session SESSION_ID` and `loginctl lock-sessions` start the locker;
calling `canopa-lock` directly attaches to the same foreground locker. Concurrent
requests share one locker. `canopa-lock ensure` waits for compositor readiness;
`canopa-lock status` reports the selected session and readiness as JSON.
The session target includes this listener.
Restarting/stopping the listener preserves an already active locker.
The unit stays inactive if the required PAM service file has not been installed.

Session selection uses `--session ID`, then `XDG_SESSION_ID`, then the calling PID's
session. A user service without a PID-attached session falls back to the only owned
graphical Wayland session. Multiple candidates require explicit selection. Foreign
user sessions are refused. Each session permits only one listener.

Direct and listener-started lockers set logind's `LockedHint` after the compositor
confirms locking, and clear it after authenticated compositor unlock. A crash does
not clear it. Live lockers reconnect after logind restarts; an unavailable logind never
prevents direct secure locking. A logind `Unlock` signal cannot bypass PAM.
See the [logind interface contract](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html).
`LockedHint` is advisory; the compositor's session lock is the security boundary.

The compositor supplies keyboard layout and Caps Lock state. Clock/date, password
feedback and selected widgets appear on every monitor, including hotplugged outputs.
By default the locker inherits the wallpaper daemon's current per-output images
unless its own configuration supplies a background. It caches prepared backgrounds
while locked. Explicit lock images work without a wallpaper daemon. Missing or invalid lock
images fall back to the theme background and do not prevent session locking.

`--ready-fd FD` writes `1` and closes a duplicate of that inherited descriptor only
after the compositor's `locked` event. It is suitable for an external idle/suspend
coordinator; starting the process alone is not proof of lock readiness. This feature
is also used by the listener's sleep coordinator. See [Stasis integration](idle.md)
for optional automatic idle, monitor power and suspend policy. The listener delays
external sleep until compositor readiness within logind's bounded delay window;
it acquires a fresh inhibitor after resume.

A denied lock request exits unsuccessfully. Rendering/authentication failures,
Escape and process termination never request unlock. Under ext-session-lock,
killing the locker leaves the session locked; recovery is compositor policy.
Preview writes a PNG without acquiring a lock or starting PAM. See the shared
authentication input controls below for accessibility and text support.

## Shared configuration and themes

Both applications use `~/.config/canopa/config.kdl`, standard XDG discovery and
`--canopa-config`/`--defaults`. They have independent schema owners, included in
`canopa config check`. Missing namespaces use defaults. Paths are relative to the
configuration file. Unknown/duplicate settings are rejected. Wallpaper `reload`
prepares a complete candidate and preserves current assignments on failure.
Lock configuration and appearance are read at each invocation.

```kdl
schema-version 1
appearance {
    color-scheme "dark"
    // Existing theme files and palette/typography overrides also apply.
    accent "#b0dec3"
}
wallpaper {
    image "wallpapers/forest.jpg"
    fit "fill"
    color "#10191d"
    output "DP-1" { image "wallpapers/mountains.png"; fit "fit"; }
}
lockscreen {
    use-wallpaper #true
    // image "wallpapers/lock.jpg"  // explicit default overrides daemon inheritance
    // output "DP-1" { image "wallpapers/lock-wide.jpg"; }
    dim 65                        // blend wallpaper toward the theme background, 0..100
    font "Inter"                  // defaults to the shared theme's first font family
    clock-format "%H:%M"
    date-format "%A, %B %-d"
    clock-size 80                 // 8..160; scales with the available viewport
    text-size 19                  // 8..160
    auth-timeout 120               // total PAM transaction timeout, seconds
    widgets "username" "keyboard" "battery" "media" "weather" "avatar"
    avatar "avatar.png"
    weather-file "weather.json"
}
```

Widgets are passive and independently selectable; `widgets;` hides them all.
Media displays the shell's selected/pinned player's label, without playback controls.
Battery uses UPower changes. Unavailable data is hidden. Avatar is a local raster.
Weather reads a local UTF-8 JSON snapshot on redraw (normally once per minute):

```json
{"temperature_celsius": 18.5, "description": "Clear", "updated_unix": 1793491200}
```

A provider must write the current Unix timestamp, preferably via atomic replacement.
Files above 16 KiB, invalid temperatures, timestamps more than five minutes in the
future, and snapshots older than two hours are ignored. No location/network request
is made by the lockscreen. Disable `media`, `weather`, `avatar` or `username` if those
fields should not be visible while locked.

## Layout and custom widgets

`layout` replaces the default placement. Region arguments list widgets in drawing
order. Regions are `top-left`, `top-center`, `top-right`, `center-left`, `center`,
`center-right`, `bottom-left`, `bottom-center`, and `bottom-right`. They align to the
corresponding viewport edge/center. Each region flows as a column by default, or
`flow="row"`. `x`/`y` offsets are logical pixels (-4096..4096); `margin`/`gap` are
0..256. Geometry scales with the output and stays inside its bounds.

```kdl
lockscreen {
    widgets "username" "battery" "status"
    layout {
        margin 32
        gap 12
        top-left "clock" "date"
        center "username" "authentication"
        bottom-right "battery" "status" flow="row" x=-8 y=-8
    }
    widget "status" {
        type "script"
        command { argv "python3" "/home/me/.config/canopa/lock-status.py"; }
        interval 30
        timeout 5
        format "{output}"
        size 15
        color "muted"
        preview "Example status"
    }
}
```

`authentication` must appear exactly once. `clock` and `date` appear when placed;
the six optional built-ins and custom widgets also need enabling in `widgets`.
Each enabled widget needs one placement; duplicates and unknown names are rejected.
At small sizes, regions shrink to fit and other widgets overlapping authentication
are hidden. Without `layout`, the existing central clock/authentication and bottom
widget stack remain; enabled custom widgets join the bottom stack.

Use the [Widgets editor](settings.md#widgets) to arrange the layout. To hide
passive widgets on a particular screen, add a list to that output's override.
Existing image and fit fields can share the same block.

```kdl
lockscreen {
    output "DP-1" { hide-widgets "username" "media"; }
}
```

`hide-widgets` also accepts `clock`, `date` and configured custom widget names.
Authentication and its keyboard layout switcher stay available. The single
lockscreen accessibility tree omits passive widget labels hidden on any output.

Custom widgets share a bounded command reader from `canopa-widgets`. `command`
also accepts a shell-word string (for example `command "uptime -p"`); it is parsed
as argv without shell expansion. Use explicit `sh -c` only when shell syntax is
intended. Commands run with the user's ordinary privileges and configuration-file
directory as cwd, receive no stdin or authentication channel, and begin only after
compositor-confirmed locking. They are trusted local configuration, not a sandbox.
Only enable data you want visible on the locked screen.

Commands run sequentially per widget: `interval` seconds after completion
(1..86400, default 30), with `timeout` seconds per run (1..60, default 5).
Output must be successful UTF-8 and at most 16 KiB; display text is limited to 1024
characters and four wrapped lines. Failures hide the widget. Unchanged output does
not trigger a redraw. Timeout and normal lockscreen exit terminate the command
process group; the command leader also dies if the locker is killed. Programs
that intentionally detach children must manage those children themselves.
`format` substitutes `{output}`. `size` is 8..160; `color` accepts `text`, `muted`,
`accent`, `warning`, `danger`, `success`, or `#rrggbb`.
`preview` supplies placeholder text only in preview; previews and configuration
checks never execute commands or start PAM.

The default clock redraws at minute boundaries; custom clock/date formats update
once per second. Widget changes and typing also redraw. Appearance uses the shared
Canopa dark/light palettes and theme colors, with local font, size, clock and dim
settings. The preview command can select a configured output with `--monitor DP-1`.

## Validation

```sh
cargo test --locked -p canopa-session --lib --test integration
python3 scripts/verify-session-tools.py
python3 scripts/verify-lockscreen-integration.py
bash scripts/check.sh --workspace
```

The native script starts a private Weston/Niri session and uses bubblewrap to mount
an acceptance-only PAM service and C module. It never edits the host's `/etc/pam.d`
or locks the user's compositor. Its fixture accepts only its known test password;
never install the fixture module as a real PAM service. Results and screenshots
are retained under `.scratch/wallpaper-lockscreen/evidence/`.
The second script uses a private logind-compatible D-Bus service and real
`loginctl` transport, plus deterministic multi-step/device PAM fixtures. Its results
live under `.scratch/lockscreen-integration/evidence/`. These fixtures verify the
conversation and lifecycle, not a physical token/fingerprint reader or a live
distribution's PAM policy.

## Shared authentication input

Lock, Greeter, Polkit and Pinentry use the same bounded UTF-8 input. Left/Right
moves by grapheme, Shift extends a selection, Home/End moves to an edge, and
Backspace/Delete removes the selected or adjacent grapheme. Ctrl+A selects all;
Ctrl+U clears. Insertion replaces the selection, with rejected oversized input
leaving it intact. Deleted bytes and the unused buffer tail are erased.

When the compositor supplies several XKB layout groups, **F6** or the Keyboard
control cycles the local selection. Compositor-side group changes take priority.
The selector uses the supplied keymap, modifiers and compose table. It does not
change the session's keyboard configuration. Configure multiple layouts in the
compositor to offer them here.

Software text uses advanced shaping, bidirectional layout and installed-font
fallback. Install fonts covering the desired scripts. Native AT-SPI exposes prompt,
status, visible request context and control actions. Hidden fields have the password
role and expose only bullets, including through the text interface. Accessible
editing, selection and submission follow the same consumer state/policy checks as
keyboard input; stale prompt actions are rejected. A screen reader and accessibility
bus must be available in the relevant session, including the dedicated greeter
session when accessibility is needed before login.

Owned secret buffers use fixed private memory mappings, best-effort `mlock` and
`MADV_DONTDUMP`, in addition to process core-dump protection and explicit erasure.
Memory-lock limits can prevent `mlock`; authentication remains usable. These
protections apply to Canopa's owned storage, not to copies maintained by PAM,
D-Bus, GnuPG, compositor/input libraries or the operating system.

### Physical seat and device acceptance

The maintained native scenarios use private compositors, buses and PAM mounts.
They verify multi-output/hotplug behavior and the sleep coordination protocol,
including real delay-inhibitor descriptors. They do not establish a real machine's
DRM/VT handoff, kernel suspend/resume or distribution authentication policy.
Before replacing a login manager on a test installation, validate its greetd PAM
stack, selected Cage/Weston DRM backend, login/logout/failed handoff, VT switching,
lock/unlock on every physical output and hotplug while locked. Then check external
`loginctl lock-session`, lid/power-triggered sleep, resume and authentication with
the intended distribution password/fingerprint/FIDO policy. Keep an independent
recovery login available while testing that installation.

An opt-in `scripts/test-lockscreen-device.py` can test a real FIDO key without
installing a PAM policy. Select `--enroll /path/to/pamu2fcfg` or
`--u2f-enrollment` (a test-only touch credential for U2F-capable keys), and provide
`--module /path/to/pam_u2f.so`, `--device /dev/hidrawN` and `--output DIRECTORY`.
Run it with the native fixture's Python dependencies. It uses a private compositor
and PAM mount, requests two touches and deletes its temporary mapping afterward.
The first touch creates a non-resident credential for `pam://canopa-device-test`;
the second authenticates through Canopa's actual PAM prompt and unlock path. It
does not read SSH credentials, reset the key, create a resident account, install
packages or change host login configuration. The U2F mode requires libfido2/OpenSSL
development headers and a C compiler. It creates a non-resident, presence-required credential without changing PIN policy.
The pam-u2f global configuration is hidden inside the test mount: the module
requires root ownership for configuration files, so the helper uses its defaults.
A FIDO2-only key that requires PIN entry during enrollment needs an interactive
enrollment setup; never send that PIN through chat.
This test covers the actual device/module conversation on a private graphical seat,
not its enrollment in the host's login policy.
