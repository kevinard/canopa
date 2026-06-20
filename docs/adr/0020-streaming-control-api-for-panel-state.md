# Streaming control API for panel state

Notify's private control API supports a notification state subscription for panel provider clients instead of relying on polling. This keeps the open notification center current for replacements, progress, dismissals, and rule changes while avoiding periodic wakeups.
