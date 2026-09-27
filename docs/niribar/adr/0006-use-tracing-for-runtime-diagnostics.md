# Use tracing for runtime diagnostics

Niribar uses `tracing` as the single runtime diagnostics pipeline instead of adding ad hoc `eprintln!` calls or a separate logging system. Default output is human-readable stderr suitable for `journalctl`, with warning and error events visible by default; deeper lifecycle, event-flow, and performance diagnostics are enabled through environment filters such as `RUST_LOG`.

Runtime diagnostics must avoid user content by default: focused window titles, script output, command strings, dmenu entries, panel search text, raw argv, and desktop application Exec lines are not logged. Events should identify stable domain facts such as **Bar Surface Identity**, **Output**, **Module Name**, **Module Kind**, integration availability, counts, durations, and process outcomes.

`NIRIBAR_PERF=1` remains the compatibility switch for periodic performance summaries, but those summaries are emitted through `tracing` so diagnostics share one filtering and formatting path. The shipped systemd unit does not set logging environment variables; users can opt into debug or performance output with local environment settings or systemd drop-ins.
