# Canopa login screen

`canopa-greeter` is a graphical frontend for **greetd**. It shares software rendering,
themes, layout, secret input and widget observation with Canopa Lock. Greetd owns
PAM authentication, credentials, logind session setup and desktop lifetime. The
frontend creates an ordinary fullscreen Wayland window and needs a compositor;
the launcher supports Cage and Weston's kiosk shell. Cage uses its
last-connected-monitor mode; Weston uses its kiosk output policy.

## Build and preview

```sh
cargo build --locked -p canopa-greeter -p canopa-session -p canopa-cli
canopa greeter --defaults check-config
canopa greeter --canopa-config packaging/greetd/greeter.kdl preview /tmp/login.png
```

`canopa-greeter` is the direct executable. Normal startup requires `GREETD_SOCK`
from greetd and an unprivileged account; running it as root is refused. Preview and
configuration checks need neither a compositor, greetd nor PAM, and execute no
custom widget commands.

The default configuration is `/etc/canopa/greeter.kdl`; it never discovers the
logged-in user's `~/.config/canopa/config.kdl`. A missing system file uses embedded
defaults. A present unreadable/invalid file fails. Normal system configuration and
its parent directories must be root-owned and not group/world writable. Explicit
`--canopa-config PATH` selects a development/admin-provided file. `--defaults`
selects embedded defaults. Shared-schema validation also recognizes the `greeter`
namespace; `canopa config check --file FILE` validates it with the other owners.
Use `canopa greeter check-config` to check the actual default system greeter file.

## Installation

Install greetd, either Cage or Weston 15+, Niri (including `niri-session`), fonts and the usual Wayland
runtime libraries through the distribution. The application itself has no GPU
renderer, GTK or PAM execution dependency. The selected compositor owns the
compositor/seat/VT side. Weston must provide `kiosk-shell.so` and support
`weston -- PROGRAM` terminating with its child.

Install `canopa-greeter` as a root-owned executable accessible to the greeter
account, normally `/usr/local/bin/canopa-greeter`. Create/use the distribution's
unprivileged `greeter` account with a private writable runtime/home directory.
Install root-owned `packaging/greetd/greeter.kdl` as `/etc/canopa/greeter.kdl`.
Install `packaging/wayland-sessions/canopa.desktop` in
`/usr/local/share/wayland-sessions/` to offer Canopa alongside the distribution's
installed sessions. Session desktop entries are system files, independent of the
greeter account's application menu and home directory.
Use publicly readable system assets such as `/usr/local/share/canopa/login.jpg`;
a user's encrypted/private home is not available for login presentation.

Merge the following example into `/etc/greetd/config.toml`:

```toml
[terminal]
vt = 1

[default_session]
command = "dbus-run-session /usr/local/bin/canopa-greeter session --compositor cage"
user = "greeter"
```

Select Weston by changing `--compositor cage` to `--compositor weston`.
The release includes both `config.toml` and `config-weston.toml` examples.
Selection is a launcher argument in greetd's command, before the Wayland client
starts; it is independent of the user's desktop session choice. The launcher
replaces itself with the chosen compositor, preserving signals and process lifetime.
It forwards `--canopa-config`/`--defaults` to the frontend without shell expansion.
The direct `canopa-greeter` command still connects to an existing compositor.
Weston can return zero when its child fails; authentication success is determined
by greetd's authenticated handoff, independently of the compositor's exit code.

`--renderer auto` (default) leaves renderer selection to the compositor;
`--renderer software` requests Pixman for either choice. The software greeter
renderer is independent of this compositor setting. `--compositor-bin PATH`
selects a nonstandard executable, and repeated `--compositor-arg=ARG` passes
literal additional arguments. Weston ignores discovered `weston.ini` by default.
For example, nested development can pass `--compositor-arg=--backend=wayland`.
Use `session --compositor weston --print-command` to inspect argv without starting
anything or connecting to greetd.

See [compositor measurements and reproduction](greeter-performance.md) for startup,
idle CPU and memory comparisons, including the nested-renderer scope.
The measured lower combined RSS keeps Cage as the packaged default. Weston
remains available through the explicit launcher choice.

The distribution's greetd PAM stack must include its normal authentication,
account and session policy (`pam_systemd` where appropriate). The locker's
`/etc/pam.d/canopa-lock` service is separate and is not a login PAM template.
Configure token/fingerprint enrollment and modules through that greetd stack.
For an Arch installation, the optional oo7 package supplies
[keyring unlock and Secret portal integration](installation.md#optional-oo7-keyring).

After checking the configuration, select greetd as the system's display manager
using the distribution's procedure. Its VT must not be used by another display
manager/getty. Repository scripts do not install/enable greetd, edit host PAM or
replace the running login manager. The updater installs the greeter alongside the
user tools for preview/development; the system greeter binary is administered
separately. Release archives include examples under `share/canopa/greetd/` and a
Canopa desktop entry under `share/wayland-sessions/` for other session choosers.

Greetd starts the chosen desktop after the greeter session exits. Both compositor
launch modes exit with their greeter child, completing that handoff. On logout, greetd starts the
login screen again. Canopa's configured session uses `niri-session`, which performs
Niri's systemd environment/session lifecycle. Enable the existing Canopa user
companion units for the authenticated account as described in
[session tools](session-tools.md). The greeter compositor has its own lifetime.

## Users, sessions and appearance

```kdl
schema-version 1
appearance { color-scheme "dark"; }
greeter {
    discover-users #true             // default; manual entry always works
    user-source "auto"              // AccountsService, then NSS on failure
    users "alice" "bob"             // optional manual names, listed first
    default-user "alice"            // optional
    discover-sessions #true          // default; no manual session list required
    view {
        widgets "avatar" "keyboard" "battery"
        auth-timeout 120
        image "/usr/local/share/canopa/login.jpg"
        layout {
            top-left "clock" "date"
            center "avatar" "authentication"
            bottom-center "keyboard" "battery" flow="row"
        }
    }
}
```

At each frontend startup, installed `.desktop` session entries populate the selector
from `/usr/local/share/wayland-sessions`, then `/usr/share/wayland-sessions`.
`session-dirs "PATH" ...` replaces those directories with an ordered list of
absolute paths. User `XDG_DATA_HOME`, `XDG_DATA_DIRS` and home directories are not
searched. Normal system discovery requires root-owned source files and ancestor
directories without group/world write access; explicit `--canopa-config` development
mode permits fixture/admin-provided sources, matching its configuration policy.

Names follow the greeter's `LC_ALL`/`LC_MESSAGES`/`LANG` locale. Entries marked
`Hidden`, `NoDisplay`, `Terminal` or `DBusActivatable` are skipped. `TryExec` and
the executable must be available; pathless executables are resolved using absolute
components of the greeter's PATH. Earlier directory entries mask later files with
the same filename, including hidden/invalid entries. Discovered sessions are
sorted by display name and ID. Session IDs are filenames without `.desktop`.

`Exec` uses desktop-entry quoting and field codes, then receives shell-safe quoting
for greetd's launch path. `%c`, `%k` and `%i` are expanded as argument data; file/URL
codes have no selections and are removed, `%%` is literal, and unknown/ambiguous
codes are rejected. There is no shell expansion or execution during discovery,
including for `TryExec`. `Path` is honored using GNU `env --chdir`. The resulting
argv/environment remains bounded to 8 KiB. DesktopNames becomes the colon-separated
`XDG_CURRENT_DESKTOP`; the filename supplies `XDG_SESSION_DESKTOP`, and
`XDG_SESSION_TYPE` is `wayland`. These values reach greetd before PAM session setup.

Manual definitions remain useful for custom launchers or overriding an installed
entry with the same ID. They appear first in configuration order:

```kdl
greeter {
    session "niri" {
        label "Niri with local setup"
        command { argv "/usr/local/bin/my-niri-session"; }
        env "XDG_SESSION_TYPE=wayland" "XDG_CURRENT_DESKTOP=niri"
    }
}
```

Set `discover-sessions #false` for a completely curated manual list. An empty
catalog displays an explanatory message and does not start authentication; there
is no assumed `niri-session` fallback. Scanning is bounded to eight directories
per kind, 256 examined files, 64 KiB per file and 128 total catalog entries. The
frontend keeps one snapshot until restart, with no background polling. Schema
checking remains independent of installed sessions. Preview and this diagnostic
resolve the catalog without connecting to greetd or executing commands:

```sh
canopa greeter list-sessions
```

X11 entries are enabled only with an explicit X-server/session wrapper accepting
the desktop command as argv. For example, `x11-wrapper { argv
"/usr/local/libexec/my-xsession"; }` enables `/usr/local/share/xsessions` and
`/usr/share/xsessions`; `xsession-dirs` replaces those paths. The administrator's
wrapper must perform the distribution's X-server setup and execute its supplied
arguments. X11 IDs are prefixed `x11:`, labels include `(X11)` and the PAM
environment uses `XDG_SESSION_TYPE=x11`. Canopa does not launch a raw X11 desktop
command without that wrapper. Wrapper command construction is tested; physical
X-server/seat behavior requires target-machine acceptance.

Manual environment entries are explicit `NAME=value` assignments; the greeter's
process environment is not copied into the desktop request.
Usernames can always be typed. User discovery runs asynchronously once at frontend
startup. `user-source "auto"` (default) first tries AccountsService and falls back
to NSS if the service is missing, denied, fails or times out. A successful empty
AccountsService list stays empty, respecting its visibility policy. Choose
`user-source "accountsservice"` or `user-source "nss"` to use only that source.

AccountsService's `org.freedesktop.Accounts.ListCachedUsers` is queried over the
system bus. The distribution's optional `accountsservice` package provides this
API. Only `Uid`, `UserName`, `SystemAccount`, `RealName` and (when avatars are enabled)
`IconFile` are read. Root/system accounts and invalid usernames are omitted; non-local cached accounts remain
eligible without a UID threshold. Password hints and home-directory metadata are
not read, and no account mutations are made.

NSS uses libc's `setpwent`/`getpwent_r` interface, so `/etc/nsswitch.conf` and the
installed NSS providers determine the roster. It works without AccountsService.
Some directory services disable enumeration; manual login still works for users
that are absent from the roster. Only canonical username, UID, login-shell
eligibility and the first comma-separated GECOS component are used. Other GECOS
fields, home paths and password fields are not copied into the account list;
there is no shadow lookup. NSS record buffers are erased after use.

NSS chooser filtering uses `UID_MIN`/`UID_MAX` from `/etc/login.defs`, falling back
to 1000..60000. Override these for directory-service or distribution-specific UID
ranges. UID 0 is always omitted, and shells ending in `nologin` or `false` are
omitted by default. An empty shell remains eligible. For example:

```kdl
greeter {
    user-source "nss"
    nss-uid-min 1000
    nss-uid-max 300000
    nss-include-nologin #false
}
```

These are chooser filters. Explicit/manual usernames remain available, and greetd/PAM
controls account locks, expiry, authentication and permission to log in.

The chooser displays a name with its canonical username; only the username goes
to greetd. Configured users keep their order and receive display-name metadata
when available; other accounts follow in display-name order. `default-user`
retains its prefilling behavior. Metadata arriving after typing/clearing a
username or during authentication never changes that input, account or transaction.
If the initial chooser is untouched and empty, it selects the first returned user
without starting authentication.

The UI does not wait for discovery. AccountsService has a two-second overall
deadline, at most 128 cached account paths and eight concurrent user reads. NSS
runs in a fresh single-threaded helper process with its own two-second deadline;
automatic fallback can therefore take up to four seconds. NSS examines at most
4096 records, retains at most 128 users and bounds each record and the total
helper response to 64 KiB. Timeout, cancellation or failure terminates and reaps
the helper process group. The helper receives no greetd/notification endpoints,
protects its process against dumps and is killed if its parent disappears.
Connections and workers retire after discovery, with no account polling.

Set `discover-users #false` for a curated-only chooser and to disable both
AccountsService and NSS. Schema validation and previews perform neither query.
Explicit inspection is available without a compositor or greetd:

```sh
canopa greeter list-users
canopa greeter list-users --source nss
```

This prints the merged username/display-name list and optional UID; unavailable
discovery falls back to configured users. Account changes appear at the next
frontend startup or explicit inspection.

User avatars are enabled by default through the `avatar` widget. The selected
user's image follows this order: explicit `user-avatar`, AccountsService `IconFile`,
then `avatar-dir/USERNAME` (default `/var/lib/AccountsService/icons`). The directory
also works for NSS/manual users without AccountsService. Files can be extensionless;
content must be a supported raster image. Missing, unreadable, invalid or oversized
images fall through to the next source, then to a static `view { avatar "..."; }`
image or the user's initials. Empty username input shows a generic `?` placeholder.
No home directories are searched or remote images downloaded. An explicit
AccountsService/configured path is read with the greeter account's permissions.

```kdl
greeter {
    user-avatars #true
    avatar-dir "/usr/local/share/canopa/faces"
    user-avatar "alice" "/usr/local/share/canopa/alice.png"
    view {
        widgets "avatar" "keyboard" "battery"
        layout { center "avatar" "authentication"; }
    }
}
```

`user-avatar` is repeatable for distinct usernames (at most 64), and relative
configured paths resolve beside the greeter configuration. These entries provide
images, without adding users to the chooser or granting login access. Set
`user-avatars #false` to disable avatars, image loading and IconFile queries.
The roster switch `discover-users` is independent of avatar presentation for manual users.
For an explicit widget/layout list, include `avatar` in both to enable it.

Only the selected user's image is decoded, asynchronously in an isolated helper
with a two-second deadline. A newer selection cancels older work and late results
cannot overwrite the current avatar or alter authentication. Each candidate is
limited to 2 MiB, 2048 pixels per dimension and 4 megapixels; the codec's best-effort
allocation limit is 32 MiB. Each request tries at most three files, 6 MiB encoded
and 48 MiB admitted source RGBA in total. Output is a center-cropped 128×128 RGBA
thumbnail. The cache retains at most eight thumbnails (512 KiB), including misses,
for this frontend lifetime; it is not polled or written to disk. Images appear in
a circular mask using the shared presentation widget. Preview loads only an
explicitly configured avatar for its selected user, without account discovery.
`list-users` can include optional `icon_file` metadata but never decodes images.

Enter continues/submits; Escape clears nonempty input, then cancels the transaction;
Up/Down selects a configured user while entering a username; Tab cycles sessions.
Clickable User/Session/Continue controls provide the same actions. During a session
start, selection is frozen until greetd acknowledges or refuses the handoff.
Input supports keyboard, pointer and protected AT-SPI actions, with complex-script
shaping and layout selection; see [shared input](session-tools.md#shared-authentication-input).

`view` uses the lockscreen's presentation schema: wallpaper fit/dimming, clock/date,
font sizes, nine-region layout and named passive script widgets. Authentication
must be placed exactly once. Greeter defaults show avatar/keyboard/battery and omit
personal media. Media is refused. Weather/custom widgets require explicit
system assets/configuration. Commands run as the greeter account with no stdin,
`GREETD_SOCK`, notification endpoint or authentication descriptors. They are trusted
administrator configuration. See [layout and script settings](session-tools.md).
The fullscreen window follows the kiosk compositor's active output/size/scale;
it does not acquire a session lock or set the user's logind LockedHint.

## Authentication and failure behavior

The client supports any sequence of greetd secret/visible/info/error messages.
Information/error notices are displayed and acknowledged; secret/PIN/OTP responses
are fresh per prompt. Passwordless device authentication can complete without
entering a password. Actual factors and account/session policy are controlled by
PAM through greetd. Device modules run serially under greetd's PAM transaction.

A PAM success alone does not launch a session: the current UI generation must
explicitly authorize the selected command. Cancellation invalidates that generation.
An in-flight request is drained after acknowledged cancellation before retry, so a
late reply cannot authorize a desktop or cancel a later attempt. The whole attempt
has a configurable 5..600-second UI deadline (default 120), and failed attempts
have a minimum two-second retry interval from their start.
After an authentication error, greetd can report that its PAM worker already
exited during cleanup. Canopa requires a further successful reset before offering
a fresh attempt; it does not treat that cleanup error as authentication success.

Transport messages are bounded to 64 KiB, control RPCs have a three-second
timeout and secret request buffers are erased. A disconnected/malformed service
before handoff or unacknowledged cancellation exits unsuccessfully without
session authorization. Once an authenticated start request has been sent, greetd
may accept it even if its acknowledgement is lost; that session decision belongs
to greetd.
Greetd remains responsible for terminating its PAM worker/session. Some greetd
versions serialize cancellation behind an outstanding PAM/device operation; their
module timeout determines when backend cleanup completes. Configure finite device
module timeouts. Canopa does not bypass a blocked conversation or start a competing
PAM stack. Expired-password changes are handled only if greetd/PAM supplies the
required conversation; there is no separate Canopa password-changing policy.

## Validation

```sh
cargo test --locked -p canopa-greeter -p canopa-session-ui --lib --test configuration --test sessions --test accounts --test nss --test avatars
python3 scripts/test-nss-discovery.py
python3 scripts/verify-greeter.py
python3 scripts/verify-greeter.py --cage /usr/bin/cage --output /tmp/canopa-greeter-cage
python3 scripts/verify-greeter.py --compositor weston --output /tmp/canopa-greeter-weston
python3 scripts/test-greetd-transport.py --greetd /usr/bin/greetd
python3 scripts/verify-lockscreen-integration.py
bash scripts/check.sh --workspace
```

The native fixture runs a private Weston/Niri desktop and a test-owned greetd
protocol peer. `--compositor cage|weston` tests the corresponding nested kiosk
through the product launcher (`--cage PATH` remains a compatibility alias).
The peer checks authentication
ordering and records command/environment handoff but never authenticates host
accounts or executes the configured desktop. Physical VT/DRM access, the installed
machine's greetd/PAM policy and physical authentication devices require acceptance
on that machine. Evidence is retained under `.scratch/greeter/`.

The real-greetd transport test runs only as an unprivileged user, with private
user/PID namespaces, `/run`, account records and test-only PAM files. It verifies
wrong-password retry and actual post-greeter session execution, including literal
argument/environment boundaries. A fixture shim bypasses supplementary-group
setup, which single-ID namespaces cannot perform. This does not validate host
privilege switching, logind/seat setup or the distribution's PAM policy.

The NSS tests use a synthetic libc provider to exercise filtering, budgets,
fallback, cancellation and helper cleanup. `test-nss-discovery.py` additionally
uses Bubblewrap user/mount/PID namespaces with private passwd, nsswitch and
login.defs files to test real libc enumeration and nameservice routing. These
checks do not change host accounts or validate a particular remote directory.

Protocol and deployment references: [greetd manual](https://manpages.debian.org/unstable/greetd/greetd.1.en.html),
[greetd IPC](https://manpages.debian.org/unstable/greetd/greetd-ipc.7.en.html),
[Cage manual](https://github.com/cage-kiosk/cage/blob/master/cage.1.scd).
[Weston kiosk shell](https://wayland.pages.freedesktop.org/weston/toc/kiosk-shell.html).
[Desktop-entry commands](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html)
and [session environment keys](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html).
[AccountsService interfaces](https://gitlab.freedesktop.org/accountsservice/accountsservice/-/tree/main/data).

[NSS enumeration](https://man7.org/linux/man-pages/man3/getpwent_r.3.html) and
[UID defaults](https://man7.org/linux/man-pages/man5/login.defs.5.html).
[AccountsService IconFile contract](https://sources.debian.org/src/accountsservice/0.6.37-3/data/org.freedesktop.Accounts.User.xml/).

## Remembered choices, refresh and power

The greeter remembers the last successfully started user and that user's desktop
session ID. It writes only after greetd acknowledges session startup; failed or
cancelled authentication and refused session launches do not update preferences.
An explicit `default-user` takes priority over the remembered user. Switching users
recalls each user's last successful desktop when it is still installed. State is
bounded, mode 0600, and atomically replaced in the greeter account's
`$XDG_STATE_HOME/canopa/greeter.json` (or `~/.local/state/canopa/greeter.json`).
No credentials or desktop commands are stored. Invalid/unreadable state uses the
configured defaults. Set `remember-selection #false` to disable loading/saving.
Preview and configuration checks do not read or write this file.

AccountsService signals refresh account names and avatar sources. Local NSS file
changes, installed session entries and selected avatar files use inotify, including
atomic replacement. A changed or removed desktop command cancels an in-flight
login attempt; source updates cannot redirect it to a different command. Session
handoff already accepted by greetd is not changed. Press **F5** to refresh catalogs
explicitly, including remote NSS sources, which have no universal change signal.
Typed usernames and current authentication responses survive unrelated updates.

The Power button (**F10**) appears when logind permits shutdown or restart for the
greeter account. Choosing an action opens a separate confirmation. Logind policy
is queried again after confirmation, and the final request also undergoes logind's
normal authorization. `challenge`, `no`, unavailable services and errors do not
authorize an action. The unauthenticated greeter does not supply an administrator
identity. Up/Down selects an action, Enter continues/confirms, and Escape returns.
Power controls are available before authentication. Set `power-controls #false`
to hide them. These preferences configure UI behavior, not system authorization.
