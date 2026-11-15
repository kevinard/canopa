# Maintaining Canopa Notify

Run these commands from the Canopa workspace root. Use `bash scripts/cargo.sh`
rather than plain `cargo` for anything that compiles (see
[building.md](../building.md)). The code layout and runtime design are described
in [architecture/notify.md](../architecture/notify.md).

## Build profiles

The default features are `gui` and `audio`. `gui` adds Iced, Wayland, image
preparation, AccessKit and Niri IPC; `audio` adds Rodio. The daemon core, storage,
body parser, control socket and D-Bus interfaces build without either:

```sh
bash scripts/cargo.sh check -p canopa-notify --all-targets --no-default-features --locked
bash scripts/test-notify.sh --no-default-features
bash scripts/cargo.sh build -p canopa-notify --no-default-features --features gui --locked
```

A core build runs `daemon --headless`, `ctl` and `panel-provider` and rejects
`--service` with a feature-specific error. A GUI build without audio renders
banners and does not advertise `sound`. Test presenters exist in core builds, so
lifecycle and policy tests need no display or audio device.

The panel protocol is the single workspace crate `crates/canopa-panel-protocol`,
shared with Canopa Shell. `bash scripts/check-panel-protocol.sh` verifies that
relationship. Dependency forks are maintained as described in
[vendor-maintenance.md](../vendor-maintenance.md).

## Tests

`scripts/test-notify.sh` runs `cargo test -p canopa-notify -p canopa-panel-protocol
--locked` inside `dbus-run-session`, so tests never touch the desktop's
notification service. Extra arguments are passed to Cargo.

Notify has one integration test target, `integration`; each file in
`crates/canopa-notify/tests/` is a module of it. Select modules with a filter:

```sh
bash scripts/test-notify.sh --test integration dbus_contract::
bash scripts/test-notify.sh --test integration -- storage_failures:: legacy_import::
bash scripts/test-notify.sh --lib
```

- **D-Bus contract** (`dbus_contract::`) needs `dbus-run-session` and `dbus-send`
  from the `dbus` package. It checks introspection, method replies, malformed
  calls and lifecycle signal order against headless executables; `dbus-send`
  independently checks the server-information reply shape.
- **Storage interruption** (`storage_failures::`, `legacy_import::`) exercises
  SQLite busy errors, SIGKILL during an uncommitted migration and SIGXFSZ during a
  legacy snapshot write. The last case needs `prlimit` from util-linux. These cover
  process interruption, not power loss.
- **Malformed markup.** `intake::tests::fuzz_smoke` runs deterministic mutation
  and round-trip checks in both core and full builds. It is bounded smoke
  coverage, not exhaustive fuzzing.

Broad validation uses the root `bash scripts/check.sh --workspace` and
`bash scripts/check.sh --core`; see [testing.md](../testing.md).

### Configuration load stress

Build the integration executable, then pass the path Cargo prints:

```sh
bash scripts/test-notify.sh --test integration --no-run
python3 scripts/stress-configuration.py --test-binary /path/to/integration-HASH \
  --artifacts /tmp/configuration-load --runs 5 --cpu-workers 4
```

Each run uses a private session bus and records its test log and Linux CPU, memory
and I/O pressure. SIGTERM and Ctrl-C stop its own workers and keep partial results.
Run it after other builds finish; a concurrent build is a separate stress scenario.

## Lint policy

CI denies warnings with `bash scripts/cargo.sh clippy --workspace --all-targets
--locked -- -D warnings`. Intentional exceptions use narrow attributes with a
reason. Do not add blanket allowances to hide a new warning.

## Running the daemon during development

The service unit runs:

```sh
canopa-notify daemon --service --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock"
```

Service mode owns `org.freedesktop.Notifications`, so it fails if another
notification server is running. For development, run a headless daemon on its own
socket and bus name instead:

```sh
canopa-notify daemon --headless --socket /tmp/canopa-notify.sock \
  --database /tmp/canopa-notify.db --dev-bus-name org.canopa.NotifyTest
canopa-notify ctl --socket /tmp/canopa-notify.sock status --json
```

Headless mode has no renderer, sound, link opener or portal, and its
notifications never expire on their own because no banner displays them.

Control commands never start the daemon. Useful diagnostics:

- `ctl status --json` reports availability, DND, the configuration source, format,
  fingerprint, generation and last error, `presentation_error`,
  `sender_publication_error`, `link_opening` (available, pending, sanitized last
  error) and the timeout counters `timeout_records`, `timeout_retrying` and
  `timeout_failed`.
- `ctl active --json` lists retained active notifications; the Center also shows
  live transient ones.
- If the renderer stops after startup, status reports `banner renderer stopped;
  restart the daemon to restore banners`. Receipt and the Center keep working. The
  process keeps running, so `Restart=on-failure` does not recover it; restart the
  unit explicitly.
- A `ctl` exchange has one 30 s deadline and is never retried. If it fails, check
  state before repeating a state-changing command such as `dnd toggle` or `clear`.

### Graphical smoke check

Inside a Wayland session that has no other notification server:

```sh
scripts/manual/run-service-daemon.sh            # foreground service daemon
scripts/manual/notify-send-scenarios.sh smoke   # also: actions, replace, progress, sound, burst, all
scripts/manual/watch-notification-signals.sh    # in a second terminal
```

`run-service-daemon.sh` uses `CANOPA_NOTIFY_BIN` (default: `cargo run`),
`CANOPA_NOTIFY_SOCKET` and `CANOPA_NOTIFY_DATABASE` (a test database under
`$XDG_RUNTIME_DIR`). With the `actions` scenario, the signal watcher should show
`ActivationToken` before `ActionInvoked` when the compositor provides a token.

### Legacy history import

When the default database (`$XDG_STATE_HOME/canopa-notify/history.db`) does not
exist and no `--database` is given, startup imports a socket-adjacent
`<socket>.db`:

- It takes the source's ownership lock until the copy is published, so stop any
  daemon still using the source, even on another socket.
- It copies with SQLite `VACUUM INTO` (committed WAL data included), verifies the
  copy and publishes it without overwriting. The source is kept.
- An existing destination is always reused without reading the source.
- A failure before publication leaves both files intact and exits without serving.
  Fix the source or permissions and retry; do not delete the source to silence the
  error.
- After an abrupt interruption, the next default-path startup removes reserved
  `.history-import-*` snapshots and their SQLite sidecars under the destination
  lock. Symlinks and unrelated files are left alone.

## Notification portal check

`scripts/test-notification-portal.py` runs the Notify backend with the public
`xdg-desktop-portal` frontend on a private desktop and exercises default and named
actions, typed targets, withdrawal and application exit:

```sh
python3 scripts/test-notification-portal.py --binary target/debug/canopa-notify \
  --output /tmp/notification-portal --scheme dark --scale 1.0 \
  --shell-binary target/debug/canopa-shell
```

`--shell-binary` adds native Notification Center input; `--scheme light` and
`--scale 1.25` cover the second appearance.

## Desktop acceptance

`scripts/test-desktop.py` requires a Canopa Shell binary, Niri, Sway, grim,
software Mesa, Python GI with Gio and AT-SPI, the AT-SPI launcher and registry, a
C compiler, `pkg-config` and `wayland-scanner`. It builds a small pointer helper,
starts a private bus and fresh XDG directories, exercises banners and the Center
with sender callbacks, records screenshots, logs and compositor versions, and
stops only its own process groups:

```sh
python3 scripts/test-desktop.py --notify target/debug/canopa-notify \
  --shell target/debug/canopa-shell --artifacts /tmp/notify-desktop \
  [--color-scheme dark|light] [--scale 1.0|1.25|1.5|2.0]
```

The manually dispatched **Desktop acceptance** workflow
(`.github/workflows/desktop.yml`) runs the maintained matrix on a self-hosted
runner labelled `canopa-desktop`. Check a runner without touching the live desktop
with `dbus-run-session -- python3 scripts/check-desktop-runner.py`. Runner setup is
in [ci.md](../ci.md). An unavailable graphical job is an unmet gate, not a pass.

## Screen-reader procedure

The desktop check does not run Orca. Test screen-reader behaviour by hand in an
isolated graphical session; never replace the developer's own notification service
or screen reader.

Requirements: `orca`, `speech-dispatcher`, `espeak-ng` and `at-spi2-core`. Paths
below are Arch paths.

1. Create a private session with fresh XDG directories:

   ```sh
   fixture=$(mktemp -d /tmp/canopa-orca.XXXXXX)
   mkdir -p "$fixture"/{runtime,config,data,state,cache}
   chmod 700 "$fixture/runtime"
   env -u AT_SPI_BUS_ADDRESS -u DISPLAY -u WAYLAND_DISPLAY -u SWAYSOCK \
     XDG_RUNTIME_DIR="$fixture/runtime" XDG_CONFIG_HOME="$fixture/config" \
     XDG_DATA_HOME="$fixture/data" XDG_STATE_HOME="$fixture/state" \
     XDG_CACHE_HOME="$fixture/cache" \
     dbus-run-session -- bash --noprofile --norc
   ```

2. Inside it, start a nested compositor (for example Niri inside Sway), set
   `WAYLAND_DISPLAY` to its socket, then start the Canopa Shell and
   `canopa-notify daemon --service`.
3. Start accessibility services if the private bus has no activation manager:
   run `/usr/lib/at-spi-bus-launcher --launch-immediately --a11y=1` when
   `org.a11y.Bus` is not owned, export the `AT_SPI_BUS_ADDRESS` it reports, and
   start `/usr/lib/at-spi2-registryd`.
4. Start Speech Dispatcher with explicit configuration, socket and PID-file paths
   in the fixture. To capture speech without a sound device, point ALSA at its
   `file` plugin with a `null` slave; set `SPEECHD_PLUGIN_DIR` if the audio plugins
   are relocated. Then start `orca`. Speech Dispatcher's protocol log shows each
   SSML submission.
5. Exercise the product and compare the spoken text:
   - A new banner is announced once, politely, and does not take focus; geometry
     updates and Center refreshes do not repeat it.
   - In the Center, Tab moves through named buttons, radio choices (with their
     selection state) and links (with the link role and destination).
   - Flat review reads a body in order, including the alternative text of images.

Orca's default desktop layout, with NumLock off:

| Interaction | Key |
| --- | --- |
| Read focused name, role and context | Keypad Enter (twice for detail) |
| Toggle flat review | Keypad minus |
| Read the whole window in flat review | Keypad plus twice |
| Current / previous / next line | Keypad 8 / 7 / 9 |
| Current / previous / next item | Keypad 5 / 4 / 6 |

Niri does not run compositor bindings for `zwp_virtual_keyboard_v1` input, so
`wtype` cannot trigger Orca commands bound in the nested Niri. Two alternatives:

- Call Orca's remote controller directly:

  ```sh
  gdbus call --session --dest org.gnome.Orca.Service \
    --object-path /org/gnome/Orca/Service/FlatReviewPresenter \
    --method org.gnome.Orca.Module.ExecuteCommand SayAll true
  gdbus call --session --dest org.gnome.Orca.Service \
    --object-path /org/gnome/Orca/Service/WhereAmIPresenter \
    --method org.gnome.Orca.Module.ExecuteCommand WhereAmIBasic true
  ```

  `ListCommands` on the same interface lists the available commands.
- Bind those `gdbus` calls in the outer Sway configuration with
  `bindsym --no-repeat F6 exec ...` (Sway evaluates bindings for virtual keyboards),
  using the fixture's `DBUS_SESSION_BUS_ADDRESS`, then send
  `env WAYLAND_DISPLAY="$OUTER_WAYLAND_DISPLAY" wtype -k F6` to the outer socket.

Both run Orca's real presenters but do not test physical key delivery or Orca's
own keypad grabs. Stop only the processes you started (by PID); do not use
`pkill` or `orca --replace`.

## Release measurement

After builds finish, measure a release binary:

```sh
python3 scripts/measure-runtime.py /path/to/release/canopa-notify [--count 1000]
```

It runs the headless daemon on its own bus and state, warms up with 32 receipts,
then measures serial transient and retained Notify/Close round trips with 4 KiB
bodies. The JSON output includes
receipt percentiles, memory, CPU, platform, binary hash and clean shutdown. Compare
equivalent machines and workloads; this is not a burst or rendering benchmark.

## Dependency attributions

[dependency-attributions.md](../dependency-attributions.md) records the resolved
Linux dependency graph. Regenerate it when the lockfile or features change:

```sh
python3 scripts/dependency-attributions.py --output docs/dependency-attributions.md
```
