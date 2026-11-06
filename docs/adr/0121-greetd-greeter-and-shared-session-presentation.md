# 0121: Greetd greeter and shared session presentation

Status: implemented; validation is recorded in `.scratch/greeter/PRD.md`.

Canopa's login screen should share the lockscreen's appearance and passive widgets.
Login creates a new authenticated session and has different privileges/lifetime
from unlocking an existing session. Greetd owns that privileged boundary.

`canopa-session-ui` extracts the existing software background/text renderer, layout,
fixed-capacity erased input and scoped widget observation. Canopa Session retains
its public module paths through reexports, plus its Wayland lock/wallpaper roles,
PAM worker, IPC and logind reporting. The shared crate runs no PAM transactions and
owns no Wayland roles. Widgets remain application-selected passive data; neither
consumer imports Shell Module/Panel actions.

`canopa-greeter` runs as an unprivileged greeter account inside a kiosk compositor.
It owns one fullscreen xdg-shell window, user/session selection and the authentication
view. Its `session --compositor cage|weston` launcher replaces itself with the
chosen compositor and passes frontend arguments literally. Cage uses the last
connected output; Weston uses kiosk-shell and disables discovered user configuration.
The direct frontend still works inside an existing Wayland compositor. Greetd's IPC
carries username, successive prompt responses and the selected session's argv/env.
Greetd owns PAM session setup, credential switching and desktop launch/teardown.
The client never runs the selected desktop itself or sets a session's LockedHint.

Authentication success requires a current UI generation to authorize handoff.
Cancellation/timeout invalidates that generation, requests backend cancellation and
drains an outstanding reply before retry. This avoids reusing a connection with a
stale response or letting a broken old request cancel a newer attempt. Control RPCs
are bounded; unavailable/refused cancellation fails closed without session start.
Greetd versions can serialize cancellation behind PAM device waits, so backend
worker cleanup remains subject to greetd and module timeouts. The client never
starts an independent competing PAM stack.
After a failed transaction, greetd may remove its configuring session while
reporting a dead-worker cleanup error. The frontend performs one further bounded
reset and requires success before retry. Ordinary active cancellation still
requires a successful acknowledgement.

The normal source is root-owned `/etc/canopa/greeter.kdl`, independent of user XDG
configuration. Its `greeter` namespace contains curated users and optional session overrides;
`view` delegates to the shared presentation schema. Manual usernames always work.
Defaults expose avatar/keyboard/battery and no private media. Scripts/assets are explicit
administrator configuration and execute only as the greeter account. Scripts do
not inherit greetd/notification endpoints or authentication descriptors. Preview and
validation execute no commands and do not connect to greetd.

Installed session entries are discovered once per frontend startup, using fixed
system Wayland-session directories rather than user XDG data paths. Ordered
directory precedence, visibility/TryExec, localized names, desktop Exec argument
rules and session environment metadata are applied without executing commands.
Root-owned non-writable sources are required outside explicit development mode.
Manual IDs override discoveries; discovery can be disabled for curated lists.
X11 discovery requires an administrator-provided X-server/session wrapper, with
separate IDs and X11 environment. Empty catalogs remain visible and cannot begin
authentication. Config validation stays host-independent; `list-sessions` exposes
the resolved catalog. See [session discovery](../greeter.md) for bounds and schema.

AccountsService supplies an optional cached-user chooser. A one-shot asynchronous
system-bus reader retrieves only UID, canonical username, system-account status
and display name, plus optional IconFile metadata, pinned to the replying service owner. Root/system accounts are
excluded from discovery; non-local cached accounts remain eligible. Explicit
users/defaults and typed names retain priority. Incoming presentation data never
changes a live authentication transaction. The read is bounded, cancellable and
does not delay first paint; its connection retires when complete. Missing service
or failures fall back to NSS in the default auto mode; a successful empty service
list remains authoritative. Explicit source selection can require either service.
NSS enumeration runs in an isolated single-threaded helper, with bounded records,
output and a two-second timeout. Cancellation kills/reaps its process group; parent
death kills the helper. It uses the configured NSS providers, login.defs UID bounds
and optional chooser overrides, omitting root and nologin/false shells by default.
Only username, UID, shell eligibility and the first GECOS component are used;
record buffers are erased. No shadow query is performed. `discover-users #false`
disables both sources, and schema/preview paths perform neither query.
Greetd/PAM remains account-policy
authority; no AccountsService mutation, password hints or home metadata is used.

The shared avatar widget accepts prepared user thumbnails and initials without
owning account lookup. Greeter images follow canonical username selection, with
explicit per-user overrides, optional AccountsService IconFile and a configurable
system avatar directory. NSS/manual users use the same directory/override policy;
no home scan or remote fetch is performed. One selected-user loader coalesces
pending requests, cancels superseded helpers and refuses stale replies. It reuses
the NSS helper supervisor's two-second process-group deadline/cleanup boundary,
uses bounded canopa-assets decoding and keeps eight small in-memory thumbnails.
Missing images fall back to a static configured image or initials. Disabling
avatars suppresses IconFile queries and image loads. Preview uses only explicit
configured assets. Login identity, prompt input and PAM policy stay independent.

Configuration tooling recognizes this schema owner, and packaging includes the
binary, greetd/Cage examples and a Canopa Wayland-session desktop entry. The local
updater installs a development/user copy; privileged installation and choosing the
host display manager remain administrator operations. No host login configuration
or PAM policy is changed by implementation or acceptance.

Acceptance uses a private Wayland desktop, an independent test greetd protocol peer
and real nested Cage and Weston processes. These verify UI/protocol/lifetime behavior,
not the target machine's VT/DRM, enrollment or installed greetd/PAM policy.
An additional user/PID-namespace test uses real greetd with fixture PAM and account
records to check failed-password retry and actual session execution after greeter
exit. Supplementary-group setup is shimmed only in that isolated test; it does not
establish native privilege/seat acceptance.

The compositor comparison uses identical release frontend/configuration, output
size and software rendering, alternating runs after warm-up. First-frame callbacks,
whole candidate process-tree CPU and RSS are recorded with raw samples and hashes.
See [measurements](../greeter-performance.md) for the default choice and its limits.
