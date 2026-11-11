# 0137 Shared system-service clients

Status: accepted.

The zbus and libpulse clients for NetworkManager (with systemd-networkd and iwd),
BlueZ, audio, UPower, power-profiles-daemon, logind and MPRIS lived in
`canopa-shell` and, for UPower, `canopa-widgets`. Settings needs the same clients
for live pages and could not use them there.

They now live in `canopa-services`. The crate has no Iced, Wayland, UI or
panel-protocol dependency and adds no external crate: it uses the workspace's
zbus, Tokio, futures, Serde, thiserror and (behind the `audio` feature)
libpulse-binding. It keeps the distribution principle: system services own state
and policy, Canopa owns presentation, and no Canopa daemon sits in between.

Each client follows one shape:

- a typed snapshot, with a one-shot read where that is useful;
- a change stream (`watch`) of `ServiceState<T>`, where `Unavailable` is a
  normal state: the service is not installed, not running, or the last read
  failed. Streams report an outage once, retry with bounded backoff, and follow
  owner changes, as the Shell's subscriptions already did;
- commands with typed errors: the shared `Error` (`Unavailable`,
  `NotAuthorized`, `TimedOut`, `Changed`, `Invalid`, `Refused`) or a
  service-specific type that already existed (`WifiBackendError`, logind's
  `Failure`, `AudioError`, `MprisError`).

Identity rules move with the code: commands address the owner, device and
connection a snapshot came from, and revalidate before acting. Wording stays
with the callers. Bluetooth agent questions, logind failures and Wi-Fi errors are
typed; the Shell keeps its sentences. Audio and MPRIS errors keep the messages
their panels already showed.

The provider split is unchanged. `canopa-wifi-provider`, `canopa-audio-provider`
and `canopa-media-provider` stay separate processes speaking the panel protocol
(Shell ADR 0010): that gives crash isolation and keeps service permissions out of
the bar. They now link the shared clients instead of Shell modules. The headless
provider build (`--no-default-features`) still excludes host rendering, and the
dependency-boundary scripts pass unchanged.

The Shell keeps what is presentation or Shell policy: Iced subscriptions, panel
content and protocol sessions, the private `org.canopa.Shell.Media` selection
interface (Shell ADR 0018), session-action configuration names and notices, and
the bounded StatusNotifierItem host, which is the Shell's own service rather than
a client of a system service. `canopa-widgets` keeps its lock-screen API and
delegates UPower and the MPRIS player types to the new crate. `canopa-session`
keeps its own logind client for locking; it may adopt this one later.

Test fixtures are part of the crate behind `test-support`: a private
`TestBus`, fake logind (with a session that records `SetBrightness`), UPower,
power-profiles-daemon, BlueZ, MPRIS players, NetworkManager and iwd Wi-Fi
daemons, and a private PipeWire Pulse server. Settings enables the feature in its
development dependencies.

The [crate documentation](../../crates/canopa-services/src/lib.rs) lists the
modules; each module starts with a compiled example for page authors.
