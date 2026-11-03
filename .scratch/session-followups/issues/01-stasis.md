# Stasis idle and sleep integration

Status: ready-for-agent

Implement PRD item 1 first. Stasis remains an external executable with native Rune
configuration. Canopa owns a bounded session-specific control/readiness endpoint
for the managed locker and a supervisor that validates the listener's acquired logind sleep delay
inhibitor before launching Stasis. A positive LockedHint or process spawn alone
is never a substitute for the compositor acknowledgement. Automatic suspend calls
a Canopa command which refuses to request sleep without actual lock readiness;
external sleep is delayed until readiness within logind's bounded delay window.

The existing foreground `canopa-lock` behavior and direct fallback are retained.
When the session listener is present, foreground and readiness requests attach to
its managed locker, including concurrent requests. Ending the listener must not
unlock/kill an active locker. Build independent protocol/lifecycle and native
Wayland/sleep tests before treating the integration as ready.
