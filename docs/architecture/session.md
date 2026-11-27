# Session architecture

This document covers the programs around the desktop session: wallpaper, lock screen,
idle supervision, the greetd login screen, the polkit agent, pinentry and the keyring
prompt bridge, the shared presentation crate they use, and how the supported Canopa
session starts. User guides: [session tools](../session-tools.md),
[login screen](../greeter.md), [polkit](../polkit.md), [pinentry](../pinentry.md).

Earlier decision records were consolidated into this document; the originals are in
the git history at commit cb300936 under docs/adr/.

## Components

| Crate | Binary | Surface | Lifetime |
| --- | --- | --- | --- |
| `canopa-session` | `canopa-wallpaper` | wlr-layer-shell background per output; picker is an xdg toplevel `org.canopa.WallpaperPicker` | user service `canopa-wallpaper.service` |
| `canopa-session` | `canopa-lock` | ext-session-lock-v1 surface per output | on demand; listener `canopa-lock.service` |
| `canopa-session` | `canopa-idle` | none | user service `canopa-idle.service`, supervises Stasis |
| `canopa-greeter` | `canopa-greeter` | fullscreen xdg toplevel `org.canopa.Greeter` in Niri, Cage or Weston | started by greetd |
| `canopa-greeter` | `canopa-greeter-displays` | none | run as root through `pkexec` by Settings |
| `canopa-polkit` | `canopa-polkit` | xdg toplevel `org.canopa.Polkit` per request | user service `canopa-polkit.service` |
| `canopa-pinentry` | `canopa-pinentry` | xdg toplevel `org.canopa.Pinentry` / `org.canopa.Keyring` | started by gpg-agent or the keyring prompter |
| `canopa-session-ui` | (library) | Iced presentation for the interactive surfaces above | — |

The interactive surfaces use Iced widget trees and its standard wgpu compositor,
with tiny-skia software fallback when GPU initialization is unavailable. Frames
present directly to their native Wayland surfaces, without CPU readback. Prepared
backgrounds consume their RGB source and retain one shared RGBA allocation across
the host and widget tree. Each surface retains its widget tree and bounded
prepared-image caches; pointer motion schedules drawing only when Iced reports a
visual change, and native cursor requests occur on enter or a shape change.
Hosts advance a scene revision after model changes. Renderer wakeups (widget
redraw requests, timed redraws and failure retries) only repaint the retained
widget trees; a renderer relayout request rebuilds them once. Clock and weather observations
invalidate scenes only when their displayed values change; presentation does no
weather file access or model serialization. Wayland frame callbacks throttle each
surface independently with renderer vsync waits disabled. Sleeping outputs keep
pending updates without blocking keyboard/PAM processing on visible outputs.
Graphics setup and presentation failures retry on a coalesced timer in every
session client. The pinned renderer's panicking surface constructors are contained
at the graphics boundary; failed outputs select software independently. A deferred
output re-arms its retry after an earlier wakeup. Presentation failures leave
authentication conversations and the compositor's lock intact. The shared compositor retains
only a display connection, so retired windows and outputs release their roles.
The wallpaper daemon submits static images directly without initializing Iced
graphics. Rendering, clock and passive-data updates remain event driven.

Visual tokens (`canopa-session-ui/src/view/style.rs`) derive from the appearance:
`Look` reads the palette and the theme's `ui` radii (button, input, menu-row, card,
panel), so the session surfaces share the desktop's shapes; nothing in the view
hard-codes a colour beyond white and black overlays.

Motion (`canopa-session-ui/src/motion.rs`) is a pure function of the time since an
effect started: full-screen surfaces fade and rise in, a new step (chooser, power
options, details) cross-fades, the field shakes once when a response is rejected,
and a spinner turns while the system checks a response. A resting lock screen
(`SessionView::resting`, set by the locker) morphs between its glance view and the
form: the clock and date grow and rise, the form and the notification card give way
to each other while the media card stays on the bottom row, and the presentation fades a clearer copy of the wallpaper
(`Background::with_rest`) over the softened one. Hosts enable motion; a
surface asks for the next frame callback only while an effect runs or it waits,
and stays idle otherwise. Reduced motion keeps the spinner and drops movement.
Dialogs leave their opening to the compositor. Previews fix the motion clock to
render any moment. After a rejected response the lock, login and polkit surfaces
keep the field: a response typed there is kept while the conversation restarts
and answers its first secret prompt. Public labels strip controls, cap length, fit their
available width and clip to their bounds. Fingerprints retain monospace hex groups
and their key-identity caption, and 12-hour clocks mirror part order in RTL.
`canopa-session` re-exports the presentation modules of `canopa-session-ui` under its old paths.

Source: [`crates/canopa-session/src/`](../../crates/canopa-session/src/),
[`crates/canopa-session-ui/src/`](../../crates/canopa-session-ui/src/),
[`crates/canopa-greeter/src/`](../../crates/canopa-greeter/src/),
[`crates/canopa-polkit/`](../../crates/canopa-polkit/),
[`crates/canopa-pinentry/src/`](../../crates/canopa-pinentry/src/),
[`packaging/keyring/`](../../packaging/keyring/).

## Ownership

Canopa presents conversations and stores preferences. Policy and credentials stay with
the system components:

| Concern | Owner | Canopa's part |
| --- | --- | --- |
| Screen unlock policy | PAM service `canopa-lock` | Conversation UI, isolated PAM worker |
| Login, account policy, session start | greetd and its PAM stack | Conversation UI over greetd IPC |
| Privileged action authorization | polkit and its agent helper | Conversation UI over `PolkitAgentSession` |
| GnuPG passphrases, caching | gpg-agent | Assuan server and dialog |
| Keyring prompts | GCR `SystemPrompter` protocol | Bridge to the pinentry dialog |
| Idle timeouts and inhibition policy | Stasis (Rune file) | Supervisor, commands, readiness |
| Session lock security boundary | the compositor (ext-session-lock) | Lock surfaces, readiness reporting |
| Lock and sleep signals, `LockedHint` | logind | Listener, delay inhibitor |

## Supported session

The Arch packages provide a dedicated session that does not touch the user's own Niri
setup:

1. The display manager runs `/usr/share/wayland-sessions/canopa.desktop`
   (`Exec=/usr/bin/canopa-session`, `TryExec=/usr/lib/canopa/niri`,
   `DesktopNames=canopa;niri`).
2. `canopa-session` ([`scripts/canopa-session.py`](../../scripts/canopa-session.py))
   refuses to start when `canopa-compositor.service`, `niri.service` or
   `graphical-session.target` is already active. It sets `CANOPA_SESSION=1`,
   `NIRI_CONFIG=$XDG_CONFIG_HOME/canopa/niri.kdl`, `XDG_CURRENT_DESKTOP=canopa:niri`,
   `XDG_SESSION_DESKTOP=canopa`, `XDG_SESSION_TYPE=wayland` and `CANOPA_SESSION_PID`,
   then runs `canopa-settings init-session`.
3. It imports the environment into the systemd user manager and D-Bus activation,
   stops `xdg-desktop-portal.service` and `xdg-desktop-portal-canopa.service` so they
   restart with the new environment, clears failed Canopa units, and runs
   `systemctl --user --wait start canopa-compositor.service`.
4. `canopa-compositor.service` runs the bundled Niri build `/usr/lib/canopa/niri
   --session` (Type=notify) and wants `canopa-session.target`. Niri imports
   `XDG_CURRENT_DESKTOP=niri` before reporting readiness, so `ExecStartPost` restores
   `canopa:niri` in the user manager and D-Bus activation environments before the
   session units start, and the shipped `niri-defaults.kdl` sets it in an
   `environment` block for the applications Niri spawns. Niri also removes
   `NIRI_CONFIG` from what it spawns; Settings accepts `CANOPA_SESSION=1` instead.
5. `canopa-session.target` (`ConditionEnvironment=CANOPA_SESSION=1`, bound to the
   compositor) wants the Shell, Notify, wallpaper, lock, polkit, idle and
   `canopa-display-profiles.service` units.
6. On exit or a signal, the launcher starts `canopa-shutdown.target`, which conflicts
   with the graphical session targets, and removes the variables again.

`init-session` creates missing files only, with mode 0600, in `~/.config/canopa/`:
`niri.kdl` (including `/usr/share/canopa/niri-defaults.kdl`, then `niri-input.kdl`,
`niri-displays.kdl` and `niri-custom.kdl`), those includes and `stasis.rune`. It then
regenerates `niri-shortcuts.kdl` and validates with Niri; a failure there does not
block login. See [Settings architecture](settings.md).

The system user units are the files in `packaging/systemd/` with `[Install]` removed,
`%h/.local/bin/` replaced by `/usr/bin/` and `niri.service` by
`canopa-compositor.service`; the idle unit also gains
`ConditionFileIsExecutable=/usr/bin/stasis`. Nothing needs enabling, and nothing
starts in other sessions.

The portable layout (release archive, `scripts/update.sh`) uses
`packaging/wayland-sessions/canopa.desktop`, which runs upstream `niri-session`, and
the repository units with `WantedBy=graphical-session.target`; the user enables
`canopa-session.target`.

## Wallpaper

`canopa-wallpaper daemon` creates one background layer surface per output, with no
input region or exclusive zone. Commands arrive on a private socket
`$XDG_RUNTIME_DIR/canopa-wallpaper-<key>.sock`, where `<key>` is the first 16 hex
digits of the SHA-256 of `WAYLAND_DISPLAY`; the runtime directory must be private and
owned by the user. Each connection carries one bounded JSON request.

The effective assignment is the `wallpaper` configuration plus a saved overlay in
`$XDG_STATE_HOME/canopa/wallpaper.json` (default image, up to 64 per-output entries,
slideshow). Temporary assignments live only in memory. `reload` re-reads the
configuration under the overlay; `reset` replaces the overlay with an empty one. A candidate (configuration
change, `set`, slideshow frame, hotplug) is fully prepared for every output before
anything is replaced; a failure keeps the current images. Each distinct image is
decoded once per candidate, shared between outputs and dropped after submission; the
compositor keeps the shared-memory buffer.

The built-in light and dark wallpapers are embedded through `canopa-session-ui`.
The daemon resolves them when an output has no configured or saved image and a
solid color or high-contrast appearance is inactive. It decodes them once per
candidate, and only when an output shows them. `status` marks those outputs with
`built_in`, so a lock that follows the wallpaper renders the same image.

One inotify observer (`canopa_file::watch`) covers the configuration, its theme file,
the discovered `config.kdl` path (even when absent), the images and the slideshow
folder. It coalesces events while a candidate is being admitted. Automatic reloads keep
the overlay; the explicit `reload` request resets it and writes an empty state.

The picker is a separate xdg window that renders previews with the same code and
applies through the daemon's socket.

## Lock screen

### Locker

`canopa-lock` acquires ext-session-lock-v1 and creates a surface per output, including
hotplugged ones. Authentication begins only after the compositor's `locked` event.
Each attempt runs `canopa-lock internal-auth --fd N` as a child process over a private
socket with bounded frames:

- `pam_start("canopa-lock")`, `pam_authenticate` with `PAM_DISALLOW_NULL_AUTHTOK`,
  then `pam_acct_mgmt`; both must succeed.
- On `PAM_NEW_AUTHTOK_REQD`, the UI asks for consent, then the same transaction calls
  `pam_chauthtok(PAM_CHANGE_EXPIRED_AUTHTOK)` followed by a fresh account check.
- Every prompt receives a fresh response buffer. Cancellation, timeout
  (`auth-timeout`) and worker death drop the transaction; process-group termination,
  parent-death signals and generation numbers stop a stale worker from unlocking.

Only an authenticated result unlocks. Rendering and authentication failures fail
closed. An invalid configuration falls back to defaults so the Lock action never
does nothing. The background comes from the configuration, else from the lock-screen
image the Wallpaper portal stored in the Shell's store
(`$XDG_STATE_HOME/canopa/shell/wallpaper/selection.json`, read only by
`canopa_session::portal_lock_image`), else, with `use-wallpaper`, from the wallpaper
daemon's `status` (300 ms timeout), including its embedded wallpaper.

### Listener and control endpoint

`canopa-lock daemon` serves one logind session, selected from `--session`,
`XDG_SESSION_ID`, the caller's PID or the only owned graphical Wayland session. A
flock prevents a second listener. It:

- subscribes to the session's `Lock` signal and to `PrepareForSleep`;
- holds `Inhibit("sleep", "Canopa Lock", ..., "delay")` and releases it only after a
  locker confirms the lock, then takes a new one after resume;
- serves `canopa-lock-control-<key>.sock` (same key as the wallpaper) for foreground
  attach, readiness (`ensure`) and `status`;
- spawns lockers as children, sharing one locker between concurrent requests, and
  restarts a locker that exits without unlocking after confirming the lock (750 ms
  delay, at most five times per 60 s);
- reconnects to logind without stopping the locker, and reports readiness with
  `sd_notify` after subscribing, binding the socket and taking the inhibitor.

Lockers set `LockedHint` after the compositor confirms and clear it after an
authenticated unlock. The hint is advisory; the compositor lock is the boundary. The
listener and the locker start helpers through `canopa_file::executable::current`, so
an upgrade that replaces the installed file does not break a running lock.
`KillMode=process` keeps an active locker alive when the listener restarts.

### Composition

`lockscreen` configuration (owned by `canopa-session-ui::settings`) places widgets in
nine anchors with row or column flow, bounded margins, gaps and offsets, scaled per
output. `authentication` appears exactly once and wins overlap conflicts. Passive data
comes from `canopa-widgets` (UPower, the Shell's selected MPRIS player, a weather
file, script commands) and Notify's lock-screen feed through `canopa-notify-client`.
Script commands start only after locking, without input or authentication
descriptors, with bounded output and run time, and are killed with their process
group.

## Idle

`canopa-idle run` checks that the session's lock listener runs as sleep coordinator,
takes a per-display flock (`canopa-idle-supervisor-<key>.lock`), and starts Stasis in
its own process group with `CANOPA_LOCK`, `CANOPA_IDLE`, `CANOPA_NIRI` (absolute
paths), `XDG_SESSION_ID` and `TOKIO_WORKER_THREADS=1` unless set. It reports
readiness after Stasis delivers a valid `watch` frame (within 8 s) and stops Stasis
when the coordinator goes away. The packaged Rune policy
([`packaging/stasis/stasis.rune`](../../packaging/stasis/stasis.rune)) locks with
`$CANOPA_LOCK`, powers monitors off with `$CANOPA_NIRI msg action
power-off-monitors` and suspends with `$CANOPA_IDLE suspend`, which waits for a
confirmed lock before calling logind `Suspend(true)`. Other commands are forwarded to
Stasis.

## Login screen

`canopa-greeter session` starts the login compositor
([`compositor.rs`](../../crates/canopa-greeter/src/compositor.rs)). For Cage and
Weston it builds the command (`cage -s -m last -- greeter ...`, or
`weston --shell=kiosk-shell.so --no-config --idle-time=0 -- greeter ...`) and execs it;
they exit with the greeter. For Niri, the default, it stays the parent:

1. It creates `$XDG_RUNTIME_DIR/canopa-greeter-<pid>/` (0700) with a generated
   `niri.kdl` (0600) and binds a datagram socket there. The configuration binds no
   keys, starts nothing, disables Xwayland, the hot corner and power-key handling,
   fills outputs with the greeter's background colour, opens `org.canopa.Greeter`
   fullscreen, and includes `/etc/canopa/greeter-displays.kdl` only when it passes the
   same root-ownership check as `greeter.kdl`.
2. It starts `/usr/lib/canopa/niri` (or `niri` from `PATH`) with `--config`, no
   `--session`, `NIRI_DISABLE_SYSTEM_MANAGER_NOTIFY=1` and
   `NIRI_KEEP_LAST_FRAME_ON_EXIT=1`. Niri's startup command is
   `canopa-greeter internal-report-display`, which sends Niri's `WAYLAND_DISPLAY` to the
   socket (30 s deadline, aborted if Niri exits).
3. It runs the greeter as its own child on that display, waits on the two pidfds,
   stops Niri (SIGTERM, SIGKILL after 5 s) when the greeter exits, removes the
   directory and exits with the greeter's result. If Niri exits first, the greeter is
   stopped and the launcher fails. Both children have `PR_SET_PDEATHSIG` set.

The greeter itself requires `GREETD_SOCK` and a non-root account, and speaks greetd's
JSON IPC (length-prefixed, 64 KiB bound, 3 s control timeout).

Authentication uses a generation number per attempt. greetd success only starts the
session the current generation selected; cancelling invalidates the generation and
drains the outstanding reply before a new attempt. After a failed attempt, greetd may
report a dead PAM worker during cleanup; the greeter performs one more reset and
requires it to succeed. The selected session's argv and environment are fixed for the
attempt; a changed or removed entry cancels it.

Catalogs:

- **Sessions** are parsed from fixed system directories (root-owned, not
  group/world-writable) without executing anything, following desktop-entry `Exec`
  rules and producing `XDG_CURRENT_DESKTOP`, `XDG_SESSION_DESKTOP` and
  `XDG_SESSION_TYPE`.
- **Users** come from AccountsService (`ListCachedUsers` and per-user properties,
  pinned to the replying owner, 2 s deadline) or from a single-threaded NSS helper
  process (`internal-nss-users`, 2 s deadline, record and output bounds, killed with
  its process group).
- **Avatars** are decoded for the selected user only by an `internal-avatar` helper
  with the same supervision, into a 128×128 thumbnail; eight are cached in memory.
- Refresh: inotify on the session directories, avatar files, `/etc/passwd`,
  `/etc/nsswitch.conf`, `/etc/login.defs`; AccountsService signals and owner changes;
  F5.

After greetd acknowledges the start, the greeter sets the presentation's `blank`
flag (background only), replaces the background with its solid `view` colour, and
exits after the second frame callback, which follows a frame submitted after the
switch (500 ms fallback). The Canopa Niri defaults use the dark palette background
`#10191d`, the default greeter colour, as the session's background colour.

`canopa-greeter-displays apply` reads at most 64 KiB of JSON
(`{"outputs":[{"name", "off", "mode":{"width","height","refresh","custom"},
"scale", "transform", "position":{"x","y"}}]}`), rejects unknown fields,
connector names outside `[A-Za-z0-9._-]`, duplicates, an all-off layout and
out-of-range values, generates `output` nodes itself and replaces
`/etc/canopa/greeter-displays.kdl` atomically (0644) in a root-owned directory;
`reset` removes it. As root it always writes that path; `--target` is refused as root.
The polkit action `org.canopa.greeter.set-login-displays` requires administrator
authentication (`auth_admin_keep` for active sessions). Settings sends the applied
outputs from Niri IPC, not its draft
([`display.rs`](../../crates/canopa-settings/src/display.rs)).

Remembered choices are written to the greeter account's
`$XDG_STATE_HOME/canopa/greeter.json` only after greetd acknowledges the start. Power
actions use logind `CanPowerOff`/`CanReboot` (`yes` only) and are re-checked after
confirmation.

## Polkit agent

A C adapter ([`crates/canopa-polkit/native/agent.c`](../../crates/canopa-polkit/native/agent.c))
subclasses `PolkitAgentListener` and drives `PolkitAgentSession` on a private GLib
main context, away from the Wayland loop. Canopa never calls the
`AuthenticationAgentResponse` API: the native helper delivers the receipt. Incoming
calls are filtered to the authority's unique bus name. The agent removes
`POLKIT_DEBUG`, `NOTIFY_SOCKET` and `GREETD_SOCK` before initializing the library and
sets `GIO_USE_VFS=local`. One request is admitted at a time; generations and prompt
serials reject stale answers. A watchdog exits the process when native cancellation
does not return, so systemd can restart it. Identity labels and pictures are resolved
for the selected UID only (AccountsService `FindUserById`, NSS fallback) in an
`internal-identity` helper.

## Pinentry and keyring prompts

`canopa-pinentry` implements the Assuan pinentry protocol on stdin/stdout, one dialog
at a time, connecting to Wayland only while a prompt is shown. Quality, `CHECKPIN`
and `GENPIN` inquiries run in a worker with a two-second absolute deadline; a
malformed or timed-out inquiry ends the connection so leftover bytes cannot be read as
commands.

`canopa-keyring-prompter` ([`packaging/keyring/prompter.c`](../../packaging/keyring/prompter.c))
is a C GCR `SystemPrompter`. When the user manager's `CANOPA_SESSION_PID` belongs to
the active graphical session and `canopa-compositor.service` has a main PID, it runs
`/usr/bin/canopa-pinentry keyring-dialog` on that compositor's socket, with responses
on private pipes. Otherwise GCR's own dialog is used. The user opts in by linking the
D-Bus activation file.

## Shared presentation

`canopa-session-ui` holds everything visual and nothing that authenticates:

- **Rendering**: backgrounds with fit modes and admission limits (`background.rs`),
  Iced widgets and the standard compositor in `presentation.rs` and `view/iced.rs`. Each output
  owns its widget cache and pointer state. Prompt, generation, control or enabled
  changes invalidate pending presses; output and dialog retirement drop renderer
  caches, and retiring the last surface releases the compositors and GPU device
  until the next presentation. Native hosts deliver pointer motion, press, release and scrolling, then
  dispatch the resulting control IDs to their own controllers.
- **Secret entry**: a dedicated Iced widget owns only response pixels and
  grapheme-boundary caret positions. Keyboard editing stays with the controller
  and protected `Secret` storage. Revealed/echoed responses are shaped in a
  transient cosmic-text buffer, avoiding ordinary Iced string values, clipboard
  operations and text-event queues. Pointer selection returns positions for the
  controller to validate.
- **Views** (`view/`): three arrangements. *Form* (sign-in and unlock): identity,
  pill field with an inline submit button and an eye control, status lines.
  *Sheet* (choosers, power, password change): a card with a heading. *Dialog*
  (polkit, pinentry): heading, request card and footer actions; dialogs report their
  natural height and the client grows the window when a later step needs room.
  System controls sit as chips in the bottom corners unless a layout region uses them.
  Buttons use `canopa-ui` treatments, icon controls have tooltips and oversized
  chooser/sheet content scrolls inside its output instead of shrinking every row.
- **Layout** (`layout.rs`): the nine-anchor planner, also used by the Settings
  Widgets diagram.
- **Input** (`input.rs`, `secret.rs`): bounded UTF-8 editing by grapheme with
  selection; secrets in dedicated anonymous mappings with best-effort `mlock` and
  `MADV_DONTDUMP`, erased on edit and teardown; local XKB group selection from the
  compositor's keymap.
- **Accessibility** (`accessibility.rs`): an AT-SPI tree through accesskit. Hidden
  responses publish only bullets; incoming actions carry a presentation revision and
  are re-validated by the consumer before they act.
- **Widgets** (`widgets.rs`): scoped observation of `canopa-widgets` data and Notify's
  lock-screen feed.
- **Helpers** (`helper.rs`): cancellable isolated child processes that own their
  process group until it is killed and reaped; used by the greeter's NSS and avatar
  helpers and the polkit identity helper.
- **Requesters** (`process.rs`): display names of the processes that ask for
  authentication, labeled as context only.

Each consumer owns its surface role, authentication state machine, configuration
selection and privacy choices.

## Configuration

Each program owns one namespace of the shared `config.kdl` and validates it with
`check-config`, which `canopa config check` runs: `wallpaper`, `lockscreen`, `polkit`,
`pinentry` (user file) and `greeter`. The greeter reads only
`/etc/canopa/greeter.kdl`, which must be root-owned with no group or world write
access along its path. Lock and pinentry read the configuration per invocation, the
wallpaper daemon watches it, polkit reads it at startup.

## Design decisions

**Separate small processes for wallpaper and lock.** The wallpaper daemon and the
locker are independent binaries. Interactive surfaces use Iced’s standard GPU
renderer with software fallback and redraw on demand. This keeps the lock
screen's security-relevant code out of the Shell's process. Consequence: shared data
(battery, media, weather) lives in `canopa-widgets` as toolkit-free models, and the
lock screen imports no Shell actions.

**Wallpaper assignments are an overlay.** Runtime choices are saved as a private state
file on top of `config.kdl` rather than rewriting the configuration. Users can edit
the configuration freely and still keep per-monitor choices across logins; the cost is
that the explicit `reload` must reset the overlay to mean "use the configuration".

**Candidate admission before replacement.** Every wallpaper change is prepared for all
outputs within fixed byte and pixel budgets before anything is shown. A broken image
or configuration never blanks a monitor, and memory use stays bounded.

**Logind listener separate from the locker.** The listener is a small resident process
that turns logind's `Lock` and `PrepareForSleep` into locker children; the locker
itself starts on demand. Stopping or upgrading the listener cannot unlock the screen,
and the sleep delay inhibitor is released only after a confirmed lock, so suspend never
shows the desktop on resume. The listener also restarts a locker that crashed while
locked, within a budget, because ext-session-lock otherwise leaves a locked screen
with no prompt. Each restart first reads `LockedHint`: once another locker (a direct
`canopa-lock` run) has unlocked the session, recovery ends instead of locking it again.

**PAM in an isolated worker.** Each attempt runs in a child process. Blocking PAM
modules (fingerprint, tokens) cannot freeze the UI, cancellation can kill the whole
transaction, and a stale worker cannot unlock thanks to generation checks. The UI
does not race several PAM stacks; module order is host policy.

**Expired passwords are changed inside the unlock transaction.** On
`PAM_NEW_AUTHTOK_REQD` the lock screen asks for consent and calls `pam_chauthtok` in
the same transaction, so the change goes through the configured password stack.
Settings changes passwords through `passwd` instead, and never handles password
hashes (see [Settings architecture](settings.md)).

**greetd owns login.** Logging in creates a new session with different privileges from
unlocking one. The greeter is an unprivileged greetd client and never runs the desktop
itself, never starts its own PAM stack and never sets `LockedHint`. The attempt
generation guarantees that only the attempt on screen can authorize a session.

**The bundled Niri as login compositor, chosen by the launcher.** The greeter is an
ordinary xdg window, so it runs in an existing compositor instead of implementing
one. The packaged default is the Niri build the desktop already ships, which removes
Cage as a dependency and lets both sides of the login share one renderer, one
background colour and one display layout; Cage and Weston remain choices on greetd's
command line. Niri does not exit with its client, so the launcher supervises both
processes rather than exec'ing the compositor. See
[ADR 0151](../adr/0151-niri-login-compositor.md).

**A neutral last frame.** greetd starts the desktop only after the greeter exits, so
some gap between the two compositors is unavoidable. The greeter's last frame is its
plain background colour and the Niri build is asked to keep it on screen
(`DRM_IOCTL_MODE_CLOSEFB`), so whatever stays visible during the switch shows no
account data, and the desktop's Niri starts on the same colour.

**The login display layout is generated, never copied.** Settings cannot share the
user's `niri-displays.kdl` with the greeter account, so a root helper writes a system
file. A Niri configuration can start programs, so the helper accepts only structured
values and writes only `output` nodes it generates, and the launcher includes the file
only when root owns it.

**System-only greeter configuration and catalogs.** The greeter reads root-owned
system files only (configuration, session entries), never user XDG paths, because it
runs before any user has logged in. Discovery never executes `Exec` or `TryExec`.

**Account discovery off the main thread, NSS in a helper.** AccountsService and NSS
lookups can hang (directory services), so they run asynchronously with deadlines; NSS
runs in a separate process that can be killed. Typed usernames always work, and
discovered data never changes an attempt in progress.

**Live catalogs without polling.** Users, sessions and avatars refresh from inotify,
AccountsService signals and F5. Remote NSS has no change signal, so F5 covers it.

**Remember only successful choices.** The greeter saves the last user and per-user
session only after greetd acknowledges the session start, so a typo or a broken
session is never remembered. `default-user` overrides the remembered user but never
starts an attempt by itself.

**Power actions only when logind says `yes`.** The unauthenticated greeter has no
administrator identity to answer a polkit `challenge`, so it offers shutdown and
restart only for an unconditional `yes`, confirms, and asks logind again.

**Polkit through the native agent library.** The agent uses `PolkitAgentSession`
rather than its own PAM stack or the response API, so polkit's helper keeps checking
credentials and delivering receipts. The library runs on its own GLib context and a
watchdog covers native hangs, because a stuck agent would block every authorization
in the session.

**Pinentry as a protocol implementation.** The Assuan pinentry protocol is implemented
in Rust from its documentation; no GnuPG frontend code is copied or linked. It runs on
demand with no daemon. Quality, constraint and generation inquiries follow the
reference pinentry: enforced constraints need an affirmative answer, and generated
passphrases must be revealed before acceptance.

**Keyring prompts through a GCR bridge.** The `SystemPrompter` protocol and its
encrypted exchange are GCR's, so a small C bridge uses GCR and delegates only the
dialog to `canopa-pinentry`, and only when the active graphical session is the Canopa
compositor. Other desktops keep GCR's dialog. The routing is opt-in by a user link.

**Idle policy stays in Stasis.** Canopa does not reimplement idle detection or
inhibition; Stasis's Rune file is the policy source, and `canopa-idle` only supplies
commands and checks that locking is coordinated. The lock listener, not Stasis, owns
the sleep inhibitor, so restarting Stasis cannot open a window where suspend skips
the lock.

**Iced widgets inside native session hosts.** The existing SCTK/calloop loops keep
xdg, layer and ext-session-lock roles. Iced owns layout and widget interaction,
without replacing lock confirmation, readiness descriptors, logind coordination
or authentication worker lifetimes. Ordinary window-close operations never become
unlock actions. See [ADR 0150](../adr/0150-iced-session-presentation.md).

**Shared presentation, separate state machines.** `canopa-session-ui` owns rendering,
input editing, secure buffers and accessibility for all four authentication surfaces;
each consumer keeps its own authentication logic and surface role. Accessibility
actions are revalidated by the consumer against the current presentation revision,
so a screen reader cannot act on a stale prompt.

**One visual language in three arrangements.** Form, Sheet and Dialog cover sign-in
and unlock, choosers and request dialogs. Requesters are named for context only
("Requested by"), because a process chooses its own name; details keep the process ID.

**A dedicated session.** The packaged session uses a pinned Niri build at
`/usr/lib/canopa/niri`, its own compositor unit and target, and configuration under
`~/.config/canopa/`, so an existing Niri setup is never changed and companions start
only in the Canopa session. The launcher stops running portal processes before
starting so they pick up the Canopa environment.

## Known limitations

- Physical DRM/VT handoff, suspend/resume and distribution PAM policy are not covered by the automated tests; the kept last frame in particular has not been checked on hardware.
- With Niri, the login screen shows on the focused output only, and touchpad workspace swipes can move it out of view.
- `canopa-session` keeps its own logind client instead of the one in `canopa-services`.
