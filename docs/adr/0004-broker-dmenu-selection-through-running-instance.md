# Broker Dmenu Selection Through the Running Instance

Niribar's dmenu-style CLI mode is standalone from the caller's perspective, but the running Niribar instance owns the visible selector surface. The CLI reads standard input, sends one bounded selection request over IPC, waits for the daemon to return selection, cancellation, or failure, and then writes the selected value to standard output. This keeps focus, output placement, panel-menu rendering, and configuration ownership inside the already-running bar instead of creating a second ad hoc UI lifecycle.

Existing one-line IPC commands such as `toggle-panel` remain supported, but request/response features use structured framed messages. Dmenu requests carry arbitrary entry text, delimiter-derived values, prompt metadata, and later richer menu-entry metadata, so encoding them as line commands would make the IPC boundary fragile.
