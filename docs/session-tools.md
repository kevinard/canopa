# Wallpaper and lockscreen

Build from the workspace root with Wayland, libxkbcommon and Linux-PAM development
libraries installed:

```sh
cargo build --locked -p canopa-session -p canopa-cli
```

Install `canopa-wallpaper` and `canopa-lock` beside `canopa` (normally
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
Runtime assignments last until reload or process exit; edit startup configuration
for persistence across logins. Changing a file on disk requires `set` or `reload`.

`fill` crops centrally to fill the monitor; `fit` preserves the whole image with
solid-color letterboxing; `stretch` fills without preserving proportions; `center`
uses original pixel dimensions and crops oversized images. PNG/JPEG are supported.
Alpha composites onto the configured background. EXIF orientation is applied.

The daemon has no animation, periodic image polling, or redraw timer. It sleeps
until IPC or Wayland output events. Images are decoded/resized only when needed;
after submission the compositor retains the shared-memory backing and the daemon
drops its decoded pixels and mapping. Admission is bounded to 64 MiB encoded,
32 megapixels and a 256 MiB decoder allocation budget per image; output buffers
also have a 32-megapixel limit. These are admission limits, not an RSS guarantee.

To start with the graphical session, install
`packaging/systemd/canopa-wallpaper.service` under `~/.config/systemd/user/`, then
run `systemctl --user daemon-reload` and
`systemctl --user enable --now canopa-wallpaper.service`. `canopa-session.target`
now includes it. The unit waits for Niri readiness, matching the other companions.

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
Backspace deletes. Escape clears nonempty input, or cancels the current PAM
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
fingerprint stack. Expired-password changes are not performed by the locker.

## Logind and systemd

Install `packaging/systemd/canopa-lock.service` with the other user units, then:

```sh
systemctl --user daemon-reload
systemctl --user enable --now canopa-lock.service
loginctl lock-session
```

The unit runs `canopa-lock daemon`, a small event-driven listener. It becomes ready
after subscribing to the selected session's `Lock` signal. With it running,
`loginctl lock-session SESSION_ID` and `loginctl lock-sessions` start the locker;
calling `canopa-lock` directly also works. The session target includes this listener.
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
does not install an idle daemon, sleep inhibitor, or automatic suspend policy.

A denied lock request exits unsuccessfully. Rendering/authentication failures,
Escape and process termination never request unlock. Under ext-session-lock,
killing the locker leaves the session locked; recovery is compositor policy.
Preview writes a PNG without acquiring a lock or starting PAM. Software text does
not yet expose an AT-SPI accessibility tree or complex-script shaping.

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
cargo test --locked -p canopa-session --lib --test configuration
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
