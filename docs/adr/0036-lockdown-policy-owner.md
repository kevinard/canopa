# ADR0036: Portal-owned persistent Lockdown policy

Status: implemented and advertised; scoped wire/native acceptance passes, 2026-10-29.

## Context

Lockdown exposes seven read/write booleans that disable entire portal operations.
GNOME binds these to mutable GSettings values. The installed [frontend 1.22.1](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/src/xdp-context.c)
creates one cached proxy and consumes standard PropertiesChanged signals. Its
FileChooser SaveFile/SaveFiles, OpenURI, Print, Camera and Location implementations
consult that cache before performing operations. Replacing only the getters
would lose both persistence and runtime mutation behavior.

The common KDL document contains independently owned configuration namespaces.
It is edited by users and tools. An atomic rename after a fingerprint check cannot
provide a compare-and-swap against uncoordinated editors. Notify already separates
mutable Do Not Disturb state from shared configuration; Files separates browsing
overrides and operational history from configuration defaults.

## Decision

The portal module owns Lockdown runtime policy in a private SQLite database at
`$XDG_STATE_HOME/canopa/portal/lockdown.sqlite3`, with the usual absolute-HOME
`.local/state` fallback. Missing state means all seven switches are false. Merely
starting or reading the module creates no state files. The first changed write
creates a private database. Use the existing bundled Rust SQLite dependency.
No common KDL rewrite or additional human configuration language is introduced.

An explicit `--lockdown-policy` selects another database. `--lockdown-read-only`
publishes an existing valid source while denying writes; missing administrative
state rejects startup/reload. The existing `--defaults` option
continues to select embedded appearance; it does not reset runtime privacy state.
An administrator can supply a read-only selected database. Unreadable, malformed,
foreign-schema or unsupported-version state rejects startup. A failed reload
retains the last valid snapshot. Do not substitute false values for failed reads.

The module's small interface owns open, reload, change publication and shutdown.
One bounded worker serializes persistence outside the async executor. Each write
uses an immediate transaction, reads current values, changes the requested switch
and commits before publishing. Concurrent SQLite writers serialize; other switches
are retained. Source replacement/removal requires a successful explicit reload
before further mutation. Direct file replacement during an active transaction is
unsupported; identity checks detect observed replacement, not filesystem-wide CAS.

Publish committed changes even when the initiating caller disconnects. Emit only
actual value changes, in commit order, and complete a successful Set after signal
publication. GetAll reads one complete snapshot. A bounded event handoff prevents
unbounded queued notifications or workers. Normal shutdown closes admission and
drains accepted work while its signal relay remains available. A failed signal
cannot roll back an already committed database transaction; a later Get/restart
still observes durable state.

Use a focused D-Bus adapter over this module. zbus 5.19's property macro emits on
every setter call, including no-ops, and couples notification to the caller's
future. Implement its Interface dispatch explicitly to preserve conditional,
caller-independent publication and coherent GetAll. Keep this unstable dependency
seam local and cover exact property introspection and wire behavior with tests.
No version property is added to the published Lockdown interface.

## Validation

Verify all seven exact names, boolean types and read/write annotations; Get,
GetAll, Set, standard changed-value signals, no-ops and bad types. Exercise durable
restart, concurrent writers, rejected reloads, read-only/unwritable sources, source
replacement and shutdown/caller loss through the same module interface. Sources
must be bounded regular files; filesystem and SQLite errors remain content-free.

Real frontend acceptance must prove that live changes disable saving and app
handlers before a dialog or launcher runs, and that clearing them restores the
existing Rust/Iced flow. Use only a private bus, frontend, nested Niri and disposable
state/files/apps in both appearances. Never alter the live user policy or GNOME
settings. Advertise only after scoped wire/native acceptance. Record actual checks
and limitations in `.scratch/xdg-portals/IMPLEMENTATION.md`.

The implementation now passes 66 portal unit and eight integration tests, with
one separate native observer fixture excluded from that command. Nine native
groups pass with shipped descriptor/activation files in dark/100% and light/125%,
including live frontend enforcement and restored Iced behavior. Headless
compilation, warnings-denied Clippy and Settings activation compatibility pass.
