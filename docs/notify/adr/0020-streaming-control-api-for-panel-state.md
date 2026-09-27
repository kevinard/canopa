# Streaming control API for panel state

Notify's private control API supports a notification state subscription for panel provider clients instead of relying on polling. This keeps the open notification center current for replacements, progress, dismissals, and rule changes while avoiding periodic wakeups.

This is downstream of Notification Daemon design. The Notification Daemon owns notification state, lifecycle, policy, history, and rules; the Notification Center is an interactive view over that daemon state. Streaming panel state must not make Notification Center requirements define core daemon state or lifecycle rules.

Implementation should deepen the in-process Notify Control API module before changing the Unix socket framing. Because Canopa Notify is not released yet, legacy line-command compatibility is not required. The module should return domain-shaped replies for typed Control Commands, diagnostic status/active responses, Notification Center Projection, and current-state replacement subscriptions; command-line text, JSON, socket frames, and panel protocol messages are adapter formatting. Diagnostic control commands should become adapters over the same module instead of remaining a separate shallow path.

The in-process Notify Control API module should depend directly on the Notification Daemon. A narrower internal trait would be a hypothetical seam until there are two real adapters.

Notification Center Projection is derived state. The subscription implementation should rebuild each emitted projection from the Notification Daemon instead of maintaining a separate projection store.
