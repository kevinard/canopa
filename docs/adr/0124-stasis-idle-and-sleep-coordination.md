# 0124: Stasis idle policy and Canopa sleep coordination

Status: accepted

Canopa delegates idle timeouts, profiles and inhibition policy to external Stasis
with its native Rune configuration. `canopa-idle` supervises one owned backend,
validates the selected session's lock coordinator and streams Stasis watch state.
The shell consumes passive snapshots through `canopa-widgets`; actions and
subscription lifetimes remain shell-owned. Existing Wayland manual inhibitors
remain a separate, explicit module backend.

The lock listener owns a private per-display, same-UID control endpoint. Foreground
requests wait until authenticated unlock; readiness requests wait for the actual
ext-session-lock compositor acknowledgement. Concurrent requests share the same
locker generation. A closed readiness descriptor is failure, never success.

The listener subscribes to logind's sleep events and holds its delay-inhibitor FD
before announcing service readiness. It releases that FD after compositor lock
readiness when preparing for sleep, and reacquires it after resume or reconnect.
This lifetime is independent of the optional idle-policy backend. Canopa's own
suspend helper refuses to request sleep before readiness. External/forced sleep
is subject to logind's maximum-delay and authorization policy.

No dependency fork or Stasis source is embedded. Native acceptance runs pinned
upstream Stasis, independent Wayland and PAM peers, and real FDs passed over a
private logind D-Bus service. Physical sleep and privileged seat transitions are
separate deployment acceptance. See [setup](../idle.md).
