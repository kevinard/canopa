# App identity uses desktop entry first

Notify identifies notification-sending applications primarily by their desktop entry and falls back to the application name only when no desktop entry is available. Desktop entry is the stronger desktop identity for user rules and history grouping, while the application name remains useful as display/debug metadata.
