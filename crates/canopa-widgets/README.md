# canopa-widgets

Passive data sources for widgets in Canopa Shell, the lock screen and the login
screen. The crate reads and observes; it has no Iced dependency, no windows, no
actions and no credentials. Consumers own layout, what is safe to show while
locked, controls and process lifetime.

| Module | Source | Main API |
| --- | --- | --- |
| `battery` | UPower display device, through `canopa-services` | `BatteryState`, `BatteryEvent`, `run_upower_task` |
| `media` | The Shell's selected MPRIS player (`org.canopa.Shell.Media`) | `Snapshot` (`current`, `reconcile`), `Request`, `observe` |
| `idle` | Stasis state from `canopa-idle watch` | `Snapshot` (`parse`), `companion`, `observe` |
| `script` | A configured command's output | `Script`, `capture`, `observe` |
| `weather` | A JSON snapshot file written by an external provider | `Weather` (`read`, `label`) |

Every source is bounded:

- `media::observe` only reads. Player selection and control stay with the Shell,
  so the lock screen shows the player the Shell has selected or pinned.
- `idle` snapshots are at most 16 KiB and must name a known state; `observe`
  restarts the watcher with a growing delay after it exits.
- `script::capture` runs an explicit argv in its own process group with a
  timeout, no stdin, at most 16 KiB of output (trimmed to 1,024 characters
  without control characters), and kills the group if the consumer goes away.
- `weather::Weather::read` rejects malformed, future-dated or more than two hours
  old snapshots. The provider that writes the file owns location, credentials and
  network access.

A new widget adds a typed snapshot and observer here and is rendered by each
consumer.

See [Shared foundations](../../docs/architecture/foundations.md#widget-data).

Licensed under MIT.
