# Manual idle inhibition through bar surfaces

The `idle-inhibitor` Module provides a manual keep-awake toggle, initially off on
each shell launch. Multiple instances share one preference and source state.
It uses `zwp_idle_inhibit_manager_v1`, which integrates with Niri's compositor idle
handling without a child process or a separate D-Bus inhibition policy.

Module Data Source Runtime owns the preference and per-surface acknowledgements.
It sends at most one request per Bar Surface at a time, coalescing further clicks
until completion. A newly opened Bar Surface acquires the current preference;
removing all outputs reports unavailable while retaining the preference for
restored outputs. Removing the final Module resets the preference and releases
any remaining inhibitors. Nothing is persisted across shell launches.

The shared Wayland adapter binds the optional protocol and owns an inhibitor per
surface. Requests are idempotent, absence is reported, and surface destruction
destroys the inhibitor. Retired-window requests complete without accumulating
adapter retries. Shell messages explicitly convert to adapter actions, matching
Notify's existing approach and allowing the added protocol operation without
forking the upstream message-generation macro.

Active means the adapter created an inhibitor; this protocol has no compositor
acknowledgement of whether policy currently honors it. Inhibition is conditional
on surface visibility and the idle daemon's use of compositor idle notifications.
It does not prevent explicit lock, suspend, or shutdown actions. Unsupported
compositors show an Unavailable Module that remains visible.
